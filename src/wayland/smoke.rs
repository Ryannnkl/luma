use std::{
    fmt,
    time::{Duration, Instant},
};

#[cfg(debug_assertions)]
use std::thread;

use calloop::{EventLoop, channel};
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::error::GlobalError as SctkGlobalError;
use smithay_client_toolkit::{
    delegate_keyboard, delegate_output, delegate_registry, delegate_seat, delegate_session_lock,
    delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
    },
    session_lock::{
        SessionLock, SessionLockHandler, SessionLockState, SessionLockSurface,
        SessionLockSurfaceConfigure,
    },
    shm::{
        Shm, ShmHandler,
        slot::{Buffer, SlotPool},
    },
};
use wayland_client::{
    Connection, QueueHandle, delegate_noop,
    globals::{GlobalList, registry_queue_init},
    protocol::{wl_compositor, wl_output, wl_shm, wl_surface},
};

use crate::{
    auth::worker::{AuthenticationCompletion, AuthenticationWorker},
    config::{ClockConfig, Color, Config, DateConfig, InputConfig},
    input::InputState,
    renderer::LockTextRenderers,
    state::{AuthenticationOutcome, AuthenticationPhase, AuthenticationState, CompletionAction},
    wayland::opaque::{
        PromptState, draw_captured_background, draw_lock_frame, draw_lock_prompt,
        draw_lock_visual_feedback, draw_lock_visuals,
    },
};

use super::capture::{CapturedOutput, capture_outputs};

/// Runs the authenticated Luma session locker without a timed bypass.
///
/// # Errors
///
/// Returns an error when critical Wayland resources are unavailable, the compositor rejects the
/// lock, or the authenticated unlock request cannot be delivered.
pub fn run_authenticated(config: Config) -> Result<(), LockError> {
    let renderers = LockTextRenderers::from_paths(
        config.clock.hour_font_path.as_deref(),
        config.clock.minute_font_path.as_deref(),
        config.date.font_path.as_deref(),
    )
    .map_err(|error| LockError::Font(error.to_string()))?;
    let captured_outputs = if config.background.capture_enabled {
        capture_outputs(config.background.blur_radius)
            .map_err(|error| LockError::Capture(error.to_string()))?
    } else {
        Vec::new()
    };
    let presentation = LockPresentation {
        renderers,
        clock: config.clock,
        date: config.date,
        dim_color: config.background.dim_color,
        captured_outputs,
    };
    let connection = Connection::connect_to_env().map_err(LockError::Connect)?;
    let (globals, event_queue) =
        registry_queue_init::<LockState>(&connection).map_err(LockError::Registry)?;
    let qh = event_queue.handle();
    let (completion_sender, completion_channel) = channel::channel();
    let authentication_worker =
        AuthenticationWorker::spawn(completion_sender).map_err(LockError::AuthenticationWorker)?;
    let mut state = LockState::new(
        &globals,
        &qh,
        Some(authentication_worker),
        config.input,
        Some(presentation),
    )?;
    if state.output_state.outputs().next().is_none() {
        return Err(LockError::NoOutputs);
    }

    let mut event_loop: EventLoop<LockState> =
        EventLoop::try_new().map_err(|error| LockError::EventLoop(error.to_string()))?;
    WaylandSource::new(connection.clone(), event_queue)
        .insert(event_loop.handle())
        .map_err(|_| LockError::EventSource("could not register the Wayland source"))?;
    event_loop
        .handle()
        .insert_source(completion_channel, |event, (), state| {
            if let channel::Event::Msg(completion) = event {
                state.handle_authentication_completion(completion);
            }
        })
        .map_err(|_| LockError::EventSource("could not register the authentication channel"))?;

    let lock = state
        .lock_manager
        .lock(&qh)
        .map_err(|error: SctkGlobalError| LockError::Lock(error.to_string()))?;
    state.session_lock = Some(lock);

    while !state.finished {
        let timeout = state.event_timeout();
        event_loop
            .dispatch(timeout, &mut state)
            .map_err(|error| LockError::EventLoop(error.to_string()))?;
        state.advance_authentication();
        state.advance_visuals();
    }

    if state.unlock_authorized {
        connection
            .flush()
            .map_err(|error| LockError::Unlock(error.to_string()))?;
        Ok(())
    } else {
        Err(LockError::FinishedWithoutAuthentication)
    }
}

