//! ACPI (Advanced Configuration and Power Interface) Subsystem

pub mod madt;
pub mod rsdp;
pub mod sdt;

use lunix_common::BootInfo;
use madt::{MadtHeader, MadtInfo};
use rsdp::{find_rsdp, RsdpDescriptorV1, RsdpDescriptorV2};
use sdt::SdtHeader;
use spin::Mutex;

#[derive(Debug, Clone)]
pub struct AcpiTables {
    pub rsdp_address: u64,
    pub revision: u8,
    pub madt: Option<MadtInfo>,
    pub fadt_address: Option<u64>,
    pub hpet_address: Option<u64>,
}

pub static ACPI_INFO: Mutex<Option<AcpiTables>> = Mutex::new(None);

pub fn init(boot_info: &BootInfo) {
    lunix_serial_println!("[acpi] Initializing ACPI 2.0 Subsystem...");

    let rsdp_addr = match unsafe { find_rsdp(boot_info.rsdp_addr) } {
        Some(addr) => addr,
        None => {
            lunix_serial_println!("[-] ACPI: Could not locate RSDP table!");
            return;
        }
    };

    lunix_serial_println!("  [ACPI] RSDP located at 0x{:X}", rsdp_addr);
    let rsdp_v1 = unsafe { &*(rsdp_addr as *const RsdpDescriptorV1) };
    let revision = rsdp_v1.revision;

    let mut madt_info = None;
    let mut fadt_address = None;
    let mut hpet_address = None;

    if revision >= 2 {
        // Use XSDT (64-bit pointers)
        let rsdp_v2 = unsafe { &*(rsdp_addr as *const RsdpDescriptorV2) };
        let xsdt_addr = rsdp_v2.xsdt_address;
        lunix_serial_println!("  [ACPI] Using XSDT at 0x{:X}", xsdt_addr);

        if xsdt_addr != 0 {
            let xsdt_header = unsafe { &*(xsdt_addr as *const SdtHeader) };
            if xsdt_header.is_valid() && &xsdt_header.signature == b"XSDT" {
                let entry_count = (xsdt_header.length as usize - core::mem::size_of::<SdtHeader>()) / 8;
                let entries_ptr = unsafe { (xsdt_addr as *const u8).add(core::mem::size_of::<SdtHeader>()) as *const u64 };

                for i in 0..entry_count {
                    let table_addr = unsafe { core::ptr::read_unaligned(entries_ptr.add(i)) };
                    if table_addr == 0 {
                        continue;
                    }
                    let table_header = unsafe { &*(table_addr as *const SdtHeader) };
                    if !table_header.is_valid() {
                        continue;
                    }

                    lunix_serial_println!("    [SDT] Found Table: {} (OEM: {}) at 0x{:X}",
                        table_header.signature_str(),
                        table_header.oem_id_str(),
                        table_addr
                    );

                    match &table_header.signature {
                        b"APIC" => {
                            madt_info = MadtInfo::parse(table_addr as *const MadtHeader);
                        }
                        b"FACP" => {
                            fadt_address = Some(table_addr);
                        }
                        b"HPET" => {
                            hpet_address = Some(table_addr);
                        }
                        _ => {}
                    }
                }
            }
        }
    } else {
        // Use RSDT (32-bit pointers)
        let rsdt_addr = rsdp_v1.rsdt_address as u64;
        lunix_serial_println!("  [ACPI] Using RSDT at 0x{:X}", rsdt_addr);

        if rsdt_addr != 0 {
            let rsdt_header = unsafe { &*(rsdt_addr as *const SdtHeader) };
            if rsdt_header.is_valid() && &rsdt_header.signature == b"RSDT" {
                let entry_count = (rsdt_header.length as usize - core::mem::size_of::<SdtHeader>()) / 4;
                let entries_ptr = unsafe { (rsdt_addr as *const u8).add(core::mem::size_of::<SdtHeader>()) as *const u32 };

                for i in 0..entry_count {
                    let table_addr = unsafe { core::ptr::read_unaligned(entries_ptr.add(i)) } as u64;
                    if table_addr == 0 {
                        continue;
                    }
                    let table_header = unsafe { &*(table_addr as *const SdtHeader) };
                    if !table_header.is_valid() {
                        continue;
                    }

                    lunix_serial_println!("    [SDT] Found Table: {} at 0x{:X}", table_header.signature_str(), table_addr);

                    match &table_header.signature {
                        b"APIC" => {
                            madt_info = MadtInfo::parse(table_addr as *const MadtHeader);
                        }
                        b"FACP" => {
                            fadt_address = Some(table_addr);
                        }
                        b"HPET" => {
                            hpet_address = Some(table_addr);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    if let Some(ref madt) = madt_info {
        lunix_serial_println!("  [MADT] Local APIC Base: 0x{:X}, Detected Cores: {}, IOAPICs: {}",
            madt.local_apic_address,
            madt.processors.len(),
            madt.ioapics.len()
        );
        for (idx, proc) in madt.processors.iter().enumerate() {
            lunix_serial_println!("    CPU #{}: APIC ID {}, ACPI ID {}, Enabled: {}",
                idx, proc.apic_id, proc.acpi_processor_id, proc.is_enabled()
            );
        }
        for (idx, io) in madt.ioapics.iter().enumerate() {
            lunix_serial_println!("    IOAPIC #{}: ID {}, MMIO Base 0x{:X}, GSI Base {}",
                idx, io.ioapic_id, io.ioapic_address, io.gsi_base
            );
        }
    }

    let tables = AcpiTables {
        rsdp_address: rsdp_addr,
        revision,
        madt: madt_info,
        fadt_address,
        hpet_address,
    };

    *ACPI_INFO.lock() = Some(tables);
    lunix_serial_println!("[acpi] ACPI subsystem initialized successfully.");
}

pub fn get_madt() -> Option<MadtInfo> {
    ACPI_INFO.lock().as_ref().and_then(|info| info.madt.clone())
}
