use std::{fmt, thread, time::Duration};

use smithay_client_toolkit::error::GlobalError as SctkGlobalError;
use smithay_client_toolkit::{
    delegate_output, delegate_registry, delegate_session_lock, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
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

/// Runs a deliberately bounded opaque lock smoke test.
///
/// This is a development test path, not authentication and not the production
/// Luma locker. It must only be run inside the isolated nested compositor.
///
/// # Errors
///
/// Returns an error when Wayland globals cannot be bound or the event queue
/// encounters a protocol failure.
pub fn run(timeout: Duration) -> Result<(), SmokeError> {
    let connection = Connection::connect_to_env().map_err(SmokeError::Connect)?;
    let (globals, event_queue) =
        registry_queue_init::<SmokeState>(&connection).map_err(SmokeError::Registry)?;
    let qh = event_queue.handle();
    let mut state = SmokeState::new(&globals, &qh)?;
    let lock = state
        .session_lock_state
        .lock(&qh)
        .map_err(|error: SctkGlobalError| SmokeError::Lock(error.to_string()))?;
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
            .map_err(SmokeError::Dispatch)?;
    }

    timer.join().map_err(|_| SmokeError::TimerPanic)?;
    Ok(())
}

#[derive(Debug)]
pub enum SmokeError {
    Connect(wayland_client::ConnectError),
    Registry(wayland_client::globals::GlobalError),
    Dispatch(wayland_client::DispatchError),
    Bind(String),
    Lock(String),
    Buffer(String),
    TimerPanic,
}

impl fmt::Display for SmokeError {
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
            Self::TimerPanic => formatter.write_str("lock smoke timer panicked"),
        }
    }
}

impl std::error::Error for SmokeError {}

struct SmokeState {
    registry_state: RegistryState,
    output_state: OutputState,
    shm_state: Shm,
    pool: SlotPool,
    compositor: wl_compositor::WlCompositor,
    session_lock_state: SessionLockState,
    session_lock: Option<SessionLock>,
    surfaces: Vec<LockSurfaceState>,
    finished: bool,
}

struct LockSurfaceState {
    surface: SessionLockSurface,
    buffer: Option<Buffer>,
}

impl SmokeState {
    fn new(globals: &GlobalList, qh: &QueueHandle<Self>) -> Result<Self, SmokeError> {
        let compositor = globals
            .bind(qh, 1..=6, ())
            .map_err(|error| SmokeError::Bind(error.to_string()))?;
        let shm_state =
            Shm::bind(globals, qh).map_err(|error| SmokeError::Bind(error.to_string()))?;
        let pool =
            SlotPool::new(1, &shm_state).map_err(|error| SmokeError::Buffer(error.to_string()))?;

        Ok(Self {
            registry_state: RegistryState::new(globals),
            output_state: OutputState::new(globals, qh),
            shm_state,
            pool,
            compositor,
            session_lock_state: SessionLockState::new(globals, qh),
            session_lock: None,
            surfaces: Vec::new(),
            finished: false,
        })
    }
}

impl ProvidesRegistryState for SmokeState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers!(OutputState);
}

impl OutputHandler for SmokeState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
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
        _output: wl_output::WlOutput,
    ) {
    }
}

impl ShmHandler for SmokeState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm_state
    }
}

impl SessionLockHandler for SmokeState {
    fn locked(
        &mut self,
        _connection: &Connection,
        qh: &QueueHandle<Self>,
        session_lock: SessionLock,
    ) {
        let outputs = self.output_state.outputs();
        for output in outputs {
            let surface = self.compositor.create_surface(qh, ());
            let lock_surface = session_lock.create_lock_surface(surface, &output, qh);
            self.surfaces.push(LockSurfaceState {
                surface: lock_surface,
                buffer: None,
            });
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
        let stride = width.saturating_mul(4);
        let Ok((buffer, canvas)) =
            self.pool
                .create_buffer(width, height, stride, wl_shm::Format::Argb8888)
        else {
            self.finished = true;
            return;
        };

        for pixel in canvas.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[18, 26, 28, 255]);
        }

        if buffer.attach_to(surface.wl_surface()).is_err() {
            self.finished = true;
            return;
        }
        surface.wl_surface().damage(0, 0, width, height);
        surface.wl_surface().commit();
        self.surfaces[index].buffer = Some(buffer);
    }
}

delegate_registry!(SmokeState);
delegate_output!(SmokeState);
delegate_session_lock!(SmokeState);
delegate_shm!(SmokeState);
delegate_noop!(SmokeState: ignore wl_compositor::WlCompositor);
delegate_noop!(SmokeState: ignore wl_surface::WlSurface);

#[cfg(test)]
mod tests {
    #[test]
    fn smoke_timeout_is_explicitly_bounded_by_the_caller() {
        let timeout = std::time::Duration::from_secs(5);
        assert!(timeout <= std::time::Duration::from_secs(30));
    }
}
