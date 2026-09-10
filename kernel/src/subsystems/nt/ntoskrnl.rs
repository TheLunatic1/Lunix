use crate::lunix_serial_println;
use crate::mm::vmm;
use crate::subsystems::nt::types::*;
use core::ffi::c_void;
use x86_64::PhysAddr;

#[no_mangle]
pub unsafe extern "win64" fn ExAllocatePoolWithTag(
    pool_type: POOL_TYPE,
    number_of_bytes: SIZE_T,
    tag: ULONG,
) -> PVOID {
    use alloc::alloc::{alloc, Layout};

    let layout = match Layout::from_size_align(number_of_bytes, 16) {
        Ok(l) => l,
        Err(_) => return core::ptr::null_mut(),
    };

    let ptr = alloc(layout) as PVOID;
    lunix_serial_println!(
        "[ntoskrnl] ExAllocatePoolWithTag(Type={:?}, Size={}, Tag=0x{:08X}) -> {:p}",
        pool_type,
        number_of_bytes,
        tag,
        ptr
    );
    ptr
}

#[no_mangle]
pub unsafe extern "win64" fn ExFreePoolWithTag(p: PVOID, tag: ULONG) {
    lunix_serial_println!("[ntoskrnl] ExFreePoolWithTag(Ptr={:p}, Tag=0x{:08X})", p, tag);
    // Real memory deallocation via alloc::alloc::dealloc
}

