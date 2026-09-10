use crate::display::font::{FONT_BASIC, FONT_WIDTH};
use lunix_common::{FramebufferInfo, PixelFormat};

#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0 };
    pub const WHITE: Color = Color { r: 255, g: 255, b: 255 };
    pub const RED: Color = Color { r: 245, g: 70, b: 70 };
    pub const GREEN: Color = Color { r: 70, g: 220, b: 120 };
    pub const BLUE: Color = Color { r: 60, g: 140, b: 255 };
    pub const CYAN: Color = Color { r: 0, g: 230, b: 255 };
    pub const MAGENTA: Color = Color { r: 220, g: 80, b: 230 };
    pub const YELLOW: Color = Color { r: 250, g: 210, b: 50 };
    pub const DARK_GRAY: Color = Color { r: 35, g: 38, b: 45 };
    pub const LIGHT_GRAY: Color = Color { r: 180, g: 185, b: 195 };
    pub const DARK_BLUE: Color = Color { r: 15, g: 20, b: 32 };
}

pub struct Framebuffer {
    pub info: FramebufferInfo,
}

impl Framebuffer {
    pub fn new(info: FramebufferInfo) -> Self {
        Self { info }
    }

    #[inline]
    pub fn draw_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x >= self.info.width || y >= self.info.height {
            return;
        }

        let pixel_offset = y * self.info.stride + x;
        let byte_offset = pixel_offset * self.info.bytes_per_pixel;
        let ptr = (self.info.base_address + byte_offset as u64) as *mut u8;

        unsafe {
            match self.info.format {
                PixelFormat::Rgb => {
                    *ptr.add(0) = color.r;
                    *ptr.add(1) = color.g;
                    *ptr.add(2) = color.b;
                    *ptr.add(3) = 0xFF;
                }
                PixelFormat::Bgr => {
                    *ptr.add(0) = color.b;
                    *ptr.add(1) = color.g;
                    *ptr.add(2) = color.r;
                    *ptr.add(3) = 0xFF;
                }
                _ => {
                    *ptr.add(0) = color.b;
                    *ptr.add(1) = color.g;
                    *ptr.add(2) = color.r;
                    *ptr.add(3) = 0xFF;
                }
            }
        }
    }

    pub fn draw_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: Color) {
        let pixel_val: u32 = match self.info.format {
            PixelFormat::Rgb => (0xFF << 24) | ((color.b as u32) << 16) | ((color.g as u32) << 8) | (color.r as u32),
            _ => (0xFF << 24) | ((color.r as u32) << 16) | ((color.g as u32) << 8) | (color.b as u32),
        };

        for row in y..(y + height).min(self.info.height) {
            let row_start = (self.info.base_address + (row * self.info.stride + x) as u64 * 4) as *mut u32;
            let count = width.min(self.info.width.saturating_sub(x));
            for col in 0..count {
                unsafe {
                    *row_start.add(col) = pixel_val;
                }
            }
        }
    }

    pub fn draw_char(&mut self, x: usize, y: usize, c: char, fg: Color, bg: Color) {
        let ascii = c as usize;
        let glyph_idx = if (32..128).contains(&ascii) {
            ascii - 32
        } else {
            0
        };

        let fg_val: u32 = match self.info.format {
            PixelFormat::Rgb => (0xFF << 24) | ((fg.b as u32) << 16) | ((fg.g as u32) << 8) | (fg.r as u32),
            _ => (0xFF << 24) | ((fg.r as u32) << 16) | ((fg.g as u32) << 8) | (fg.b as u32),
        };
        let bg_val: u32 = match self.info.format {
            PixelFormat::Rgb => (0xFF << 24) | ((bg.b as u32) << 16) | ((bg.g as u32) << 8) | (bg.r as u32),
            _ => (0xFF << 24) | ((bg.r as u32) << 16) | ((bg.g as u32) << 8) | (bg.b as u32),
        };

        let glyph = FONT_BASIC[glyph_idx];

        for (row, byte) in glyph.iter().enumerate() {
            let py = y + row;
            if py >= self.info.height {
                continue;
            }
            let row_start = (self.info.base_address + (py * self.info.stride + x) as u64 * 4) as *mut u32;
            for col in 0..FONT_WIDTH {
                if x + col >= self.info.width {
                    continue;
                }
                let bit = (byte >> (7 - col)) & 1;
                unsafe {
                    *row_start.add(col) = if bit == 1 { fg_val } else { bg_val };
                }
            }
        }
    }

    pub fn clear(&mut self, color: Color) {
        self.draw_rect(0, 0, self.info.width, self.info.height, color);
    }

    pub fn scroll_up(&mut self, rows: usize, bg: Color) {
        let stride_bytes = self.info.stride * self.info.bytes_per_pixel;
        let move_bytes = (self.info.height - rows) * stride_bytes;

        let src = (self.info.base_address + (rows * stride_bytes) as u64) as *const u64;
        let dst = self.info.base_address as *mut u64;
        let u64_count = move_bytes / 8;

        unsafe {
            core::ptr::copy(src, dst, u64_count);
        }

        self.draw_rect(0, self.info.height - rows, self.info.width, rows, bg);
    }
}
