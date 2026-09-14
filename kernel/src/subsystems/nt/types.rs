#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use core::ffi::c_void;

pub type NTSTATUS = i32;
pub type KIRQL = u8;
pub type KSPIN_LOCK = usize;
pub type PVOID = *mut c_void;
pub type ULONG = u32;
pub type ULONG_PTR = usize;
pub type SIZE_T = usize;
pub type BOOLEAN = u8;

pub const STATUS_SUCCESS: NTSTATUS = 0x00000000;
pub const STATUS_UNSUCCESSFUL: NTSTATUS = -1073741823; // 0xC0000001
pub const STATUS_NOT_IMPLEMENTED: NTSTATUS = -1073741822; // 0xC0000002
pub const STATUS_INVALID_PARAMETER: NTSTATUS = -1073741811; // 0xC000000D
pub const STATUS_INSUFFICIENT_RESOURCES: NTSTATUS = -1073741670; // 0xC000009A
pub const STATUS_DEVICE_NOT_CONNECTED: NTSTATUS = -1073741571; // 0xC000009E

pub const PASSIVE_LEVEL: KIRQL = 0;
pub const APC_LEVEL: KIRQL = 1;
pub const DISPATCH_LEVEL: KIRQL = 2;
pub const HIGH_LEVEL: KIRQL = 15;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UNICODE_STRING {
    pub length: u16,
    pub maximum_length: u16,
    pub buffer: *mut u16,
}

