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
pub unsafe extern "win64" fn ExAllocatePool(pool_type: POOL_TYPE, number_of_bytes: SIZE_T) -> PVOID {
    ExAllocatePoolWithTag(pool_type, number_of_bytes, 0x6C6F6F50) // "Pool"
}

#[no_mangle]
pub unsafe extern "win64" fn ExFreePool(p: PVOID) {
    ExFreePoolWithTag(p, 0x6C6F6F50);
}

#[no_mangle]
pub unsafe extern "win64" fn KeInitializeEvent(event: PKEVENT, event_type: EVENT_TYPE, initial_state: BOOLEAN) {
    if event.is_null() {
        return;
    }
    (*event).header.type_code = event_type as u8;
    (*event).header.size = (core::mem::size_of::<KEVENT>() / 4) as u8;
    (*event).header.signal_state = if initial_state != 0 { 1 } else { 0 };
    (*event).header.inserted = 0;
    (*event).header.absolute = 0;
    lunix_serial_println!("[ntoskrnl] KeInitializeEvent(Event={:p}, Type={:?}, Init={})", event, event_type, initial_state);
}

#[no_mangle]
pub unsafe extern "win64" fn KeSetEvent(event: PKEVENT, increment: i32, wait: BOOLEAN) -> i32 {
    let _ = (increment, wait);
    if event.is_null() {
        return 0;
    }
    let prev = (*event).header.signal_state;
    (*event).header.signal_state = 1;
    lunix_serial_println!("[ntoskrnl] KeSetEvent(Event={:p}) -> PrevState={}", event, prev);
    prev
}

#[no_mangle]
pub unsafe extern "win64" fn KeResetEvent(event: PKEVENT) -> i32 {
    if event.is_null() {
        return 0;
    }
    let prev = (*event).header.signal_state;
    (*event).header.signal_state = 0;
    lunix_serial_println!("[ntoskrnl] KeResetEvent(Event={:p}) -> PrevState={}", event, prev);
    prev
}

#[no_mangle]
pub unsafe extern "win64" fn KeClearEvent(event: PKEVENT) {
    if !event.is_null() {
        (*event).header.signal_state = 0;
    }
}

