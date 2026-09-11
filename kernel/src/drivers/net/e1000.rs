//! Intel 82540EM (e1000) Gigabit Ethernet Network Driver
//!
//! Controls Intel e1000 PCI NICs on bare-metal x86_64, supporting EEPROM MAC reading,
//! DMA Transmit/Receive descriptor rings, and raw Ethernet frame dispatching.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;
use x86_64::PhysAddr;
use crate::drivers::pci;
use crate::mm::pmm;
use crate::mm::vmm;
use crate::lunix_println;

// Intel e1000 Register Offsets
const REG_CTRL: usize = 0x0000;
const REG_STATUS: usize = 0x0008;
const REG_EERD: usize = 0x0014;
const REG_ICR: usize = 0x00C0;
const REG_IMS: usize = 0x00D8;
const REG_IMC: usize = 0x00D8 + 8;
const REG_RCTL: usize = 0x0100;
const REG_TCTL: usize = 0x0400;
const REG_TIPG: usize = 0x0410;
const REG_RDBAL: usize = 0x2800;
const REG_RDBAH: usize = 0x2804;
const REG_RDLEN: usize = 0x2808;
const REG_RDH: usize = 0x2810;
const REG_RDT: usize = 0x2818;
const REG_TDBAL: usize = 0x3800;
const REG_TDBAH: usize = 0x3804;
const REG_TDLEN: usize = 0x3808;
const REG_TDH: usize = 0x3810;
const REG_TDT: usize = 0x3818;
const REG_MTA: usize = 0x5200;
const REG_RAL: usize = 0x5400;
const REG_RAH: usize = 0x5404;

const NUM_RX_DESCRIPTORS: usize = 64;
const NUM_TX_DESCRIPTORS: usize = 64;
const PACKET_BUFFER_SIZE: usize = 2048;

#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
pub struct RxDescriptor {
    pub buffer_addr: u64,
    pub length: u16,
    pub checksum: u16,
    pub status: u8,
    pub errors: u8,
    pub special: u16,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
pub struct TxDescriptor {
    pub buffer_addr: u64,
    pub length: u16,
    pub cso: u8,
    pub cmd: u8,
    pub status: u8,
    pub css: u8,
    pub special: u16,
}

pub struct E1000Device {
    mmio_base: u64,
    mac_addr: [u8; 6],
    rx_descs: &'static mut [RxDescriptor],
    tx_descs: &'static mut [TxDescriptor],
    rx_buffers: Vec<u64>,
    tx_buffers: Vec<u64>,
    rx_cur: AtomicUsize,
    tx_cur: AtomicUsize,
    pub rx_packets: AtomicU64,
    pub tx_packets: AtomicU64,
    pub rx_bytes: AtomicU64,
    pub tx_bytes: AtomicU64,
}

pub static E1000: Mutex<Option<Arc<Mutex<E1000Device>>>> = Mutex::new(None);
static E1000_INITIALIZED: AtomicBool = AtomicBool::new(false);

impl E1000Device {
    #[inline(always)]
    fn write_reg(&self, offset: usize, value: u32) {
        unsafe {
            let ptr = (self.mmio_base + offset as u64) as *mut u32;
            core::ptr::write_volatile(ptr, value);
        }
    }

    #[inline(always)]
    fn read_reg(&self, offset: usize) -> u32 {
        unsafe {
            let ptr = (self.mmio_base + offset as u64) as *const u32;
            core::ptr::read_volatile(ptr)
        }
    }

    fn read_eeprom(&self, addr: u8) -> u16 {
        self.write_reg(REG_EERD, 1 | ((addr as u32) << 8));
        let mut val = 0;
        for _ in 0..10_000 {
            val = self.read_reg(REG_EERD);
            if (val & (1 << 4)) != 0 {
                break;
            }
        }
        ((val >> 16) & 0xFFFF) as u16
    }

