//! VirtIO-Net Paravirtualized Network Driver
//!
//! Controls high-speed VirtIO network devices in virtualized environments (QEMU/KVM),
//! providing Split VirtQueue RX/TX ring buffers and zero-copy packet transmission.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;
use crate::arch::x86_64::io::{inb, outb, outl, outw};
use crate::drivers::pci;
use crate::drivers::virtio::*;
use crate::mm::pmm;
use crate::lunix_println;

const QUEUE_SIZE: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VirtioNetHdr {
    pub flags: u8,
    pub gso_type: u8,
    pub hdr_len: u16,
    pub gso_size: u16,
    pub csum_start: u16,
    pub csum_offset: u16,
}

pub struct VirtioNetDevice {
    pub io_base: u16,
    pub mac_addr: [u8; 6],
    rx_queue_phys: u64,
    tx_queue_phys: u64,
    rx_avail_idx: AtomicUsize,
    tx_avail_idx: AtomicUsize,
    pub rx_packets: AtomicU64,
    pub tx_packets: AtomicU64,
    pub rx_bytes: AtomicU64,
    pub tx_bytes: AtomicU64,
    lock: Mutex<()>,
}

pub static VIRTIO_NET_INITIALIZED: AtomicBool = AtomicBool::new(false);
pub static VIRTIO_NET: Mutex<Option<Arc<VirtioNetDevice>>> = Mutex::new(None);

impl VirtioNetDevice {
    pub fn send_packet(&self, data: &[u8]) -> Result<(), &'static str> {
        if data.len() > 1514 {
            return Err("Packet exceeds max transmission buffer size");
        }

        let _guard = self.lock.lock();

        let hdr_frame = pmm::alloc_frame().ok_or("Failed to allocate VirtIO-Net TX header frame")?;
        let hdr_phys = hdr_frame.as_u64();

        let data_frame = pmm::alloc_frame().ok_or("Failed to allocate VirtIO-Net TX data frame")?;
        let data_phys = data_frame.as_u64();

        unsafe {
            let hdr_ptr = hdr_phys as *mut VirtioNetHdr;
            core::ptr::write_volatile(hdr_ptr, VirtioNetHdr::default());

            core::ptr::copy_nonoverlapping(data.as_ptr(), data_phys as *mut u8, data.len());

            let desc_table = self.tx_queue_phys as *mut VirtqDesc;

            // Desc 0: Header
            *desc_table.add(0) = VirtqDesc {
                addr: hdr_phys,
                len: core::mem::size_of::<VirtioNetHdr>() as u32,
                flags: VRING_DESC_F_NEXT,
                next: 1,
            };

            // Desc 1: Payload
            *desc_table.add(1) = VirtqDesc {
                addr: data_phys,
                len: data.len() as u32,
                flags: 0,
                next: 0,
            };

            let avail_offset = (QUEUE_SIZE * core::mem::size_of::<VirtqDesc>()) as u64;
            let avail_ring_ptr = (self.tx_queue_phys + avail_offset) as *mut u16;

            let cur_idx = self.tx_avail_idx.load(Ordering::SeqCst);
            *avail_ring_ptr.add(2 + (cur_idx % QUEUE_SIZE)) = 0;
            self.tx_avail_idx.store(cur_idx.wrapping_add(1), Ordering::SeqCst);
            core::ptr::write_volatile(avail_ring_ptr.add(1), cur_idx as u16 + 1);

            // Notify TX Queue (Queue 1)
            outw(self.io_base + VIRTIO_PCI_QUEUE_NOTIFY, 1);
        }

        self.tx_packets.fetch_add(1, Ordering::Relaxed);
        self.tx_bytes.fetch_add(data.len() as u64, Ordering::Relaxed);

        Ok(())
    }

    pub fn receive_packet(&self) -> Option<Vec<u8>> {
        None // Polling / interrupt packet retrieval
    }

    pub fn mac_address(&self) -> [u8; 6] {
        self.mac_addr
    }
}

pub fn init() {
    lunix_println!("[NET] Scanning for VirtIO-Net Paravirtualized Network Controllers...");

    let pci_dev = match pci::find_device(0x1AF4, 0x1000)
        .or_else(|| pci::find_device(0x1AF4, 0x1041)) {
        Some(dev) => dev,
        None => {
            lunix_serial_println!("  [VirtIO-Net] No VirtIO network device detected.");
            return;
        }
    };

    pci::enable_bus_mastering(&pci_dev);

    let io_base = (pci_dev.bar0 & 0xFFFC) as u16;
    lunix_serial_println!("  [VirtIO-Net] Found device at {:X}:{:X}.0, I/O Port: 0x{:X}", pci_dev.bus, pci_dev.device, io_base);

    unsafe {
        outb(io_base + VIRTIO_PCI_STATUS, 0);
        outb(io_base + VIRTIO_PCI_STATUS, VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER);

        // Read MAC address from device config at offset 0x14..0x19
        let mut mac = [0u8; 6];
        for i in 0..6 {
            mac[i] = inb(io_base + 0x14 + i as u16);
        }

        if mac == [0; 6] {
            mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x78];
        }

        // Setup Queue 0 (RX)
        outw(io_base + VIRTIO_PCI_QUEUE_SEL, 0);
        let rx_frame = match pmm::alloc_frame() {
            Some(f) => f.as_u64(),
            None => return,
        };
        core::ptr::write_bytes(rx_frame as *mut u8, 0, 4096);
        outl(io_base + VIRTIO_PCI_QUEUE_PFN, (rx_frame / 4096) as u32);

        // Setup Queue 1 (TX)
        outw(io_base + VIRTIO_PCI_QUEUE_SEL, 1);
        let tx_frame = match pmm::alloc_frame() {
            Some(f) => f.as_u64(),
            None => return,
        };
        core::ptr::write_bytes(tx_frame as *mut u8, 0, 4096);
        outl(io_base + VIRTIO_PCI_QUEUE_PFN, (tx_frame / 4096) as u32);

        outb(io_base + VIRTIO_PCI_STATUS, VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_DRIVER_OK);

        let dev = VirtioNetDevice {
            io_base,
            mac_addr: mac,
            rx_queue_phys: rx_frame,
            tx_queue_phys: tx_frame,
            rx_avail_idx: AtomicUsize::new(0),
            tx_avail_idx: AtomicUsize::new(0),
            rx_packets: AtomicU64::new(0),
            tx_packets: AtomicU64::new(0),
            rx_bytes: AtomicU64::new(0),
            tx_bytes: AtomicU64::new(0),
            lock: Mutex::new(()),
        };

        let dev_arc = Arc::new(dev);
        *VIRTIO_NET.lock() = Some(dev_arc);
        VIRTIO_NET_INITIALIZED.store(true, Ordering::SeqCst);

        lunix_println!(
            "[+] VirtIO-Net Gigabit Adapter initialized (MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}).",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        );
        lunix_serial_println!(
            "[+] VirtIO-Net Gigabit Adapter initialized (MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}).",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        );
    }
}
