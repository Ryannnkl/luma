use std::fmt;

use smithay_client_toolkit::{
    delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shm::{Shm, ShmHandler, raw::RawPool},
};
use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum, delegate_noop,
    globals::{GlobalError, registry_queue_init},
    protocol::{wl_buffer, wl_output, wl_shm},
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};

use crate::renderer::{BackgroundImage, ImageError};

const MAX_CAPTURE_PIXELS: u64 = 16_777_216;
const MAX_TOTAL_CAPTURE_BYTES: u64 = 268_435_456;

pub(crate) struct CapturedOutput {
    pub name: String,
    pub image: BackgroundImage,
}

pub(crate) fn capture_outputs(blur_radius: u32) -> Result<Vec<CapturedOutput>, CaptureError> {
    let connection = Connection::connect_to_env().map_err(CaptureError::Connect)?;
    let (globals, mut event_queue) =
        registry_queue_init::<CaptureState>(&connection).map_err(CaptureError::Registry)?;
    let qh = event_queue.handle();
    let shm = Shm::bind(&globals, &qh).map_err(|error| CaptureError::Bind(error.to_string()))?;
    let manager = globals
        .bind(&qh, 1..=1, ())
        .map_err(|error| CaptureError::Bind(error.to_string()))?;
    let mut state = CaptureState {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        manager,
        pending: Vec::new(),
        completed: Vec::new(),
        failure: None,
        blur_radius,
        allocated_bytes: 0,
    };
    event_queue
        .roundtrip(&mut state)
        .map_err(CaptureError::Dispatch)?;

    let outputs: Vec<_> = state.output_state.outputs().collect();
    if outputs.is_empty() {
        return Err(CaptureError::NoOutputs);
    }
    for output in outputs {
        let name = state
            .output_state
            .info(&output)
            .and_then(|info| info.name)
            .ok_or(CaptureError::UnnamedOutput)?;
        let index = state.pending.len();
        let frame = state.manager.capture_output(0, &output, &qh, index);
        state.pending.push(PendingCapture {
            name,
            frame,
            parameters: None,
            pool: None,
            buffer: None,
            y_inverted: false,
            finished: false,
        });
    }

    while state.completed.len() < state.pending.len() && state.failure.is_none() {
        event_queue
            .blocking_dispatch(&mut state)
            .map_err(CaptureError::Dispatch)?;
    }
    if let Some(failure) = state.failure {
        return Err(failure);
    }
    if state.completed.len() != state.pending.len() {
        return Err(CaptureError::Incomplete);
    }
    Ok(state.completed)
}

struct CaptureState {
    registry_state: RegistryState,
    output_state: OutputState,
    shm: Shm,
    manager: ZwlrScreencopyManagerV1,
    pending: Vec<PendingCapture>,
    completed: Vec<CapturedOutput>,
    failure: Option<CaptureError>,
    blur_radius: u32,
    allocated_bytes: u64,
}

struct PendingCapture {
    name: String,
    frame: ZwlrScreencopyFrameV1,
    parameters: Option<BufferParameters>,
    pool: Option<RawPool>,
    buffer: Option<wl_buffer::WlBuffer>,
    y_inverted: bool,
    finished: bool,
}

#[derive(Clone, Copy)]
struct BufferParameters {
    format: wl_shm::Format,
    width: u32,
    height: u32,
    stride: u32,
}

impl CaptureState {
    fn start_copy(&mut self, index: usize, queue_handle: &QueueHandle<Self>) {
        let Some(pending) = self.pending.get_mut(index) else {
            self.failure = Some(CaptureError::Protocol("unknown capture frame"));
            return;
        };
        if pending.buffer.is_some() || pending.finished {
            return;
        }
        let Some(parameters) = pending.parameters else {
            self.failure = Some(CaptureError::UnsupportedFormat);
            return;
        };
        let pixel_count = u64::from(parameters.width).saturating_mul(u64::from(parameters.height));
        let buffer_bytes =
            u64::from(parameters.stride).saturating_mul(u64::from(parameters.height));
        let minimum_stride = u64::from(parameters.width).saturating_mul(4);
        if pixel_count == 0
            || pixel_count > MAX_CAPTURE_PIXELS
            || u64::from(parameters.stride) < minimum_stride
            || buffer_bytes > MAX_TOTAL_CAPTURE_BYTES.saturating_sub(self.allocated_bytes)
        {
            self.failure = Some(CaptureError::Buffer(
                "capture exceeds the configured memory safety limit".to_owned(),
            ));
            return;
        }
        let (Ok(width), Ok(height), Ok(stride)) = (
            i32::try_from(parameters.width),
            i32::try_from(parameters.height),
            i32::try_from(parameters.stride),
        ) else {
            self.failure = Some(CaptureError::Buffer(
                "capture dimensions exceed wl_shm limits".to_owned(),
            ));
            return;
        };
        let Ok(buffer_size) = usize::try_from(buffer_bytes) else {
            self.failure = Some(CaptureError::Buffer(
                "capture buffer exceeds platform limits".to_owned(),
            ));
            return;
        };
        let mut pool = match RawPool::new(buffer_size, &self.shm) {
            Ok(pool) => pool,
            Err(error) => {
                self.failure = Some(CaptureError::Buffer(error.to_string()));
                return;
            }
        };
        pool.mmap().fill(0);
        let buffer = pool.create_buffer(
            0,
            width,
            height,
            stride,
            parameters.format,
            (),
            queue_handle,
        );
        pending.frame.copy(&buffer);
        pending.pool = Some(pool);
        pending.buffer = Some(buffer);
        self.allocated_bytes = self.allocated_bytes.saturating_add(buffer_bytes);
    }

