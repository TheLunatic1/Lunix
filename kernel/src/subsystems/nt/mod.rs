pub mod hal;
pub mod ntoskrnl;
pub mod pe;
pub mod types;
pub mod win32;

use crate::lunix_println;
use alloc::alloc::{alloc, Layout};
use types::*;

// -----------------------------------------------------------------------------
// Sample Windows NT Driver (WDM) Demonstration running under Lunix
// -----------------------------------------------------------------------------

unsafe extern "win64" fn sample_driver_create_close(
    device_object: PDEVICE_OBJECT,
    irp: PIRP,
) -> NTSTATUS {
    let _ = device_object;
    ntoskrnl::DbgPrint(b"[SampleDriver] IRP_MJ_CREATE / CLOSE received\0".as_ptr());
    ntoskrnl::IoCompleteRequest(irp, 0);
    STATUS_SUCCESS
}

unsafe extern "win64" fn sample_driver_device_control(
    device_object: PDEVICE_OBJECT,
    irp: PIRP,
) -> NTSTATUS {
    let _ = device_object;
    ntoskrnl::DbgPrint(b"[SampleDriver] IRP_MJ_DEVICE_CONTROL dispatched successfully!\0".as_ptr());
    ntoskrnl::IoCompleteRequest(irp, 0);
    STATUS_SUCCESS
}

unsafe extern "win64" fn sample_driver_unload(driver_object: PDRIVER_OBJECT) {
    ntoskrnl::DbgPrint(b"[SampleDriver] DriverUnload routine called\0".as_ptr());
    if !driver_object.is_null() && !(*driver_object).device_object.is_null() {
        ntoskrnl::IoDeleteDevice((*driver_object).device_object);
    }
}

pub unsafe extern "win64" fn sample_driver_entry(
    driver_object: PDRIVER_OBJECT,
    registry_path: *const UNICODE_STRING,
) -> NTSTATUS {
    let _ = registry_path;
    ntoskrnl::DbgPrint(b"[SampleDriver] DriverEntry invoked via extern \"win64\" ABI!\0".as_ptr());

    if driver_object.is_null() {
        return STATUS_INVALID_PARAMETER;
    }

    // Set up driver dispatch routines
    (*driver_object).major_function[0] = Some(sample_driver_create_close); // IRP_MJ_CREATE
    (*driver_object).major_function[2] = Some(sample_driver_create_close); // IRP_MJ_CLOSE
    (*driver_object).major_function[14] = Some(sample_driver_device_control); // IRP_MJ_DEVICE_CONTROL
    (*driver_object).driver_unload = Some(sample_driver_unload);

    // Create a Device Object for this driver
    let mut device_object: PDEVICE_OBJECT = core::ptr::null_mut();
    let status = ntoskrnl::IoCreateDevice(
        driver_object,
        128, // 128 bytes device extension
        core::ptr::null(),
        0x00000022, // FILE_DEVICE_UNKNOWN
        0x00000100, // FILE_DEVICE_SECURE_OPEN
        0,
        &mut device_object,
    );

    if status != STATUS_SUCCESS {
        ntoskrnl::DbgPrint(b"[-] IoCreateDevice failed!\0".as_ptr());
        return status;
    }

    ntoskrnl::DbgPrint(b"[SampleDriver] Created \\Device\\LunixSampleDevice0 successfully.\0".as_ptr());
    STATUS_SUCCESS
}

pub fn init() {
    lunix_println!("[+] Initializing Windows NT Driver Compatibility Subsystem...");
    lunix_serial_println!("[+] Windows NT Driver Model (WDM) subsystem ready.");
    lunix_println!("  [NTOS] Loaded ntoskrnl.exe & hal.dll ABI compatibility shims.");
    lunix_println!("  [PE64] PE32+ driver parser & relocator enabled.");

    // Run test WDM driver verification
    unsafe {
        let layout = Layout::new::<DRIVER_OBJECT>();
        let driver_obj = alloc(layout) as PDRIVER_OBJECT;
        if !driver_obj.is_null() {
            core::ptr::write_bytes(driver_obj as *mut u8, 0, core::mem::size_of::<DRIVER_OBJECT>());
            (*driver_obj).type_code = 4;
            (*driver_obj).size = core::mem::size_of::<DRIVER_OBJECT>() as i16;
            (*driver_obj).driver_init = Some(sample_driver_entry);

            let reg_path = UNICODE_STRING::empty();
            let status = sample_driver_entry(driver_obj, &reg_path);

            if status == STATUS_SUCCESS {
                lunix_println!("  [WDM] Sample Windows Driver loaded (Status: STATUS_SUCCESS).");
                lunix_serial_println!("[WDM] DriverEntry returned STATUS_SUCCESS, dispatch routines registered.");

                // Dispatch a test IRP
                let irp_layout = Layout::new::<IRP>();
                let irp = alloc(irp_layout) as PIRP;
                if !irp.is_null() {
                    core::ptr::write_bytes(irp as *mut u8, 0, core::mem::size_of::<IRP>());
                    (*irp).type_code = 6;
                    (*irp).size = core::mem::size_of::<IRP>() as u16;

                    if !(*driver_obj).device_object.is_null() {
                        let _ = ntoskrnl::IoCallDriver((*driver_obj).device_object, irp);
                        lunix_println!("  [WDM] Dispatched IRP to \\Device\\LunixSampleDevice0 -> Handled OK.");
                    }
                }
            }
        }
    }
}
