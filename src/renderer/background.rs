use std::fmt;

const BYTES_PER_PIXEL: usize = 4;
const MAX_PIXELS: usize = 16_777_216;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackgroundImage {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl BackgroundImage {
    /// Normalizes a captured native-endian ARGB8888 buffer into a packed opaque image.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported dimensions, strides, or incomplete buffers.
    pub fn from_argb8888(
        width: u32,
        height: u32,
        stride: u32,
        source: &[u8],
        y_inverted: bool,
    ) -> Result<Self, ImageError> {
        let width = usize::try_from(width).map_err(|_| ImageError::Dimensions)?;
        let height = usize::try_from(height).map_err(|_| ImageError::Dimensions)?;
        let stride = usize::try_from(stride).map_err(|_| ImageError::Dimensions)?;
        let row_bytes = width
            .checked_mul(BYTES_PER_PIXEL)
            .ok_or(ImageError::Dimensions)?;
        let pixel_count = width.checked_mul(height).ok_or(ImageError::Dimensions)?;
        if width == 0 || height == 0 || pixel_count > MAX_PIXELS || stride < row_bytes {
            return Err(ImageError::Dimensions);
        }
        let source_size = stride.checked_mul(height).ok_or(ImageError::Dimensions)?;
        if source.len() < source_size {
            return Err(ImageError::BufferTooSmall);
        }
        let output_size = row_bytes
            .checked_mul(height)
            .ok_or(ImageError::Dimensions)?;
        let mut pixels = vec![0; output_size];
        for destination_row in 0..height {
            let source_row = if y_inverted {
                height - destination_row - 1
            } else {
                destination_row
            };
            let source_start = source_row * stride;
            let destination_start = destination_row * row_bytes;
            pixels[destination_start..destination_start + row_bytes]
                .copy_from_slice(&source[source_start..source_start + row_bytes]);
        }
        for pixel in pixels.chunks_exact_mut(BYTES_PER_PIXEL) {
            set_alpha(pixel, 255);
        }

        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn blur(&mut self, radius: u32) {
        if radius == 0 || self.width <= 1 || self.height <= 1 {
            return;
        }
        let pass_radius = usize::try_from(radius.div_ceil(3)).unwrap_or(1).max(1);
        let mut scratch = vec![0; self.pixels.len()];
        for _ in 0..3 {
            blur_horizontal(
                &self.pixels,
                &mut scratch,
                self.width,
                self.height,
                pass_radius,
            );
            blur_vertical(
                &scratch,
                &mut self.pixels,
                self.width,
                self.height,
                pass_radius,
            );
        }
    }

    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageError {
    Dimensions,
    BufferTooSmall,
}

impl fmt::Display for ImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dimensions => formatter.write_str("captured image dimensions are unsupported"),
            Self::BufferTooSmall => formatter.write_str("captured image buffer is incomplete"),
        }
    }
}

impl std::error::Error for ImageError {}

fn blur_horizontal(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    radius: usize,
) {
    for y in 0..height {
        let mut sums = [0_u64; 3];
        let initial_end = radius.min(width - 1);
        for x in 0..=initial_end {
            add_pixel(&mut sums, pixel(source, width, x, y));
        }
        for x in 0..width {
            let start = x.saturating_sub(radius);
            let end = x.saturating_add(radius).min(width - 1);
            write_average(pixel_mut(destination, width, x, y), sums, end - start + 1);
            if x >= radius {
                subtract_pixel(&mut sums, pixel(source, width, x - radius, y));
            }
            if x.saturating_add(radius).saturating_add(1) < width {
                add_pixel(&mut sums, pixel(source, width, x + radius + 1, y));
            }
        }
    }
}

fn blur_vertical(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    radius: usize,
) {
    for x in 0..width {
        let mut sums = [0_u64; 3];
        let initial_end = radius.min(height - 1);
        for y in 0..=initial_end {
            add_pixel(&mut sums, pixel(source, width, x, y));
        }
        for y in 0..height {
            let start = y.saturating_sub(radius);
            let end = y.saturating_add(radius).min(height - 1);
            write_average(pixel_mut(destination, width, x, y), sums, end - start + 1);
            if y >= radius {
                subtract_pixel(&mut sums, pixel(source, width, x, y - radius));
            }
            if y.saturating_add(radius).saturating_add(1) < height {
                add_pixel(&mut sums, pixel(source, width, x, y + radius + 1));
            }
        }
    }
}

fn pixel(source: &[u8], width: usize, x: usize, y: usize) -> &[u8] {
    let start = (y * width + x) * BYTES_PER_PIXEL;
    &source[start..start + BYTES_PER_PIXEL]
}

