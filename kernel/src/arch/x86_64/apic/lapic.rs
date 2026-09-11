//! Local APIC (Advanced Programmable Interrupt Controller) Driver

use core::sync::atomic::{compiler_fence, Ordering};

// Local APIC Register Offsets
const LAPIC_ID: u32 = 0x020;
const LAPIC_VER: u32 = 0x030;
const LAPIC_TPR: u32 = 0x080;
const LAPIC_EOI: u32 = 0x0B0;
const LAPIC_LDR: u32 = 0x0D0;
const LAPIC_DFR: u32 = 0x0E0;
const LAPIC_SVR: u32 = 0x0F0;
const LAPIC_ESR: u32 = 0x280;
const LAPIC_ICR_LOW: u32 = 0x300;
const LAPIC_ICR_HIGH: u32 = 0x310;
const LAPIC_LVT_TIMER: u32 = 0x320;
const LAPIC_LVT_LINT0: u32 = 0x350;
const LAPIC_LVT_LINT1: u32 = 0x360;
const LAPIC_LVT_ERROR: u32 = 0x370;
const LAPIC_TIMER_INIT_CNT: u32 = 0x380;
const LAPIC_TIMER_CURR_CNT: u32 = 0x390;
const LAPIC_TIMER_DIV_CFG: u32 = 0x3E0;

// SVR Flags
const APIC_SW_ENABLE: u32 = 0x100;
pub const SPURIOUS_VECTOR: u8 = 0xFF;
pub const TIMER_VECTOR: u8 = 0x20;

static mut LAPIC_BASE: u64 = 0xFEE00000;

#[inline]
unsafe fn lapic_read(reg: u32) -> u32 {
    let ptr = (LAPIC_BASE + reg as u64) as *const u32;
    core::ptr::read_volatile(ptr)
}

#[inline]
unsafe fn lapic_write(reg: u32, val: u32) {
    let ptr = (LAPIC_BASE + reg as u64) as *mut u32;
    core::ptr::write_volatile(ptr, val);
    compiler_fence(Ordering::SeqCst);
}

/// Initializes the Local APIC for the current CPU core.
pub unsafe fn init(base_addr: u64) {
    LAPIC_BASE = base_addr;

    // Enable Local APIC globally in IA32_APIC_BASE MSR (0x1B)
    let apic_base_msr = crate::arch::x86_64::io::rdmsr(0x1B);
    if (apic_base_msr & (1 << 11)) == 0 {
        crate::arch::x86_64::io::wrmsr(0x1B, apic_base_msr | (1 << 11));
    }

    // Set Flat Destination Format
    lapic_write(LAPIC_DFR, 0xFFFF_FFFF);

    // Set Logical Destination Register to ID 1
    let ldr = (lapic_read(LAPIC_LDR) & 0x00FF_FFFF) | 1;
    lapic_write(LAPIC_LDR, ldr);

    // Clear Task Priority Register to accept all interrupts
    lapic_write(LAPIC_TPR, 0);

    // Disable LINT0 and LINT1
    lapic_write(LAPIC_LVT_LINT0, 0x10000); // Masked
    lapic_write(LAPIC_LVT_LINT1, 0x10000); // Masked

    // Map Error Interrupt
    lapic_write(LAPIC_LVT_ERROR, 0xFE);

    // Clear error status
    lapic_write(LAPIC_ESR, 0);
    lapic_write(LAPIC_ESR, 0);

    // Send EOI to clear any pending interrupt state
    lapic_write(LAPIC_EOI, 0);

    // Enable APIC by setting bit 8 in SVR and configuring spurious interrupt vector
    lapic_write(LAPIC_SVR, APIC_SW_ENABLE | (SPURIOUS_VECTOR as u32));

    lunix_serial_println!("  [LAPIC] Initialized on CPU core (APIC ID: {}, SVR: 0x{:X})", id(), lapic_read(LAPIC_SVR));
}

/// Returns the Local APIC ID of the calling CPU core.
pub fn id() -> u8 {
    unsafe { (lapic_read(LAPIC_ID) >> 24) as u8 }
}

/// Sends End of Interrupt (EOI) signal to the Local APIC.
#[inline]
pub fn eoi() {
    unsafe {
        lapic_write(LAPIC_EOI, 0);
    }
}

/// Configures Local APIC timer in Periodic mode.
pub unsafe fn start_periodic_timer(initial_count: u32, divide_value: u32) {
    // Set divide configuration (0x3 = Divide by 16)
    lapic_write(LAPIC_TIMER_DIV_CFG, divide_value);

    // Set Periodic mode (bit 17) and vector
    lapic_write(LAPIC_LVT_TIMER, 0x20000 | (TIMER_VECTOR as u32));

    // Set initial count
    lapic_write(LAPIC_TIMER_INIT_CNT, initial_count);
}

/// Sends an Inter-Processor Interrupt (IPI) to a target APIC ID.
/// Sends an Inter-Processor Interrupt (IPI) to a target APIC ID.
pub unsafe fn send_ipi(dest_apic_id: u8, delivery_mode: u32, vector: u8, level: u32, trigger: u32) {
    let mut timeout = 10_000;
    while (lapic_read(LAPIC_ICR_LOW) & (1 << 12)) != 0 && timeout > 0 {
        core::hint::spin_loop();
        timeout -= 1;
    }

    // Set target APIC ID in high ICR
    lapic_write(LAPIC_ICR_HIGH, (dest_apic_id as u32) << 24);

    // Write command to low ICR
    let icr_low = (vector as u32) | delivery_mode | level | trigger;
    lapic_write(LAPIC_ICR_LOW, icr_low);

    let mut timeout = 10_000;
    while (lapic_read(LAPIC_ICR_LOW) & (1 << 12)) != 0 && timeout > 0 {
        core::hint::spin_loop();
        timeout -= 1;
    }
}
