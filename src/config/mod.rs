mod color;
mod load;
mod model;

pub use color::Color;
pub use load::{LoadError, default_path};
pub use model::{BackgroundConfig, ClockConfig, Config, DateConfig, InputConfig, ValidationError};
