//! APIC (Advanced Programmable Interrupt Controller) Subsystem

pub mod ioapic;
pub mod lapic;

use crate::arch::x86_64::acpi;
use crate::arch::x86_64::io;
use ioapic::{route_gsi, IoApic, IOAPICS};

pub static mut APIC_TICKS_PER_MS: u32 = 0;

pub fn init() {
    lunix_serial_println!("[apic] Initializing APIC Architecture...");

    let madt = match acpi::get_madt() {
        Some(m) => m,
        None => {
            lunix_serial_println!("[-] APIC: No MADT table found, falling back to legacy PIC.");
            return;
        }
    };

    // 1. Mask Legacy 8259 PIC
    unsafe {
        io::outb(0x21, 0xFF);
        io::outb(0xA1, 0xFF);
        io::io_wait();
    }
    lunix_serial_println!("  [APIC] Legacy 8259 PIC masked.");

    // Map Local APIC MMIO
    let _ = crate::mm::vmm::map_mmio_range(x86_64::PhysAddr::new(madt.local_apic_address), 4096);

    // 2. Initialize Local APIC
    unsafe {
        lapic::init(madt.local_apic_address);
    }

    // 3. Initialize IOAPICs
    {
        let mut ioapic_list = IOAPICS.lock();
        ioapic_list.clear();
        for io in madt.ioapics.iter() {
            let _ = crate::mm::vmm::map_mmio_range(x86_64::PhysAddr::new(io.ioapic_address as u64), 4096);
            unsafe {
                let instance = IoApic::new(io.ioapic_address as u64, io.gsi_base);
                ioapic_list.push(instance);
            }
        }
    }

    // 4. Route Standard Interrupts (Keyboard IRQ1, Storage IRQ14/15)
    let bsp_lapic_id = lapic::id();

    // Map Keyboard (IRQ1): Check for MADT Interrupt Source Override
    let mut kbd_gsi = 1;
    let mut kbd_active_low = false;
    let mut kbd_level = false;

    for iso in madt.interrupt_overrides.iter() {
        if iso.source_irq == 1 {
            kbd_gsi = iso.gsi;
            kbd_active_low = (iso.flags & 2) != 0;
            kbd_level = (iso.flags & 8) != 0;
            break;
        }
    }
    route_gsi(kbd_gsi, 0x21, bsp_lapic_id, kbd_active_low, kbd_level);

    // Map Mouse (IRQ12): Check for MADT Interrupt Source Override
    let mut mouse_gsi = 12;
    let mut mouse_active_low = false;
    let mut mouse_level = false;

    for iso in madt.interrupt_overrides.iter() {
        if iso.source_irq == 12 {
            mouse_gsi = iso.gsi;
            mouse_active_low = (iso.flags & 2) != 0;
            mouse_level = (iso.flags & 8) != 0;
            break;
        }
    }
    route_gsi(mouse_gsi, 0x2C, bsp_lapic_id, mouse_active_low, mouse_level);

    // Map Serial COM1 (IRQ4): Check for MADT Interrupt Source Override
    let mut serial_gsi = 4;
    let mut serial_active_low = false;
    let mut serial_level = false;

    for iso in madt.interrupt_overrides.iter() {
        if iso.source_irq == 4 {
            serial_gsi = iso.gsi;
            serial_active_low = (iso.flags & 2) != 0;
            serial_level = (iso.flags & 8) != 0;
            break;
        }
    }
    route_gsi(serial_gsi, 0x24, bsp_lapic_id, serial_active_low, serial_level);

    // 5. Calibrate APIC Timer
    calibrate_and_start_timer();

    lunix_serial_println!("[apic] APIC subsystem initialized successfully.");
}

fn calibrate_and_start_timer() {
    unsafe {
        // Set divide configuration to 16
        lapic::start_periodic_timer(0xFFFF_FFFF, 0x3);

        // Wait ~10ms using PIT channel 2 or I/O port delay loop
        let start_ticks = 0xFFFF_FFFF;
        for _ in 0..100_000 {
            io::io_wait();
        }

        // Read current count
        let curr = (0xFEE00000u64 + 0x390) as *const u32;
        let elapsed = start_ticks - core::ptr::read_volatile(curr);

        let ticks_per_ms = (elapsed / 10).max(1000);
        APIC_TICKS_PER_MS = ticks_per_ms;

        lunix_serial_println!("  [APIC] Calibrated Timer: {} ticks/ms. Starting 1000 Hz periodic interrupts.", ticks_per_ms);

        // Start periodic 1ms timer (1000 Hz)
        lapic::start_periodic_timer(ticks_per_ms, 0x3);
    }
}
