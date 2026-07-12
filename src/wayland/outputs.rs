use std::fmt;

use smithay_client_toolkit::{
    delegate_output, delegate_registry,
    output::{Mode, OutputHandler, OutputInfo, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
};
use wayland_client::{
    Connection, DispatchError, EventQueue, QueueHandle,
    globals::{GlobalError, registry_queue_init},
    protocol::wl_output,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputSnapshot {
    pub global_id: u32,
    pub name: Option<String>,
    pub description: Option<String>,
    pub logical_position: Option<(i32, i32)>,
    pub logical_size: Option<(i32, i32)>,
    pub scale_factor: i32,
    pub transform: String,
    pub current_mode: Option<ModeSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModeSnapshot {
    pub width: i32,
    pub height: i32,
    pub refresh_rate_millihertz: i32,
}

impl From<&OutputInfo> for OutputSnapshot {
    fn from(info: &OutputInfo) -> Self {
        Self {
            global_id: info.id,
            name: info.name.clone(),
            description: info.description.clone(),
            logical_position: info.logical_position,
            logical_size: info.logical_size,
            scale_factor: info.scale_factor,
            transform: format!("{:?}", info.transform),
            current_mode: current_mode(&info.modes),
        }
    }
}

fn current_mode(modes: &[Mode]) -> Option<ModeSnapshot> {
    modes
        .iter()
        .find(|mode| mode.current)
        .map(|mode| ModeSnapshot {
            width: mode.dimensions.0,
            height: mode.dimensions.1,
            refresh_rate_millihertz: mode.refresh_rate,
        })
}

/// Owns the Wayland event queue and keeps output metadata current while the
/// connection is alive.
pub struct OutputTracker {
    connection: Connection,
    event_queue: EventQueue<OutputStateHolder>,
    state: OutputStateHolder,
}

impl OutputTracker {
    /// Connects to the active compositor, binds all current outputs, and waits
    /// for their initial metadata.
    ///
    /// # Errors
    ///
    /// Returns an error when Wayland is unavailable or registry initialization
    /// fails.
    pub fn connect() -> Result<Self, OutputError> {
        let connection = Connection::connect_to_env().map_err(OutputError::Connect)?;
        let (globals, event_queue) =
            registry_queue_init::<OutputStateHolder>(&connection).map_err(OutputError::Registry)?;
        let qh = event_queue.handle();
        let state = OutputStateHolder {
            registry_state: RegistryState::new(&globals),
            output_state: OutputState::new(&globals, &qh),
            snapshots: Vec::new(),
        };

        let mut tracker = Self {
            connection,
            event_queue,
            state,
        };
        tracker.roundtrip()?;
        Ok(tracker)
    }

    /// Dispatches pending output events and returns the number of dispatched
    /// events.
    ///
    /// # Errors
    ///
    /// Returns an error when the compositor closes the connection or sends an
    /// invalid Wayland message.
    pub fn roundtrip(&mut self) -> Result<usize, OutputError> {
        self.event_queue
            .roundtrip(&mut self.state)
            .map_err(OutputError::Dispatch)
    }

    #[must_use]
    pub fn snapshots(&self) -> &[OutputSnapshot] {
        &self.state.snapshots
    }

    #[must_use]
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}

#[derive(Debug)]
pub enum OutputError {
    Connect(wayland_client::ConnectError),
    Registry(GlobalError),
    Dispatch(DispatchError),
}

impl fmt::Display for OutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect(source) => write!(formatter, "could not connect to Wayland: {source}"),
            Self::Registry(source) => {
                write!(formatter, "could not read the Wayland registry: {source}")
            }
            Self::Dispatch(source) => write!(formatter, "Wayland output event failed: {source}"),
        }
    }
}

impl std::error::Error for OutputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Connect(source) => Some(source),
            Self::Registry(source) => Some(source),
            Self::Dispatch(source) => Some(source),
        }
    }
}

struct OutputStateHolder {
    registry_state: RegistryState,
    output_state: OutputState,
    snapshots: Vec<OutputSnapshot>,
}

impl ProvidesRegistryState for OutputStateHolder {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers!(OutputState);
}

impl OutputHandler for OutputStateHolder {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.refresh(&output);
    }

    fn update_output(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.refresh(&output);
    }

    fn output_destroyed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(info) = self.output_state.info(&output) {
            self.snapshots
                .retain(|snapshot| snapshot.global_id != info.id);
        }
    }
}

impl OutputStateHolder {
    fn refresh(&mut self, output: &wl_output::WlOutput) {
        let Some(info) = self.output_state.info(output) else {
            return;
        };
        let snapshot = OutputSnapshot::from(&info);

        if let Some(existing) = self
            .snapshots
            .iter_mut()
            .find(|existing| existing.global_id == snapshot.global_id)
        {
            *existing = snapshot;
        } else {
            self.snapshots.push(snapshot);
        }
    }
}

delegate_registry!(OutputStateHolder);
delegate_output!(OutputStateHolder);

#[cfg(test)]
mod tests {
    use smithay_client_toolkit::output::Mode;

    use super::{ModeSnapshot, current_mode};

    #[test]
    fn selects_current_mode() {
        let modes = vec![Mode {
            dimensions: (1920, 1080),
            refresh_rate: 60_000,
            current: true,
            preferred: true,
        }];

        assert_eq!(
            current_mode(&modes),
            Some(ModeSnapshot {
                width: 1920,
                height: 1080,
                refresh_rate_millihertz: 60_000,
            })
        );
    }

    #[test]
    fn returns_none_without_current_mode() {
        let modes = vec![Mode {
            dimensions: (1280, 720),
            refresh_rate: 60_000,
            current: false,
            preferred: true,
        }];

        assert_eq!(current_mode(&modes), None);
    }
}