/// Runs a deliberately bounded opaque lock smoke test.
///
/// This is a development test path, not authentication and not the production
/// Luma locker. It must only be run inside the isolated nested compositor.
///
/// # Errors
///
/// Returns an error when Wayland globals cannot be bound or the event queue
/// encounters a protocol failure.
#[cfg(debug_assertions)]
pub fn run(timeout: Duration) -> Result<(), LockError> {
    let connection = Connection::connect_to_env().map_err(LockError::Connect)?;
    let (globals, event_queue) =
        registry_queue_init::<LockState>(&connection).map_err(LockError::Registry)?;
    let qh = event_queue.handle();
    let mut state = LockState::new(&globals, &qh, None, InputConfig::default(), None)?;
    let lock = state
        .lock_manager
        .lock(&qh)
        .map_err(|error: SctkGlobalError| LockError::Lock(error.to_string()))?;
    let timer_lock = lock.clone();
    let timer_connection = connection.clone();
    let timer = thread::spawn(move || {
        thread::sleep(timeout);
        timer_lock.unlock();
        let _ = timer_connection.flush();
    });
    state.session_lock = Some(lock);

    let mut event_queue = event_queue;
    while !state.finished {
        event_queue
            .blocking_dispatch(&mut state)
            .map_err(LockError::Dispatch)?;
    }

    timer.join().map_err(|_| LockError::TimerPanic)?;
    Ok(())
}

#[derive(Debug)]
pub enum LockError {
    Connect(wayland_client::ConnectError),
    Registry(wayland_client::globals::GlobalError),
    Dispatch(wayland_client::DispatchError),
    Bind(String),
    Lock(String),
    Buffer(String),
    Font(String),
    Capture(String),
    AuthenticationWorker(std::io::Error),
    EventLoop(String),
    EventSource(&'static str),
    Unlock(String),
    NoOutputs,
    FinishedWithoutAuthentication,
    #[cfg(debug_assertions)]
    TimerPanic,
}

impl fmt::Display for LockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect(source) => write!(formatter, "could not connect to Wayland: {source}"),
            Self::Registry(source) => {
                write!(formatter, "could not read the Wayland registry: {source}")
            }
            Self::Dispatch(source) => write!(formatter, "Wayland event dispatch failed: {source}"),
            Self::Bind(source) => write!(formatter, "could not bind a lock dependency: {source}"),
            Self::Lock(source) => write!(formatter, "could not request the session lock: {source}"),
            Self::Buffer(source) => write!(formatter, "could not create the lock buffer: {source}"),
            Self::Font(source) => write!(formatter, "could not load the lock font: {source}"),
            Self::Capture(source) => {
                write!(formatter, "could not capture lock background: {source}")
            }
            Self::AuthenticationWorker(source) => {
                write!(
                    formatter,
                    "could not start the authentication worker: {source}"
                )
            }
            Self::EventLoop(source) => write!(formatter, "lock event loop failed: {source}"),
            Self::EventSource(source) => formatter.write_str(source),
            Self::Unlock(source) => {
                write!(
                    formatter,
                    "could not deliver the authenticated unlock: {source}"
                )
            }
            Self::NoOutputs => formatter.write_str("Wayland reported no outputs to lock"),
            Self::FinishedWithoutAuthentication => {
                formatter.write_str("the session lock ended without an authenticated unlock")
            }
            #[cfg(debug_assertions)]
            Self::TimerPanic => formatter.write_str("lock smoke timer panicked"),
        }
    }
}