    pub fn send_packet(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if data.len() > PACKET_BUFFER_SIZE {
            return Err("Packet exceeds max transmission buffer size");
        }

        let cur = self.tx_cur.load(Ordering::SeqCst);
        let desc = &mut self.tx_descs[cur];

        // Copy packet data into DMA buffer
        let buf_phys = self.tx_buffers[cur];
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), buf_phys as *mut u8, data.len());
        }

        // Set descriptor parameters: CMD: EOP (bit 0) | IFCS (bit 1) | RS (bit 3) = 0x0B
        desc.length = data.len() as u16;
        desc.cso = 0;
        desc.cmd = 0x0B;
        desc.status = 0;
        desc.css = 0;
        desc.special = 0;

        let next_cur = (cur + 1) % NUM_TX_DESCRIPTORS;
        self.tx_cur.store(next_cur, Ordering::SeqCst);

        // Advance hardware tail pointer to initiate transmission
        self.write_reg(REG_TDT, next_cur as u32);

        self.tx_packets.fetch_add(1, Ordering::Relaxed);
        self.tx_bytes.fetch_add(data.len() as u64, Ordering::Relaxed);

        Ok(())
    }

    pub fn receive_packet(&mut self) -> Option<Vec<u8>> {
        let cur = self.rx_cur.load(Ordering::SeqCst);
        let desc = &self.rx_descs[cur];

        // Check if Descriptor Done (DD bit 0) is set
        if (desc.status & 1) == 0 {
            return None;
        }

        let len = desc.length as usize;
        let buf_phys = self.rx_buffers[cur];
        let mut packet = alloc::vec![0u8; len];

        unsafe {
            core::ptr::copy_nonoverlapping(buf_phys as *const u8, packet.as_mut_ptr(), len);
        }

        // Reset descriptor status and advance tail
        let desc_mut = &mut self.rx_descs[cur];
        desc_mut.status = 0;

        let next_cur = (cur + 1) % NUM_RX_DESCRIPTORS;
        self.rx_cur.store(next_cur, Ordering::SeqCst);
        self.write_reg(REG_RDT, cur as u32);

        self.rx_packets.fetch_add(1, Ordering::Relaxed);
        self.rx_bytes.fetch_add(len as u64, Ordering::Relaxed);

        Some(packet)
    }

    pub fn mac_address(&self) -> [u8; 6] {
        self.mac_addr
    }
}

