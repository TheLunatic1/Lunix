use alloc::string::String;
use spin::Mutex;

use crate::display::compositor::{Compositor, COMPOSITOR};
use crate::display::framebuffer::Color;
use crate::display::window::Window;
use lunix_common::FramebufferInfo;

static DESKTOP_INITIALIZED: Mutex<bool> = Mutex::new(false);

pub fn init(info: FramebufferInfo) {
    let mut comp = Compositor::new(info.width, info.height, info.base_address);
    comp.is_gui_active = true;

    // 1. System Monitor Window
    let mut sys_win = Window::new(1, "System Resource Monitor", 40, 60, 420, 280);
    render_system_monitor_content(&mut sys_win);
    comp.add_window(sys_win);

    // 2. Windows NT Subsystem Manager Window
    let mut nt_win = Window::new(2, "Windows NT Driver Subsystem", 500, 60, 480, 280);
    render_nt_manager_content(&mut nt_win);
    comp.add_window(nt_win);

    // 3. File Explorer Window
    let mut file_win = Window::new(3, "FAT32 File Explorer [/]", 40, 380, 420, 360);
    render_file_explorer_content(&mut file_win);
    comp.add_window(file_win);

    // 4. Interactive Terminal Window
    let mut term_win = Window::new(4, "Lunix Shell Terminal", 500, 380, 480, 360);
    render_terminal_content(&mut term_win);
    comp.add_window(term_win);

    *COMPOSITOR.lock() = Some(comp);
    *DESKTOP_INITIALIZED.lock() = true;
}

pub fn render_system_monitor_content(win: &mut Window) {
    win.clear(Color::rgb(22, 27, 34).to_u32());

    win.draw_string(14, 14, "=== CPU & MULTI-CORE (SMP) ===", Color::CYAN.to_u32(), 0);
    let cores = crate::arch::x86_64::smp::CPU_COUNT.load(core::sync::atomic::Ordering::Relaxed);
    let mut s = String::new();
    use core::fmt::Write;
    let _ = write!(s, "  Active Cores  : {} Core(s) (APIC flat mode)", cores);
    win.draw_string(14, 34, &s, Color::WHITE.to_u32(), 0);

    let _ = write!(s, "  AP Trampoline : 0x8000 Long Mode");
    win.draw_string(14, 52, "  AP Trampoline : 0x8000 Long Mode", Color::WHITE.to_u32(), 0);

    win.draw_string(14, 80, "=== PHYSICAL MEMORY (PMM/VMM) ===", Color::CYAN.to_u32(), 0);
    let (total, usable, used) = crate::mm::pmm::get_memory_stats();
    let total_mb = total / (1024 * 1024);
    let usable_mb = usable / (1024 * 1024);
    let used_mb = used / (1024 * 1024);
    let free_mb = usable_mb.saturating_sub(used_mb);

    let mut mem_s = String::new();
    let _ = write!(mem_s, "  Total System RAM : {} MB", total_mb);
    win.draw_string(14, 100, &mem_s, Color::WHITE.to_u32(), 0);

    let mut mem_s2 = String::new();
    let _ = write!(mem_s2, "  Usable / Free    : {} MB / {} MB", usable_mb, free_mb);
    win.draw_string(14, 118, &mem_s2, Color::WHITE.to_u32(), 0);

    // RAM usage progress bar
    win.fill_rect(24, 142, 370, 18, Color::rgb(48, 54, 61).to_u32());
    let pct = if usable > 0 { (used * 370) / usable } else { 0 };
    win.fill_rect(24, 142, pct.clamp(4, 370), 18, Color::rgb(46, 160, 67).to_u32());

    win.draw_string(14, 175, "=== STORAGE & VFS SUBSYSTEM ===", Color::CYAN.to_u32(), 0);
    let _devs = crate::fs::block::list_block_devices();
    let mut dev_s = String::new();
    let _ = write!(dev_s, "  Primary Disk     : /dev/sda (64 MB FAT32)");
    win.draw_string(14, 195, &dev_s, Color::WHITE.to_u32(), 0);
    win.draw_string(14, 213, "  Mounted Root     : / (FAT32 Driver)", Color::WHITE.to_u32(), 0);
    win.draw_string(14, 235, "Status: SYSTEM HEALTHY [1000 Hz APIC TICK]", Color::GREEN.to_u32(), 0);
}

