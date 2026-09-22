//! Linux input event devices: `/dev/input/event0` (keyboard) and `/dev/input/event1`
//! (mouse).
//!
//! The PS/2 drivers feed raw scancodes and mouse packets in; userspace (Xorg's
//! `xf86-input-evdev`, libinput, ...) reads `struct input_event` records and queries the
//! device with the `EVIOC*` ioctls, exactly as on Linux.

use crate::fs::file::{FileHandle, SeekFrom};
use alloc::collections::VecDeque;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

// Event types
const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;

const REL_X: u16 = 0x00;
const REL_Y: u16 = 0x01;

const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;

/// Serialized `struct input_event` (24 bytes on x86_64).
type Raw = [u8; 24];

const MAX_QUEUED: usize = 1024;

pub const KEYBOARD: usize = 0;
pub const MOUSE: usize = 1;

static QUEUES: [Mutex<VecDeque<Raw>>; 2] = [Mutex::new(VecDeque::new()), Mutex::new(VecDeque::new())];
static EXTENDED: AtomicBool = AtomicBool::new(false);
static LAST_MOUSE_BUTTONS: Mutex<u8> = Mutex::new(0);

fn event(typ: u16, code: u16, value: i32) -> Raw {
    let ms = crate::drivers::timer::get_ticks();
    let (sec, usec) = ((ms / 1000) as i64, ((ms % 1000) * 1000) as i64);
    let mut r = [0u8; 24];
    r[0..8].copy_from_slice(&sec.to_le_bytes());
    r[8..16].copy_from_slice(&usec.to_le_bytes());
    r[16..18].copy_from_slice(&typ.to_le_bytes());
    r[18..20].copy_from_slice(&code.to_le_bytes());
    r[20..24].copy_from_slice(&value.to_le_bytes());
    r
}

fn push(dev: usize, evs: &[Raw]) {
    let mut q = QUEUES[dev].lock();
    if q.len() + evs.len() > MAX_QUEUED {
        q.clear(); // reader is not keeping up: drop stale input rather than block the ISR
    }
    q.extend(evs.iter().copied());
}

/// Feed one raw scancode-set-1 byte from the PS/2 keyboard.
pub fn feed_scancode(code: u8) {
    if code == 0xE0 {
        EXTENDED.store(true, Ordering::Relaxed);
        return;
    }
    if code == 0xE1 {
        return; // Pause/Break sequence: not translated
    }
    let extended = EXTENDED.swap(false, Ordering::Relaxed);
    let released = code & 0x80 != 0;
    let make = (code & 0x7F) as u16;
    let key = if extended {
        match make {
            0x1C => 96,  // KP Enter
            0x1D => 97,  // Right Ctrl
            0x35 => 98,  // KP /
            0x38 => 100, // Right Alt
            0x47 => 102, // Home
            0x48 => 103, // Up
            0x49 => 104, // Page Up
            0x4B => 105, // Left
            0x4D => 106, // Right
            0x4F => 107, // End
            0x50 => 108, // Down
            0x51 => 109, // Page Down
            0x52 => 110, // Insert
            0x53 => 111, // Delete
            0x5B => 125, // Left Meta
            0x5C => 126, // Right Meta
            0x5D => 127, // Compose/Menu
            _ => return,
        }
    } else if (1..=0x58).contains(&make) {
        make // Linux keycodes equal set-1 make codes for the main block
    } else {
        return;
    };
    push(KEYBOARD, &[event(EV_KEY, key, if released { 0 } else { 1 }), event(EV_SYN, 0, 0)]);
}

/// Feed a decoded PS/2 mouse packet (`dy` is PS/2-style: positive = up).
pub fn mouse_packet(dx: i32, dy: i32, buttons: u8) {
    let mut evs: alloc::vec::Vec<Raw> = alloc::vec::Vec::with_capacity(6);
    if dx != 0 {
        evs.push(event(EV_REL, REL_X, dx));
    }
    if dy != 0 {
        evs.push(event(EV_REL, REL_Y, -dy));
    }
    {
        let mut last = LAST_MOUSE_BUTTONS.lock();
        for (bit, code) in [(0x01u8, BTN_LEFT), (0x02, BTN_RIGHT), (0x04, BTN_MIDDLE)] {
            if (*last ^ buttons) & bit != 0 {
                evs.push(event(EV_KEY, code, (buttons & bit != 0) as i32));
            }
        }
        *last = buttons;
    }
    if !evs.is_empty() {
        evs.push(event(EV_SYN, 0, 0));
        push(MOUSE, &evs);
    }
}

pub struct EventDev {
    dev: usize,
}

impl EventDev {
    pub fn new(dev: usize) -> Self {
        EventDev { dev }
    }