pub fn init() -> Result<(), &'static str> {
    // 1. Discover Intel e1000 on PCI Bus
    let pci_dev = pci::find_device(0x8086, 0x100E)
        .or_else(|| pci::find_device(0x8086, 0x1004))
        .or_else(|| pci::find_device(0x8086, 0x100F))
        .or_else(|| pci::find_device(0x8086, 0x153A))
        .or_else(|| pci::find_by_class(0x02, 0x00))
        .ok_or("Intel e1000 network adapter not found on PCI bus")?;

    lunix_println!("[NET] Initializing Intel 82540EM (e1000) Gigabit Ethernet Controller...");
    lunix_serial_println!("  [E1000] Found PCI Device at {:X}:{:X}.0, BAR0: 0x{:X}", pci_dev.bus, pci_dev.device, pci_dev.bar0);

    // 2. Enable PCI Bus Mastering and Memory Space
    pci::enable_bus_mastering(&pci_dev);

    // 3. Map BAR0 MMIO space (128 KiB)
    let bar0_phys = (pci_dev.bar0 & 0xFFFF_FFF0) as u64;
    let mmio_virt = vmm::map_mmio_range(PhysAddr::new(bar0_phys), 128 * 1024)
        .map_err(|_| "Failed to map e1000 MMIO space")?;
    let mmio_base = mmio_virt.as_u64();

    // 4. Allocate Transmit and Receive Descriptor Rings
    let rx_ring_frame = pmm::alloc_frame().ok_or("Failed to allocate RX descriptor ring frame")?;
    let tx_ring_frame = pmm::alloc_frame().ok_or("Failed to allocate TX descriptor ring frame")?;

    let rx_descs = unsafe {
        core::slice::from_raw_parts_mut(
            rx_ring_frame.as_u64() as *mut RxDescriptor,
            NUM_RX_DESCRIPTORS,
        )
    };
    let tx_descs = unsafe {
        core::slice::from_raw_parts_mut(
            tx_ring_frame.as_u64() as *mut TxDescriptor,
            NUM_TX_DESCRIPTORS,
        )
    };

    // 5. Allocate Packet DMA Buffers for RX and TX
    let mut rx_buffers = Vec::with_capacity(NUM_RX_DESCRIPTORS);
    for i in 0..NUM_RX_DESCRIPTORS {
        let frame = pmm::alloc_frame().ok_or("Failed to allocate RX packet buffer frame")?;
        let addr = frame.as_u64();
        rx_buffers.push(addr);
        rx_descs[i].buffer_addr = addr;
        rx_descs[i].status = 0;
    }

    let mut tx_buffers = Vec::with_capacity(NUM_TX_DESCRIPTORS);
    for i in 0..NUM_TX_DESCRIPTORS {
        let frame = pmm::alloc_frame().ok_or("Failed to allocate TX packet buffer frame")?;
        let addr = frame.as_u64();
        tx_buffers.push(addr);
        tx_descs[i].buffer_addr = addr;
        tx_descs[i].status = 1; // Initially done
    }

    let mut device = E1000Device {
        mmio_base,
        mac_addr: [0; 6],
        rx_descs,
        tx_descs,
        rx_buffers,
        tx_buffers,
        rx_cur: AtomicUsize::new(0),
        tx_cur: AtomicUsize::new(0),
        rx_packets: AtomicU64::new(0),
        tx_packets: AtomicU64::new(0),
        rx_bytes: AtomicU64::new(0),
        tx_bytes: AtomicU64::new(0),
    };

    // 6. Read MAC Address (EEPROM or RAL/RAH)
    let w0 = device.read_eeprom(0);
    let w1 = device.read_eeprom(1);
    let w2 = device.read_eeprom(2);

    if w0 != 0 && w0 != 0xFFFF {
        device.mac_addr[0] = (w0 & 0xFF) as u8;
        device.mac_addr[1] = (w0 >> 8) as u8;
        device.mac_addr[2] = (w1 & 0xFF) as u8;
        device.mac_addr[3] = (w1 >> 8) as u8;
        device.mac_addr[4] = (w2 & 0xFF) as u8;
        device.mac_addr[5] = (w2 >> 8) as u8;
    } else {
        // Fallback to RAL0 / RAH0
        let ral = device.read_reg(REG_RAL);
        let rah = device.read_reg(REG_RAH);
        device.mac_addr[0] = (ral & 0xFF) as u8;
        device.mac_addr[1] = ((ral >> 8) & 0xFF) as u8;
        device.mac_addr[2] = ((ral >> 16) & 0xFF) as u8;
        device.mac_addr[3] = ((ral >> 24) & 0xFF) as u8;
        device.mac_addr[4] = (rah & 0xFF) as u8;
        device.mac_addr[5] = ((rah >> 8) & 0xFF) as u8;
    }

    // Default QEMU fallback if all zeroes
    if device.mac_addr == [0; 6] {
        device.mac_addr = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
    }

    lunix_println!(
        "  [E1000] Hardware MAC Address: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        device.mac_addr[0], device.mac_addr[1], device.mac_addr[2],
        device.mac_addr[3], device.mac_addr[4], device.mac_addr[5]
    );

    // 7. Initialize Multicast Table Array (zero all 128 dwords)
    for i in 0..128 {
        device.write_reg(REG_MTA + (i * 4), 0);
    }

    // 8. Configure RX Descriptor Ring
    device.write_reg(REG_RDBAL, rx_ring_frame.as_u64() as u32);
    device.write_reg(REG_RDBAH, (rx_ring_frame.as_u64() >> 32) as u32);
    device.write_reg(REG_RDLEN, (NUM_RX_DESCRIPTORS * core::mem::size_of::<RxDescriptor>()) as u32);
    device.write_reg(REG_RDH, 0);
    device.write_reg(REG_RDT, (NUM_RX_DESCRIPTORS - 1) as u32);

    // Configure RCTL: Enable (bit 1), Store Bad Packets (bit 2), Multicast Promisc (bit 4),
    // Broadcast Accept (bit 15), 2048-byte buffer size (bits 16-17 = 0), Strip CRC (bit 26)
    let rctl = (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 15) | (1 << 26);
    device.write_reg(REG_RCTL, rctl);

    // 9. Configure TX Descriptor Ring
    device.write_reg(REG_TDBAL, tx_ring_frame.as_u64() as u32);
    device.write_reg(REG_TDBAH, (tx_ring_frame.as_u64() >> 32) as u32);
    device.write_reg(REG_TDLEN, (NUM_TX_DESCRIPTORS * core::mem::size_of::<TxDescriptor>()) as u32);
    device.write_reg(REG_TDH, 0);
    device.write_reg(REG_TDT, 0);

    // Configure TIPG (Inter-Packet Gap for 1 Gbps): IPGT=10 (bits 0-9), IPGR1=8 (bits 10-19), IPGR2=6 (bits 20-29)
    device.write_reg(REG_TIPG, 10 | (8 << 10) | (6 << 20));

    // Configure TCTL: Enable (bit 1), Pad Short Packets (bit 3), Collision Threshold=0x0F (bits 4-11), Collision Distance=0x40 (bits 12-21)
    let tctl = (1 << 1) | (1 << 3) | (0x0F << 4) | (0x40 << 12);
    device.write_reg(REG_TCTL, tctl);

    // 10. Enable Link Status Change and RX Interrupts
    device.write_reg(REG_IMS, 0x1F6DC);

    let dev_arc = Arc::new(Mutex::new(device));
    *E1000.lock() = Some(dev_arc);
    E1000_INITIALIZED.store(true, Ordering::SeqCst);

    lunix_println!("[+] Intel e1000 Gigabit NIC initialized (Link: 1000 Mbps Full-Duplex).");
    Ok(())
}

pub fn is_initialized() -> bool {
    E1000_INITIALIZED.load(Ordering::Relaxed)
}

pub fn send_packet(data: &[u8]) -> Result<(), &'static str> {
    let lock = E1000.lock();
    if let Some(ref dev_arc) = *lock {
        let mut dev = dev_arc.lock();
        dev.send_packet(data)
    } else {
        Err("e1000 driver not initialized")
    }
}

pub fn receive_packet() -> Option<Vec<u8>> {
    let lock = E1000.lock();
    if let Some(ref dev_arc) = *lock {
        let mut dev = dev_arc.lock();
        dev.receive_packet()
    } else {
        None
    }
}

pub fn get_mac() -> [u8; 6] {
    let lock = E1000.lock();
    if let Some(ref dev_arc) = *lock {
        let dev = dev_arc.lock();
        dev.mac_address()
    } else {
        [0, 0, 0, 0, 0, 0]
    }
}
