use crate::arch::x86_64::io::outb;
use core::sync::atomic::{AtomicU64, Ordering};

const PIT_CHANNEL_0_DATA: u16 = 0x40;
const PIT_COMMAND_MODE: u16 = 0x43;
const PIT_FREQUENCY_HZ: u64 = 1193182;
const TARGET_FREQUENCY_HZ: u64 = 1000; // 1000 Hz = 1ms tick

static TICKS: AtomicU64 = AtomicU64::new(0);

pub fn init() {
    let divisor: u16 = (PIT_FREQUENCY_HZ / TARGET_FREQUENCY_HZ) as u16;

    unsafe {
        // Channel 0, Access mode low/high byte, Rate Generator (mode 2), 16-bit binary
        outb(PIT_COMMAND_MODE, 0x34);
        outb(PIT_CHANNEL_0_DATA, (divisor & 0xFF) as u8);
        outb(PIT_CHANNEL_0_DATA, ((divisor >> 8) & 0xFF) as u8);
    }
}

pub fn on_tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

pub fn get_ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

pub fn get_uptime_ms() -> u64 {
    get_ticks()
}

pub fn get_uptime_seconds() -> u64 {
    get_ticks() / 1000
}

pub fn sleep_ms(ms: u64) {
    let start = get_ticks();
    while get_ticks() - start < ms {
        x86_64::instructions::hlt();
    }
}
