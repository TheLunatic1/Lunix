use crate::arch::x86_64::io::{inb, inl, inw, outb, outl, outw};
use crate::drivers::pci;
use crate::lunix_serial_println;
use crate::subsystems::nt::types::ULONG;
use core::ffi::c_void;

#[no_mangle]
pub unsafe extern "win64" fn READ_PORT_UCHAR(port: *const u8) -> u8 {
    let port_num = port as usize as u16;
    inb(port_num)
}

#[no_mangle]
pub unsafe extern "win64" fn WRITE_PORT_UCHAR(port: *mut u8, value: u8) {
    let port_num = port as usize as u16;
    outb(port_num, value);
}

#[no_mangle]
pub unsafe extern "win64" fn READ_PORT_USHORT(port: *const u16) -> u16 {
    let port_num = port as usize as u16;
    inw(port_num)
}

#[no_mangle]
pub unsafe extern "win64" fn WRITE_PORT_USHORT(port: *mut u16, value: u16) {
    let port_num = port as usize as u16;
    outw(port_num, value);
}

#[no_mangle]
pub unsafe extern "win64" fn READ_PORT_ULONG(port: *const u32) -> u32 {
    let port_num = port as usize as u16;
    inl(port_num)
}

#[no_mangle]
pub unsafe extern "win64" fn WRITE_PORT_ULONG(port: *mut u32, value: u32) {
    let port_num = port as usize as u16;
    outl(port_num, value);
}

#[no_mangle]
pub unsafe extern "win64" fn HalGetBusDataByOffset(
    bus_data_type: ULONG,
    bus_number: ULONG,
    slot_number: ULONG,
    buffer: *mut c_void,
    offset: ULONG,
    length: ULONG,
) -> ULONG {
    lunix_serial_println!(
        "[HAL] HalGetBusDataByOffset(Type={}, Bus={}, Slot={}, Offset={}, Len={})",
        bus_data_type,
        bus_number,
        slot_number,
        offset,
        length
    );

    if buffer.is_null() || length == 0 {
        return 0;
    }

    let dev = (slot_number & 0x1F) as u8;
    let func = ((slot_number >> 5) & 0x07) as u8;
    let bus = bus_number as u8;

    let buf_slice = core::slice::from_raw_parts_mut(buffer as *mut u8, length as usize);

    for (i, byte) in buf_slice.iter_mut().enumerate() {
        let cur_offset = (offset as u8) + (i as u8);
        let reg_offset = cur_offset & !3;
        let dword = pci::pci_read_u32(bus, dev, func, reg_offset);
        let shift = (cur_offset & 3) * 8;
        *byte = ((dword >> shift) & 0xFF) as u8;
    }

    length
}
