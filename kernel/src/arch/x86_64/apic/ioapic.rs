//! I/O APIC (I/O Advanced Programmable Interrupt Controller) Driver

use spin::Mutex;

const IOREGSEL: u32 = 0x00;
const IOWIN: u32 = 0x10;

const IOAPIC_ID: u32 = 0x00;
const IOAPIC_VER: u32 = 0x01;
const IOAPIC_ARB: u32 = 0x02;
const IOAPIC_REDTBL: u32 = 0x10;

#[derive(Debug, Clone, Copy)]
pub struct IoApic {
    pub base_addr: u64,
    pub gsi_base: u32,
    pub max_redirection_entries: u8,
}

pub static IOAPICS: Mutex<alloc::vec::Vec<IoApic>> = Mutex::new(alloc::vec::Vec::new());

impl IoApic {
    pub unsafe fn new(base_addr: u64, gsi_base: u32) -> Self {
        let mut ioapic = Self {
            base_addr,
            gsi_base,
            max_redirection_entries: 0,
        };

        let ver_reg = ioapic.read(IOAPIC_VER);
        ioapic.max_redirection_entries = (((ver_reg >> 16) & 0xFF) + 1) as u8;

        lunix_serial_println!("  [IOAPIC] MMIO Base: 0x{:X}, GSI Base: {}, Max Redirections: {}",
            base_addr, gsi_base, ioapic.max_redirection_entries
        );

        // Mask all redirection entries by default
        for i in 0..ioapic.max_redirection_entries {
            ioapic.set_entry(i, 0x10000 | (0x20 + i as u64)); // Masked bit 16
        }

        ioapic
    }

    #[inline]
    unsafe fn read(&self, reg: u32) -> u32 {
        let sel_ptr = (self.base_addr + IOREGSEL as u64) as *mut u32;
        let win_ptr = (self.base_addr + IOWIN as u64) as *const u32;

        core::ptr::write_volatile(sel_ptr, reg);
        core::ptr::read_volatile(win_ptr)
    }

    #[inline]
    unsafe fn write(&self, reg: u32, val: u32) {
        let sel_ptr = (self.base_addr + IOREGSEL as u64) as *mut u32;
        let win_ptr = (self.base_addr + IOWIN as u64) as *mut u32;

        core::ptr::write_volatile(sel_ptr, reg);
        core::ptr::write_volatile(win_ptr, val);
    }

    pub unsafe fn set_entry(&self, index: u8, value: u64) {
        let low_reg = IOAPIC_REDTBL + 2 * index as u32;
        let high_reg = IOAPIC_REDTBL + 2 * index as u32 + 1;

        let low = value as u32;
        let high = (value >> 32) as u32;

        self.write(low_reg, low);
        self.write(high_reg, high);
    }

    pub unsafe fn route_irq(
        &self,
        irq_index: u8,
        vector: u8,
        dest_apic_id: u8,
        active_low: bool,
        level_triggered: bool,
    ) {
        if irq_index >= self.max_redirection_entries {
            return;
        }

        let mut entry_low: u64 = vector as u64; // Delivery mode: Fixed (000), Dest mode: Physical (0)
        if active_low {
            entry_low |= 1 << 13; // Polarity active low
        }
        if level_triggered {
            entry_low |= 1 << 15; // Trigger mode: level
        }
        // Bit 16 is 0 (Unmasked)

        let entry_high: u64 = (dest_apic_id as u64) << 56;
        let full_entry = entry_low | entry_high;

        self.set_entry(irq_index, full_entry);
    }
}

pub fn route_gsi(
    gsi: u32,
    vector: u8,
    dest_apic_id: u8,
    active_low: bool,
    level_triggered: bool,
) {
    let ioapics = IOAPICS.lock();
    for ioapic in ioapics.iter() {
        if gsi >= ioapic.gsi_base && gsi < ioapic.gsi_base + ioapic.max_redirection_entries as u32 {
            let index = (gsi - ioapic.gsi_base) as u8;
            unsafe {
                ioapic.route_irq(index, vector, dest_apic_id, active_low, level_triggered);
            }
            lunix_serial_println!("  [IOAPIC] Routed GSI {} -> IDT Vector 0x{:X} (Dest LAPIC ID: {})",
                gsi, vector, dest_apic_id
            );
            return;
        }
    }
}
