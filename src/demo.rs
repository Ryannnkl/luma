use std::time::{Duration, Instant};

use chrono::Local;
use eframe::egui::{
    self, Align2, Color32, ColorImage, Event, FontFamily, FontId, Key, Pos2, Rect, TextureHandle,
    TextureOptions, Vec2,
};

use crate::config::{BackgroundConfig, Color, Config, DateConfig, DemoLabelConfig, InputConfig};

pub fn run(config: Config) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Luma Demo")
            .with_inner_size([config.window.width, config.window.height])
            .with_min_inner_size([640.0, 480.0])
            .with_maximized(config.window.maximized),
        ..Default::default()
    };

    eframe::run_native(
        "Luma Demo",
        options,
        Box::new(|creation_context| Ok(Box::new(DemoApp::new(creation_context, config)))),
    )
}

struct DemoApp {
    config: Config,
    background: TextureHandle,
    input_length: usize,
    feedback_started_at: Option<Instant>,
}

impl DemoApp {
    fn new(creation_context: &eframe::CreationContext<'_>, config: Config) -> Self {
        let background = creation_context.egui_ctx.load_texture(
            "luma-demo-background",
            create_background(&config.background),
            TextureOptions::LINEAR,
        );

        Self {
            config,
            background,
            input_length: 0,
            feedback_started_at: None,
        }
    }

    fn handle_input(&mut self, context: &egui::Context) {
        let events = context.input(|input| input.events.clone());

        for event in events {
            match event {
                Event::Text(text) => self.push_text(&text),
                Event::Key {
                    key: Key::Backspace,
                    pressed: true,
                    ..
                } => self.input_length = self.input_length.saturating_sub(1),
                Event::Key {
                    key: Key::Enter,
                    pressed: true,
                    ..
                } => {
                    self.input_length = 0;
                    self.feedback_started_at = Some(Instant::now());
                }
                Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    ..
                } => context.send_viewport_cmd(egui::ViewportCommand::Close),
                _ => {}
            }
        }
    }

    fn push_text(&mut self, text: &str) {
        if !self.config.input.enabled {
            return;
        }

        let character_count = text
            .chars()
            .filter(|character| !character.is_control())
            .count();
        self.input_length =
            (self.input_length + character_count).min(self.config.input.max_characters);
    }

    fn is_showing_feedback(&self) -> bool {
        self.feedback_started_at.is_some_and(|started_at| {
            started_at.elapsed() < Duration::from_millis(self.config.input.feedback_duration_ms)
        })
    }

    fn paint(&self, ui: &egui::Ui) {
        let rect = ui.max_rect();
        let painter = ui.painter();

        painter.image(
            self.background.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        painter.rect_filled(rect, 0.0, to_egui_color(self.config.background.dim_color));

        paint_demo_label(ui, &self.config.demo_label);
        paint_clock(ui, &self.config);
        paint_date(ui, &self.config.date);
        paint_password_indicator(
            ui,
            &self.config.input,
            self.input_length,
            self.is_showing_feedback(),
        );
    }
}

impl eframe::App for DemoApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_input(context);

        let repaint_interval = if self.is_showing_feedback() {
            Duration::from_millis(50)
        } else {
            Duration::from_secs(1)
        };
        context.request_repaint_after(repaint_interval);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.paint(ui);
    }
}

fn paint_demo_label(ui: &egui::Ui, config: &DemoLabelConfig) {
    if !config.enabled {
        return;
    }

    let rect = ui.max_rect();
    let label_rect = Rect::from_center_size(
        position(rect, config.x, config.y),
        Vec2::new(config.width, config.height),
    );

    ui.painter().rect_filled(
        label_rect,
        config.height / 2.0,
        to_egui_color(config.background_color),
    );
    ui.painter().text(
        label_rect.center(),
        Align2::CENTER_CENTER,
        &config.text,
        FontId::new(config.text_size, FontFamily::Proportional),
        to_egui_color(config.text_color),
    );
}

fn paint_clock(ui: &egui::Ui, config: &Config) {
    if !config.clock.enabled {
        return;
    }

    let rect = ui.max_rect();
    let now = Local::now();
    let hours = now.format(&config.clock.hour_format).to_string();
    let minutes = now.format(&config.clock.minute_format).to_string();
    let clock_size = (rect.height() * config.clock.size_ratio)
        .clamp(config.clock.min_size, config.clock.max_size);
    let line_gap = clock_size * config.clock.line_gap_ratio;
    let center = position(rect, config.clock.x, config.clock.y);
    let font = FontId::new(clock_size, FontFamily::Proportional);

    ui.painter().text(
        center
            + Vec2::new(
                clock_size * config.clock.hour_offset_x_ratio,
                -line_gap * 0.55,
            ),
        Align2::CENTER_CENTER,
        hours,
        font.clone(),
        to_egui_color(config.clock.hour_color),
    );
    ui.painter().text(
        center
            + Vec2::new(
                clock_size * config.clock.minute_offset_x_ratio,
                line_gap * 0.55,
            ),
        Align2::CENTER_CENTER,
        minutes,
        font,
        to_egui_color(config.clock.minute_color),
    );
}

