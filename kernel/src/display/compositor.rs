use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use spin::Mutex;

use crate::display::font::{get_glyph, FONT_WIDTH};
use crate::display::framebuffer::Color;
use crate::display::window::Window;
use crate::drivers::mouse::MouseState;

const TITLEBAR_HEIGHT: usize = 26;
const BORDER_SIZE: usize = 2;
const TOPBAR_HEIGHT: usize = 30;

// 16x16 Hardware-Style Arrow Mouse Cursor (1 = white, 2 = black outline, 3 = shadow, 0 = transparent)
#[rustfmt::skip]
const MOUSE_CURSOR_16X16: [u8; 16 * 16] = [
    2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    2, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    2, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    2, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    2, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    2, 1, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    2, 1, 1, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0,
    2, 1, 1, 1, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0,
    2, 1, 1, 1, 1, 2, 2, 2, 2, 2, 0, 0, 0, 0, 0, 0,
    2, 1, 1, 2, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    2, 1, 2, 0, 2, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    2, 2, 0, 0, 2, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    2, 0, 0, 0, 0, 2, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 2, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub struct Compositor {
    pub width: usize,
    pub height: usize,
    pub back_buffer: Vec<u32>,
    pub windows: Vec<Window>,
    pub active_window_id: Option<usize>,
    pub is_gui_active: bool,
    pub fb_base_ptr: *mut u32,
    pub drag_window_id: Option<usize>,
    pub drag_offset_x: i32,
    pub drag_offset_y: i32,
}

unsafe impl Send for Compositor {}
unsafe impl Sync for Compositor {}

pub static COMPOSITOR: Mutex<Option<Compositor>> = Mutex::new(None);

pub fn is_gui_active() -> bool {
    if let Some(guard) = COMPOSITOR.try_lock() {
        if let Some(ref comp) = *guard {
            return comp.is_gui_active;
        }
    }
    false
}

impl Compositor {
    pub fn new(width: usize, height: usize, fb_base: u64) -> Self {
        let back_buffer = vec![Color::rgb(14, 17, 24).to_u32(); width * height];
        Self {
            width,
            height,
            back_buffer,
            windows: Vec::new(),
            active_window_id: None,
            is_gui_active: false,
            fb_base_ptr: fb_base as *mut u32,
            drag_window_id: None,
            drag_offset_x: 0,
            drag_offset_y: 0,
        }
    }

    pub fn add_window(&mut self, window: Window) -> usize {
        let id = window.id;
        self.windows.push(window);
        self.active_window_id = Some(id);
        id
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
                    self.back_buffer[row_offset + px] = color;
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

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let x_end = (x + w).min(self.width);
        let y_end = (y + h).min(self.height);

        for row in y..y_end {
            let row_offset = row * self.width;
            for col in x..x_end {
                self.back_buffer[row_offset + col] = color;
            }
        }
    }

    /// Renders desktop wallpaper with subtle cybernetic dot grid
    pub fn render_wallpaper(&mut self) {
        let w = self.width;
        let h = self.height;

        // Gradient background (Deep Slate Blue -> Dark Onyx)
        for y in 0..h {
            let factor = (y * 255) / h;
            let r = (10 + factor / 12) as u8;
            let g = (14 + factor / 10) as u8;
            let b = (24 + factor / 8) as u8;
            let col = Color::rgb(r, g, b).to_u32();

            let row_offset = y * w;
            for x in 0..w {
                // Subtle dot grid every 32 pixels
                if (x % 32 == 0) && (y % 32 == 0) && y > TOPBAR_HEIGHT {
                    self.back_buffer[row_offset + x] = Color::rgb(45, 65, 95).to_u32();
                } else {
                    self.back_buffer[row_offset + x] = col;
                }
            }
        }
    }

    /// Renders the top menu & status bar
    pub fn render_topbar(&mut self) {
        let w = self.width;

        // Top bar background
        self.fill_rect(0, 0, w, TOPBAR_HEIGHT, Color::rgb(22, 27, 34).to_u32());
        self.fill_rect(0, TOPBAR_HEIGHT - 1, w, 1, Color::rgb(48, 54, 61).to_u32());

        // Lunix OS logo badge
        self.fill_rect(8, 4, 84, 22, Color::rgb(35, 134, 54).to_u32());
        self.draw_string(14, 7, " LUNIX ", Color::WHITE.to_u32(), 0);

        // Active window tabs
        let tabs: Vec<(String, bool)> = self
            .windows
            .iter()
            .map(|w| (w.title.clone(), self.active_window_id == Some(w.id)))
            .collect();

        let mut tab_x = 104;
        for (title, is_active) in tabs {
            let tab_bg = if is_active {
                Color::rgb(45, 55, 72).to_u32()
            } else {
                Color::rgb(30, 35, 45).to_u32()
            };
            let tab_w = 160;

            self.fill_rect(tab_x, 4, tab_w, 22, tab_bg);
            let tab_title = if title.len() > 16 {
                &title[..16]
            } else {
                &title
            };
            self.draw_string(tab_x + 8, 7, tab_title, Color::WHITE.to_u32(), 0);
            tab_x += tab_w + 6;
        }

        // Right side stats: Uptime, CPU, RAM
        let uptime_s = crate::drivers::timer::get_uptime_seconds();
        let hrs = uptime_s / 3600;
        let mins = (uptime_s % 3600) / 60;
        let secs = uptime_s % 60;

        let cores = crate::arch::x86_64::smp::CPU_COUNT.load(core::sync::atomic::Ordering::Relaxed);
        let (_, usable, used) = crate::mm::pmm::get_memory_stats();
        let free_mb = (usable.saturating_sub(used)) / (1024 * 1024);

        let mut stats_str = String::new();
        use core::fmt::Write;
        let _ = write!(stats_str, "CPU: {} Cores | RAM Free: {} MB | {:02}:{:02}:{:02} ", cores, free_mb, hrs, mins, secs);

        let stats_x = w.saturating_sub(stats_str.len() * FONT_WIDTH + 16);
        self.draw_string(stats_x, 7, &stats_str, Color::CYAN.to_u32(), 0);
    }

    /// Renders a window with chrome, titlebar, shadow, and client buffer
    pub fn render_window(&mut self, win: &WindowView) {
        if win.is_minimized {
            return;
        }

        let wx = win.x;
        let wy = win.y;
        let ww = win.width;
        let wh = win.height;
        let total_w = ww + BORDER_SIZE * 2;
        let total_h = wh + TITLEBAR_HEIGHT + BORDER_SIZE * 2;

        // 1. Drop shadow (4px outer shadow)
        for sy in 0..total_h {
            let py = wy + sy as i32 + 4;
            if py < 0 || py >= self.height as i32 {
                continue;
            }
            let row_offset = (py as usize) * self.width;

            for sx in 0..total_w {
                let px = wx + sx as i32 + 4;
                if px < 0 || px >= self.width as i32 {
                    continue;
                }
                // Darken existing background pixel by half
                let orig = self.back_buffer[row_offset + px as usize];
                let r = ((orig >> 16) & 0xFF) / 2;
                let g = ((orig >> 8) & 0xFF) / 2;
                let b = (orig & 0xFF) / 2;
                self.back_buffer[row_offset + px as usize] = (r << 16) | (g << 8) | b;
            }
        }

        // 2. Window Frame Border
        let border_color = if win.is_focused {
            Color::rgb(88, 166, 255).to_u32() // Cyan-Blue border when focused
        } else {
            Color::rgb(60, 68, 77).to_u32()
        };

        let x_start = wx.max(0) as usize;
        let y_start = wy.max(0) as usize;
        self.fill_rect(x_start, y_start, total_w, total_h, border_color);

        // 3. Window Titlebar
        let title_bg = if win.is_focused {
            Color::rgb(31, 111, 235).to_u32() // Modern Azure Blue
        } else {
            Color::rgb(36, 41, 46).to_u32()
        };

        let tx = (wx + BORDER_SIZE as i32).max(0) as usize;
        let ty = (wy + BORDER_SIZE as i32).max(0) as usize;
        self.fill_rect(tx, ty, ww, TITLEBAR_HEIGHT, title_bg);

        // Title text
        self.draw_string(tx + 10, ty + 5, &win.title, Color::WHITE.to_u32(), 0);

        // Window Control Buttons: Close [X], Max [+], Min [-]
        let btn_y = ty + 4;
        let close_x = tx + ww.saturating_sub(22);
        let max_x = close_x.saturating_sub(20);
        let min_x = max_x.saturating_sub(20);

        self.fill_rect(close_x, btn_y, 16, 16, Color::rgb(218, 54, 51).to_u32()); // Red
        self.draw_string(close_x + 4, btn_y + 1, "x", Color::WHITE.to_u32(), 0);

        self.fill_rect(max_x, btn_y, 16, 16, Color::rgb(46, 160, 67).to_u32()); // Green
        self.draw_string(max_x + 4, btn_y + 1, "+", Color::WHITE.to_u32(), 0);

        self.fill_rect(min_x, btn_y, 16, 16, Color::rgb(210, 153, 34).to_u32()); // Yellow
        self.draw_string(min_x + 4, btn_y + 1, "-", Color::WHITE.to_u32(), 0);

        // 4. Blit Client Buffer
        let client_x = wx + BORDER_SIZE as i32;
        let client_y = wy + (TITLEBAR_HEIGHT + BORDER_SIZE) as i32;

        for cy in 0..wh {
            let target_y = client_y + cy as i32;
            if target_y < 0 || target_y >= self.height as i32 {
                continue;
            }
            let dest_row = (target_y as usize) * self.width;
            let src_row = cy * ww;

            for cx in 0..ww {
                let target_x = client_x + cx as i32;
                if target_x < 0 || target_x >= self.width as i32 {
                    continue;
                }
                self.back_buffer[dest_row + (target_x as usize)] = win.buffer[src_row + cx];
            }
        }
    }

    /// Renders mouse pointer on top of all composited layers
    pub fn render_cursor(&mut self, mouse: MouseState) {
        let mx = mouse.x;
        let my = mouse.y;

        for cy in 0..16 {
            let py = my + cy;
            if py < 0 || py >= self.height as i32 {
                continue;
            }
            let row_offset = (py as usize) * self.width;

            for cx in 0..16 {
                let px = mx + cx;
                if px < 0 || px >= self.width as i32 {
                    continue;
                }

                let pixel_type = MOUSE_CURSOR_16X16[(cy as usize) * 16 + (cx as usize)];
                match pixel_type {
                    1 => self.back_buffer[row_offset + (px as usize)] = 0x00FFFFFF, // White pointer
                    2 => self.back_buffer[row_offset + (px as usize)] = 0x00000000, // Black outline
                    _ => {}
                }
            }
        }
    }

    /// Handles mouse clicks for dragging windows, focusing, and buttons
    pub fn handle_mouse(&mut self, mouse: MouseState) {
        if mouse.left_button {
            if let Some(drag_id) = self.drag_window_id {
                // Currently dragging a window
                if let Some(win) = self.windows.iter_mut().find(|w| w.id == drag_id) {
                    win.x = mouse.x - self.drag_offset_x;
                    win.y = mouse.y - self.drag_offset_y;
                }
            } else {
                // Check if user clicked on a window titlebar
                let mut clicked_win_idx = None;
                for (idx, win) in self.windows.iter().enumerate().rev() {
                    let total_w = win.width + BORDER_SIZE * 2;
                    let title_h = TITLEBAR_HEIGHT + BORDER_SIZE;

                    if mouse.x >= win.x && mouse.x <= win.x + total_w as i32 &&
                       mouse.y >= win.y && mouse.y <= win.y + title_h as i32 {
                        clicked_win_idx = Some(idx);
                        break;
                    }
                }

                if let Some(idx) = clicked_win_idx {
                    let win = self.windows.remove(idx);
                    let win_id = win.id;
                    self.drag_offset_x = mouse.x - win.x;
                    self.drag_offset_y = mouse.y - win.y;
                    self.drag_window_id = Some(win_id);
                    self.active_window_id = Some(win_id);

                    // Move to front (top of Z-order)
                    self.windows.push(win);

                    // Update focus flags
                    for w in self.windows.iter_mut() {
                        w.is_focused = w.id == win_id;
                    }
                }
            }
        } else {
            self.drag_window_id = None;
        }
    }

    /// Full frame composition and hardware VRAM blit
    pub fn render_frame(&mut self) {
        // 1. Wallpaper
        self.render_wallpaper();

        // 2. Windows in Z-order
        let win_count = self.windows.len();
        for i in 0..win_count {
            let win = self.windows[i].clone_view();
            self.render_window(&win);
        }

        // 3. Top Menu & Taskbar
        self.render_topbar();

        // 4. Mouse Cursor
        let mouse = crate::drivers::mouse::get_mouse_state();
        self.handle_mouse(mouse);
        self.render_cursor(mouse);

        // 5. High-speed 64-bit word hardware VRAM blit
        unsafe {
            let src = self.back_buffer.as_ptr() as *const u64;
            let dst = self.fb_base_ptr as *mut u64;
            let num_u64 = (self.width * self.height * 4) / 8;
            core::ptr::copy_nonoverlapping(src, dst, num_u64);
        }
    }
}

impl Window {
    pub fn clone_view(&self) -> WindowView {
        WindowView {
            id: self.id,
            title: self.title.clone(),
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            buffer: self.buffer.clone(),
            is_minimized: self.is_minimized,
            is_focused: self.is_focused,
        }
    }
}

pub struct WindowView {
    pub id: usize,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: usize,
    pub height: usize,
    pub buffer: Vec<u32>,
    pub is_minimized: bool,
    pub is_focused: bool,
}