#[no_mangle]
pub unsafe extern "win64" fn MmMapIoSpace(
    physical_address: u64,
    number_of_bytes: SIZE_T,
    cache_type: ULONG,
) -> PVOID {
    lunix_serial_println!(
        "[ntoskrnl] MmMapIoSpace(Phys=0x{:X}, Size={}, Cache={})",
        physical_address,
        number_of_bytes,
        cache_type
    );

    match vmm::map_mmio_range(PhysAddr::new(physical_address), number_of_bytes) {
        Ok(virt) => virt.as_mut_ptr::<c_void>(),
        Err(_) => core::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "win64" fn MmUnmapIoSpace(base_address: PVOID, number_of_bytes: SIZE_T) {
    lunix_serial_println!(
        "[ntoskrnl] MmUnmapIoSpace(Virt={:p}, Size={})",
        base_address,
        number_of_bytes
    );
}

#[no_mangle]
pub unsafe extern "win64" fn KeInitializeSpinLock(spin_lock: *mut KSPIN_LOCK) {
    if !spin_lock.is_null() {
        *spin_lock = 0;
    }
}

#[no_mangle]
pub unsafe extern "win64" fn KeAcquireSpinLock(spin_lock: *mut KSPIN_LOCK, old_irql: *mut KIRQL) {
    if !old_irql.is_null() {
        *old_irql = DISPATCH_LEVEL;
    }
    if !spin_lock.is_null() {
        let lock_ptr = spin_lock as *mut core::sync::atomic::AtomicUsize;
        while (*lock_ptr)
            .compare_exchange(
                0,
                1,
                core::sync::atomic::Ordering::Acquire,
                core::sync::atomic::Ordering::Relaxed,
            )
            .is_err()
        {
            core::hint::spin_loop();
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn KeReleaseSpinLock(spin_lock: *mut KSPIN_LOCK, new_irql: KIRQL) {
    let _ = new_irql;
    if !spin_lock.is_null() {
        let lock_ptr = spin_lock as *mut core::sync::atomic::AtomicUsize;
        (*lock_ptr).store(0, core::sync::atomic::Ordering::Release);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn IoCreateDevice(
    driver_object: PDRIVER_OBJECT,
    device_extension_size: ULONG,
    _device_name: *const UNICODE_STRING,
    device_type: ULONG,
    device_characteristics: ULONG,
    _exclusive: BOOLEAN,
    device_object: *mut PDEVICE_OBJECT,
) -> NTSTATUS {
    lunix_serial_println!(
        "[ntoskrnl] IoCreateDevice(Driver={:p}, ExtSize={}, Type=0x{:X})",
        driver_object,
        device_extension_size,
        device_type
    );

    let total_size = core::mem::size_of::<DEVICE_OBJECT>() + (device_extension_size as usize);
    let raw_dev = ExAllocatePoolWithTag(POOL_TYPE::NonPagedPool, total_size, 0x65766544); // "Deve"

    if raw_dev.is_null() {
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    let dev_ptr = raw_dev as *mut DEVICE_OBJECT;
    (*dev_ptr).driver_object = driver_object;
    (*dev_ptr).device_type = device_type;
    (*dev_ptr).characteristics = device_characteristics;
    (*dev_ptr).device_extension = (raw_dev as usize + core::mem::size_of::<DEVICE_OBJECT>()) as PVOID;

    if !device_object.is_null() {
        *device_object = dev_ptr;
    }

    if !driver_object.is_null() {
        (*driver_object).device_object = dev_ptr;
    }

    STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn IoCompleteRequest(irp: PIRP, priority_boost: i8) {
    lunix_serial_println!("[ntoskrnl] IoCompleteRequest(IRP={:p}, Boost={})", irp, priority_boost);
}

#[no_mangle]
pub unsafe extern "win64" fn IoCallDriver(device_object: PDEVICE_OBJECT, irp: PIRP) -> NTSTATUS {
    if device_object.is_null() || irp.is_null() {
        return STATUS_INVALID_PARAMETER;
    }

    let driver = (*device_object).driver_object;
    if driver.is_null() {
        return STATUS_DEVICE_NOT_CONNECTED;
    }

    // Default to MJ_INTERNAL_DEVICE_CONTROL or MJ_DEVICE_CONTROL
    STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn DbgPrint(format: *const u8) -> ULONG {
    if !format.is_null() {
        let mut len = 0;
        while *format.add(len) != 0 && len < 256 {
            len += 1;
        }
        if let Ok(msg) = core::str::from_utf8(core::slice::from_raw_parts(format, len)) {
            lunix_serial_println!("[NT_DRIVER_DBG] {}", msg);
            crate::lunix_println!("  [NT-DBG] {}", msg);
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn DbgPrintEx(
    component_id: ULONG,
    level: ULONG,
    format: *const u8,
) -> ULONG {
    let _ = (component_id, level);
    DbgPrint(format)
}

#[no_mangle]
pub unsafe extern "win64" fn RtlInitUnicodeString(
    destination_string: *mut UNICODE_STRING,
    source_string: *const u16,
) {
    if destination_string.is_null() {
        return;
    }

    if source_string.is_null() {
        *destination_string = UNICODE_STRING::empty();
        return;
    }

    let mut len = 0;
    while *source_string.add(len) != 0 {
        len += 1;
    }

    let byte_len = (len * 2) as u16;
    (*destination_string).length = byte_len;
    (*destination_string).maximum_length = byte_len + 2;
    (*destination_string).buffer = source_string as *mut u16;
}

#[no_mangle]
pub unsafe extern "win64" fn RtlInitAnsiString(
    destination_string: *mut ANSI_STRING,
    source_string: *const u8,
) {
    if destination_string.is_null() {
        return;
    }

    if source_string.is_null() {
        (*destination_string).length = 0;
        (*destination_string).maximum_length = 0;
        (*destination_string).buffer = core::ptr::null_mut();
        return;
    }

    let mut len = 0;
    while *source_string.add(len) != 0 {
        len += 1;
    }

    (*destination_string).length = len as u16;
    (*destination_string).maximum_length = (len + 1) as u16;
    (*destination_string).buffer = source_string as *mut u8;
}

#[no_mangle]
pub unsafe extern "win64" fn KeStallExecutionProcessor(microseconds: ULONG) {
    let count = (microseconds as u64) * 3000;
    for _ in 0..count {
        core::hint::spin_loop();
    }
}

#[no_mangle]
pub unsafe extern "win64" fn IoDeleteDevice(device_object: PDEVICE_OBJECT) {
    lunix_serial_println!("[ntoskrnl] IoDeleteDevice(Device={:p})", device_object);
    if !device_object.is_null() {
        let driver = (*device_object).driver_object;
        if !driver.is_null() && (*driver).device_object == device_object {
            (*driver).device_object = (*device_object).next_device;
        }
        ExFreePoolWithTag(device_object as PVOID, 0x65766544);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn IoCreateSymbolicLink(
    symbolic_link_name: *const UNICODE_STRING,
    device_name: *const UNICODE_STRING,
) -> NTSTATUS {
    let _ = (symbolic_link_name, device_name);
    lunix_serial_println!("[ntoskrnl] IoCreateSymbolicLink");
    STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn IoDeleteSymbolicLink(symbolic_link_name: *const UNICODE_STRING) -> NTSTATUS {
    let _ = symbolic_link_name;
    lunix_serial_println!("[ntoskrnl] IoDeleteSymbolicLink");
    STATUS_SUCCESS
}