fn paint_date(ui: &egui::Ui, config: &DateConfig) {
    if !config.enabled {
        return;
    }

    let rect = ui.max_rect();
    ui.painter().text(
        position(rect, config.x, config.y),
        Align2::CENTER_CENTER,
        Local::now().format(&config.format).to_string(),
        FontId::new(config.size, FontFamily::Proportional),
        to_egui_color(config.color),
    );
}

#[allow(clippy::cast_precision_loss)]
fn paint_password_indicator(
    ui: &egui::Ui,
    config: &InputConfig,
    input_length: usize,
    showing_feedback: bool,
) {
    if !config.enabled {
        return;
    }

    let rect = ui.max_rect();
    let indicator_rect = Rect::from_center_size(
        position(rect, config.x, config.y),
        Vec2::new(config.width, config.height),
    );
    let fill = if showing_feedback {
        to_egui_color(config.feedback_background_color)
    } else {
        to_egui_color(config.background_color)
    };

    ui.painter()
        .rect_filled(indicator_rect, config.height / 2.0, fill);

    if showing_feedback {
        ui.painter().text(
            indicator_rect.center(),
            Align2::CENTER_CENTER,
            &config.feedback_text,
            FontId::new(11.0, FontFamily::Proportional),
            to_egui_color(config.feedback_text_color),
        );
        return;
    }

    let visible_dots = input_length.clamp(config.min_dots, config.max_characters);
    let spacing = config.dot_spacing;
    let start_x =
        indicator_rect.center().x - (visible_dots.saturating_sub(1) as f32 * spacing / 2.0);

    for index in 0..visible_dots {
        let is_filled = index < input_length;
        let color = if is_filled {
            to_egui_color(config.filled_dot_color)
        } else {
            to_egui_color(config.empty_dot_color)
        };
        ui.painter().circle_filled(
            Pos2::new(start_x + index as f32 * spacing, indicator_rect.center().y),
            if is_filled {
                config.filled_dot_radius
            } else {
                config.empty_dot_radius
            },
            color,
        );
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn create_background(config: &BackgroundConfig) -> ColorImage {
    const WIDTH: usize = 160;
    const HEIGHT: usize = 90;
    let mut pixels = Vec::with_capacity(WIDTH * HEIGHT);

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let x = x as f32 / WIDTH as f32;
            let y = y as f32 / HEIGHT as f32;
            let mut color = config.base_color.channels();
            for spot in &config.spots {
                let weight = soft_spot(x, y, spot.x, spot.y, spot.falloff) * spot.strength;
                color = blend(color, spot.color.channels(), weight);
            }

            pixels.push(Color32::from_rgb(color[0], color[1], color[2]));
        }
    }

    ColorImage::new([WIDTH, HEIGHT], pixels)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn blend(base: [u8; 4], overlay: [u8; 4], weight: f32) -> [u8; 4] {
    let blend_channel = |base: u8, overlay: u8| {
        (f32::from(base) + (f32::from(overlay) - f32::from(base)) * weight.clamp(0.0, 1.0)).round()
            as u8
    };

    [
        blend_channel(base[0], overlay[0]),
        blend_channel(base[1], overlay[1]),
        blend_channel(base[2], overlay[2]),
        255,
    ]
}

fn position(rect: Rect, x: f32, y: f32) -> Pos2 {
    Pos2::new(
        rect.left() + rect.width() * x,
        rect.top() + rect.height() * y,
    )
}

fn to_egui_color(color: Color) -> Color32 {
    let [red, green, blue, alpha] = color.channels();
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}

fn soft_spot(x: f32, y: f32, center_x: f32, center_y: f32, falloff: f32) -> f32 {
    let distance = (x - center_x).powi(2) + (y - center_y).powi(2);
    (-distance * falloff).exp()
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use crate::config::Config;

    use super::DemoApp;

    #[test]
    fn input_length_is_bounded() {
        let mut app = test_app();

        app.push_text("a very long demo input");

        assert_eq!(app.input_length, app.config.input.max_characters);
    }

    #[test]
    fn control_characters_are_ignored() {
        let mut app = test_app();

        app.push_text("a\nb\tc");

        assert_eq!(app.input_length, 3);
    }

    fn test_app() -> DemoApp {
        // The texture is not inspected by input-model tests.
        let context = egui::Context::default();
        let background = context.load_texture(
            "test-background",
            egui::ColorImage::filled([1, 1], egui::Color32::BLACK),
            egui::TextureOptions::LINEAR,
        );

        DemoApp {
            config: Config::default(),
            background,
            input_length: 0,
            feedback_started_at: None,
        }
    }
}
