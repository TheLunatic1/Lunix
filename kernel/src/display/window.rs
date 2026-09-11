use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::display::font::{get_glyph, FONT_WIDTH};
use crate::display::framebuffer::Color;

pub struct Window {
    pub id: usize,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: usize,
    pub height: usize,
    pub buffer: Vec<u32>, // ARGB / BGR32 client area pixel buffer
    pub is_minimized: bool,
    pub is_focused: bool,
    pub bg_color: u32,
}

impl Window {
    pub fn new(id: usize, title: &str, x: i32, y: i32, width: usize, height: usize) -> Self {
        let bg = Color::rgb(20, 24, 32).to_u32();
        let buffer = vec![bg; width * height];

        Self {
            id,
            title: String::from(title),
            x,
            y,
            width,
            height,
            buffer,
            is_minimized: false,
            is_focused: true,
            bg_color: bg,
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            self.buffer[y * self.width + x] = color;
        }
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let x_end = (x + w).min(self.width);
        let y_end = (y + h).min(self.height);

        for row in y..y_end {
            let row_offset = row * self.width;
            for col in x..x_end {
                self.buffer[row_offset + col] = color;
            }
        }
    }

    pub fn draw_char(&mut self, x: usize, y: usize, c: char, fg: u32, bg: u32) {
        let glyph = get_glyph(c);

        for (row, &byte) in glyph.iter().enumerate() {
            let py = y + row;
            if py >= self.height {
                break;
            }
            let row_offset = py * self.width;

            for col in 0..8 {
                let px = x + col;
                if px >= self.width {
                    break;
                }
                let color = if (byte & (0x80 >> col)) != 0 { fg } else { bg };
                if color != 0 {
                    self.buffer[row_offset + px] = color;
                }
            }
        }
    }

    pub fn draw_string(&mut self, mut x: usize, y: usize, text: &str, fg: u32, bg: u32) {
        for c in text.chars() {
            self.draw_char(x, y, c, fg, bg);
            x += FONT_WIDTH;
            if x + FONT_WIDTH > self.width {
                break;
            }
        }
    }

    pub fn clear(&mut self, color: u32) {
        self.buffer.fill(color);
    }
}
