//! A simple RGB888 frame buffer with the few operations the bridge needs:
//! BGRA -> RGB conversion, rotation (0/90/180/270), and nearest-neighbour scaling.

/// Clockwise rotation to apply to a captured frame before it is sent to the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    R0,
    R90,
    R180,
    R270,
}

impl Rotation {
    pub fn degrees(self) -> u16 {
        match self {
            Rotation::R0 => 0,
            Rotation::R90 => 90,
            Rotation::R180 => 180,
            Rotation::R270 => 270,
        }
    }

    fn from_degrees(degrees: u16) -> Self {
        match degrees % 360 {
            90 => Rotation::R90,
            180 => Rotation::R180,
            270 => Rotation::R270,
            _ => Rotation::R0,
        }
    }

    /// Compose two rotations (e.g. auto-detected base + user `--rotate-180`).
    pub fn add(self, other: Rotation) -> Rotation {
        Rotation::from_degrees(self.degrees() + other.degrees())
    }

    /// Apply this rotation, returning a new frame.
    pub fn apply(self, src: &RgbFrame) -> RgbFrame {
        match self {
            Rotation::R0 => src.clone(),
            Rotation::R90 => rotate(src, true),
            Rotation::R180 => src.rotated_180(),
            Rotation::R270 => rotate(src, false),
        }
    }
}

/// Contiguous RGB888 (3 bytes per pixel) image.
#[derive(Clone)]
pub struct RgbFrame {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl RgbFrame {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0u8; (width as usize) * (height as usize) * 3],
        }
    }

    /// Convert a contiguous BGRA8 buffer (as produced by Windows Graphics Capture)
    /// into RGB888.
    pub fn from_bgra(bgra: &[u8], width: u32, height: u32) -> Self {
        let pixels = (width as usize) * (height as usize);
        let mut data = Vec::with_capacity(pixels * 3);
        for chunk in bgra.chunks_exact(4).take(pixels) {
            // B, G, R, A -> R, G, B
            data.push(chunk[2]);
            data.push(chunk[1]);
            data.push(chunk[0]);
        }
        // Guard against a short/odd source buffer.
        data.resize(pixels * 3, 0);
        Self {
            width,
            height,
            data,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Build a simple SMPTE-style colour-bar test pattern (USB bring-up aid).
    pub fn color_bars(width: u32, height: u32) -> Self {
        const COLORS: [(u8, u8, u8); 8] = [
            (255, 255, 255),
            (255, 255, 0),
            (0, 255, 255),
            (0, 255, 0),
            (255, 0, 255),
            (255, 0, 0),
            (0, 0, 255),
            (0, 0, 0),
        ];
        let mut frame = Self::new(width, height);
        for y in 0..height as usize {
            for x in 0..width as usize {
                let band = (x * COLORS.len() / width.max(1) as usize).min(COLORS.len() - 1);
                let (r, g, b) = COLORS[band];
                let offset = (y * width as usize + x) * 3;
                frame.data[offset] = r;
                frame.data[offset + 1] = g;
                frame.data[offset + 2] = b;
            }
        }
        frame
    }

    /// Return a copy rotated 180°.
    pub fn rotated_180(&self) -> Self {
        let mut out = Self::new(self.width, self.height);
        let row_bytes = self.width as usize * 3;
        for y in 0..self.height as usize {
            let src_row = y * row_bytes;
            let dst_row = (self.height as usize - 1 - y) * row_bytes;
            for x in 0..self.width as usize {
                let s = src_row + x * 3;
                let d = dst_row + (self.width as usize - 1 - x) * 3;
                out.data[d..d + 3].copy_from_slice(&self.data[s..s + 3]);
            }
        }
        out
    }

    /// Nearest-neighbour scale to `(dst_w, dst_h)`.
    pub fn scaled_nearest(&self, dst_w: u32, dst_h: u32) -> Self {
        if self.width == dst_w && self.height == dst_h {
            return self.clone();
        }

        let mut out = Self::new(dst_w, dst_h);
        let x_ratio = self.width as f32 / dst_w as f32;
        let y_ratio = self.height as f32 / dst_h as f32;

        for dy in 0..dst_h as usize {
            let sy = ((dy as f32 * y_ratio) as usize).min(self.height as usize - 1);
            for dx in 0..dst_w as usize {
                let sx = ((dx as f32 * x_ratio) as usize).min(self.width as usize - 1);
                let s = (sy * self.width as usize + sx) * 3;
                let d = (dy * dst_w as usize + dx) * 3;
                out.data[d..d + 3].copy_from_slice(&self.data[s..s + 3]);
            }
        }
        out
    }
}

/// 90° rotation. `clockwise = true` -> R90, `false` -> R270.
fn rotate(src: &RgbFrame, clockwise: bool) -> RgbFrame {
    let sw = src.width as usize;
    let sh = src.height as usize;
    // Rotated dimensions: width <- height, height <- width.
    let mut out = RgbFrame::new(src.height, src.width);
    let dw = sh;

    for sy in 0..sh {
        for sx in 0..sw {
            let (dx, dy) = if clockwise {
                (sh - 1 - sy, sx)
            } else {
                (sy, sw - 1 - sx)
            };
            let s = (sy * sw + sx) * 3;
            let d = (dy * dw + dx) * 3;
            out.data[d..d + 3].copy_from_slice(&src.data[s..s + 3]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_from(width: u32, height: u32, colors: &[(u8, u8, u8)]) -> RgbFrame {
        let mut f = RgbFrame::new(width, height);
        for (i, (r, g, b)) in colors.iter().enumerate() {
            f.data[i * 3] = *r;
            f.data[i * 3 + 1] = *g;
            f.data[i * 3 + 2] = *b;
        }
        f
    }

    #[test]
    fn r90_swaps_dimensions() {
        let f = RgbFrame::new(4, 2);
        let r = Rotation::R90.apply(&f);
        assert_eq!((r.width(), r.height()), (2, 4));
    }

    #[test]
    fn r90_then_r270_is_identity() {
        let f = frame_from(3, 2, &[(1, 2, 3); 6]);
        let round = Rotation::R270.apply(&Rotation::R90.apply(&f));
        assert_eq!((round.width(), round.height()), (3, 2));
        assert_eq!(round.as_slice(), f.as_slice());
    }

    #[test]
    fn r180_twice_is_identity() {
        let f = frame_from(2, 3, &[(9, 8, 7); 6]);
        let round = Rotation::R180.apply(&Rotation::R180.apply(&f));
        assert_eq!(round.as_slice(), f.as_slice());
    }

    #[test]
    fn r90_clockwise_maps_pixels() {
        // 2x1: A (left), B (right) -> 1x2 where A is top, B is bottom.
        let f = frame_from(2, 1, &[(255, 0, 0), (0, 0, 255)]);
        let r = Rotation::R90.apply(&f);
        assert_eq!((r.width(), r.height()), (1, 2));
        assert_eq!(&r.as_slice()[0..3], &[255, 0, 0]); // A on top
        assert_eq!(&r.as_slice()[3..6], &[0, 0, 255]); // B on bottom
    }

    #[test]
    fn rotation_add_is_modular() {
        assert_eq!(Rotation::R90.add(Rotation::R180), Rotation::R270);
        assert_eq!(Rotation::R270.add(Rotation::R90), Rotation::R0);
        assert_eq!(Rotation::R180.add(Rotation::R180), Rotation::R0);
    }
}