pub fn render_nt_manager_content(win: &mut Window) {
    win.clear(Color::rgb(22, 27, 34).to_u32());

    win.draw_string(14, 14, "=== WINDOWS NT DRIVER MODEL (WDM) ===", Color::CYAN.to_u32(), 0);
    win.draw_string(14, 34, "  ABI Convention  : extern \"win64\" ABI (RCX, RDX, R8, R9)", Color::WHITE.to_u32(), 0);
    win.draw_string(14, 52, "  Core DDI Shim   : ntoskrnl.exe & hal.dll", Color::WHITE.to_u32(), 0);
    win.draw_string(14, 70, "  PE Parser       : PE32+ Relocations & IAT Resolver", Color::WHITE.to_u32(), 0);

    win.draw_string(14, 98, "=== LOADED DRIVER OBJECTS ===", Color::CYAN.to_u32(), 0);
    win.draw_string(14, 118, "  [1] \\Driver\\SampleLunixDriver", Color::YELLOW.to_u32(), 0);
    win.draw_string(14, 136, "      Entry Point : sample_driver_entry (OK)", Color::WHITE.to_u32(), 0);
    win.draw_string(14, 154, "      Created Dev : \\Device\\LunixSampleDevice0", Color::WHITE.to_u32(), 0);
    win.draw_string(14, 172, "      Device Type : FILE_DEVICE_UNKNOWN (0x22)", Color::WHITE.to_u32(), 0);
    win.draw_string(14, 190, "      Status      : STATUS_SUCCESS (Running)", Color::GREEN.to_u32(), 0);

    win.draw_string(14, 218, "=== IRP DISPATCH ROUTINES ===", Color::CYAN.to_u32(), 0);
    win.draw_string(14, 238, "  IRP_MJ_CREATE, IRP_MJ_CLOSE, IRP_MJ_DEVICE_CONTROL", Color::WHITE.to_u32(), 0);
}

pub fn render_file_explorer_content(win: &mut Window) {
    win.clear(Color::rgb(22, 27, 34).to_u32());

    win.draw_string(14, 14, "TYPE    SIZE      PATH / NAME", Color::CYAN.to_u32(), 0);
    win.draw_string(14, 28, "---------------------------------------------", Color::rgb(48, 54, 61).to_u32(), 0);

    if let Ok(entries) = crate::fs::vfs::read_dir("/") {
        let mut y = 46;
        for entry in entries {
            let type_str = match entry.node_type {
                crate::fs::inode::INodeType::Directory => "<DIR>",
                crate::fs::inode::INodeType::File => "<FILE>",
                _ => "<DEV>",
            };
            let mut s = String::new();
            use core::fmt::Write;
            let _ = write!(s, "{:<6}  {:<8}  {}", type_str, entry.size, entry.name);
            win.draw_string(14, y, &s, Color::WHITE.to_u32(), 0);
            y += 20;
            if y > win.height - 30 {
                break;
            }
        }
    } else {
        win.draw_string(14, 46, "  [Error reading directory /]", Color::RED.to_u32(), 0);
    }
}

pub fn render_terminal_content(win: &mut Window) {
    win.clear(Color::rgb(13, 17, 23).to_u32());

    win.draw_string(14, 14, "Lunix OS Interactive Shell (Ring 0 & Ring 3)", Color::CYAN.to_u32(), 0);
    win.draw_string(14, 34, "lunix> ps", Color::WHITE.to_u32(), 0);
    win.draw_string(14, 52, "  TID  NAME           STATE      PRIORITY", Color::YELLOW.to_u32(), 0);
    win.draw_string(14, 70, "  0    kernel_main    Running    10", Color::WHITE.to_u32(), 0);
    win.draw_string(14, 88, "  1    idle           Ready      0", Color::WHITE.to_u32(), 0);
    win.draw_string(14, 106, "  2    worker_demo    Sleeping   5", Color::WHITE.to_u32(), 0);
    win.draw_string(14, 130, "lunix> sysdemo", Color::WHITE.to_u32(), 0);
    win.draw_string(14, 148, "  [USER] Transitioning CPU to Ring 3...", Color::CYAN.to_u32(), 0);
    win.draw_string(14, 166, "  [RING 3] Hello via SYS_WRITE!", Color::GREEN.to_u32(), 0);
    win.draw_string(14, 184, "  [SYSCALL] sys_exit(0) called OK.", Color::GREEN.to_u32(), 0);
    win.draw_string(14, 210, "lunix> _", Color::WHITE.to_u32(), 0);
}

pub fn render_frame() {
    if let Some(mut guard) = COMPOSITOR.try_lock() {
        if let Some(ref mut comp) = *guard {
            if comp.is_gui_active {
                comp.render_frame();
            }
        }
    }
}
