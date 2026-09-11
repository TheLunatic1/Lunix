use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

pub trait BlockDevice: Send + Sync {
    fn name(&self) -> &str;
    fn block_size(&self) -> usize {
        512
    }
    fn total_blocks(&self) -> u64;
    fn read_blocks(&self, start_block: u64, count: usize, buf: &mut [u8]) -> Result<(), &'static str>;
    fn write_blocks(&self, start_block: u64, count: usize, buf: &[u8]) -> Result<(), &'static str>;
}

static BLOCK_DEVICES: Mutex<Vec<Arc<dyn BlockDevice>>> = Mutex::new(Vec::new());

pub fn register_block_device(device: Arc<dyn BlockDevice>) {
    lunix_serial_println!("  [BLOCK] Registered block device: /dev/{} (Blocks: {}, Size: {} MB)",
        device.name(),
        device.total_blocks(),
        (device.total_blocks() * device.block_size() as u64) / (1024 * 1024)
    );
    BLOCK_DEVICES.lock().push(device);
}

pub fn get_block_device(name: &str) -> Option<Arc<dyn BlockDevice>> {
    let devices = BLOCK_DEVICES.lock();
    for dev in devices.iter() {
        if dev.name() == name {
            return Some(dev.clone());
        }
    }
    None
}

pub fn list_block_devices() -> Vec<String> {
    BLOCK_DEVICES
        .lock()
        .iter()
        .map(|d| String::from(d.name()))
        .collect()
}
