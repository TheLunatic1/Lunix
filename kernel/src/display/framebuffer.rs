use crate::display::font::FONT_BASIC;
use lunix_common::{FramebufferInfo, PixelFormat};

#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn to_u32(self) -> u32 {
        (0xFF << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

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

        for (i, row) in (y..(y + height).min(self.info.height)).enumerate() {
            if (i & 1) == 0 {
                crate::arch::x86_64::serial::poll_hardware();
            }
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
        let fg64 = fg_val as u64;
        let bg64 = bg_val as u64;

        for (row, &byte) in glyph.iter().enumerate() {
            if (row & 1) == 0 {
                crate::arch::x86_64::serial::poll_hardware();
            }
            let py = y + row;
            if py >= self.info.height || x + 8 > self.info.width {
                continue;
            }
            let row_start = (self.info.base_address + (py * self.info.stride + x) as u64 * 4) as *mut u64;
            
            let p0 = if (byte & 0x80) != 0 { fg64 } else { bg64 };
            let p1 = if (byte & 0x40) != 0 { fg64 } else { bg64 };
            let p2 = if (byte & 0x20) != 0 { fg64 } else { bg64 };
            let p3 = if (byte & 0x10) != 0 { fg64 } else { bg64 };
            let p4 = if (byte & 0x08) != 0 { fg64 } else { bg64 };
            let p5 = if (byte & 0x04) != 0 { fg64 } else { bg64 };
            let p6 = if (byte & 0x02) != 0 { fg64 } else { bg64 };
            let p7 = if (byte & 0x01) != 0 { fg64 } else { bg64 };

            unsafe {
                *row_start.add(0) = p0 | (p1 << 32);
                *row_start.add(1) = p2 | (p3 << 32);
                *row_start.add(2) = p4 | (p5 << 32);
                *row_start.add(3) = p6 | (p7 << 32);
            }
        }
    }

    pub fn clear(&mut self, color: Color) {
        self.draw_rect(0, 0, self.info.width, self.info.height, color);
    }

    pub fn scroll_up(&mut self, rows: usize, bg: Color) {
        self.scroll_region_up(0, self.info.height, rows, bg);
    }

    pub fn scroll_region_up(&mut self, top: usize, bottom: usize, rows: usize, bg: Color) {
        if top >= bottom || rows >= (bottom - top) {
            self.draw_rect(0, top, self.info.width, bottom - top, bg);
            return;
        }

        let stride_bytes = self.info.stride * self.info.bytes_per_pixel;
        let move_height = (bottom - top) - rows;
        let move_bytes = move_height * stride_bytes;

        let src = (self.info.base_address + ((top + rows) * stride_bytes) as u64) as *const u8;
        let dst = (self.info.base_address + (top * stride_bytes) as u64) as *mut u8;

        unsafe {
            let chunk_size = 1024;
            let mut offset = 0;
            while offset < move_bytes {
                let current_chunk = chunk_size.min(move_bytes - offset);
                core::ptr::copy(src.add(offset), dst.add(offset), current_chunk);
                offset += current_chunk;
                crate::arch::x86_64::serial::poll_hardware();
            }
        }

        self.draw_rect(0, bottom - rows, self.info.width, rows, bg);
    }
}
