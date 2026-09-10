use crate::arch::x86_64::io::{inl, outl};
use crate::lunix_println;


const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

#[derive(Debug, Clone, Copy)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub header_type: u8,
    pub bar0: u32,
    pub bar1: u32,
    pub interrupt_line: u8,
}

impl PciDevice {
    pub fn vendor_name(&self) -> &'static str {
        match self.vendor_id {
            0x8086 => "Intel Corporation",
            0x10DE => "NVIDIA Corporation",
            0x1002 => "AMD / ATI Technologies",
            0x10EC => "Realtek Semiconductor",
            0x1AF4 => "Red Hat / VirtIO",
            0x1B36 => "QEMU Virtual Device",
            0x15AD => "VMware Inc.",
            0x1022 => "AMD Inc.",
            0x1234 => "Bochs / QEMU VGA",
            _ => "Unknown Vendor",
        }
    }

    pub fn class_name(&self) -> &'static str {
        match self.class_code {
            0x01 => "Mass Storage Controller",
            0x02 => "Network Controller",
            0x03 => "Display / GPU Controller",
            0x04 => "Multimedia Controller",
            0x05 => "Memory Controller",
            0x06 => "Bridge Device",
            0x07 => "Communication Controller",
            0x08 => "System Peripheral",
            0x09 => "Input Device Controller",
            0x0C => "Serial Bus Controller (USB/PCIe)",
            _ => "Other Peripheral",
        }
    }
}

pub fn pci_read_u32(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let address = ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xFC)
        | 0x8000_0000;

    unsafe {
        outl(PCI_CONFIG_ADDRESS, address);
        inl(PCI_CONFIG_DATA)
    }
}

pub fn pci_write_u32(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    let address = ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xFC)
        | 0x8000_0000;

    unsafe {
        outl(PCI_CONFIG_ADDRESS, address);
        outl(PCI_CONFIG_DATA, value);
    }
}

pub fn pci_read_u16(bus: u8, device: u8, function: u8, offset: u8) -> u16 {
    let dword = pci_read_u32(bus, device, function, offset);
    ((dword >> ((offset & 2) * 8)) & 0xFFFF) as u16
}

pub fn scan_device(bus: u8, device: u8, function: u8) -> Option<PciDevice> {
    let vendor_id = pci_read_u16(bus, device, function, 0);
    if vendor_id == 0xFFFF || vendor_id == 0x0000 {
        return None;
    }

    let device_id = pci_read_u16(bus, device, function, 2);
    let class_rev = pci_read_u32(bus, device, function, 8);
    let class_code = ((class_rev >> 24) & 0xFF) as u8;
    let subclass = ((class_rev >> 16) & 0xFF) as u8;
    let prog_if = ((class_rev >> 8) & 0xFF) as u8;

    let header_type = ((pci_read_u32(bus, device, function, 0x0C) >> 16) & 0xFF) as u8;
    let bar0 = pci_read_u32(bus, device, function, 0x10);
    let bar1 = pci_read_u32(bus, device, function, 0x14);

    let interrupt_info = pci_read_u32(bus, device, function, 0x3C);
    let interrupt_line = (interrupt_info & 0xFF) as u8;

    Some(PciDevice {
        bus,
        device,
        function,
        vendor_id,
        device_id,
        class_code,
        subclass,
        prog_if,
        header_type,
        bar0,
        bar1,
        interrupt_line,
    })
}

pub fn scan_bus() {
    use crate::arch::x86_64::serial::{write_dec, write_hex, write_str};

    lunix_println!("[+] Scanning PCI Bus for Hardware Devices...");
    write_str("[+] Scanning PCI Bus for Hardware Devices...\n");

    let mut device_count = 0;

    for bus in 0..=1 {
        for device in 0..32 {
            if let Some(dev0) = scan_device(bus, device, 0) {
                device_count += 1;
                
                write_str("  PCI Device [");
                write_hex(dev0.bus as u64);
                write_str(":");
                write_hex(dev0.device as u64);
                write_str(".0]: Vendor=");
                write_hex(dev0.vendor_id as u64);
                write_str(" (");
                write_str(dev0.vendor_name());
                write_str("), Device=");
                write_hex(dev0.device_id as u64);
                write_str(" (");
                write_str(dev0.class_name());
                write_str(")\n");

                lunix_println!(
                    "  PCI [{}:{}.0] {:X}:{:X} | {} | {}",
                    dev0.bus,
                    dev0.device,
                    dev0.vendor_id,
                    dev0.device_id,
                    dev0.vendor_name(),
                    dev0.class_name()
                );

                // If multi-function device, scan functions 1..7
                if (dev0.header_type & 0x80) != 0 {
                    for function in 1..8 {
                        if let Some(dev) = scan_device(bus, device, function) {
                            device_count += 1;
                            
                            write_str("  PCI Device [");
                            write_hex(dev.bus as u64);
                            write_str(":");
                            write_hex(dev.device as u64);
                            write_str(".");
                            write_dec(dev.function as usize);
                            write_str("]: Vendor=");
                            write_hex(dev.vendor_id as u64);
                            write_str(" (");
                            write_str(dev.vendor_name());
                            write_str("), Device=");
                            write_hex(dev.device_id as u64);
                            write_str(" (");
                            write_str(dev.class_name());
                            write_str(")\n");

                            lunix_println!(
                                "  PCI [{}:{}.{}] {:X}:{:X} | {} | {}",
                                dev.bus,
                                dev.device,
                                dev.function,
                                dev.vendor_id,
                                dev.device_id,
                                dev.vendor_name(),
                                dev.class_name()
                            );
                        }
                    }
                }
            }
        }
    }

    write_str("[+] Discovered ");
    write_dec(device_count);
    write_str(" PCI hardware device(s).\n");
    lunix_println!("[+] Discovered {} PCI hardware device(s).", device_count);
}

pub fn init() {
    scan_bus();
}
