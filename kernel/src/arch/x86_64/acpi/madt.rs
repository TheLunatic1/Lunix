//! ACPI MADT (Multiple APIC Description Table) Parser

use super::sdt::SdtHeader;
use alloc::vec::Vec;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct MadtHeader {
    pub header: SdtHeader,
    pub local_apic_address: u32,
    pub flags: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct ProcessorLocalApic {
    pub acpi_processor_id: u8,
    pub apic_id: u8,
    pub flags: u32,
}

impl ProcessorLocalApic {
    pub fn is_enabled(&self) -> bool {
        (self.flags & 1) != 0 || (self.flags & 2) != 0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IoApicEntry {
    pub ioapic_id: u8,
    pub ioapic_address: u32,
    pub gsi_base: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct InterruptSourceOverride {
    pub bus: u8,
    pub source_irq: u8,
    pub gsi: u32,
    pub flags: u16,
}

#[derive(Debug, Clone)]
pub struct MadtInfo {
    pub local_apic_address: u64,
    pub is_pcat_compat: bool,
    pub processors: Vec<ProcessorLocalApic>,
    pub ioapics: Vec<IoApicEntry>,
    pub interrupt_overrides: Vec<InterruptSourceOverride>,
}

impl MadtInfo {
    pub fn parse(madt_ptr: *const MadtHeader) -> Option<Self> {
        let madt = unsafe { &*madt_ptr };
        if !madt.header.is_valid() {
            return None;
        }

        let mut local_apic_addr = madt.local_apic_address as u64;
        let is_pcat_compat = (madt.flags & 1) != 0;

        let mut processors = Vec::new();
        let mut ioapics = Vec::new();
        let mut interrupt_overrides = Vec::new();

        let total_length = madt.header.length as usize;
        let header_size = core::mem::size_of::<MadtHeader>();

        let mut offset = header_size;
        let base_ptr = madt_ptr as *const u8;

        while offset < total_length {
            let entry_type = unsafe { *base_ptr.add(offset) };
            let entry_len = unsafe { *base_ptr.add(offset + 1) } as usize;

            if entry_len < 2 || offset + entry_len > total_length {
                break;
            }

            match entry_type {
                0 => {
                    // Processor Local APIC
                    if entry_len >= 8 {
                        let acpi_proc_id = unsafe { *base_ptr.add(offset + 2) };
                        let apic_id = unsafe { *base_ptr.add(offset + 3) };
                        let flags = unsafe {
                            core::ptr::read_unaligned(base_ptr.add(offset + 4) as *const u32)
                        };
                        processors.push(ProcessorLocalApic {
                            acpi_processor_id: acpi_proc_id,
                            apic_id,
                            flags,
                        });
                    }
                }
                1 => {
                    // I/O APIC
                    if entry_len >= 12 {
                        let ioapic_id = unsafe { *base_ptr.add(offset + 2) };
                        let ioapic_address = unsafe {
                            core::ptr::read_unaligned(base_ptr.add(offset + 4) as *const u32)
                        };
                        let gsi_base = unsafe {
                            core::ptr::read_unaligned(base_ptr.add(offset + 8) as *const u32)
                        };
                        ioapics.push(IoApicEntry {
                            ioapic_id,
                            ioapic_address,
                            gsi_base,
                        });
                    }
                }
                2 => {
                    // Interrupt Source Override
                    if entry_len >= 10 {
                        let bus = unsafe { *base_ptr.add(offset + 2) };
                        let source_irq = unsafe { *base_ptr.add(offset + 3) };
                        let gsi = unsafe {
                            core::ptr::read_unaligned(base_ptr.add(offset + 4) as *const u32)
                        };
                        let flags = unsafe {
                            core::ptr::read_unaligned(base_ptr.add(offset + 8) as *const u16)
                        };
                        interrupt_overrides.push(InterruptSourceOverride {
                            bus,
                            source_irq,
                            gsi,
                            flags,
                        });
                    }
                }
                5 => {
                    // Local APIC Address Override (64-bit address)
                    if entry_len >= 12 {
                        let addr_override = unsafe {
                            core::ptr::read_unaligned(base_ptr.add(offset + 4) as *const u64)
                        };
                        local_apic_addr = addr_override;
                    }
                }
                _ => {}
            }

            offset += entry_len;
        }

        Some(MadtInfo {
            local_apic_address: local_apic_addr,
            is_pcat_compat,
            processors,
            ioapics,
            interrupt_overrides,
        })
    }
}