fn pixel_mut(destination: &mut [u8], width: usize, x: usize, y: usize) -> &mut [u8] {
    let start = (y * width + x) * BYTES_PER_PIXEL;
    &mut destination[start..start + BYTES_PER_PIXEL]
}

fn add_pixel(sums: &mut [u64; 3], pixel: &[u8]) {
    for (sum, channel) in sums.iter_mut().zip(color_channels(pixel)) {
        *sum += u64::from(*channel);
    }
}

fn subtract_pixel(sums: &mut [u64; 3], pixel: &[u8]) {
    for (sum, channel) in sums.iter_mut().zip(color_channels(pixel)) {
        *sum = sum.saturating_sub(u64::from(*channel));
    }
}

fn write_average(pixel: &mut [u8], sums: [u64; 3], count: usize) {
    let divisor = u64::try_from(count).unwrap_or(1);
    for (channel, sum) in color_channels_mut(pixel).zip(sums) {
        *channel = u8::try_from(sum / divisor).unwrap_or(u8::MAX);
    }
    set_alpha(pixel, 255);
}

#[cfg(target_endian = "little")]
fn color_channels(pixel: &[u8]) -> impl Iterator<Item = &u8> {
    pixel[..3].iter()
}

#[cfg(target_endian = "big")]
fn color_channels(pixel: &[u8]) -> impl Iterator<Item = &u8> {
    pixel[1..].iter()
}

#[cfg(target_endian = "little")]
fn color_channels_mut(pixel: &mut [u8]) -> impl Iterator<Item = &mut u8> {
    pixel[..3].iter_mut()
}

#[cfg(target_endian = "big")]
fn color_channels_mut(pixel: &mut [u8]) -> impl Iterator<Item = &mut u8> {
    pixel[1..].iter_mut()
}

#[cfg(target_endian = "little")]
fn set_alpha(pixel: &mut [u8], alpha: u8) {
    pixel[3] = alpha;
}

#[cfg(target_endian = "big")]
fn set_alpha(pixel: &mut [u8], alpha: u8) {
    pixel[0] = alpha;
}

#[cfg(test)]
mod tests {
    use super::{BackgroundImage, ImageError};

    #[test]
    fn normalizes_stride_orientation_and_alpha() {
        let source = [
            1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0, 9, 10, 11, 12, 13, 14, 15, 16, 0, 0, 0, 0,
        ];

        let image = BackgroundImage::from_argb8888(2, 2, 12, &source, true)
            .expect("valid image should normalize");

        assert_eq!(image.width(), 2);
        assert_eq!(image.height(), 2);
        assert_eq!(&image.pixels()[..3], &[9, 10, 11]);
        assert_eq!(&image.pixels()[8..11], &[1, 2, 3]);
        assert!(
            image
                .pixels()
                .chunks_exact(4)
                .all(|pixel| alpha(pixel) == 255)
        );
    }

    #[test]
    fn rejects_incomplete_or_excessive_images() {
        assert_eq!(
            BackgroundImage::from_argb8888(2, 2, 8, &[0; 8], false),
            Err(ImageError::BufferTooSmall)
        );
        assert_eq!(
            BackgroundImage::from_argb8888(16_777_217, 1, u32::MAX, &[], false),
            Err(ImageError::Dimensions)
        );
    }

    #[test]
    fn zero_radius_preserves_pixels() {
        let mut image = image_with_center_pixel();
        let original = image.pixels().to_vec();

        image.blur(0);

        assert_eq!(image.pixels(), original);
    }

    #[test]
    fn blur_spreads_color_and_keeps_the_frame_opaque() {
        let mut image = image_with_center_pixel();

        image.blur(6);

        assert!(
            image
                .pixels()
                .chunks_exact(4)
                .all(|pixel| alpha(pixel) == 255)
        );
        assert!(
            image
                .pixels()
                .chunks_exact(4)
                .filter(|pixel| pixel[0] > 0)
                .count()
                > 1
        );
        assert!(image.pixels()[center_index()] < 255);
    }

    fn image_with_center_pixel() -> BackgroundImage {
        let mut pixels = vec![0; 9 * 9 * 4];
        pixels[center_index()] = 255;
        BackgroundImage::from_argb8888(9, 9, 36, &pixels, false).expect("test image must be valid")
    }

    const fn center_index() -> usize {
        (4 * 9 + 4) * 4
    }

    fn alpha(pixel: &[u8]) -> u8 {
        #[cfg(target_endian = "little")]
        return pixel[3];
        #[cfg(target_endian = "big")]
        return pixel[0];
    }
}
