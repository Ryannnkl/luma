use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};

use crate::config::Color;

#[derive(Clone, Copy, Debug)]
pub struct ClipRectangle {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

pub struct TextRenderer {
    font: FontRef<'static>,
}

impl TextRenderer {
    /// Creates a renderer backed by Luma's embedded font.
    ///
    /// # Errors
    ///
    /// Returns an error when the embedded font cannot be parsed.
    pub fn new() -> Result<Self, ab_glyph::InvalidFont> {
        FontRef::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).map(|font| Self { font })
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::too_many_arguments
    )]
    pub fn draw_centered(
        &self,
        canvas: &mut [u8],
        canvas_width: usize,
        canvas_height: usize,
        clip: ClipRectangle,
        center: (f32, f32),
        size: f32,
        text: &str,
        color: Color,
    ) {
        if text.is_empty() || size <= 0.0 || !size.is_finite() {
            return;
        }

        let scaled = self.font.as_scaled(PxScale::from(size));
        let mut cursor_x = 0.0;
        let mut previous = None;
        let mut glyphs = Vec::with_capacity(text.chars().count());

        for character in text.chars() {
            let glyph_id = scaled.glyph_id(character);
            if let Some(previous) = previous {
                cursor_x += scaled.kern(previous, glyph_id);
            }
            glyphs.push(glyph_id.with_scale_and_position(size, point(cursor_x, scaled.ascent())));
            cursor_x += scaled.h_advance(glyph_id);
            previous = Some(glyph_id);
        }

        let bounds = glyphs
            .iter()
            .filter_map(|glyph| self.font.outline_glyph(glyph.clone()))
            .map(|glyph| glyph.px_bounds())
            .reduce(|left, right| ab_glyph::Rect {
                min: point(left.min.x.min(right.min.x), left.min.y.min(right.min.y)),
                max: point(left.max.x.max(right.max.x), left.max.y.max(right.max.y)),
            });
        let Some(bounds) = bounds else {
            return;
        };
        let offset = point(
            center.0 - bounds.min.x.midpoint(bounds.max.x),
            center.1 - bounds.min.y.midpoint(bounds.max.y),
        );

        for mut glyph in glyphs {
            glyph.position += offset;
            let Some(outlined) = self.font.outline_glyph(glyph) else {
                continue;
            };
            let glyph_bounds = outlined.px_bounds();
            outlined.draw(|x, y, coverage| {
                let pixel_x = glyph_bounds.min.x as i32 + i32::try_from(x).unwrap_or(i32::MAX);
                let pixel_y = glyph_bounds.min.y as i32 + i32::try_from(y).unwrap_or(i32::MAX);
                blend_pixel(
                    canvas,
                    canvas_width,
                    canvas_height,
                    clip,
                    pixel_x,
                    pixel_y,
                    color,
                    coverage,
                );
            });
        }
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_arguments
)]
fn blend_pixel(
    canvas: &mut [u8],
    width: usize,
    height: usize,
    clip: ClipRectangle,
    x: i32,
    y: i32,
    color: Color,
    coverage: f32,
) {
    let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
        return;
    };
    let clip_end_x = clip.x.saturating_add(clip.width).min(width);
    let clip_end_y = clip.y.saturating_add(clip.height).min(height);
    if x < clip.x || x >= clip_end_x || y < clip.y || y >= clip_end_y {
        return;
    }
    let Some(index) = y
        .checked_mul(width)
        .and_then(|row| row.checked_add(x))
        .and_then(|pixel| pixel.checked_mul(4))
    else {
        return;
    };
    let Some(pixel) = canvas.get_mut(index..index.saturating_add(4)) else {
        return;
    };

    let [red, green, blue, alpha] = color.channels();
    let alpha = (coverage.clamp(0.0, 1.0) * f32::from(alpha) / 255.0).clamp(0.0, 1.0);
    #[cfg(target_endian = "little")]
    let foreground = [blue, green, red];
    #[cfg(target_endian = "big")]
    let foreground = [red, green, blue];
    #[cfg(target_endian = "little")]
    let channels = &mut pixel[..3];
    #[cfg(target_endian = "big")]
    let channels = &mut pixel[1..4];

    for (channel, foreground) in channels.iter_mut().zip(foreground) {
        *channel = (f32::from(foreground).mul_add(alpha, f32::from(*channel) * (1.0 - alpha)))
            .round() as u8;
    }
    #[cfg(target_endian = "little")]
    {
        pixel[3] = 255;
    }
    #[cfg(target_endian = "big")]
    {
        pixel[0] = 255;
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Color;

    use super::{ClipRectangle, TextRenderer};

    #[test]
    fn embedded_font_is_valid() {
        assert!(TextRenderer::new().is_ok());
    }

    #[test]
    fn draws_text_only_inside_the_clip_and_keeps_pixels_opaque() {
        let renderer = TextRenderer::new().expect("embedded font must be valid");
        let width = 160;
        let height = 80;
        let background = encoded(Color::rgb(12, 24, 36));
        let mut canvas = background.repeat(width * height);
        let clip = ClipRectangle {
            x: 40,
            y: 20,
            width: 80,
            height: 40,
        };

        renderer.draw_centered(
            &mut canvas,
            width,
            height,
            clip,
            (80.0, 40.0),
            36.0,
            "19",
            Color::rgb(200, 240, 220),
        );

        assert!(canvas.chunks_exact(4).all(|pixel| alpha(pixel) == 255));
        assert!(canvas.chunks_exact(4).any(|pixel| pixel != background));
        for (index, pixel) in canvas.chunks_exact(4).enumerate() {
            let x = index % width;
            let y = index / width;
            if !(40..120).contains(&x) || !(20..60).contains(&y) {
                assert_eq!(pixel, background);
            }
        }
    }

    #[test]
    fn ignores_empty_text() {
        let renderer = TextRenderer::new().expect("embedded font must be valid");
        let mut canvas = vec![7; 16 * 16 * 4];
        let original = canvas.clone();
        renderer.draw_centered(
            &mut canvas,
            16,
            16,
            ClipRectangle {
                x: 0,
                y: 0,
                width: 16,
                height: 16,
            },
            (8.0, 8.0),
            12.0,
            "",
            Color::rgb(255, 255, 255),
        );
        assert_eq!(canvas, original);
    }

    fn encoded(color: Color) -> [u8; 4] {
        let [red, green, blue, _] = color.channels();
        #[cfg(target_endian = "little")]
        return [blue, green, red, 255];
        #[cfg(target_endian = "big")]
        return [255, red, green, blue];
    }

    fn alpha(pixel: &[u8]) -> u8 {
        #[cfg(target_endian = "little")]
        return pixel[3];
        #[cfg(target_endian = "big")]
        return pixel[0];
    }
}
