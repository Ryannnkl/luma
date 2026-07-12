use std::fmt;

use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    globals::{Global, GlobalError, GlobalListContents, registry_queue_init},
    protocol::{wl_compositor, wl_output, wl_registry, wl_seat, wl_shm},
};
use wayland_protocols::ext::session_lock::v1::client::ext_session_lock_manager_v1::ExtSessionLockManagerV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Capabilities {
    pub session_lock_version: Option<u32>,
    pub compositor_version: Option<u32>,
    pub shm_version: Option<u32>,
    pub output_count: usize,
    pub seat_count: usize,
}

impl Capabilities {
    /// Returns whether the compositor advertises the minimum globals needed by
    /// Luma's first opaque lock-surface implementation.
    #[must_use]
    pub const fn supports_lock_foundation(&self) -> bool {
        self.session_lock_version.is_some()
            && self.compositor_version.is_some()
            && self.shm_version.is_some()
            && self.output_count > 0
    }

    #[must_use]
    pub fn missing_requirements(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();

        if self.session_lock_version.is_none() {
            missing.push("ext_session_lock_manager_v1");
        }
        if self.compositor_version.is_none() {
            missing.push("wl_compositor");
        }
        if self.shm_version.is_none() {
            missing.push("wl_shm");
        }
        if self.output_count == 0 {
            missing.push("wl_output");
        }

        missing
    }
}

/// Connects to the active Wayland compositor and inspects its advertised globals.
///
/// This function never requests a session lock or creates a surface.
///
/// # Errors
///
/// Returns an error when no Wayland connection is available or the initial
/// registry roundtrip fails.
pub fn probe() -> Result<Capabilities, ProbeError> {
    let connection = Connection::connect_to_env().map_err(ProbeError::Connect)?;
    let (globals, _event_queue) =
        registry_queue_init::<ProbeState>(&connection).map_err(ProbeError::Registry)?;

    Ok(analyze(&globals.contents().clone_list()))
}

#[derive(Debug)]
pub enum ProbeError {
    Connect(wayland_client::ConnectError),
    Registry(GlobalError),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect(source) => write!(formatter, "could not connect to Wayland: {source}"),
            Self::Registry(source) => {
                write!(formatter, "could not read the Wayland registry: {source}")
            }
        }
    }
}

impl std::error::Error for ProbeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Connect(source) => Some(source),
            Self::Registry(source) => Some(source),
        }
    }
}

struct ProbeState;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for ProbeState {
    fn event(
        _state: &mut Self,
        _registry: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _globals: &GlobalListContents,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}

fn analyze(globals: &[Global]) -> Capabilities {
    Capabilities {
        session_lock_version: max_version(globals, ExtSessionLockManagerV1::interface().name),
        compositor_version: max_version(globals, wl_compositor::WlCompositor::interface().name),
        shm_version: max_version(globals, wl_shm::WlShm::interface().name),
        output_count: count(globals, wl_output::WlOutput::interface().name),
        seat_count: count(globals, wl_seat::WlSeat::interface().name),
    }
}

fn max_version(globals: &[Global], interface: &str) -> Option<u32> {
    globals
        .iter()
        .filter(|global| global.interface == interface)
        .map(|global| global.version)
        .max()
}

fn count(globals: &[Global], interface: &str) -> usize {
    globals
        .iter()
        .filter(|global| global.interface == interface)
        .count()
}

#[cfg(test)]
mod tests {
    use wayland_client::{Proxy, globals::Global, protocol};
    use wayland_protocols::ext::session_lock::v1::client::ext_session_lock_manager_v1::ExtSessionLockManagerV1;

    use super::analyze;

    #[test]
    fn recognizes_complete_lock_foundation() {
        let globals = vec![
            global(ExtSessionLockManagerV1::interface().name, 1),
            global(protocol::wl_compositor::WlCompositor::interface().name, 6),
            global(protocol::wl_shm::WlShm::interface().name, 1),
            global(protocol::wl_output::WlOutput::interface().name, 4),
            global(protocol::wl_seat::WlSeat::interface().name, 9),
        ];

        let capabilities = analyze(&globals);

        assert!(capabilities.supports_lock_foundation());
        assert_eq!(capabilities.output_count, 1);
        assert_eq!(capabilities.seat_count, 1);
        assert!(capabilities.missing_requirements().is_empty());
    }

    #[test]
    fn reports_missing_lock_protocol() {
        let globals = vec![
            global(protocol::wl_compositor::WlCompositor::interface().name, 6),
            global(protocol::wl_shm::WlShm::interface().name, 1),
            global(protocol::wl_output::WlOutput::interface().name, 4),
        ];

        let capabilities = analyze(&globals);

        assert!(!capabilities.supports_lock_foundation());
        assert_eq!(
            capabilities.missing_requirements(),
            ["ext_session_lock_manager_v1"]
        );
    }

    #[test]
    fn counts_multiple_outputs() {
        let globals = vec![
            global(protocol::wl_output::WlOutput::interface().name, 4),
            global(protocol::wl_output::WlOutput::interface().name, 4),
        ];

        assert_eq!(analyze(&globals).output_count, 2);
    }

    fn global(interface: &str, version: u32) -> Global {
        Global {
            name: 1,
            interface: interface.to_owned(),
            version,
        }
    }
}
