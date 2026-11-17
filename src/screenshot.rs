use crate::wire::Result;
use png::{BitDepth, ColorType, Encoder};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoordinateMap {
    pub screenshot_width: u32,
    pub screenshot_height: u32,
    pub remote_width: u32,
    pub remote_height: u32,
    pub origin_x: i32,
    pub origin_y: i32,
}

impl CoordinateMap {
    pub fn remote_point(self, x: i64, y: i64) -> Result<(i32, i32)> {
        if x < 0 || y < 0 || x >= self.screenshot_width as i64 || y >= self.screenshot_height as i64
        {
            return Err(format!(
                "Coordinates must be inside the latest {}×{} screenshot.",
                self.screenshot_width, self.screenshot_height
            )
            .into());
        }
        let canvas_x = (x as u64 * self.remote_width as u64 / self.screenshot_width as u64) as i64;
        let canvas_y =
            (y as u64 * self.remote_height as u64 / self.screenshot_height as u64) as i64;
        let remote_x = canvas_x + self.origin_x as i64;
        let remote_y = canvas_y + self.origin_y as i64;
        if remote_x < i32::MIN as i64
            || remote_x > i32::MAX as i64
            || remote_y < i32::MIN as i64
            || remote_y > i32::MAX as i64
        {
            return Err("The remote coordinate is outside the supported range.".into());
        }
        Ok((remote_x as i32, remote_y as i32))
    }
}

#[derive(Debug)]
pub struct Screenshot {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub coordinates: CoordinateMap,
}

pub fn encode(
    width: u32,
    height: u32,
    pixels: &[u32],
    origin: (i32, i32),
    max_width: u32,
) -> Result<Screenshot> {
    if width == 0 || height == 0 || pixels.len() != width as usize * height as usize {
        return Err("The remote screen image is invalid.".into());
    }
    let scale = (max_width as f64 / width as f64).min(1.0);
    let output_width = ((width as f64 * scale).round() as u32).max(1);
    let output_height = ((height as f64 * scale).round() as u32).max(1);
    let rgb = if output_width == width && output_height == height {
        rgb_pixels(pixels)
    } else {
        downscale(width, height, pixels, output_width, output_height)
    };
    let mut png = Vec::new();
    {
        let mut encoder = Encoder::new(&mut png, output_width, output_height);
        encoder.set_color(ColorType::Rgb);
        encoder.set_depth(BitDepth::Eight);
        encoder.write_header()?.write_image_data(&rgb)?;
    }
    Ok(Screenshot {
        png,
        width: output_width,
        height: output_height,
        coordinates: CoordinateMap {
            screenshot_width: output_width,
            screenshot_height: output_height,
            remote_width: width,
            remote_height: height,
            origin_x: origin.0,
            origin_y: origin.1,
        },
    })
}

fn rgb_pixels(pixels: &[u32]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(pixels.len() * 3);
    for pixel in pixels {
        rgb.extend_from_slice(&[(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8]);
    }
    rgb
}

fn downscale(
    width: u32,
    height: u32,
    pixels: &[u32],
    output_width: u32,
    output_height: u32,
) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(output_width as usize * output_height as usize * 3);
    for y in 0..output_height {
        let y0 = y as u64 * height as u64 / output_height as u64;
        let y1 = (((y + 1) as u64 * height as u64 / output_height as u64).max(y0 + 1))
            .min(height as u64);
        for x in 0..output_width {
            let x0 = x as u64 * width as u64 / output_width as u64;
            let x1 = (((x + 1) as u64 * width as u64 / output_width as u64).max(x0 + 1))
                .min(width as u64);
            let mut sums = [0u64; 3];
            for source_y in y0..y1 {
                for source_x in x0..x1 {
                    let pixel = pixels[(source_y * width as u64 + source_x) as usize];
                    sums[0] += ((pixel >> 16) & 0xff) as u64;
                    sums[1] += ((pixel >> 8) & 0xff) as u64;
                    sums[2] += (pixel & 0xff) as u64;
                }
            }
            let count = (x1 - x0) * (y1 - y0);
            rgb.extend(sums.into_iter().map(|sum| (sum / count) as u8));
        }
    }
    rgb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_mapping_scales_and_applies_virtual_origin() {
        let map = CoordinateMap {
            screenshot_width: 640,
            screenshot_height: 360,
            remote_width: 1920,
            remote_height: 1080,
            origin_x: -1920,
            origin_y: -100,
        };
        assert_eq!(map.remote_point(0, 0).unwrap(), (-1920, -100));
        assert_eq!(map.remote_point(639, 359).unwrap(), (-3, 977));
        assert!(map.remote_point(640, 0).is_err());
        assert!(map.remote_point(-1, 0).is_err());
    }

    #[test]
    fn png_and_box_downscale_have_expected_dimensions_and_color() {
        let pixels = vec![0x00ff0000, 0x0000ff00, 0x000000ff, 0x00ffffff];
        let image = encode(2, 2, &pixels, (0, 0), 1).unwrap();
        assert_eq!((image.width, image.height), (1, 1));
        let decoder = png::Decoder::new(image.png.as_slice());
        let mut reader = decoder.read_info().unwrap();
        let mut output = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut output).unwrap();
        assert_eq!((info.width, info.height), (1, 1));
        assert_eq!(&output[..3], &[127, 127, 127]);
    }
}
