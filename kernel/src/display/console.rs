use crate::display::font::{FONT_HEIGHT, FONT_WIDTH};
use crate::display::framebuffer::{Color, Framebuffer};
use core::fmt;
use lunix_common::FramebufferInfo;
use spin::Mutex;

const MAX_COLS: usize = 160;
const MAX_ROWS: usize = 64;

#[derive(Clone, Copy)]
pub struct ConsoleCell {
    pub c: char,
    pub fg: Color,
    pub bg: Color,
}

impl ConsoleCell {
    pub const fn empty(bg: Color) -> Self {
        Self {
            c: ' ',
            fg: Color::WHITE,
            bg,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Esc {
    None,
    Esc,
    Csi,
    Skip,
}

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
    pub cells: [[ConsoleCell; MAX_COLS]; MAX_ROWS],
    esc: Esc,
    params: [u16; 4],
    nparams: usize,
    have_param: bool,
    private: bool,
    cursor_visible: bool,
    /// KD_GRAPHICS: a userspace program (the X server) owns the screen. The console keeps
    /// tracking text in its cell grid but does not draw it.
    graphics: bool,
}

impl Console {
    pub fn new(info: FramebufferInfo) -> Self {
        let mut fb = Framebuffer::new(info);
        fb.clear(Color::DARK_BLUE);

        let margin_left = 20;
        let margin_top = 40;
        let cols = ((info.width - margin_left * 2) / FONT_WIDTH).min(MAX_COLS);
        let rows = ((info.height - margin_top - 20) / FONT_HEIGHT).min(MAX_ROWS);

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
            cells: [[ConsoleCell::empty(Color::DARK_BLUE); MAX_COLS]; MAX_ROWS],
            esc: Esc::None,
            params: [0; 4],
            nparams: 0,
            have_param: false,
            private: false,
            cursor_visible: true,
            graphics: false,
        };

        console.draw_header();
        console
    }

    pub fn draw_header(&mut self) {
        // Draw top status bar
        self.framebuffer.draw_rect(0, 0, self.framebuffer.info.width, 30, Color::DARK_GRAY);
        
        let title = " Arch Linux [x86_64] | Lunix Bare-Metal Linux Kernel 6.8.0 ";
        let mut x = 10;
        for c in title.chars() {
            self.framebuffer.draw_char(x, 7, c, Color::CYAN, Color::DARK_GRAY);
            x += FONT_WIDTH;
        }

        // Draw accent line
        self.framebuffer.draw_rect(0, 30, self.framebuffer.info.width, 2, Color::CYAN);
    }

    fn draw_cell(&mut self, row: usize, col: usize) {
        if self.graphics {
            return;
        }
        let cell = self.cells[row][col];
        let x = self.margin_left + col * FONT_WIDTH;
        let y = self.margin_top + row * FONT_HEIGHT;
        self.framebuffer.draw_char(x, y, cell.c, cell.fg, cell.bg);
    }

    fn erase_cells(&mut self, row: usize, from: usize, to: usize) {
        for col in from..to.min(self.cols) {
            self.cells[row][col] = ConsoleCell::empty(self.bg_color);
            self.draw_cell(row, col);
        }
    }

    /// Draw (or undraw) the block cursor at the current position.
    fn paint_cursor(&mut self, on: bool) {
        if self.graphics || self.cursor_x >= self.cols || self.cursor_y >= self.rows {
            return;
        }
        let (row, col) = (self.cursor_y, self.cursor_x);
        if on && self.cursor_visible {
            let cell = self.cells[row][col];
            let x = self.margin_left + col * FONT_WIDTH;
            let y = self.margin_top + row * FONT_HEIGHT;
            self.framebuffer.draw_char(x, y, cell.c, cell.bg, Color::LIGHT_GRAY);
        } else {
            self.draw_cell(row, col);
        }
    }

    pub fn write_char(&mut self, c: char) {
        self.paint_cursor(false);
        self.put_char(c);
        self.paint_cursor(true);
    }

    /// Interpret one character: plain text, control characters, or part of an
    /// ANSI/VT100 escape sequence (what the Linux VT console understands).
    fn put_char(&mut self, c: char) {
        crate::arch::x86_64::serial::poll_hardware();
        crate::drivers::keyboard::poll_ps2_hardware();

        match self.esc {
            Esc::Esc => {
                self.esc = Esc::None;
                match c {
                    '[' => {
                        self.esc = Esc::Csi;
                        self.params = [0; 4];
                        self.nparams = 0;
                        self.have_param = false;
                        self.private = false;
                    }
                    // Character-set selection takes one more byte; OSC strings are not supported.
                    '(' | ')' | '*' | '+' | '#' => self.esc = Esc::Skip,
                    'c' => {
                        self.fg_color = Color::WHITE;
                        self.bg_color = Color::DARK_BLUE;
                    }
                    _ => {}
                }
                return;
            }
            Esc::Skip => {
                self.esc = Esc::None;
                return;
            }
            Esc::Csi => {
                match c {
                    '0'..='9' => {
                        let i = self.nparams.min(3);
                        self.params[i] = self.params[i].saturating_mul(10).saturating_add(c as u16 - '0' as u16);
                        self.have_param = true;
                    }
                    ';' => {
                        self.nparams = (self.nparams + 1).min(3);
                        self.have_param = false;
                    }
                    '?' | '>' | '=' | '!' => self.private = true,
                    '\u{40}'..='\u{7e}' => {
                        if self.have_param || self.nparams > 0 {
                            self.nparams = (self.nparams + 1).min(4);
                        }
                        self.esc = Esc::None;
                        self.csi_dispatch(c);
                    }
                    _ => {}
                }
                return;
            }
            Esc::None => {}
        }

        match c {
            '\x1b' => self.esc = Esc::Esc,
            '\n' => self.new_line(),
            '\r' => self.cursor_x = 0,
            '\t' => {
                let next = ((self.cursor_x / 8) + 1) * 8;
                self.cursor_x = next.min(self.cols.saturating_sub(1));
            }
            '\x08' => {
                // Backspace only moves the cursor; the program erases with a space.
                self.cursor_x = self.cursor_x.saturating_sub(1);
            }
            '\x07' | '\0' => {}
            ch if (ch as u32) < 0x20 => {}
            ch => {
                if self.cursor_x >= self.cols {
                    self.new_line();
                }
                self.cells[self.cursor_y][self.cursor_x] = ConsoleCell {
                    c: ch,
                    fg: self.fg_color,
                    bg: self.bg_color,
                };
                self.draw_cell(self.cursor_y, self.cursor_x);
                self.cursor_x += 1;
            }
        }
    }

    fn param(&self, i: usize, default: usize) -> usize {
        if i < self.nparams && self.params[i] != 0 {
            self.params[i] as usize
        } else {
            default
        }
    }

    fn csi_dispatch(&mut self, final_byte: char) {
        let (rows, cols) = (self.rows, self.cols);
        let (cx, cy) = (self.cursor_x.min(cols.saturating_sub(1)), self.cursor_y);
        match final_byte {
            'A' => self.cursor_y = cy.saturating_sub(self.param(0, 1)),
            'B' => self.cursor_y = (cy + self.param(0, 1)).min(rows - 1),
            'C' => self.cursor_x = (cx + self.param(0, 1)).min(cols - 1),
            'D' => self.cursor_x = cx.saturating_sub(self.param(0, 1)),
            'G' => self.cursor_x = (self.param(0, 1) - 1).min(cols - 1),
            'H' | 'f' => {
                self.cursor_y = (self.param(0, 1) - 1).min(rows - 1);
                self.cursor_x = (self.param(1, 1) - 1).min(cols - 1);
            }
            'J' => {
                let mode = if self.nparams > 0 { self.params[0] } else { 0 };
                match mode {
                    0 => {
                        self.erase_cells(cy, cx, cols);
                        for r in cy + 1..rows {
                            self.erase_cells(r, 0, cols);
                        }
                    }
                    1 => {
                        for r in 0..cy {
                            self.erase_cells(r, 0, cols);
                        }
                        self.erase_cells(cy, 0, cx + 1);
                    }
                    _ => {
                        for r in 0..rows {
                            self.erase_cells(r, 0, cols);
                        }
                    }
                }
            }
            'K' => {
                let mode = if self.nparams > 0 { self.params[0] } else { 0 };
                match mode {
                    0 => self.erase_cells(cy, cx, cols),
                    1 => self.erase_cells(cy, 0, cx + 1),
                    _ => self.erase_cells(cy, 0, cols),
                }
            }
            'X' => self.erase_cells(cy, cx, cx + self.param(0, 1)),
            'P' => {
                // Delete characters: shift the rest of the row left.
                let n = self.param(0, 1).min(cols - cx);
                for c in cx..cols - n {
                    self.cells[cy][c] = self.cells[cy][c + n];
                }
                for c in cols - n..cols {
                    self.cells[cy][c] = ConsoleCell::empty(self.bg_color);
                }
                for c in cx..cols {
                    self.draw_cell(cy, c);
                }
            }
            '@' => {
                // Insert blanks: shift the rest of the row right.
                let n = self.param(0, 1).min(cols - cx);
                for c in (cx + n..cols).rev() {
                    self.cells[cy][c] = self.cells[cy][c - n];
                }
                for c in cx..cx + n {
                    self.cells[cy][c] = ConsoleCell::empty(self.bg_color);
                }
                for c in cx..cols {
                    self.draw_cell(cy, c);
                }
            }
            'm' => self.sgr(),
            'h' | 'l' if self.private => {
                // ?25h / ?25l show and hide the cursor; other private modes
                // (bracketed paste, application keys, ...) need no console support.
                if self.nparams > 0 && self.params[0] == 25 {
                    self.cursor_visible = final_byte == 'h';
                }
            }
            _ => {}
        }
    }

    fn sgr(&mut self) {
        const PALETTE: [Color; 8] = [
            Color::rgb(0, 0, 0),
            Color::RED,
            Color::GREEN,
            Color::YELLOW,
            Color::BLUE,
            Color::MAGENTA,
            Color::CYAN,
            Color::LIGHT_GRAY,
        ];
        let n = self.nparams.max(1);
        for i in 0..n {
            let p = if self.nparams == 0 { 0 } else { self.params[i] };
            match p {
                0 => {
                    self.fg_color = Color::WHITE;
                    self.bg_color = Color::DARK_BLUE;
                }
                7 => core::mem::swap(&mut self.fg_color, &mut self.bg_color),
                30..=37 => self.fg_color = PALETTE[(p - 30) as usize],
                90..=97 => self.fg_color = PALETTE[(p - 90) as usize],
                39 => self.fg_color = Color::WHITE,
                40..=47 => self.bg_color = PALETTE[(p - 40) as usize],
                49 => self.bg_color = Color::DARK_BLUE,
                _ => {}
            }
        }
    }

    pub fn write_string(&mut self, s: &str) {
        self.paint_cursor(false);
        for c in s.chars() {
            self.put_char(c);
        }
        self.paint_cursor(true);
    }

    pub fn new_line(&mut self) {
        crate::arch::x86_64::serial::poll_hardware();
        crate::drivers::keyboard::poll_ps2_hardware();
        self.cursor_x = 0;
        if self.cursor_y + 1 < self.rows {
            self.cursor_y += 1;
        } else {
            // Scroll text region up in RAM cells and redraw smoothly from memory
            for row in 0..self.rows - 1 {
                self.cells[row] = self.cells[row + 1];
            }
            self.cells[self.rows - 1] = [ConsoleCell::empty(self.bg_color); MAX_COLS];

            if !self.graphics {
                for row in 0..self.rows {
                    let y = self.margin_top + row * FONT_HEIGHT;
                    for col in 0..self.cols {
                        let cell = self.cells[row][col];
                        let x = self.margin_left + col * FONT_WIDTH;
                        self.framebuffer.draw_char(x, y, cell.c, cell.fg, cell.bg);
                    }
                }
            }
        }
    }

    pub fn redraw_screen(&mut self) {
        self.framebuffer.clear(self.bg_color);
        self.draw_header();
        for row in 0..self.rows {
            let y = self.margin_top + row * FONT_HEIGHT;
            for col in 0..self.cols {
                let cell = self.cells[row][col];
                let x = self.margin_left + col * FONT_WIDTH;
                self.framebuffer.draw_char(x, y, cell.c, cell.fg, cell.bg);
            }
        }
    }

    /// Switch between text mode (console draws) and graphics mode (a program owns the
    /// screen). Returning to text mode repaints the console.
    pub fn set_graphics(&mut self, on: bool) {
        if self.graphics == on {
            return;
        }
        self.graphics = on;
        if !on {
            self.redraw_screen();
        }
    }

    pub fn is_graphics(&self) -> bool {
        self.graphics
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

/// `KDSETMODE`: true = KD_GRAPHICS (the console stops drawing), false = KD_TEXT.
pub fn set_graphics_mode(on: bool) {
    if let Some(ref mut c) = *CONSOLE.lock() {
        c.set_graphics(on);
    }
}

pub fn graphics_mode() -> bool {
    CONSOLE.lock().as_ref().map_or(false, |c| c.is_graphics())
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