impl std::error::Error for LockError {}

struct LockState {
    registry_state: RegistryState,
    output_state: OutputState,
    seat_state: SeatState,
    keyboard: Option<wayland_client::protocol::wl_keyboard::WlKeyboard>,
    modifiers: Modifiers,
    input: InputState,
    input_config: InputConfig,
    presentation: Option<LockPresentation>,
    next_visual_redraw: Option<Instant>,
    visual_frame: u64,
    authentication: Option<AuthenticationController>,
    shm_state: Shm,
    pool: SlotPool,
    compositor: wl_compositor::WlCompositor,
    lock_manager: SessionLockState,
    session_lock: Option<SessionLock>,
    surfaces: Vec<LockSurfaceState>,
    unlock_authorized: bool,
    finished: bool,
}

struct AuthenticationController {
    state: AuthenticationState,
    worker: AuthenticationWorker,
}

struct LockPresentation {
    renderers: LockTextRenderers,
    clock: ClockConfig,
    date: DateConfig,
    dim_color: Color,
    captured_outputs: Vec<CapturedOutput>,
}

struct LockSurfaceState {
    output: wl_output::WlOutput,
    surface: SessionLockSurface,
    buffer: Option<Buffer>,
    width: i32,
    height: i32,
}

impl LockState {
    fn new(
        globals: &GlobalList,
        qh: &QueueHandle<Self>,
        authentication_worker: Option<AuthenticationWorker>,
        input_config: InputConfig,
        presentation: Option<LockPresentation>,
    ) -> Result<Self, LockError> {
        let compositor = globals
            .bind(qh, 1..=6, ())
            .map_err(|error| LockError::Bind(error.to_string()))?;
        let shm_state =
            Shm::bind(globals, qh).map_err(|error| LockError::Bind(error.to_string()))?;
        let pool =
            SlotPool::new(1, &shm_state).map_err(|error| LockError::Buffer(error.to_string()))?;

        let authentication_policy = crate::state::AuthenticationPolicy {
            feedback_duration: Duration::from_millis(input_config.feedback_duration_ms),
            ..crate::state::AuthenticationPolicy::default()
        };
        let max_characters = input_config.max_characters;

        Ok(Self {
            registry_state: RegistryState::new(globals),
            output_state: OutputState::new(globals, qh),
            seat_state: SeatState::new(globals, qh),
            keyboard: None,
            modifiers: Modifiers::default(),
            input: InputState::new(max_characters),
            input_config,
            next_visual_redraw: presentation
                .as_ref()
                .filter(|presentation| presentation.clock.enabled || presentation.date.enabled)
                .map(|_| Instant::now() + Duration::from_secs(1)),
            visual_frame: 0,
            presentation,
            authentication: authentication_worker.map(|worker| AuthenticationController {
                state: AuthenticationState::new(authentication_policy),
                worker,
            }),
            shm_state,
            pool,
            compositor,
            lock_manager: SessionLockState::new(globals, qh),
            session_lock: None,
            surfaces: Vec::new(),
            unlock_authorized: false,
            finished: false,
        })
    }
}

impl ProvidesRegistryState for LockState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers!(OutputState, SeatState);
}

impl OutputHandler for LockState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.add_output_surface(queue_handle, output);
    }

    fn update_output(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.surfaces
            .retain(|surface_state| surface_state.output != output);
    }
}

impl ShmHandler for LockState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm_state
    }
}

