use std::{fmt, ops::RangeInclusive};

use serde::Deserialize;

use super::Color;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub window: WindowConfig,
    pub background: BackgroundConfig,
    pub clock: ClockConfig,
    pub date: DateConfig,
    pub input: InputConfig,
    pub demo_label: DemoLabelConfig,
}

impl Config {
    /// Checks all configurable values before they reach the renderer.
    ///
    /// # Errors
    ///
    /// Returns the first field that is non-finite, outside its supported range,
    /// internally inconsistent, empty, or excessively long.
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_range(self.window.width, 320.0..=16_384.0, "window.width")?;
        validate_range(self.window.height, 240.0..=16_384.0, "window.height")?;

        if self.background.spots.len() > 32 {
            return Err(ValidationError::new(
                "background.spots",
                "must contain at most 32 spots",
            ));
        }
        for (index, spot) in self.background.spots.iter().enumerate() {
            validate_range(spot.x, -1.0..=2.0, format!("background.spots[{index}].x"))?;
            validate_range(spot.y, -1.0..=2.0, format!("background.spots[{index}].y"))?;
            validate_range(
                spot.falloff,
                0.1..=100.0,
                format!("background.spots[{index}].falloff"),
            )?;
            validate_range(
                spot.strength,
                0.0..=1.0,
                format!("background.spots[{index}].strength"),
            )?;
        }

        validate_position(self.clock.x, self.clock.y, "clock")?;
        validate_range(self.clock.size_ratio, 0.01..=1.0, "clock.size_ratio")?;
        validate_range(self.clock.min_size, 8.0..=512.0, "clock.min_size")?;
        validate_range(self.clock.max_size, 8.0..=512.0, "clock.max_size")?;
        if self.clock.min_size > self.clock.max_size {
            return Err(ValidationError::new(
                "clock.min_size",
                "must not exceed clock.max_size",
            ));
        }
        validate_range(self.clock.line_gap_ratio, 0.1..=2.0, "clock.line_gap_ratio")?;
        validate_range(
            self.clock.hour_offset_x_ratio,
            -2.0..=2.0,
            "clock.hour_offset_x_ratio",
        )?;
        validate_range(
            self.clock.minute_offset_x_ratio,
            -2.0..=2.0,
            "clock.minute_offset_x_ratio",
        )?;
        validate_text(&self.clock.hour_format, "clock.hour_format", 128)?;
        validate_text(&self.clock.minute_format, "clock.minute_format", 128)?;

        validate_position(self.date.x, self.date.y, "date")?;
        validate_range(self.date.size, 8.0..=256.0, "date.size")?;
        validate_text(&self.date.format, "date.format", 128)?;

        validate_position(self.input.x, self.input.y, "input")?;
        validate_range(self.input.width, 24.0..=2_048.0, "input.width")?;
        validate_range(self.input.height, 16.0..=512.0, "input.height")?;
        validate_usize(self.input.max_characters, 1..=64, "input.max_characters")?;
        validate_usize(self.input.min_dots, 0..=64, "input.min_dots")?;
        if self.input.min_dots > self.input.max_characters {
            return Err(ValidationError::new(
                "input.min_dots",
                "must not exceed input.max_characters",
            ));
        }
        validate_range(self.input.dot_spacing, 1.0..=64.0, "input.dot_spacing")?;
        validate_range(
            self.input.empty_dot_radius,
            0.5..=32.0,
            "input.empty_dot_radius",
        )?;
        validate_range(
            self.input.filled_dot_radius,
            0.5..=32.0,
            "input.filled_dot_radius",
        )?;
        if self.input.feedback_duration_ms > 60_000 {
            return Err(ValidationError::new(
                "input.feedback_duration_ms",
                "must not exceed 60000",
            ));
        }
        validate_text(&self.input.feedback_text, "input.feedback_text", 128)?;