    fn name(&self) -> &'static str {
        if self.dev == KEYBOARD { "Lunix PS/2 Keyboard" } else { "Lunix PS/2 Mouse" }
    }

    /// Fill `size` bytes at `arg` with the capability bitmap for event type `ev`.
    fn capability_bits(&self, ev: u16, size: usize, arg: u64) -> isize {
        let buf = unsafe { core::slice::from_raw_parts_mut(arg as *mut u8, size) };
        buf.fill(0);
        let mut set = |bit: usize| {
            if bit / 8 < size {
                buf[bit / 8] |= 1 << (bit % 8);
            }
        };
        match (self.dev, ev) {
            (_, EV_SYN) => {
                set(EV_SYN as usize);
                set(EV_KEY as usize);
                if self.dev == MOUSE {
                    set(EV_REL as usize);
                }
            }
            (KEYBOARD, EV_KEY) => {
                for k in 1..=127usize {
                    set(k);
                }
            }
            (MOUSE, EV_KEY) => {
                set(BTN_LEFT as usize);
                set(BTN_RIGHT as usize);
                set(BTN_MIDDLE as usize);
            }
            (MOUSE, EV_REL) => {
                set(REL_X as usize);
                set(REL_Y as usize);
            }
            _ => {}
        }
        size as isize
    }
}

impl FileHandle for EventDev {
    /// Blocks until at least one event is queued, then returns as many whole events as fit.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        loop {
            match self.read_nonblock(buf) {
                Err("EAGAIN") => crate::task::scheduler::sleep_ms(1),
                other => return other,
            }
        }
    }

    fn read_nonblock(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        if buf.len() < 24 {
            return Err("EINVAL");
        }
        x86_64::instructions::interrupts::without_interrupts(|| {
            let mut q = QUEUES[self.dev].lock();
            if q.is_empty() {
                return Err("EAGAIN");
            }
            let mut n = 0;
            while n + 24 <= buf.len() {
                match q.pop_front() {
                    Some(ev) => {
                        buf[n..n + 24].copy_from_slice(&ev);
                        n += 24;
                    }
                    None => break,
                }
            }
            Ok(n)
        })
    }

    fn poll_readable(&self) -> bool {
        x86_64::instructions::interrupts::without_interrupts(|| !QUEUES[self.dev].lock().is_empty())
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        Ok(buf.len()) // LED / repeat-rate writes are accepted and ignored
    }

    fn seek(&mut self, _pos: SeekFrom) -> Result<u64, &'static str> {
        Ok(0)
    }

    fn size(&self) -> u64 {
        0
    }

    /// `EVIOC*` ioctls (type 'E'). Returns None for requests that are not for evdev.
    fn ioctl(&mut self, req: u64, arg: u64) -> Option<isize> {
        if (req >> 8) & 0xFF != 0x45 {
            return None;
        }
        let nr = (req & 0xFF) as u16;
        let size = ((req >> 16) & 0x3FFF) as usize;
        if arg == 0 {
            return Some(-14); // -EFAULT
        }
        let write_bytes = |bytes: &[u8]| -> isize {
            let n = bytes.len().min(size);
            unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), arg as *mut u8, n) };
            n as isize
        };
        Some(match nr {
            0x01 => write_bytes(&0x0001_0001u32.to_le_bytes()), // EVIOCGVERSION
            0x02 => {
                // EVIOCGID: struct input_id { bustype, vendor, product, version }
                let mut id = [0u8; 8];
                id[0..2].copy_from_slice(&0x0011u16.to_le_bytes()); // BUS_I8042
                id[2..4].copy_from_slice(&0x0001u16.to_le_bytes());
                id[4..6].copy_from_slice(&((self.dev + 1) as u16).to_le_bytes());
                id[6..8].copy_from_slice(&0x0100u16.to_le_bytes());
                write_bytes(&id)
            }
            0x06 => {
                // EVIOCGNAME
                let mut s = alloc::vec::Vec::from(self.name().as_bytes());
                s.push(0);
                write_bytes(&s)
            }
            0x07 => write_bytes(b"isa0060/serio0/input0\0"), // EVIOCGPHYS
            0x08 => -2, // EVIOCGUNIQ: no unique id. libevdev accepts only -ENOENT for "absent"
            0x09 | 0x18 | 0x19 | 0x1B => {
                // EVIOCGPROP / GKEY / GLED / GSW: nothing set
                unsafe { core::ptr::write_bytes(arg as *mut u8, 0, size) };
                size as isize
            }
            0x20..=0x3F => self.capability_bits(nr - 0x20, size, arg), // EVIOCGBIT(ev)
            0x40..=0x7F => -22, // EVIOCGABS: no absolute axes
            0x03 => {
                // EVIOCGREP: key repeat delay/period (ms)
                let mut v = [0u8; 8];
                v[0..4].copy_from_slice(&250u32.to_le_bytes());
                v[4..8].copy_from_slice(&33u32.to_le_bytes());
                write_bytes(&v)
            }
            0x90 | 0xA0 | 0x91 => 0, // EVIOCGRAB / SCLOCKID / REVOKE
            _ => -25,
        })
    }
}
