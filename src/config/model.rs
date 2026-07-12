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
}