#[no_mangle]
pub unsafe extern "win64" fn KeWaitForSingleObject(
    object: PVOID,
    wait_reason: ULONG,
    wait_mode: i8,
    alertable: BOOLEAN,
    timeout: PLARGE_INTEGER,
) -> NTSTATUS {
    let _ = (wait_reason, wait_mode, alertable);
    if object.is_null() {
        return STATUS_INVALID_PARAMETER;
    }

    let header = object as *mut DISPATCHER_HEADER;
    lunix_serial_println!("[ntoskrnl] KeWaitForSingleObject(Obj={:p}, SigState={})", object, (*header).signal_state);

    if (*header).signal_state > 0 {
        // SynchronizationEvent auto-resets
        if (*header).type_code == (EVENT_TYPE::SynchronizationEvent as u8) {
            (*header).signal_state = 0;
        }
        return STATUS_SUCCESS;
    }

    // If timeout is immediate 0
    if !timeout.is_null() && *timeout == 0 {
        return 0x00000102; // STATUS_TIMEOUT
    }

    // Wait loop with yield
    for _ in 0..10_000 {
        if (*header).signal_state > 0 {
            if (*header).type_code == (EVENT_TYPE::SynchronizationEvent as u8) {
                (*header).signal_state = 0;
            }
            return STATUS_SUCCESS;
        }
        core::hint::spin_loop();
    }

    STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn KeInitializeMutex(mutex: PKMUTEX, level: ULONG) {
    let _ = level;
    if mutex.is_null() {
        return;
    }
    (*mutex).header.type_code = 2; // Mutant object
    (*mutex).header.size = (core::mem::size_of::<KMUTEX>() / 4) as u8;
    (*mutex).header.signal_state = 1; // 1 = unlocked
    (*mutex).owner_thread = core::ptr::null_mut();
    (*mutex).abandoned = 0;
    (*mutex).apc_disable = 1;
    lunix_serial_println!("[ntoskrnl] KeInitializeMutex(Mutex={:p})", mutex);
}

#[no_mangle]
pub unsafe extern "win64" fn KeReleaseMutex(mutex: PKMUTEX, wait: BOOLEAN) -> i32 {
    let _ = wait;
    if mutex.is_null() {
        return 0;
    }
    let prev = (*mutex).header.signal_state;
    (*mutex).header.signal_state = 1;
    (*mutex).owner_thread = core::ptr::null_mut();
    lunix_serial_println!("[ntoskrnl] KeReleaseMutex(Mutex={:p})", mutex);
    prev
}

#[no_mangle]
pub unsafe extern "win64" fn IoAttachDevice(
    source_device: PDEVICE_OBJECT,
    target_device_name: *const UNICODE_STRING,
    attached_device: *mut PDEVICE_OBJECT,
) -> NTSTATUS {
    let _ = target_device_name;
    if source_device.is_null() {
        return STATUS_INVALID_PARAMETER;
    }

    lunix_serial_println!("[ntoskrnl] IoAttachDevice(Source={:p})", source_device);
    if !attached_device.is_null() {
        *attached_device = source_device;
    }
    STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn IoAttachDeviceToDeviceStack(
    source_device: PDEVICE_OBJECT,
    target_device: PDEVICE_OBJECT,
) -> PDEVICE_OBJECT {
    if source_device.is_null() || target_device.is_null() {
        return core::ptr::null_mut();
    }

    (*target_device).attached_device = source_device;
    lunix_serial_println!("[ntoskrnl] IoAttachDeviceToDeviceStack(Source={:p} -> Target={:p})", source_device, target_device);
    target_device
}

#[no_mangle]
pub unsafe extern "win64" fn IoDetachDevice(target_device: PDEVICE_OBJECT) {
    if !target_device.is_null() {
        (*target_device).attached_device = core::ptr::null_mut();
        lunix_serial_println!("[ntoskrnl] IoDetachDevice(Target={:p})", target_device);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn IoAllocateIrp(stack_size: i8, charge_quota: BOOLEAN) -> PIRP {
    let _ = charge_quota;
    let size = core::mem::size_of::<IRP>() + (stack_size as usize * 72);
    let raw = ExAllocatePoolWithTag(POOL_TYPE::NonPagedPool, size, 0x70724949); // "Irp"
    if raw.is_null() {
        return core::ptr::null_mut();
    }
    core::ptr::write_bytes(raw as *mut u8, 0, size);
    let irp = raw as PIRP;
    (*irp).type_code = 6;
    (*irp).size = size as u16;
    (*irp).stack_count = stack_size;
    (*irp).current_location = stack_size + 1;
    lunix_serial_println!("[ntoskrnl] IoAllocateIrp(StackSize={}) -> {:p}", stack_size, irp);
    irp
}

#[no_mangle]
pub unsafe extern "win64" fn IoFreeIrp(irp: PIRP) {
    if !irp.is_null() {
        lunix_serial_println!("[ntoskrnl] IoFreeIrp(IRP={:p})", irp);
        ExFreePoolWithTag(irp as PVOID, 0x70724949);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn IoAllocateMdl(
    virtual_address: PVOID,
    length: ULONG,
    secondary_buffer: BOOLEAN,
    charge_quota: BOOLEAN,
    irp: PIRP,
) -> PMDL {
    let _ = (secondary_buffer, charge_quota);
    let size = core::mem::size_of::<MDL>() + (((length as usize + 4095) / 4096) * 8);
    let raw = ExAllocatePoolWithTag(POOL_TYPE::NonPagedPool, size, 0x6C644D49); // "Mdl"
    if raw.is_null() {
        return core::ptr::null_mut();
    }
    core::ptr::write_bytes(raw as *mut u8, 0, size);
    let mdl = raw as PMDL;
    (*mdl).size = size as i16;
    (*mdl).start_va = ((virtual_address as usize) & !0xFFF) as PVOID;
    (*mdl).byte_offset = ((virtual_address as usize) & 0xFFF) as ULONG;
    (*mdl).byte_count = length;
    (*mdl).mapped_system_va = virtual_address;

    if !irp.is_null() {
        (*irp).mdl_address = mdl;
    }

    lunix_serial_println!("[ntoskrnl] IoAllocateMdl(VA={:p}, Len={}) -> {:p}", virtual_address, length, mdl);
    mdl
}

#[no_mangle]
pub unsafe extern "win64" fn IoFreeMdl(mdl: PMDL) {
    if !mdl.is_null() {
        lunix_serial_println!("[ntoskrnl] IoFreeMdl(MDL={:p})", mdl);
        ExFreePoolWithTag(mdl as PVOID, 0x6C644D49);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn MmProbeAndLockPages(mdl: PMDL, access_mode: i8, operation: ULONG) {
    let _ = (access_mode, operation);
    if !mdl.is_null() {
        (*mdl).mdl_flags |= 0x0001 | 0x0004; // MDL_MAPPED_TO_SYSTEM_VA | MDL_PAGES_LOCKED
        lunix_serial_println!("[ntoskrnl] MmProbeAndLockPages(MDL={:p})", mdl);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn MmUnlockPages(mdl: PMDL) {
    if !mdl.is_null() {
        (*mdl).mdl_flags &= !(0x0004); // Clear MDL_PAGES_LOCKED
        lunix_serial_println!("[ntoskrnl] MmUnlockPages(MDL={:p})", mdl);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn IoDeleteDevice(device_object: PDEVICE_OBJECT) {
    if !device_object.is_null() {
        lunix_serial_println!("[ntoskrnl] IoDeleteDevice(DeviceObject={:p})", device_object);
        ExFreePool(device_object as PVOID);
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