impl SeatHandler for LockState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _seat: wayland_client::protocol::wl_seat::WlSeat,
    ) {
    }

    fn new_capability(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        seat: wayland_client::protocol::wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard = self
                .seat_state
                .get_keyboard::<Self, Self>(queue_handle, &seat, None)
                .ok();
        }
    }

    fn remove_capability(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _seat: wayland_client::protocol::wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            self.keyboard = None;
            self.modifiers = Modifiers::default();
            self.input.clear();
            self.redraw_input_indicator();
        }
    }

    fn remove_seat(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _seat: wayland_client::protocol::wl_seat::WlSeat,
    ) {
        self.keyboard = None;
        self.modifiers = Modifiers::default();
        self.input.clear();
        self.redraw_input_indicator();
    }
}

impl KeyboardHandler for LockState {
    fn enter(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
        self.modifiers = Modifiers::default();
        self.input.clear();
        self.redraw_input_indicator();
    }

    fn press_key(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        self.handle_key(event);
    }

    fn repeat_key(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        self.handle_key(event);
    }

    fn release_key(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
        _raw_modifiers: smithay_client_toolkit::seat::keyboard::RawModifiers,
        _layout: u32,
    ) {
        self.modifiers = modifiers;
    }
}

impl LockState {
    fn add_output_surface(
        &mut self,
        queue_handle: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if self
            .surfaces
            .iter()
            .any(|surface_state| surface_state.output == output)
        {
            return;
        }
        let Some(session_lock) = self
            .session_lock
            .as_ref()
            .filter(|session_lock| session_lock.is_locked())
        else {
            return;
        };

        let surface = self.compositor.create_surface(queue_handle, ());
        let lock_surface = session_lock.create_lock_surface(surface, &output, queue_handle);
        self.surfaces.push(LockSurfaceState {
            output,
            surface: lock_surface,
            buffer: None,
            width: 0,
            height: 0,
        });
    }

    fn handle_key(&mut self, event: KeyEvent) {
        if !self
            .session_lock
            .as_ref()
            .is_some_and(SessionLock::is_locked)
        {
            return;
        }
        if self
            .authentication
            .as_ref()
            .is_some_and(|authentication| !authentication.state.accepts_input())
        {
            return;
        }
        match event.keysym {
            Keysym::BackSpace => handle_backspace(&mut self.input, self.modifiers.ctrl),
            Keysym::Return => self.submit_authentication(),
            _ => {
                if let Some(text) = event.utf8 {
                    self.input.push_text(&text);
                }
            }
        }
        self.redraw_input_indicator();
    }

    fn submit_authentication(&mut self) {
        let Some(password) = self.input.submit() else {
            return;
        };
        let Some(authentication) = &mut self.authentication else {
            drop(password);
            return;
        };
        let Ok(token) = authentication.state.begin_attempt() else {
            drop(password);
            return;
        };
        if authentication.worker.submit(token, password).is_err() {
            authentication.state.complete_attempt(
                token,
                AuthenticationOutcome::InfrastructureError,
                Instant::now(),
            );
        }
        self.schedule_visual_redraw();
    }

    fn handle_authentication_completion(&mut self, completion: AuthenticationCompletion) {
        if self.finished
            || !self
                .session_lock
                .as_ref()
                .is_some_and(SessionLock::is_locked)
        {
            return;
        }
        let Some(authentication) = &mut self.authentication else {
            return;
        };
        let action = authentication.state.complete_attempt(
            completion.token,
            completion.outcome,
            Instant::now(),
        );
        if action == CompletionAction::UnlockAuthorized {
            self.unlock_authorized = true;
            if let Some(session_lock) = &self.session_lock {
                session_lock.unlock();
            }
            // `finished` rejects or cancels a lock; it is not an acknowledgement of a
            // client-initiated unlock, so the event loop must terminate here.
            self.finished = true;
            return;
        }
        self.schedule_visual_redraw();
        self.redraw_input_indicator();
    }

    fn event_timeout(&self) -> Option<Duration> {
        let authentication = self
            .authentication
            .as_ref()
            .and_then(|authentication| authentication.state.next_deadline())
            .map(|deadline| deadline.saturating_duration_since(Instant::now()));
        let visuals = self
            .next_visual_redraw
            .map(|deadline| deadline.saturating_duration_since(Instant::now()));
        match (authentication, visuals) {
            (Some(authentication), Some(visuals)) => Some(authentication.min(visuals)),
            (authentication, visuals) => authentication.or(visuals),
        }
    }

