use std::time::{Duration, Instant};

use chrono::{Local, Timelike};
use eframe::egui::{
    self, Align2, Color32, ColorImage, Event, FontFamily, FontId, Key, Pos2, Rect, TextureHandle,
    TextureOptions, Vec2,
};

const MAX_INPUT_LENGTH: usize = 12;

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Luma Demo")
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([640.0, 480.0])
            .with_maximized(true),
        ..Default::default()
    };

    eframe::run_native(
        "Luma Demo",
        options,
        Box::new(|creation_context| Ok(Box::new(DemoApp::new(creation_context)))),
    )
}

struct DemoApp {
    background: TextureHandle,
    input_length: usize,
    feedback_started_at: Option<Instant>,
}

impl DemoApp {
    fn new(creation_context: &eframe::CreationContext<'_>) -> Self {
        let background = creation_context.egui_ctx.load_texture(
            "luma-demo-background",
            create_background(),
            TextureOptions::LINEAR,
        );

        Self {
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
        let character_count = text
            .chars()
            .filter(|character| !character.is_control())
            .count();
        self.input_length = (self.input_length + character_count).min(MAX_INPUT_LENGTH);
    }

    fn is_showing_feedback(&self) -> bool {
        self.feedback_started_at
            .is_some_and(|started_at| started_at.elapsed() < Duration::from_secs(2))
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
        painter.rect_filled(rect, 0.0, Color32::from_black_alpha(82));

        paint_demo_label(ui);
        paint_clock(ui);
        paint_password_indicator(ui, self.input_length, self.is_showing_feedback());
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

fn paint_demo_label(ui: &egui::Ui) {
    let rect = ui.max_rect();
    let label_rect = Rect::from_min_size(
        rect.left_top() + Vec2::new(24.0, 24.0),
        Vec2::new(174.0, 34.0),
    );

    ui.painter()
        .rect_filled(label_rect, 17.0, Color32::from_black_alpha(112));
    ui.painter().text(
        label_rect.center(),
        Align2::CENTER_CENTER,
        "DEMO  ·  ESC TO CLOSE",
        FontId::new(12.0, FontFamily::Proportional),
        Color32::from_white_alpha(210),
    );
}

fn paint_clock(ui: &egui::Ui) {
    let rect = ui.max_rect();
    let now = Local::now();
    let hours = format!("{:02}", now.hour());
    let minutes = format!("{:02}", now.minute());
    let clock_size = (rect.height() * 0.22).clamp(96.0, 184.0);
    let line_gap = clock_size * 0.68;
    let center = rect.center() - Vec2::new(0.0, rect.height() * 0.04);
    let font = FontId::new(clock_size, FontFamily::Proportional);

    ui.painter().text(
        center - Vec2::new(clock_size * 0.10, line_gap * 0.55),
        Align2::CENTER_CENTER,
        hours,
        font.clone(),
        Color32::from_rgb(147, 230, 190),
    );
    ui.painter().text(
        center + Vec2::new(clock_size * 0.16, line_gap * 0.55),
        Align2::CENTER_CENTER,
        minutes,
        font,
        Color32::from_rgb(246, 248, 247),
    );
}

#[allow(clippy::cast_precision_loss)]
fn paint_password_indicator(ui: &egui::Ui, input_length: usize, showing_feedback: bool) {
    let rect = ui.max_rect();
    let indicator_rect = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.bottom() - 54.0),
        Vec2::new(156.0, 34.0),
    );
    let fill = if showing_feedback {
        Color32::from_rgba_unmultiplied(38, 92, 72, 210)
    } else {
        Color32::from_black_alpha(155)
    };

    ui.painter().rect_filled(indicator_rect, 17.0, fill);

    if showing_feedback {
        ui.painter().text(
            indicator_rect.center(),
            Align2::CENTER_CENTER,
            "DEMO ONLY",
            FontId::new(11.0, FontFamily::Proportional),
            Color32::from_rgb(190, 244, 216),
        );
        return;
    }

    let visible_dots = input_length.clamp(6, MAX_INPUT_LENGTH);
    let spacing = 10.0;
    let start_x =
        indicator_rect.center().x - (visible_dots.saturating_sub(1) as f32 * spacing / 2.0);

    for index in 0..visible_dots {
        let is_filled = index < input_length;
        let color = if is_filled {
            Color32::from_rgb(236, 244, 240)
        } else {
            Color32::from_white_alpha(62)
        };
        ui.painter().circle_filled(
            Pos2::new(start_x + index as f32 * spacing, indicator_rect.center().y),
            if is_filled { 2.7 } else { 2.1 },
            color,
        );
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn create_background() -> ColorImage {
    const WIDTH: usize = 160;
    const HEIGHT: usize = 90;
    let mut pixels = Vec::with_capacity(WIDTH * HEIGHT);

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let x = x as f32 / WIDTH as f32;
            let y = y as f32 / HEIGHT as f32;
            let green = soft_spot(x, y, 0.18, 0.24, 8.0);
            let blue = soft_spot(x, y, 0.76, 0.18, 7.0);
            let amber = soft_spot(x, y, 0.69, 0.60, 12.0);
            let shadow = soft_spot(x, y, 0.50, 1.05, 4.0);

            let red = 24.0 + green * 35.0 + blue * 11.0 + amber * 94.0 - shadow * 12.0;
            let green_channel = 52.0 + green * 62.0 + blue * 42.0 + amber * 55.0 - shadow * 16.0;
            let blue_channel = 52.0 + green * 43.0 + blue * 61.0 + amber * 20.0 - shadow * 15.0;

            pixels.push(Color32::from_rgb(
                red.clamp(0.0, 255.0) as u8,
                green_channel.clamp(0.0, 255.0) as u8,
                blue_channel.clamp(0.0, 255.0) as u8,
            ));
        }
    }

    ColorImage::new([WIDTH, HEIGHT], pixels)
}

fn soft_spot(x: f32, y: f32, center_x: f32, center_y: f32, falloff: f32) -> f32 {
    let distance = (x - center_x).powi(2) + (y - center_y).powi(2);
    (-distance * falloff).exp()
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use super::{DemoApp, MAX_INPUT_LENGTH};

    #[test]
    fn input_length_is_bounded() {
        let mut app = test_app();

        app.push_text("a very long demo input");

        assert_eq!(app.input_length, MAX_INPUT_LENGTH);
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
            background,
            input_length: 0,
            feedback_started_at: None,
        }
    }
}
