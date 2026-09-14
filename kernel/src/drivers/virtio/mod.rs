//! VirtIO Virtualization Driver Architecture
//!
//! Provides Split VirtQueue data structures and driver interfaces for
//! high-speed paravirtualized I/O devices (virtio-blk and virtio-net).

pub mod blk;
pub mod net;

pub const VRING_DESC_F_NEXT: u16 = 1;
pub const VRING_DESC_F_WRITE: u16 = 2;
pub const VRING_DESC_F_INDIRECT: u16 = 4;

pub const VIRTIO_PCI_HOST_FEATURES: u16 = 0x00;
pub const VIRTIO_PCI_GUEST_FEATURES: u16 = 0x04;
pub const VIRTIO_PCI_QUEUE_PFN: u16 = 0x08;
pub const VIRTIO_PCI_QUEUE_NUM: u16 = 0x0C;
pub const VIRTIO_PCI_QUEUE_SEL: u16 = 0x0E;
pub const VIRTIO_PCI_QUEUE_NOTIFY: u16 = 0x10;
pub const VIRTIO_PCI_STATUS: u16 = 0x12;
pub const VIRTIO_PCI_ISR: u16 = 0x13;

pub const VIRTIO_STATUS_ACKNOWLEDGE: u8 = 1;
pub const VIRTIO_STATUS_DRIVER: u8 = 2;
pub const VIRTIO_STATUS_DRIVER_OK: u8 = 4;
pub const VIRTIO_STATUS_FEATURES_OK: u8 = 8;
pub const VIRTIO_STATUS_FAILED: u8 = 128;

#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
pub struct VirtqDesc {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: u16,
}

#[repr(C)]
pub struct VirtqUsedElem {
    pub id: u32,
    pub len: u32,
}

pub fn init() {
    blk::init();
    net::init();
}