    fn finish(&mut self, index: usize) {
        let Some(pending) = self.pending.get_mut(index) else {
            self.failure = Some(CaptureError::Protocol("unknown completed capture frame"));
            return;
        };
        if pending.finished {
            return;
        }
        let Some(parameters) = pending.parameters else {
            self.failure = Some(CaptureError::UnsupportedFormat);
            return;
        };
        let Some(buffer) = pending.buffer.take() else {
            self.failure = Some(CaptureError::Incomplete);
            return;
        };
        let Some(pool) = pending.pool.as_mut() else {
            self.failure = Some(CaptureError::Incomplete);
            return;
        };
        match BackgroundImage::from_argb8888(
            parameters.width,
            parameters.height,
            parameters.stride,
            pool.mmap(),
            pending.y_inverted,
        ) {
            Ok(mut image) => {
                image.blur(self.blur_radius);
                self.completed.push(CapturedOutput {
                    name: pending.name.clone(),
                    image,
                });
                pending.finished = true;
                buffer.destroy();
                pending.frame.destroy();
            }
            Err(error) => self.failure = Some(CaptureError::Image(error)),
        }
    }
}

impl ProvidesRegistryState for CaptureState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers!(OutputState);
}

impl OutputHandler for CaptureState {
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

impl ShmHandler for CaptureState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, usize> for CaptureState {
    fn event(
        state: &mut Self,
        _frame: &ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        index: &usize,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                let format = match format {
                    WEnum::Value(wl_shm::Format::Argb8888 | wl_shm::Format::Xrgb8888) => {
                        format.into_result().expect("matched known format")
                    }
                    _ => return,
                };
                if let Some(pending) = state.pending.get_mut(*index) {
                    pending.parameters = Some(BufferParameters {
                        format,
                        width,
                        height,
                        stride,
                    });
                }
                state.start_copy(*index, queue_handle);
            }
            zwlr_screencopy_frame_v1::Event::Flags { flags } => {
                if let Some(pending) = state.pending.get_mut(*index) {
                    pending.y_inverted = flags.into_result().is_ok_and(|flags| {
                        flags.contains(zwlr_screencopy_frame_v1::Flags::YInvert)
                    });
                }
            }
            zwlr_screencopy_frame_v1::Event::Ready { .. } => state.finish(*index),
            zwlr_screencopy_frame_v1::Event::Failed => {
                state.failure = Some(CaptureError::Protocol("compositor rejected screen capture"));
            }
            _ => {}
        }
    }
}

#[derive(Debug)]
pub(crate) enum CaptureError {
    Connect(wayland_client::ConnectError),
    Registry(GlobalError),
    Dispatch(wayland_client::DispatchError),
    Bind(String),
    Buffer(String),
    Image(ImageError),
    NoOutputs,
    UnnamedOutput,
    UnsupportedFormat,
    Incomplete,
    Protocol(&'static str),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect(source) => write!(formatter, "could not connect for capture: {source}"),
            Self::Registry(source) => write!(formatter, "could not read capture globals: {source}"),
            Self::Dispatch(source) => write!(formatter, "capture dispatch failed: {source}"),
            Self::Bind(source) => write!(formatter, "could not bind capture dependency: {source}"),
            Self::Buffer(source) => {
                write!(formatter, "could not allocate capture buffer: {source}")
            }
            Self::Image(source) => write!(formatter, "could not normalize capture: {source}"),
            Self::NoOutputs => formatter.write_str("no outputs are available for capture"),
            Self::UnnamedOutput => formatter.write_str("captured output has no stable name"),
            Self::UnsupportedFormat => {
                formatter.write_str("compositor offered no supported capture format")
            }
            Self::Incomplete => {
                formatter.write_str("screen capture ended without a complete frame")
            }
            Self::Protocol(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for CaptureError {}

delegate_registry!(CaptureState);
delegate_output!(CaptureState);
delegate_shm!(CaptureState);
delegate_noop!(CaptureState: ignore ZwlrScreencopyManagerV1);
delegate_noop!(CaptureState: ignore wl_buffer::WlBuffer);