        validate_position(self.demo_label.x, self.demo_label.y, "demo_label")?;
        validate_range(self.demo_label.width, 24.0..=2_048.0, "demo_label.width")?;
        validate_range(self.demo_label.height, 16.0..=512.0, "demo_label.height")?;
        validate_range(
            self.demo_label.text_size,
            8.0..=128.0,
            "demo_label.text_size",
        )?;
        validate_text(&self.demo_label.text, "demo_label.text", 128)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ValidationError {
    field: String,
    requirement: &'static str,
}

impl ValidationError {
    fn new(field: impl Into<String>, requirement: &'static str) -> Self {
        Self {
            field: field.into(),
            requirement,
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}", self.field, self.requirement)
    }
}

impl std::error::Error for ValidationError {}

fn validate_position(x: f32, y: f32, prefix: &str) -> Result<(), ValidationError> {
    validate_range(x, 0.0..=1.0, format!("{prefix}.x"))?;
    validate_range(y, 0.0..=1.0, format!("{prefix}.y"))
}

fn validate_range(
    value: f32,
    range: RangeInclusive<f32>,
    field: impl Into<String>,
) -> Result<(), ValidationError> {
    if value.is_finite() && range.contains(&value) {
        Ok(())
    } else {
        Err(ValidationError::new(field, "is outside the allowed range"))
    }
}

fn validate_usize(
    value: usize,
    range: RangeInclusive<usize>,
    field: &'static str,
) -> Result<(), ValidationError> {
    if range.contains(&value) {
        Ok(())
    } else {
        Err(ValidationError::new(field, "is outside the allowed range"))
    }
}

fn validate_text(value: &str, field: &'static str, max: usize) -> Result<(), ValidationError> {
    if value.is_empty() {
        Err(ValidationError::new(field, "must not be empty"))
    } else if value.chars().count() > max {
        Err(ValidationError::new(field, "is too long"))
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WindowConfig {
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1280.0,
            height: 720.0,
            maximized: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BackgroundConfig {
    pub base_color: Color,
    pub dim_color: Color,
    pub spots: Vec<BackgroundSpot>,
}

impl Default for BackgroundConfig {
    fn default() -> Self {
        Self {
            base_color: Color::rgb(24, 52, 52),
            dim_color: Color::rgba(0, 0, 0, 82),
            spots: vec![
                BackgroundSpot {
                    x: 0.18,
                    y: 0.24,
                    falloff: 8.0,
                    strength: 0.55,
                    color: Color::rgb(74, 116, 95),
                },
                BackgroundSpot {
                    x: 0.76,
                    y: 0.18,
                    falloff: 7.0,
                    strength: 0.48,
                    color: Color::rgb(38, 91, 105),
                },
                BackgroundSpot {
                    x: 0.69,
                    y: 0.60,
                    falloff: 12.0,
                    strength: 0.48,
                    color: Color::rgb(145, 107, 58),
                },
            ],
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackgroundSpot {
    pub x: f32,
    pub y: f32,
    pub falloff: f32,
    pub strength: f32,
    pub color: Color,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClockConfig {
    pub enabled: bool,
    pub x: f32,
    pub y: f32,
    pub size_ratio: f32,
    pub min_size: f32,
    pub max_size: f32,
    pub line_gap_ratio: f32,
    pub hour_offset_x_ratio: f32,
    pub minute_offset_x_ratio: f32,
    pub hour_format: String,
    pub minute_format: String,
    pub hour_color: Color,
    pub minute_color: Color,
}

impl Default for ClockConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            x: 0.5,
            y: 0.46,
            size_ratio: 0.22,
            min_size: 96.0,
            max_size: 184.0,
            line_gap_ratio: 0.68,
            hour_offset_x_ratio: -0.10,
            minute_offset_x_ratio: 0.16,
            hour_format: "%H".to_owned(),
            minute_format: "%M".to_owned(),
            hour_color: Color::rgb(147, 230, 190),
            minute_color: Color::rgb(246, 248, 247),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DateConfig {
    pub enabled: bool,
    pub x: f32,
    pub y: f32,
    pub format: String,
    pub size: f32,
    pub color: Color,
}

impl Default for DateConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            x: 0.5,
            y: 0.72,
            format: "%A, %d %B".to_owned(),
            size: 22.0,
            color: Color::rgba(246, 248, 247, 220),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InputConfig {
    pub enabled: bool,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub max_characters: usize,
    pub min_dots: usize,
    pub dot_spacing: f32,
    pub empty_dot_radius: f32,
    pub filled_dot_radius: f32,
    pub background_color: Color,
    pub empty_dot_color: Color,
    pub filled_dot_color: Color,
    pub feedback_background_color: Color,
    pub feedback_text_color: Color,
    pub error_color: Color,
    pub feedback_text: String,
    pub feedback_duration_ms: u64,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            x: 0.5,
            y: 0.925,
            width: 156.0,
            height: 34.0,
            max_characters: 12,
            min_dots: 6,
            dot_spacing: 10.0,
            empty_dot_radius: 2.1,
            filled_dot_radius: 2.7,
            background_color: Color::rgba(0, 0, 0, 155),
            empty_dot_color: Color::rgba(255, 255, 255, 62),
            filled_dot_color: Color::rgb(236, 244, 240),
            feedback_background_color: Color::rgba(38, 92, 72, 210),
            feedback_text_color: Color::rgb(190, 244, 216),
            error_color: Color::rgb(255, 138, 128),
            feedback_text: "DEMO ONLY".to_owned(),
            feedback_duration_ms: 2_000,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DemoLabelConfig {
    pub enabled: bool,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub text_size: f32,
    pub background_color: Color,
    pub text_color: Color,
}

impl Default for DemoLabelConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            text: "DEMO  ·  ESC TO CLOSE".to_owned(),
            x: 0.085,
            y: 0.057,
            width: 174.0,
            height: 34.0,
            text_size: 12.0,
            background_color: Color::rgba(0, 0, 0, 112),
            text_color: Color::rgba(255, 255, 255, 210),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn missing_sections_use_defaults() {
        let config: Config = toml::from_str(
            r##"
                [clock]
                enabled = false
                hour_color = "#ff0000"
            "##,
        )
        .expect("partial configuration should use defaults");

        assert!(!config.clock.enabled);
        assert_eq!(config.clock.hour_color.channels(), [255, 0, 0, 255]);
        assert!(config.input.enabled);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let error = toml::from_str::<Config>(
            r"
                [clock]
                enabeld = false
            ",
        )
        .expect_err("misspelled fields must not be silently ignored");

        assert!(error.to_string().contains("unknown field `enabeld`"));
    }

    #[test]
    fn default_configuration_is_valid() {
        Config::default()
            .validate()
            .expect("built-in defaults must always be valid");
    }

    #[test]
    fn rejects_invalid_input_limits() {
        let mut config = Config::default();
        config.input.max_characters = 4;
        config.input.min_dots = 6;

        let error = config
            .validate()
            .expect_err("minimum dots cannot exceed the input limit");

        assert_eq!(error.field, "input.min_dots");
    }

    #[test]
    fn rejects_non_finite_positions() {
        let mut config = Config::default();
        config.clock.x = f32::NAN;

        let error = config
            .validate()
            .expect_err("non-finite positions must be rejected");

        assert_eq!(error.field, "clock.x");
    }
}