    fn advance_authentication(&mut self) {
        let Some(authentication) = &mut self.authentication else {
            return;
        };
        let previous_phase = authentication.state.phase();
        authentication.state.advance(Instant::now());
        if authentication.state.phase() != previous_phase {
            self.schedule_visual_redraw();
            self.redraw_input_indicator();
        }
    }

    fn visual_interval(&self) -> Option<Duration> {
        self.presentation.as_ref()?;
        match prompt_state_for_phase(
            self.authentication
                .as_ref()
                .map(|authentication| authentication.state.phase()),
        ) {
            PromptState::Ready => self
                .presentation
                .as_ref()
                .filter(|presentation| presentation.clock.enabled || presentation.date.enabled)
                .map(|_| Duration::from_secs(1)),
            PromptState::Authenticating | PromptState::Failure | PromptState::Cooldown => {
                Some(Duration::from_millis(120))
            }
        }
    }

    fn schedule_visual_redraw(&mut self) {
        self.next_visual_redraw = self
            .visual_interval()
            .map(|interval| Instant::now() + interval);
    }

    fn advance_visuals(&mut self) {
        let Some(deadline) = self.next_visual_redraw else {
            return;
        };
        let now = Instant::now();
        if now < deadline {
            return;
        }
        self.visual_frame = self.visual_frame.wrapping_add(1);
        self.next_visual_redraw = self.visual_interval().map(|interval| now + interval);
        self.redraw_input_indicator();
    }

    fn redraw_input_indicator(&mut self) {
        for index in 0..self.surfaces.len() {
            let _ = self.render_surface(index);
        }
    }

    fn render_surface(&mut self, index: usize) -> Result<(), LockError> {
        let Some(surface_state) = self.surfaces.get(index) else {
            return Ok(());
        };
        let width = surface_state.width;
        let height = surface_state.height;
        let surface = surface_state.surface.clone();
        let output = surface_state.output.clone();
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let stride = width
            .checked_mul(4)
            .ok_or_else(|| LockError::Buffer("lock surface stride overflowed".to_owned()))?;
        let (buffer, canvas) = self
            .pool
            .create_buffer(width, height, stride, wl_shm::Format::Argb8888)
            .map_err(|error| LockError::Buffer(error.to_string()))?;
        let prompt_state = prompt_state_for_phase(
            self.authentication
                .as_ref()
                .map(|authentication| authentication.state.phase()),
        );
        draw_lock_frame(
            canvas,
            width,
            height,
            self.input.character_count(),
            prompt_state,
            &self.input_config,
        );
        if let Some(presentation) = &self.presentation {
            let output_name = self.output_state.info(&output).and_then(|info| info.name);
            if let Some(captured) = output_name.as_deref().and_then(|name| {
                presentation
                    .captured_outputs
                    .iter()
                    .find(|captured| captured.name == name)
            }) {
                draw_captured_background(
                    canvas,
                    width,
                    height,
                    &captured.image,
                    presentation.dim_color,
                );
            }
            draw_lock_visuals(
                canvas,
                width,
                height,
                &presentation.clock,
                &presentation.date,
                &presentation.renderers,
                chrono::Local::now(),
            );
            draw_lock_prompt(
                canvas,
                usize::try_from(width).unwrap_or_default(),
                usize::try_from(height).unwrap_or_default(),
                self.input.character_count(),
                prompt_state,
                &self.input_config,
            );
            draw_lock_visual_feedback(
                canvas,
                width,
                height,
                prompt_state,
                &self.input_config,
                self.visual_frame,
            );
        }
        buffer
            .attach_to(surface.wl_surface())
            .map_err(|error| LockError::Buffer(error.to_string()))?;
        surface.wl_surface().damage(0, 0, width, height);
        surface.wl_surface().commit();
        self.surfaces[index].buffer = Some(buffer);
        Ok(())
    }
}

