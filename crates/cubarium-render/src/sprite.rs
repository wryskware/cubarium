//! Authored RGBA sprites, composited through the existing surface atlas.

use cubarium_surface::{PixelImage, SurfacePoint, Vec2, unfold_pixels};

use crate::{Canvas, srgb_decode};

/// Linear premultiplied pixels. The pivot is measured from the image's upper-left
/// boundary; body +x is forward. Transparent padding never enlarges the footprint.
#[derive(Clone, Debug)]
pub struct Sprite {
    width: usize,
    height: usize,
    pivot: Vec2,
    pixels: Vec<[f32; 4]>,
    extent: f64,
}

impl Sprite {
    pub fn from_rgba(
        width: usize,
        height: usize,
        pivot: Vec2,
        bytes: &[u8],
    ) -> Result<Self, String> {
        if width == 0
            || height == 0
            || width > 64
            || height > 64
            || bytes.len() != width * height * 4
            || !pivot.is_finite()
        {
            return Err("invalid sprite dimensions, pivot or RGBA length".into());
        }
        let mut extent = 0.0f64;
        let mut pixels = Vec::with_capacity(width * height);
        for (i, rgba) in bytes.as_chunks::<4>().0.iter().enumerate() {
            let a = f32::from(rgba[3]) / 255.0;
            pixels.push([
                srgb_decode(rgba[0]) * a,
                srgb_decode(rgba[1]) * a,
                srgb_decode(rgba[2]) * a,
                a,
            ]);
            if a > 0.0 {
                let x = (i % width) as f64 + 0.5 - pivot.x;
                let y = (i / width) as f64 + 0.5 - pivot.y;
                // Half a pixel diagonal plus half a pixel of bilinear support.
                extent = extent.max(x.hypot(y) + std::f64::consts::FRAC_1_SQRT_2 + 0.5);
            }
        }
        if extent > 9.0 {
            return Err(format!(
                "sprite extent {extent:.2} exceeds the 9-pixel surface budget"
            ));
        }
        Ok(Self {
            width,
            height,
            pivot,
            pixels,
            extent,
        })
    }

    pub fn extent(&self) -> f64 {
        self.extent
    }

    fn pixel(&self, x: i32, y: i32) -> [f32; 4] {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            [0.0; 4]
        } else {
            self.pixels[y as usize * self.width + x as usize]
        }
    }

    fn sample(&self, point: Vec2) -> [f32; 4] {
        let p = point + self.pivot - Vec2::new(0.5, 0.5);
        let x = p.x.floor() as i32;
        let y = p.y.floor() as i32;
        let fx = (p.x - f64::from(x)) as f32;
        let fy = (p.y - f64::from(y)) as f32;
        let mut result = [0.0; 4];
        for (dx, dy, weight) in [
            (0, 0, (1.0 - fx) * (1.0 - fy)),
            (1, 0, fx * (1.0 - fy)),
            (0, 1, (1.0 - fx) * fy),
            (1, 1, fx * fy),
        ] {
            let pixel = self.pixel(x + dx, y + dy);
            for channel in 0..4 {
                result[channel] += pixel[channel] * weight;
            }
        }
        result
    }
}

/// Source-over blend, not additive light: dark authored outlines remain visible.
/// Shared `unfold_pixels` supplies seam orientation and unique vertex ownership.
pub fn stamp_sprite(
    canvas: &mut Canvas,
    anchor: SurfacePoint,
    heading: Vec2,
    sprite: &Sprite,
    scale: f64,
    opacity: f32,
    scratch: &mut Vec<PixelImage>,
) {
    if !scale.is_finite()
        || scale <= 0.0
        || sprite.extent * scale > 9.0
        || !opacity.is_finite()
        || opacity <= 0.0
        || sprite.extent == 0.0
    {
        return;
    }
    let Some(h) = heading.normalized() else {
        return;
    };
    let opacity = opacity.min(1.0);
    let side = Vec2::new(-h.y, h.x);
    unfold_pixels(anchor, sprite.extent * scale, scratch);
    for pixel in scratch.iter() {
        let d = pixel.local - anchor.chart();
        let rgba = sprite.sample(Vec2::new(h.dot(d) / scale, side.dot(d) / scale));
        let a = rgba[3] * opacity;
        if a <= 0.0 {
            continue;
        }
        let background = canvas.get(pixel.face, pixel.x, pixel.y);
        canvas.set(
            pixel.face,
            pixel.x,
            pixel.y,
            std::array::from_fn(|c| rgba[c] * opacity + background[c] * (1.0 - a)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cube_proto::Face;

    fn total(canvas: &Canvas) -> f64 {
        Face::ALL
            .into_iter()
            .flat_map(|f| (0..64).flat_map(move |y| (0..64).map(move |x| (f, x, y))))
            .map(|(f, x, y)| canvas.get(f, x, y).into_iter().map(f64::from).sum::<f64>())
            .sum()
    }

    #[test]
    fn alpha_uses_linear_source_over_and_does_not_add_a_halo() {
        let s = Sprite::from_rgba(1, 1, Vec2::new(0.5, 0.5), &[255, 0, 0, 128]).unwrap();
        let mut canvas = Canvas::new();
        canvas.set(Face::Front, 20, 20, [0.0, 0.0, 1.0]);
        stamp_sprite(
            &mut canvas,
            SurfacePoint::pixel_center(Face::Front, 20, 20),
            Vec2::new(1.0, 0.0),
            &s,
            1.0,
            1.0,
            &mut vec![],
        );
        let px = canvas.get(Face::Front, 20, 20);
        let a = 128.0 / 255.0;
        assert!((px[0] - a).abs() < 1e-6 && px[1] == 0.0 && (px[2] - (1.0 - a)).abs() < 1e-6);
    }

    #[test]
    fn an_asymmetric_sprite_crosses_the_seam_without_losing_or_duplicating_light() {
        let pixels = [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let s = Sprite::from_rgba(4, 1, Vec2::new(2.0, 0.5), &pixels).unwrap();
        let mut middle = Canvas::new();
        let mut seam = Canvas::new();
        stamp_sprite(
            &mut middle,
            SurfacePoint::new(Face::Front, 32.0, 32.5),
            Vec2::new(1.0, 0.0),
            &s,
            1.0,
            1.0,
            &mut vec![],
        );
        stamp_sprite(
            &mut seam,
            SurfacePoint::new(Face::Front, 63.0, 32.5),
            Vec2::new(1.0, 0.0),
            &s,
            1.0,
            1.0,
            &mut vec![],
        );
        assert!((total(&middle) - total(&seam)).abs() < 1e-6);
        assert_eq!(seam.get(Face::Front, 61, 32), [1.0, 0.0, 0.0]);
        assert_eq!(seam.get(Face::Front, 63, 32), [0.0, 0.0, 1.0]);
        assert_eq!(seam.get(Face::Right, 0, 32), [1.0, 1.0, 1.0]);
    }

    #[test]
    fn invalid_assets_and_extent_are_rejected() {
        assert!(Sprite::from_rgba(2, 2, Vec2::ZERO, &[0; 3]).is_err());
        assert!(Sprite::from_rgba(1, 1, Vec2::new(f64::NAN, 0.0), &[0; 4]).is_err());
        assert!(Sprite::from_rgba(32, 32, Vec2::ZERO, &vec![255; 32 * 32 * 4]).is_err());
    }
}
