use chrono::{DateTime, Local};

use crate::{
    config::{ClockConfig, Color, DateConfig, InputConfig},
    renderer::{ClipRectangle, TextRenderer},
};

const BACKGROUND: Rgba = Rgba::new(18, 26, 28, 255);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PromptState {
    Ready,
    Authenticating,
    Failure,
    Cooldown,
}

#[derive(Clone, Copy)]
struct Rgba {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl Rgba {
    const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    const fn from_config(color: Color) -> Self {
        let [red, green, blue, alpha] = color.channels();
        Self::new(red, green, blue, alpha)
    }
}

#[derive(Clone, Copy)]
struct Rectangle {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

/// Draws Luma's opaque fallback and authentication prompt into an ARGB8888 canvas.
///
/// The ready indicator represents only the number of entered characters. Feedback states do not
/// render either the password contents or its length.
pub(crate) fn draw_lock_frame(
    canvas: &mut [u8],
    width: i32,
    height: i32,
    password_length: usize,
    prompt_state: PromptState,
    config: &InputConfig,
) {
    let Ok(width) = usize::try_from(width) else {
        return;
    };
    let Ok(height) = usize::try_from(height) else {
        return;
    };

    fill(canvas, BACKGROUND);
    draw_lock_prompt(canvas, width, height, password_length, prompt_state, config);
}

pub(crate) fn draw_lock_prompt(
    canvas: &mut [u8],
    width: usize,
    height: usize,
    password_length: usize,
    prompt_state: PromptState,
    config: &InputConfig,
) {
    if width < 80 || height < 80 {
        return;
    }

    let prompt = prompt_rectangle(width, height, config);
    let ready_background = opaque_over(Rgba::from_config(config.background_color), BACKGROUND);
    let feedback_background = opaque_over(
        Rgba::from_config(config.feedback_background_color),
        BACKGROUND,
    );

    let prompt_background = match prompt_state {
        PromptState::Ready | PromptState::Authenticating => ready_background,
        PromptState::Failure | PromptState::Cooldown => feedback_background,
    };
    fill_rect(
        canvas,
        width,
        height,
        prompt.x,
        prompt.y,
        prompt.width,
        prompt.height,
        prompt_background,
    );

    match prompt_state {
        PromptState::Ready => {
            draw_password_dots(
                canvas,
                width,
                height,
                prompt,
                password_length,
                config,
                ready_background,
            );
        }
        PromptState::Authenticating => {
            let color = opaque_over(Rgba::from_config(config.filled_dot_color), ready_background);
            draw_centered_dots(canvas, width, height, prompt, color, config);
        }
        PromptState::Failure => {
            let color = opaque_over(
                Rgba::from_config(config.feedback_accent_color),
                feedback_background,
            );
            draw_failure_marker(canvas, width, height, prompt, color);
        }
        PromptState::Cooldown => {
            let color = opaque_over(
                Rgba::from_config(config.empty_dot_color),
                feedback_background,
            );
            draw_centered_dots(canvas, width, height, prompt, color, config);
        }
    }
}

/// Draws configured time and date text over the opaque fallback.
pub(crate) fn draw_lock_visuals(
    canvas: &mut [u8],
    width: i32,
    height: i32,
    clock: &ClockConfig,
    date: &DateConfig,
    renderer: &TextRenderer,
    now: DateTime<Local>,
) {
    let (Ok(width), Ok(height)) = (usize::try_from(width), usize::try_from(height)) else {
        return;
    };
    let clip = ClipRectangle {
        x: 0,
        y: 0,
        width,
        height,
    };

    if clock.enabled {
        #[allow(clippy::cast_precision_loss)]
        let clock_size = (height as f32 * clock.size_ratio).clamp(clock.min_size, clock.max_size);
        let line_gap = clock_size * clock.line_gap_ratio;
        #[allow(clippy::cast_precision_loss)]
        let center = (clock.x * width as f32, clock.y * height as f32);
        let hour = now.format(&clock.hour_format).to_string();
        let minute = now.format(&clock.minute_format).to_string();
        renderer.draw_centered(
            canvas,
            width,
            height,
            clip,
            (
                center.0 + clock_size * clock.hour_offset_x_ratio,
                center.1 - line_gap * 0.55,
            ),
            clock_size,
            &hour,
            clock.hour_color,
        );
        renderer.draw_centered(
            canvas,
            width,
            height,
            clip,
            (
                center.0 + clock_size * clock.minute_offset_x_ratio,
                center.1 + line_gap * 0.55,
            ),
            clock_size,
            &minute,
            clock.minute_color,
        );
    }

    if date.enabled {
        #[allow(clippy::cast_precision_loss)]
        let center = (date.x * width as f32, date.y * height as f32);
        let formatted = now.format(&date.format).to_string();
        renderer.draw_centered(
            canvas, width, height, clip, center, date.size, &formatted, date.color,
        );
    }
}

pub(crate) fn draw_lock_visual_feedback(
    canvas: &mut [u8],
    width: i32,
    height: i32,
    prompt_state: PromptState,
    config: &InputConfig,
    frame: u64,
) {
    let (Ok(width), Ok(height)) = (usize::try_from(width), usize::try_from(height)) else {
        return;
    };
    if width < 80 || height < 80 || prompt_state == PromptState::Ready {
        return;
    }

    let prompt = prompt_rectangle(width, height, config);
    let ready_background = opaque_over(Rgba::from_config(config.background_color), BACKGROUND);
    let feedback_background = opaque_over(
        Rgba::from_config(config.feedback_background_color),
        BACKGROUND,
    );
    match prompt_state {
        PromptState::Authenticating => {
            fill_prompt(canvas, width, height, prompt, ready_background);
            draw_loading_dots(
                canvas,
                width,
                height,
                prompt,
                config,
                frame,
                ready_background,
            );
        }
        PromptState::Failure => {
            fill_prompt(canvas, width, height, prompt, BACKGROUND);
            let shifted = shifted_prompt(prompt, width, frame);
            fill_prompt(canvas, width, height, shifted, feedback_background);
            let accent = opaque_over(Rgba::from_config(config.error_color), feedback_background);
            draw_border(canvas, width, height, shifted, accent);
            draw_cross(canvas, width, height, shifted, accent);
        }
        PromptState::Cooldown => {
            fill_prompt(canvas, width, height, prompt, feedback_background);
            draw_cooldown_dots(
                canvas,
                width,
                height,
                prompt,
                config,
                frame,
                feedback_background,
            );
        }
        PromptState::Ready => {}
    }
}

fn fill_prompt(canvas: &mut [u8], width: usize, height: usize, prompt: Rectangle, color: Rgba) {
    fill_rect(
        canvas,
        width,
        height,
        prompt.x,
        prompt.y,
        prompt.width,
        prompt.height,
        color,
    );
}

fn draw_loading_dots(
    canvas: &mut [u8],
    width: usize,
    height: usize,
    prompt: Rectangle,
    config: &InputConfig,
    frame: u64,
    background: Rgba,
) {
    let dot_count = 3;
    let active = usize::try_from(frame % dot_count as u64).unwrap_or_default();
    let maximum_radius = maximum_dot_radius(prompt);
    let small_radius = rounded_size(config.empty_dot_radius).min(maximum_radius);
    let large_radius = rounded_size(config.filled_dot_radius)
        .saturating_add(1)
        .min(maximum_radius);
    let spacing = fitted_spacing(
        prompt,
        dot_count,
        rounded_size(config.dot_spacing),
        large_radius,
    );
    let start_x = prompt
        .x
        .saturating_add(prompt.width / 2)
        .saturating_sub(spacing);
    let center_y = prompt.y.saturating_add(prompt.height / 2);
    let muted = opaque_over(Rgba::from_config(config.empty_dot_color), background);
    let bright = opaque_over(Rgba::from_config(config.filled_dot_color), background);
    for index in 0..dot_count {
        fill_circle(
            canvas,
            width,
            height,
            start_x.saturating_add(index.saturating_mul(spacing)),
            center_y,
            if index == active {
                large_radius
            } else {
                small_radius
            },
            if index == active { bright } else { muted },
        );
    }
}

fn draw_cooldown_dots(
    canvas: &mut [u8],
    width: usize,
    height: usize,
    prompt: Rectangle,
    config: &InputConfig,
    frame: u64,
    background: Rgba,
) {
    let dot_count = 6;
    let active = usize::try_from(frame % dot_count as u64).unwrap_or_default();
    let radius = rounded_size(config.empty_dot_radius).min(maximum_dot_radius(prompt));
    let spacing = fitted_spacing(prompt, dot_count, rounded_size(config.dot_spacing), radius);
    let span = spacing.saturating_mul(dot_count.saturating_sub(1));
    let start_x = prompt
        .x
        .saturating_add(prompt.width / 2)
        .saturating_sub(span / 2);
    let center_y = prompt.y.saturating_add(prompt.height / 2);
    let muted = opaque_over(Rgba::from_config(config.empty_dot_color), background);
    let accent = opaque_over(Rgba::from_config(config.feedback_accent_color), background);
    for index in 0..dot_count {
        fill_circle(
            canvas,
            width,
            height,
            start_x.saturating_add(index.saturating_mul(spacing)),
            center_y,
            radius,
            if index == active { accent } else { muted },
        );
    }
}

fn shifted_prompt(prompt: Rectangle, width: usize, frame: u64) -> Rectangle {
    const OFFSETS: [isize; 8] = [0, -4, 4, -3, 3, -1, 1, 0];
    let index = usize::try_from(frame)
        .unwrap_or(OFFSETS.len())
        .min(OFFSETS.len().saturating_sub(1));
    let maximum_x = width.saturating_sub(prompt.width);
    Rectangle {
        x: prompt
            .x
            .saturating_add_signed(OFFSETS[index])
            .min(maximum_x),
        ..prompt
    }
}

fn draw_border(canvas: &mut [u8], width: usize, height: usize, rectangle: Rectangle, color: Rgba) {
    let thickness = 2_usize.min(rectangle.width).min(rectangle.height);
    fill_rect(
        canvas,
        width,
        height,
        rectangle.x,
        rectangle.y,
        rectangle.width,
        thickness,
        color,
    );
    fill_rect(
        canvas,
        width,
        height,
        rectangle.x,
        rectangle
            .y
            .saturating_add(rectangle.height.saturating_sub(thickness)),
        rectangle.width,
        thickness,
        color,
    );
    fill_rect(
        canvas,
        width,
        height,
        rectangle.x,
        rectangle.y,
        thickness,
        rectangle.height,
        color,
    );
    fill_rect(
        canvas,
        width,
        height,
        rectangle
            .x
            .saturating_add(rectangle.width.saturating_sub(thickness)),
        rectangle.y,
        thickness,
        rectangle.height,
        color,
    );
}

fn draw_cross(canvas: &mut [u8], width: usize, height: usize, prompt: Rectangle, color: Rgba) {
    let size = (prompt.height / 3).clamp(5, 12);
    let start_x = prompt
        .x
        .saturating_add(prompt.width / 2)
        .saturating_sub(size / 2);
    let start_y = prompt
        .y
        .saturating_add(prompt.height / 2)
        .saturating_sub(size / 2);
    for offset in 0..=size {
        fill_circle(
            canvas,
            width,
            height,
            start_x.saturating_add(offset),
            start_y.saturating_add(offset),
            1,
            color,
        );
        fill_circle(
            canvas,
            width,
            height,
            start_x.saturating_add(size.saturating_sub(offset)),
            start_y.saturating_add(offset),
            1,
            color,
        );
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn prompt_rectangle(width: usize, height: usize, config: &InputConfig) -> Rectangle {
    let prompt_width = (config.width.round() as usize).clamp(1, width);
    let prompt_height = (config.height.round() as usize).clamp(1, height);
    let center_x = (config.x * width as f32).round() as usize;
    let center_y = (config.y * height as f32).round() as usize;
    Rectangle {
        x: center_x
            .saturating_sub(prompt_width / 2)
            .min(width.saturating_sub(prompt_width)),
        y: center_y
            .saturating_sub(prompt_height / 2)
            .min(height.saturating_sub(prompt_height)),
        width: prompt_width,
        height: prompt_height,
    }
}

fn draw_password_dots(
    canvas: &mut [u8],
    width: usize,
    height: usize,
    prompt: Rectangle,
    password_length: usize,
    config: &InputConfig,
    prompt_background: Rgba,
) {
    let dot_count = password_length.clamp(config.min_dots, config.max_characters);
    let maximum_prompt_radius = maximum_dot_radius(prompt);
    let empty_radius = rounded_size(config.empty_dot_radius).min(maximum_prompt_radius);
    let filled_radius = rounded_size(config.filled_dot_radius).min(maximum_prompt_radius);
    let maximum_radius = empty_radius.max(filled_radius);
    let spacing = fitted_spacing(
        prompt,
        dot_count,
        rounded_size(config.dot_spacing),
        maximum_radius,
    );
    let span = dot_count.saturating_sub(1).saturating_mul(spacing);
    let start_x = prompt
        .x
        .saturating_add(prompt.width / 2)
        .saturating_sub(span / 2);
    let center_y = prompt.y.saturating_add(prompt.height / 2);
    let empty_color = opaque_over(Rgba::from_config(config.empty_dot_color), prompt_background);
    let filled_color = opaque_over(
        Rgba::from_config(config.filled_dot_color),
        prompt_background,
    );

    for dot_index in 0..dot_count {
        let center_x = start_x.saturating_add(dot_index.saturating_mul(spacing));
        let (radius, color) = if dot_index < password_length {
            (filled_radius, filled_color)
        } else {
            (empty_radius, empty_color)
        };
        fill_circle(canvas, width, height, center_x, center_y, radius, color);
    }
}

fn draw_centered_dots(
    canvas: &mut [u8],
    width: usize,
    height: usize,
    prompt: Rectangle,
    color: Rgba,
    config: &InputConfig,
) {
    let dot_count = 3_usize;
    let radius = rounded_size(config.filled_dot_radius).min(maximum_dot_radius(prompt));
    let spacing = fitted_spacing(prompt, dot_count, rounded_size(config.dot_spacing), radius);
    let span = dot_count.saturating_sub(1).saturating_mul(spacing);
    let start_x = prompt
        .x
        .saturating_add(prompt.width / 2)
        .saturating_sub(span / 2);
    let center_y = prompt.y.saturating_add(prompt.height / 2);

    for dot_index in 0..dot_count {
        let center_x = start_x.saturating_add(dot_index.saturating_mul(spacing));
        fill_circle(canvas, width, height, center_x, center_y, radius, color);
    }
}

fn draw_failure_marker(
    canvas: &mut [u8],
    width: usize,
    height: usize,
    prompt: Rectangle,
    color: Rgba,
) {
    let center_x = prompt.x.saturating_add(prompt.width / 2);
    let center_y = prompt.y.saturating_add(prompt.height / 2);
    let marker_width = (prompt.height / 8).clamp(1, 4);
    let marker_height = (prompt.height / 3).clamp(3, 11);
    let marker_x = center_x.saturating_sub(marker_width / 2);
    let marker_y = center_y.saturating_sub(prompt.height / 4);
    let dot_radius = (prompt.height / 12).clamp(1, 2);
    let dot_y = prompt.y.saturating_add(prompt.height.saturating_mul(3) / 4);
    fill_rect(
        canvas,
        width,
        height,
        marker_x,
        marker_y,
        marker_width,
        marker_height,
        color,
    );
    fill_circle(canvas, width, height, center_x, dot_y, dot_radius, color);
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn rounded_size(value: f32) -> usize {
    (value.round() as usize).max(1)
}

fn fitted_spacing(
    prompt: Rectangle,
    dot_count: usize,
    configured_spacing: usize,
    radius: usize,
) -> usize {
    if dot_count <= 1 {
        return 0;
    }
    let available_span = prompt
        .width
        .saturating_sub(1)
        .saturating_sub(radius.saturating_mul(2));
    configured_spacing.min(available_span / dot_count.saturating_sub(1))
}

fn maximum_dot_radius(prompt: Rectangle) -> usize {
    prompt
        .width
        .saturating_sub(1)
        .min(prompt.height.saturating_sub(1))
        / 2
}

fn opaque_over(foreground: Rgba, background: Rgba) -> Rgba {
    let alpha = u16::from(foreground.alpha);
    let inverse_alpha = 255_u16.saturating_sub(alpha);
    let blend = |foreground: u8, background: u8| {
        let value = u16::from(foreground)
            .saturating_mul(alpha)
            .saturating_add(u16::from(background).saturating_mul(inverse_alpha))
            .saturating_add(127)
            / 255;
        u8::try_from(value).unwrap_or(u8::MAX)
    };
    Rgba::new(
        blend(foreground.red, background.red),
        blend(foreground.green, background.green),
        blend(foreground.blue, background.blue),
        255,
    )
}

fn fill(canvas: &mut [u8], color: Rgba) {
    for pixel in canvas.chunks_exact_mut(4) {
        write_pixel(pixel, color);
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_rect(
    canvas: &mut [u8],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    rectangle_width: usize,
    rectangle_height: usize,
    color: Rgba,
) {
    let end_x = x.saturating_add(rectangle_width).min(width);
    let end_y = y.saturating_add(rectangle_height).min(height);

    for row in y.min(height)..end_y {
        for column in x.min(width)..end_x {
            set_pixel(canvas, width, column, row, color);
        }
    }
}

fn fill_circle(
    canvas: &mut [u8],
    width: usize,
    height: usize,
    center_x: usize,
    center_y: usize,
    radius: usize,
    color: Rgba,
) {
    let radius_squared = radius.saturating_mul(radius);
    let start_x = center_x.saturating_sub(radius);
    let end_x = center_x.saturating_add(radius).min(width.saturating_sub(1));
    let start_y = center_y.saturating_sub(radius);
    let end_y = center_y
        .saturating_add(radius)
        .min(height.saturating_sub(1));

    for y in start_y..=end_y {
        for x in start_x..=end_x {
            let distance_x = x.abs_diff(center_x);
            let distance_y = y.abs_diff(center_y);
            if distance_x
                .saturating_mul(distance_x)
                .saturating_add(distance_y.saturating_mul(distance_y))
                <= radius_squared
            {
                set_pixel(canvas, width, x, y, color);
            }
        }
    }
}

fn set_pixel(canvas: &mut [u8], width: usize, x: usize, y: usize, color: Rgba) {
    let Some(pixel_index) = y
        .checked_mul(width)
        .and_then(|offset| offset.checked_add(x))
        .and_then(|offset| offset.checked_mul(4))
    else {
        return;
    };
    let Some(pixel) = canvas.get_mut(pixel_index..pixel_index.saturating_add(4)) else {
        return;
    };
    write_pixel(pixel, color);
}

fn write_pixel(pixel: &mut [u8], color: Rgba) {
    #[cfg(target_endian = "little")]
    pixel.copy_from_slice(&[color.blue, color.green, color.red, color.alpha]);

    #[cfg(target_endian = "big")]
    pixel.copy_from_slice(&[color.alpha, color.red, color.green, color.blue]);
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone};

    use crate::config::{Color, InputConfig};

    use super::{
        BACKGROUND, PromptState, Rgba, draw_lock_frame, draw_lock_visual_feedback,
        draw_lock_visuals, opaque_over,
    };

    #[test]
    fn draws_an_opaque_background_for_small_outputs() {
        let mut canvas = vec![0; 4 * 32 * 32];
        let config = InputConfig::default();

        draw_lock_frame(&mut canvas, 32, 32, 0, PromptState::Ready, &config);

        assert_eq!(canvas[..4], encoded(BACKGROUND));
        assert!(
            canvas
                .chunks_exact(4)
                .all(|pixel| pixel == encoded(BACKGROUND))
        );
    }

    #[test]
    fn fills_dots_without_rendering_password_contents() {
        let width = 200;
        let height = 120;
        let frame_width = i32::try_from(width).expect("test width fits in i32");
        let frame_height = i32::try_from(height).expect("test height fits in i32");
        let mut empty_canvas = vec![0; width * height * 4];
        let mut filled_canvas = vec![0; width * height * 4];
        let config = InputConfig::default();

        draw_lock_frame(
            &mut empty_canvas,
            frame_width,
            frame_height,
            0,
            PromptState::Ready,
            &config,
        );
        draw_lock_frame(
            &mut filled_canvas,
            frame_width,
            frame_height,
            1,
            PromptState::Ready,
            &config,
        );

        let center_x = 75;
        let center_y = 103;
        let pixel_index = (center_y * width + center_x) * 4;
        assert_ne!(
            empty_canvas[pixel_index..pixel_index + 4],
            filled_canvas[pixel_index..pixel_index + 4]
        );
        assert_eq!(
            filled_canvas[pixel_index..pixel_index + 4],
            encoded(opaque_over(
                Rgba::from_config(config.filled_dot_color),
                opaque_over(Rgba::from_config(config.background_color), BACKGROUND),
            ))
        );
    }

    #[test]
    fn failure_feedback_hides_password_length() {
        let width = 200;
        let height = 120;
        let mut short_password = vec![0; width * height * 4];
        let mut long_password = vec![0; width * height * 4];
        let config = InputConfig::default();

        draw_lock_frame(
            &mut short_password,
            200,
            120,
            1,
            PromptState::Failure,
            &config,
        );
        draw_lock_frame(
            &mut long_password,
            200,
            120,
            12,
            PromptState::Failure,
            &config,
        );

        assert_eq!(short_password, long_password);
        let marker_x = 100;
        let marker_y = 95;
        let marker_index = (marker_y * width + marker_x) * 4;
        assert_eq!(
            short_password[marker_index..marker_index + 4],
            encoded(opaque_over(
                Rgba::from_config(config.feedback_accent_color),
                opaque_over(
                    Rgba::from_config(config.feedback_background_color),
                    BACKGROUND,
                ),
            ))
        );
    }

    #[test]
    fn feedback_states_remain_fully_opaque() {
        for prompt_state in [
            PromptState::Authenticating,
            PromptState::Failure,
            PromptState::Cooldown,
        ] {
            let mut canvas = vec![0; 200 * 120 * 4];
            let config = InputConfig::default();

            draw_lock_frame(&mut canvas, 200, 120, 8, prompt_state, &config);

            assert!(canvas.chunks_exact(4).all(|pixel| pixel[3] == 255));
        }
    }

    #[test]
    fn applies_configured_prompt_geometry_and_color() {
        let width = 200;
        let height = 120;
        let config = InputConfig {
            x: 0.25,
            y: 0.5,
            width: 80.0,
            height: 20.0,
            background_color: Color::rgb(11, 22, 33),
            ..InputConfig::default()
        };
        let mut canvas = vec![0; width * height * 4];

        draw_lock_frame(&mut canvas, 200, 120, 0, PromptState::Ready, &config);

        let prompt_corner = (50 * width + 10) * 4;
        assert_eq!(
            canvas[prompt_corner..prompt_corner + 4],
            encoded(Rgba::new(11, 22, 33, 255))
        );
        let old_prompt_center = (103 * width + 100) * 4;
        assert_eq!(
            canvas[old_prompt_center..old_prompt_center + 4],
            encoded(BACKGROUND)
        );
    }

    #[test]
    fn keeps_the_real_lock_prompt_visible_when_demo_input_is_disabled() {
        let width = 200;
        let height = 120;
        let config = InputConfig {
            enabled: false,
            background_color: Color::rgb(11, 22, 33),
            ..InputConfig::default()
        };
        let mut canvas = vec![0; width * height * 4];

        draw_lock_frame(&mut canvas, 200, 120, 0, PromptState::Ready, &config);

        let prompt_corner = (86 * width + 22) * 4;
        assert_eq!(
            canvas[prompt_corner..prompt_corner + 4],
            encoded(Rgba::new(11, 22, 33, 255))
        );
    }

    #[test]
    fn confines_extreme_indicator_geometry_to_the_prompt() {
        let width = 200;
        let height = 120;
        let config = InputConfig {
            x: 0.5,
            y: 0.5,
            width: 24.0,
            height: 16.0,
            max_characters: 64,
            min_dots: 64,
            dot_spacing: 64.0,
            empty_dot_radius: 32.0,
            filled_dot_radius: 32.0,
            background_color: Color::rgb(1, 2, 3),
            empty_dot_color: Color::rgb(4, 5, 6),
            filled_dot_color: Color::rgb(7, 8, 9),
            feedback_background_color: Color::rgb(10, 11, 12),
            feedback_accent_color: Color::rgb(13, 14, 15),
            ..InputConfig::default()
        };

        for prompt_state in [
            PromptState::Ready,
            PromptState::Authenticating,
            PromptState::Failure,
            PromptState::Cooldown,
        ] {
            let mut canvas = vec![0; width * height * 4];
            draw_lock_frame(&mut canvas, 200, 120, 64, prompt_state, &config);

            for (index, pixel) in canvas.chunks_exact(4).enumerate() {
                let x = index % width;
                let y = index / width;
                if !(88..112).contains(&x) || !(52..68).contains(&y) {
                    assert_eq!(pixel, encoded(BACKGROUND));
                }
            }
        }
    }

    #[test]
    fn ignores_invalid_dimensions() {
        let mut canvas = vec![0; 16];
        let config = InputConfig::default();

        draw_lock_frame(&mut canvas, -1, 4, 3, PromptState::Failure, &config);

        assert_eq!(canvas, vec![0; 16]);
    }

    #[test]
    fn draws_configured_clock_and_date() {
        let width = 400;
        let height = 300;
        let mut canvas = encoded(BACKGROUND).repeat(width * height);
        let renderer = crate::renderer::TextRenderer::new().expect("embedded font must load");
        let clock = crate::config::ClockConfig::default();
        let date = crate::config::DateConfig {
            enabled: true,
            ..crate::config::DateConfig::default()
        };
        let now = Local
            .with_ymd_and_hms(2026, 7, 16, 19, 41, 0)
            .single()
            .expect("test date must be valid");

        draw_lock_visuals(&mut canvas, 400, 300, &clock, &date, &renderer, now);

        assert!(
            canvas
                .chunks_exact(4)
                .any(|pixel| pixel != encoded(BACKGROUND))
        );
        assert!(canvas.chunks_exact(4).all(|pixel| pixel[3] == 255));
    }

    #[test]
    fn visual_feedback_hides_password_length_and_animates() {
        let width = 240;
        let height = 140;
        let config = InputConfig::default();
        let mut short_password = vec![0; width * height * 4];
        let mut long_password = vec![0; width * height * 4];
        let mut next_frame = vec![0; width * height * 4];

        for (canvas, password_length) in [(&mut short_password, 1), (&mut long_password, 12)] {
            draw_lock_frame(
                canvas,
                240,
                140,
                password_length,
                PromptState::Authenticating,
                &config,
            );
            draw_lock_visual_feedback(canvas, 240, 140, PromptState::Authenticating, &config, 0);
        }
        draw_lock_frame(
            &mut next_frame,
            240,
            140,
            1,
            PromptState::Authenticating,
            &config,
        );
        draw_lock_visual_feedback(
            &mut next_frame,
            240,
            140,
            PromptState::Authenticating,
            &config,
            1,
        );

        assert_eq!(short_password, long_password);
        assert_ne!(short_password, next_frame);
        assert!(short_password.chunks_exact(4).all(|pixel| pixel[3] == 255));
    }

    fn encoded(color: super::Rgba) -> [u8; 4] {
        #[cfg(target_endian = "little")]
        return [color.blue, color.green, color.red, color.alpha];

        #[cfg(target_endian = "big")]
        return [color.alpha, color.red, color.green, color.blue];
    }
}