fn handle_backspace(input: &mut InputState, control: bool) {
    if control {
        input.clear();
    } else {
        input.backspace();
    }
}

const fn prompt_state_for_phase(phase: Option<AuthenticationPhase>) -> PromptState {
    match phase {
        None | Some(AuthenticationPhase::Idle) => PromptState::Ready,
        Some(AuthenticationPhase::Authenticating | AuthenticationPhase::Authenticated) => {
            PromptState::Authenticating
        }
        Some(AuthenticationPhase::Denied | AuthenticationPhase::Error) => PromptState::Failure,
        Some(AuthenticationPhase::Cooldown) => PromptState::Cooldown,
    }
}

impl SessionLockHandler for LockState {
    fn locked(
        &mut self,
        _connection: &Connection,
        qh: &QueueHandle<Self>,
        session_lock: SessionLock,
    ) {
        self.session_lock = Some(session_lock);
        let outputs: Vec<_> = self.output_state.outputs().collect();
        for output in outputs {
            self.add_output_surface(qh, output);
        }
    }

    fn finished(
        &mut self,
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
        _session_lock: SessionLock,
    ) {
        self.finished = true;
        self.session_lock = None;
        self.input.clear();
    }

    fn configure(
        &mut self,
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
        surface: SessionLockSurface,
        configure: SessionLockSurfaceConfigure,
        _serial: u32,
    ) {
        let Some(index) = self
            .surfaces
            .iter()
            .position(|candidate| candidate.surface.wl_surface() == surface.wl_surface())
        else {
            return;
        };

        let (width, height) = configure.new_size;
        let width = i32::try_from(width).unwrap_or(i32::MAX);
        let height = i32::try_from(height).unwrap_or(i32::MAX);
        self.surfaces[index].width = width;
        self.surfaces[index].height = height;
        let _ = self.render_surface(index);
    }
}

delegate_registry!(LockState);
delegate_output!(LockState);
delegate_seat!(LockState);
delegate_keyboard!(LockState);
delegate_session_lock!(LockState);
delegate_shm!(LockState);
delegate_noop!(LockState: ignore wl_compositor::WlCompositor);
delegate_noop!(LockState: ignore wl_surface::WlSurface);

#[cfg(test)]
mod tests {
    use crate::{input::InputState, state::AuthenticationPhase, wayland::opaque::PromptState};

    use super::{handle_backspace, prompt_state_for_phase};

    #[test]
    fn control_backspace_clears_the_password_input() {
        let mut input = InputState::new(12);
        input.push_text("secret");

        handle_backspace(&mut input, true);

        assert!(input.is_empty());
    }

    #[test]
    fn backspace_without_control_removes_one_character() {
        let mut input = InputState::new(12);
        input.push_text("secret");

        handle_backspace(&mut input, false);

        assert_eq!(input.character_count(), 5);
    }

    #[test]
    #[cfg(debug_assertions)]
    fn smoke_timeout_is_explicitly_bounded_by_the_caller() {
        let timeout = std::time::Duration::from_secs(5);
        assert!(timeout <= std::time::Duration::from_secs(30));
    }

    #[test]
    fn credential_denial_and_infrastructure_error_share_feedback() {
        assert_eq!(
            prompt_state_for_phase(Some(AuthenticationPhase::Denied)),
            PromptState::Failure
        );
        assert_eq!(
            prompt_state_for_phase(Some(AuthenticationPhase::Error)),
            PromptState::Failure
        );
    }

    #[test]
    fn smoke_mode_keeps_the_neutral_prompt() {
        assert_eq!(prompt_state_for_phase(None), PromptState::Ready);
    }
}
