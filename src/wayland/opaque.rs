const BACKGROUND: Rgba = Rgba::new(18, 26, 28, 255);
const PROMPT_BACKGROUND: Rgba = Rgba::new(8, 14, 15, 255);
const EMPTY_DOT: Rgba = Rgba::new(92, 111, 106, 255);
const FILLED_DOT: Rgba = Rgba::new(226, 239, 232, 255);

const MIN_DOT_COUNT: usize = 6;
const MAX_DOT_COUNT: usize = 12;

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
}

/// Draws Luma's opaque fallback and a password-length indicator into an ARGB8888 canvas.
///
/// The indicator intentionally represents only the number of entered characters. It never
/// reads or renders the password contents.
pub(crate) fn draw_lock_frame(canvas: &mut [u8], width: i32, height: i32, password_length: usize) {
    let Ok(width) = usize::try_from(width) else {
        return;
    };
    let Ok(height) = usize::try_from(height) else {
        return;
    };

    fill(canvas, BACKGROUND);
    if width < 80 || height < 80 {
        return;
    }

    let prompt_width = (width / 7).clamp(120, 220).min(width.saturating_sub(32));
    let prompt_height = 34;
    let prompt_x = width.saturating_sub(prompt_width) / 2;
    let prompt_y = height.saturating_sub(72);

    fill_rect(
        canvas,
        width,
        height,
        prompt_x,
        prompt_y,
        prompt_width,
        prompt_height,
        PROMPT_BACKGROUND,
    );

    let dot_count = password_length.clamp(MIN_DOT_COUNT, MAX_DOT_COUNT);
    let dot_diameter = 8;
    let gap = 10;
    let total_width = dot_count
        .saturating_mul(dot_diameter)
        .saturating_add(dot_count.saturating_sub(1).saturating_mul(gap));
    let start_x = prompt_x.saturating_add(prompt_width.saturating_sub(total_width) / 2);
    let center_y = prompt_y.saturating_add(prompt_height / 2);

    for dot_index in 0..dot_count {
        let center_x = start_x.saturating_add(dot_index.saturating_mul(dot_diameter + gap));
        let color = if dot_index < password_length {
            FILLED_DOT
        } else {
            EMPTY_DOT
        };
        fill_circle(
            canvas,
            width,
            height,
            center_x,
            center_y,
            dot_diameter / 2,
            color,
        );
    }
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
    use super::{BACKGROUND, FILLED_DOT, draw_lock_frame};

    #[test]
    fn draws_an_opaque_background_for_small_outputs() {
        let mut canvas = vec![0; 4 * 32 * 32];

        draw_lock_frame(&mut canvas, 32, 32, 0);

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

        draw_lock_frame(&mut empty_canvas, frame_width, frame_height, 0);
        draw_lock_frame(&mut filled_canvas, frame_width, frame_height, 1);

        let center_x = 51;
        let center_y = 65;
        let pixel_index = (center_y * width + center_x) * 4;
        assert_ne!(
            empty_canvas[pixel_index..pixel_index + 4],
            filled_canvas[pixel_index..pixel_index + 4]
        );
        assert_eq!(
            filled_canvas[pixel_index..pixel_index + 4],
            encoded(FILLED_DOT)
        );
    }

    #[test]
    fn ignores_invalid_dimensions() {
        let mut canvas = vec![0; 16];

        draw_lock_frame(&mut canvas, -1, 4, 3);

        assert_eq!(canvas, vec![0; 16]);
    }

    fn encoded(color: super::Rgba) -> [u8; 4] {
        #[cfg(target_endian = "little")]
        return [color.blue, color.green, color.red, color.alpha];

        #[cfg(target_endian = "big")]
        return [color.alpha, color.red, color.green, color.blue];
    }
}
