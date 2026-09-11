use crate::arch::x86_64::io::{inb, outb};
use core::sync::atomic::{AtomicI32, AtomicU8, Ordering};
use spin::Mutex;

const PS2_DATA_PORT: u16 = 0x60;
const PS2_STATUS_PORT: u16 = 0x64;
const PS2_COMMAND_PORT: u16 = 0x64;

#[derive(Debug, Clone, Copy, Default)]
pub struct MouseState {
    pub x: i32,
    pub y: i32,
    pub left_button: bool,
    pub right_button: bool,
    pub middle_button: bool,
}

static MOUSE_X: AtomicI32 = AtomicI32::new(640);
static MOUSE_Y: AtomicI32 = AtomicI32::new(400);
static MOUSE_BUTTONS: AtomicU8 = AtomicU8::new(0);

static MOUSE_PACKET: Mutex<[u8; 3]> = Mutex::new([0; 3]);
static MOUSE_CYCLE: Mutex<u8> = Mutex::new(0);

static SCREEN_WIDTH: AtomicI32 = AtomicI32::new(1280);
static SCREEN_HEIGHT: AtomicI32 = AtomicI32::new(800);

unsafe fn mouse_wait_write() {
    let mut timeout = 100_000;
    while (inb(PS2_STATUS_PORT) & 0x02) != 0 && timeout > 0 {
        core::hint::spin_loop();
        timeout -= 1;
    }
}

unsafe fn mouse_wait_read() {
    let mut timeout = 100_000;
    while (inb(PS2_STATUS_PORT) & 0x01) == 0 && timeout > 0 {
        core::hint::spin_loop();
        timeout -= 1;
    }
}

unsafe fn mouse_write(byte: u8) {
    mouse_wait_write();
    outb(PS2_COMMAND_PORT, 0xD4); // Tell 8042 next byte goes to mouse
    mouse_wait_write();
    outb(PS2_DATA_PORT, byte);
}

unsafe fn mouse_read() -> u8 {
    mouse_wait_read();
    inb(PS2_DATA_PORT)
}

pub fn init(screen_w: u32, screen_h: u32) {
    SCREEN_WIDTH.store(screen_w as i32, Ordering::Relaxed);
    SCREEN_HEIGHT.store(screen_h as i32, Ordering::Relaxed);
    MOUSE_X.store((screen_w / 2) as i32, Ordering::Relaxed);
    MOUSE_Y.store((screen_h / 2) as i32, Ordering::Relaxed);

    unsafe {
        // 1. Enable second PS/2 port (auxiliary mouse)
        mouse_wait_write();
        outb(PS2_COMMAND_PORT, 0xA8);

        // 2. Read controller configuration byte
        mouse_wait_write();
        outb(PS2_COMMAND_PORT, 0x20);
        let mut status = mouse_read();

        // Enable IRQ12 (bit 1) and enable clock (clear bit 5)
        status |= 0x02;
        status &= !0x20;

        // Write configuration byte back
        mouse_wait_write();
        outb(PS2_COMMAND_PORT, 0x60);
        mouse_wait_write();
        outb(PS2_DATA_PORT, status);

        // 3. Set default mouse settings
        mouse_write(0xF6);
        let _ = mouse_read(); // ACK (0xFA)

        // 4. Enable data reporting / streaming packets
        mouse_write(0xF4);
        let _ = mouse_read(); // ACK (0xFA)
    }
}

pub fn on_interrupt() {
    let status = unsafe { inb(PS2_STATUS_PORT) };
    if (status & 0x20) == 0 {
        // Not a mouse packet (from keyboard)
        return;
    }

    let byte = unsafe { inb(PS2_DATA_PORT) };
    let mut cycle = MOUSE_CYCLE.lock();
    let mut packet = MOUSE_PACKET.lock();

    match *cycle {
        0 => {
            // First byte must have bit 3 set (sync bit)
            if (byte & 0x08) != 0 {
                packet[0] = byte;
                *cycle = 1;
            }
        }
        1 => {
            packet[1] = byte;
            *cycle = 2;
        }
        2 => {
            packet[2] = byte;
            *cycle = 0;

            // Process complete 3-byte packet
            let flags = packet[0];
            let raw_dx = packet[1] as i16;
            let raw_dy = packet[2] as i16;

            // Sign extension
            let dx = if (flags & 0x10) != 0 {
                raw_dx | (!0xFFi16)
            } else {
                raw_dx
            };

            let dy = if (flags & 0x20) != 0 {
                raw_dy | (!0xFFi16)
            } else {
                raw_dy
            };

            // Buttons: Bit 0 = Left, Bit 1 = Right, Bit 2 = Middle
            let buttons = flags & 0x07;
            MOUSE_BUTTONS.store(buttons, Ordering::Relaxed);

            // Update mouse coordinates (Y is inverted on PS/2)
            let cur_x = MOUSE_X.load(Ordering::Relaxed);
            let cur_y = MOUSE_Y.load(Ordering::Relaxed);

            let max_x = SCREEN_WIDTH.load(Ordering::Relaxed) - 1;
            let max_y = SCREEN_HEIGHT.load(Ordering::Relaxed) - 1;

            let new_x = (cur_x + dx as i32).clamp(0, max_x);
            let new_y = (cur_y - dy as i32).clamp(0, max_y);

            MOUSE_X.store(new_x, Ordering::Relaxed);
            MOUSE_Y.store(new_y, Ordering::Relaxed);
        }
        _ => *cycle = 0,
    }
}

pub fn get_mouse_state() -> MouseState {
    let buttons = MOUSE_BUTTONS.load(Ordering::Relaxed);
    MouseState {
        x: MOUSE_X.load(Ordering::Relaxed),
        y: MOUSE_Y.load(Ordering::Relaxed),
        left_button: (buttons & 0x01) != 0,
        right_button: (buttons & 0x02) != 0,
        middle_button: (buttons & 0x04) != 0,
    }
}
