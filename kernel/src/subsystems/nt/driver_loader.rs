//! Windows NT Dynamic Device Driver (.sys) Loader & Manager
//!
//! Loads standard 64-bit PE32+ WDM drivers from VFS, applies relocations,
//! resolves ntoskrnl.exe & hal.dll imports, initializes DRIVER_OBJECT,
//! and executes DriverEntry via the native extern "win64" ABI.

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;
use crate::subsystems::nt::pe::{load_driver, LoadedDriver};
use crate::subsystems::nt::types::*;
use crate::lunix_println;

pub struct ActiveDriverInfo {
    pub name: String,
    pub path: String,
    pub image_base: usize,
    pub image_size: usize,
    pub driver_object: PDRIVER_OBJECT,
    pub device_objects: Vec<PDEVICE_OBJECT>,
}

unsafe impl Send for ActiveDriverInfo {}
unsafe impl Sync for ActiveDriverInfo {}

pub static DRIVER_TABLE: Mutex<Vec<ActiveDriverInfo>> = Mutex::new(Vec::new());

pub fn load_driver_from_vfs(path: &str) -> Result<PDRIVER_OBJECT, &'static str> {
    lunix_println!("[WDM] Loading 64-bit Windows Driver (.sys): '{}'...", path);
    lunix_serial_println!("[WDM] Loading 64-bit Windows Driver (.sys): '{}'...", path);

    // 1. Read file from VFS
    let mut file = crate::fs::vfs::open(path).map_err(|_| "Failed to open driver file in VFS")?;
    let file_size = file.size() as usize;
    if file_size == 0 || file_size > 16 * 1024 * 1024 {
        return Err("Invalid driver file size");
    }

    let mut data = alloc::vec![0u8; file_size];
    let bytes_read = file.read(&mut data).map_err(|_| "Failed to read driver binary data")?;
    drop(file);

    if bytes_read != file_size {
        return Err("Short read on driver binary file");
    }

    // 2. Load PE32+ driver image and resolve imports
    let loaded: LoadedDriver = unsafe { load_driver(&data)? };

    // 3. Prepare Registry Path UNICODE_STRING
    let mut reg_path_utf16: Vec<u16> = "\\Registry\\Machine\\System\\CurrentControlSet\\Services\\LunixDriver"
        .encode_utf16()
        .collect();
    reg_path_utf16.push(0);

    let reg_unicode = UNICODE_STRING {
        length: ((reg_path_utf16.len() - 1) * 2) as u16,
        maximum_length: (reg_path_utf16.len() * 2) as u16,
        buffer: reg_path_utf16.as_mut_ptr(),
    };

    // 4. Invoke DriverEntry(driver_object, &registry_path) via extern "win64"
    let status = unsafe {
        if let Some(entry) = (*loaded.driver_object).driver_init {
            entry(loaded.driver_object, &reg_unicode)
        } else {
            return Err("Driver has no valid DriverEntry point");
        }
    };

    if status != STATUS_SUCCESS {
        lunix_serial_println!("[-] DriverEntry returned error status: 0x{:08X}", status);
        return Err("DriverEntry failed with error status");
    }

    // 5. Gather created device objects
    let mut devices = Vec::new();
    unsafe {
        let mut dev_ptr = (*loaded.driver_object).device_object;
        while !dev_ptr.is_null() {
            devices.push(dev_ptr);
            dev_ptr = (*dev_ptr).next_device;
        }
    }

    let driver_name = if let Some(last_slash) = path.rfind('/') {
        &path[last_slash + 1..]
    } else {
        path
    };

    let info = ActiveDriverInfo {
        name: String::from(driver_name),
        path: String::from(path),
        image_base: loaded.image_base as usize,
        image_size: loaded.image_size,
        driver_object: loaded.driver_object,
        device_objects: devices,
    };

    let obj = loaded.driver_object;
    DRIVER_TABLE.lock().push(info);

    lunix_println!("[+] Windows Driver '{}' loaded successfully! (Status: STATUS_SUCCESS)", driver_name);
    lunix_serial_println!("[+] Windows Driver '{}' loaded successfully! ImageBase=0x{:X}", driver_name, loaded.image_base as usize);

    Ok(obj)
}

pub fn list_drivers() {
    let table = DRIVER_TABLE.lock();
    lunix_println!("Active Windows NT WDM Drivers ({} loaded):", table.len());
    lunix_println!("  NAME                   IMAGE BASE          SIZE     DEVICES");
    lunix_println!("  -----------------------------------------------------------");
    for d in table.iter() {
        lunix_println!("  {:<22} 0x{:016X}  {:>5} KB  {} device(s)",
            d.name, d.image_base, d.image_size / 1024, d.device_objects.len()
        );
    }
}