impl UNICODE_STRING {
    pub const fn empty() -> Self {
        Self {
            length: 0,
            maximum_length: 0,
            buffer: core::ptr::null_mut(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ANSI_STRING {
    pub length: u16,
    pub maximum_length: u16,
    pub buffer: *mut u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum POOL_TYPE {
    NonPagedPool = 0,
    PagedPool = 1,
    NonPagedPoolMustSucceed = 2,
    DontUseThisType = 3,
    NonPagedPoolCacheAligned = 4,
    PagedPoolCacheAligned = 5,
    NonPagedPoolCacheAlignedMustS = 6,
    NonPagedPoolNx = 512,
}

pub type PDRIVER_OBJECT = *mut DRIVER_OBJECT;
pub type PDEVICE_OBJECT = *mut DEVICE_OBJECT;
pub type PIRP = *mut IRP;

pub type PDRIVER_INITIALIZE = unsafe extern "win64" fn(
    driver_object: PDRIVER_OBJECT,
    registry_path: *const UNICODE_STRING,
) -> NTSTATUS;

pub type PDRIVER_UNLOAD = unsafe extern "win64" fn(driver_object: PDRIVER_OBJECT);
pub type PDRIVER_DISPATCH = unsafe extern "win64" fn(
    device_object: PDEVICE_OBJECT,
    irp: PIRP,
) -> NTSTATUS;

pub const IRP_MJ_CREATE: usize = 0;
pub const IRP_MJ_CREATE_NAMED_PIPE: usize = 1;
pub const IRP_MJ_CLOSE: usize = 2;
pub const IRP_MJ_READ: usize = 3;
pub const IRP_MJ_WRITE: usize = 4;
pub const IRP_MJ_QUERY_INFORMATION: usize = 5;
pub const IRP_MJ_SET_INFORMATION: usize = 6;
pub const IRP_MJ_QUERY_EA: usize = 7;
pub const IRP_MJ_SET_EA: usize = 8;
pub const IRP_MJ_FLUSH_BUFFERS: usize = 9;
pub const IRP_MJ_QUERY_VOLUME_INFORMATION: usize = 10;
pub const IRP_MJ_SET_VOLUME_INFORMATION: usize = 11;
pub const IRP_MJ_DIRECTORY_CONTROL: usize = 12;
pub const IRP_MJ_FILE_SYSTEM_CONTROL: usize = 13;
pub const IRP_MJ_DEVICE_CONTROL: usize = 14;
pub const IRP_MJ_INTERNAL_DEVICE_CONTROL: usize = 15;
pub const IRP_MJ_SHUTDOWN: usize = 16;
pub const IRP_MJ_LOCK_CONTROL: usize = 17;
pub const IRP_MJ_CLEANUP: usize = 18;
pub const IRP_MJ_CREATE_MAILSLOT: usize = 19;
pub const IRP_MJ_QUERY_SECURITY: usize = 20;
pub const IRP_MJ_SET_SECURITY: usize = 21;
pub const IRP_MJ_POWER: usize = 22;
pub const IRP_MJ_SYSTEM_CONTROL: usize = 23;
pub const IRP_MJ_DEVICE_CHANGE: usize = 24;
pub const IRP_MJ_QUERY_QUOTA: usize = 25;
pub const IRP_MJ_SET_QUOTA: usize = 26;
pub const IRP_MJ_PNP: usize = 27;
pub const IRP_MJ_MAXIMUM_FUNCTION: usize = 28;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EVENT_TYPE {
    NotificationEvent = 0,
    SynchronizationEvent = 1,
}

#[repr(C)]
pub struct DISPATCHER_HEADER {
    pub type_code: u8,
    pub absolute: u8,
    pub size: u8,
    pub inserted: u8,
    pub signal_state: i32,
    pub wait_list_head: [usize; 2],
}

#[repr(C)]
pub struct KEVENT {
    pub header: DISPATCHER_HEADER,
}

pub type PKEVENT = *mut KEVENT;

#[repr(C)]
pub struct KMUTEX {
    pub header: DISPATCHER_HEADER,
    pub mutant_list_entry: [usize; 2],
    pub owner_thread: PVOID,
    pub abandoned: BOOLEAN,
    pub apc_disable: u8,
}

pub type PKMUTEX = *mut KMUTEX;

pub type LARGE_INTEGER = i64;
pub type PLARGE_INTEGER = *mut LARGE_INTEGER;
pub type PHYSICAL_ADDRESS = u64;

#[repr(C)]
pub struct MDL {
    pub next: *mut MDL,
    pub size: i16,
    pub mdl_flags: i16,
    pub process: PVOID,
    pub mapped_system_va: PVOID,
    pub start_va: PVOID,
    pub byte_count: ULONG,
    pub byte_offset: ULONG,
}

pub type PMDL = *mut MDL;

#[repr(C)]
pub struct IO_STATUS_BLOCK {
    pub status: NTSTATUS,
    pub information: ULONG_PTR,
}

pub type PIO_STATUS_BLOCK = *mut IO_STATUS_BLOCK;

#[repr(C)]
pub struct DRIVER_OBJECT {
    pub type_code: i16,
    pub size: i16,
    pub device_object: PDEVICE_OBJECT,
    pub flags: ULONG,
    pub driver_start: PVOID,
    pub driver_size: ULONG,
    pub driver_section: PVOID,
    pub driver_extension: PVOID,
    pub driver_name: UNICODE_STRING,
    pub hardware_database: *const UNICODE_STRING,
    pub fast_io_dispatch: PVOID,
    pub driver_init: Option<PDRIVER_INITIALIZE>,
    pub driver_start_io: PVOID,
    pub driver_unload: Option<PDRIVER_UNLOAD>,
    pub major_function: [Option<PDRIVER_DISPATCH>; IRP_MJ_MAXIMUM_FUNCTION],
}

#[repr(C)]
pub struct DEVICE_OBJECT {
    pub type_code: i16,
    pub size: u16,
    pub reference_count: i32,
    pub driver_object: PDRIVER_OBJECT,
    pub next_device: PDEVICE_OBJECT,
    pub attached_device: PDEVICE_OBJECT,
    pub current_irp: PIRP,
    pub timer: PVOID,
    pub flags: ULONG,
    pub characteristics: ULONG,
    pub vpb: PVOID,
    pub device_extension: PVOID,
    pub device_type: ULONG,
    pub stack_size: i8,
    pub queue: [u8; 40],
    pub alignment_requirement: ULONG,
    pub device_queue: [u8; 32],
    pub dpc: [u8; 64],
    pub active_thread_count: ULONG,
    pub security_descriptor: PVOID,
    pub device_lock: [u8; 24],
    pub sector_size: u16,
    pub spare1: u16,
    pub device_object_extension: PVOID,
    pub reserved: PVOID,
}

#[repr(C)]
pub struct IRP {
    pub type_code: i16,
    pub size: u16,
    pub mdl_address: PMDL,
    pub flags: ULONG,
    pub associated_irp: PVOID,
    pub thread_list_entry: [usize; 2],
    pub io_status: IO_STATUS_BLOCK,
    pub requestor_mode: i8,
    pub pending_returned: BOOLEAN,
    pub stack_count: i8,
    pub current_location: i8,
    pub cancel: BOOLEAN,
    pub cancel_irql: KIRQL,
    pub user_buffer: PVOID,
    pub tail: [usize; 8],
}

