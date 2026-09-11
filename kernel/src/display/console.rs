use crate::display::font::{FONT_HEIGHT, FONT_WIDTH};
use crate::display::framebuffer::{Color, Framebuffer};
use core::fmt;
use lunix_common::FramebufferInfo;
use spin::Mutex;

pub struct Console {
    pub framebuffer: Framebuffer,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub fg_color: Color,
    pub bg_color: Color,
    pub margin_left: usize,
    pub margin_top: usize,
    pub rows: usize,
    pub cols: usize,
}

impl Console {
    pub fn new(info: FramebufferInfo) -> Self {
        let mut fb = Framebuffer::new(info);
        fb.clear(Color::DARK_BLUE);

        let margin_left = 20;
        let margin_top = 40;
        let cols = (info.width - margin_left * 2) / FONT_WIDTH;
        let rows = (info.height - margin_top - 20) / FONT_HEIGHT;

        let mut console = Self {
            framebuffer: fb,
            cursor_x: 0,
            cursor_y: 0,
            fg_color: Color::WHITE,
            bg_color: Color::DARK_BLUE,
            margin_left,
            margin_top,
            rows,
            cols,
        };

        console.draw_header();
        console
    }

    pub fn draw_header(&mut self) {
        // Draw top status bar
        self.framebuffer.draw_rect(0, 0, self.framebuffer.info.width, 30, Color::DARK_GRAY);
        
        let title = " LUNIX OS [x86_64 Long Mode] | Rust Bare-Metal Hybrid Kernel ";
        let mut x = 10;
        for c in title.chars() {
            self.framebuffer.draw_char(x, 7, c, Color::CYAN, Color::DARK_GRAY);
            x += FONT_WIDTH;
        }

        // Draw accent line
        self.framebuffer.draw_rect(0, 30, self.framebuffer.info.width, 2, Color::CYAN);
    }

    pub fn write_char(&mut self, c: char) {
        crate::arch::x86_64::serial::poll_hardware();
        crate::drivers::keyboard::poll_ps2_hardware();
        match c {
            '\n' => self.new_line(),
            '\r' => self.cursor_x = 0,
            '\t' => {
                let tab_spaces = 4 - (self.cursor_x % 4);
                for _ in 0..tab_spaces {
                    self.write_char(' ');
                }
            }
            '\x08' => {
                // Backspace
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                    let x = self.margin_left + self.cursor_x * FONT_WIDTH;
                    let y = self.margin_top + self.cursor_y * FONT_HEIGHT;
                    self.framebuffer.draw_char(x, y, ' ', self.fg_color, self.bg_color);
                }
            }
            ch => {
                if self.cursor_x >= self.cols {
                    self.new_line();
                }

                let x = self.margin_left + self.cursor_x * FONT_WIDTH;
                let y = self.margin_top + self.cursor_y * FONT_HEIGHT;
                self.framebuffer.draw_char(x, y, ch, self.fg_color, self.bg_color);
                self.cursor_x += 1;
            }
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for c in s.chars() {
            self.write_char(c);
        }
    }

    pub fn new_line(&mut self) {
        crate::arch::x86_64::serial::poll_hardware();
        crate::drivers::keyboard::poll_ps2_hardware();
        self.cursor_x = 0;
        if self.cursor_y + 1 < self.rows {
            self.cursor_y += 1;
        } else {
            // Scroll text region up while keeping top header intact
            let bottom = self.margin_top + self.rows * FONT_HEIGHT;
            self.framebuffer.scroll_region_up(self.margin_top, bottom, FONT_HEIGHT, self.bg_color);
        }
    }

    pub fn set_color(&mut self, fg: Color, bg: Color) {
        self.fg_color = fg;
        self.bg_color = bg;
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

pub static CONSOLE: Mutex<Option<Console>> = Mutex::new(None);
pub static FRAMEBUFFER_INFO: Mutex<Option<FramebufferInfo>> = Mutex::new(None);

pub fn init(info: FramebufferInfo) {
    *FRAMEBUFFER_INFO.lock() = Some(info);
    let console = Console::new(info);
    *CONSOLE.lock() = Some(console);
}

pub fn get_framebuffer_info() -> Option<FramebufferInfo> {
    *FRAMEBUFFER_INFO.lock()
}

#[doc(hidden)]
pub fn _print_fmt(args: fmt::Arguments) {
    use core::fmt::Write;

    // 1. Direct serial output for real-time debugging
    let mut writer = crate::arch::x86_64::serial::DirectSerialWriter;
    let _ = writer.write_fmt(args);

    // 2. Graphical framebuffer console rendering
    if let Some(ref mut console) = *CONSOLE.lock() {
        let _ = console.write_fmt(args);
    }
}

#[macro_export]
macro_rules! lunix_print {
    ($($arg:tt)*) => ($crate::display::console::_print_fmt(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! lunix_println {
    () => ($crate::lunix_print!("\n"));
    ($($arg:tt)*) => {{
        $crate::display::console::_print_fmt(format_args!($($arg)*));
        $crate::display::console::_print_fmt(format_args!("\n"));
    }};
}
