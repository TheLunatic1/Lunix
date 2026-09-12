use x86_64::structures::paging::PageTableFlags;
use x86_64::VirtAddr;
use alloc::string::String;
use crate::lunix_println;
use spin::Mutex;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Header {
    pub ident: [u8; 16],
    pub elf_type: u16,
    pub machine: u16,
    pub version: u32,
    pub entry: u64,
    pub phoff: u64,
    pub shoff: u64,
    pub flags: u32,
    pub ehsize: u16,
    pub phentsize: u16,
    pub phnum: u16,
    pub shentsize: u16,
    pub shnum: u16,
    pub shstrndx: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Phdr {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

pub const PT_LOAD: u32 = 1;
pub const USER_STACK_BASE: u64 = 0x0000_7FFF_2000_0000;
pub const USER_STACK_SIZE: usize = 64 * 1024; // 64 KiB

#[derive(Debug, Clone, Copy)]
pub struct LoadedProgram {
    pub entry_point: u64,
    pub user_stack_top: u64,
}

static CURRENT_ELF_EXEC: Mutex<Option<LoadedProgram>> = Mutex::new(None);

pub fn load_elf(elf_bytes: &[u8]) -> Result<LoadedProgram, &'static str> {
    load_elf_with_args(elf_bytes, &["prog"])
}

pub fn load_elf_with_args(elf_bytes: &[u8], args: &[&str]) -> Result<LoadedProgram, &'static str> {
    if elf_bytes.len() < core::mem::size_of::<Elf64Header>() {
        return Err("Binary too small for ELF64 header");
    }

    let header = unsafe { &*(elf_bytes.as_ptr() as *const Elf64Header) };

    // Validate ELF magic: \x7fELF
    if &header.ident[0..4] != b"\x7fELF" {
        return Err("Invalid ELF magic");
    }

    // Must be 64-bit (2) and x86_64 (0x3E)
    if header.ident[4] != 2 || header.machine != 0x3E {
        return Err("Not a 64-bit x86_64 ELF binary");
    }

    lunix_serial_println!("  [ELF64] Valid ELF found. Entry: 0x{:X}, PhNum: {}", header.entry, header.phnum);

    // Iterate program headers and map PT_LOAD segments
    let phdr_size = header.phentsize as usize;
    for i in 0..header.phnum as usize {
        let phdr_offset = header.phoff as usize + (i * phdr_size);
        if phdr_offset + phdr_size > elf_bytes.len() {
            return Err("Invalid program header offset");
        }

        let phdr = unsafe { &*(elf_bytes.as_ptr().add(phdr_offset) as *const Elf64Phdr) };

        if phdr.p_type == PT_LOAD {
            let vaddr = phdr.p_vaddr;
            let memsz = phdr.p_memsz as usize;
            let filesz = phdr.p_filesz as usize;

            lunix_serial_println!("  [ELF64] PT_LOAD: VAddr=0x{:X}, FileSz={}, MemSz={}", vaddr, filesz, memsz);

            let num_pages = (memsz + 4095) / 4096;
            for p in 0..num_pages {
                let page_vaddr = VirtAddr::new(vaddr + (p * 4096) as u64);
                let phys_frame = crate::mm::pmm::alloc_frame().ok_or("Out of memory for user page")?;

                // Map with USER_ACCESSIBLE | PRESENT | WRITABLE
                let flags = PageTableFlags::PRESENT
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::USER_ACCESSIBLE;

                let _ = crate::mm::vmm::map_page(page_vaddr, phys_frame, flags);

                // Zero-fill the physical frame
                unsafe {
                    core::ptr::write_bytes(phys_frame.as_u64() as *mut u8, 0, 4096);
                }
            }

            // Copy file segment content
            let src_start = phdr.p_offset as usize;
            let src_end = src_start + filesz;
            if src_end <= elf_bytes.len() {
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        elf_bytes.as_ptr().add(src_start),
                        vaddr as *mut u8,
                        filesz,
                    );
                }
            }
        }
    }

    // Allocate and map User Stack (64 KiB)
    let num_stack_pages = USER_STACK_SIZE / 4096;
    let mut stack_top_frame = 0u64;
    for p in 0..num_stack_pages {
        let page_vaddr = VirtAddr::new(USER_STACK_BASE + (p * 4096) as u64);
        let phys_frame = crate::mm::pmm::alloc_frame().ok_or("Out of memory for user stack")?;

        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::USER_ACCESSIBLE;

        let _ = crate::mm::vmm::map_page(page_vaddr, phys_frame, flags);
        unsafe {
            core::ptr::write_bytes(phys_frame.as_u64() as *mut u8, 0, 4096);
        }
        if p == num_stack_pages - 1 {
            stack_top_frame = phys_frame.as_u64();
        }
    }

    let user_stack_top = USER_STACK_BASE + (USER_STACK_SIZE as u64) - 256;

    // Set up standard Linux System V AMD64 initial user stack frame with argc/argv:
    unsafe {
        let string_base = (stack_top_frame + 4096 - 128) as *mut u8;
        let stack_ptr = (stack_top_frame + 4096 - 256) as *mut u64;

        let mut str_offset = 0usize;
        let mut argv_addrs = [0u64; 16];

        for (i, &arg) in args.iter().enumerate() {
            if i < 16 && str_offset + arg.len() + 1 <= 128 {
                let dst = string_base.add(str_offset);
                core::ptr::copy_nonoverlapping(arg.as_ptr(), dst, arg.len());
                *dst.add(arg.len()) = 0;
                let virt_arg = (USER_STACK_BASE + (USER_STACK_SIZE as u64) - 128) + str_offset as u64;
                argv_addrs[i] = virt_arg;
                str_offset += arg.len() + 1;
            }
        }

        let argc = args.len().min(16);
        *stack_ptr.add(0) = argc as u64; // argc
        for i in 0..argc {
            *stack_ptr.add(1 + i) = argv_addrs[i]; // argv[i]
        }
        *stack_ptr.add(1 + argc) = 0; // argv[argc] = NULL
        *stack_ptr.add(2 + argc) = 0; // envp = NULL

        let aux_base = 3 + argc;
        *stack_ptr.add(aux_base) = 6; // AT_PAGESZ
        *stack_ptr.add(aux_base + 1) = 4096;
        *stack_ptr.add(aux_base + 2) = 9; // AT_ENTRY
        *stack_ptr.add(aux_base + 3) = header.entry;
        *stack_ptr.add(aux_base + 4) = 0; // AT_NULL
    }

    Ok(LoadedProgram {
        entry_point: header.entry,
        user_stack_top,
    })
}

fn elf_runner_trampoline() {
    let prog = {
        let lock = CURRENT_ELF_EXEC.lock();
        lock.expect("No ELF program loaded for execution")
    };

    lunix_println!("  [USER] Transitioning CPU to Ring 3 at entry point 0x{:X}...", prog.entry_point);
    unsafe {
        crate::task::user::enter_user_mode(prog.entry_point, prog.user_stack_top);
    }
}

pub fn exec_elf(path: &str) -> Result<usize, String> {
    exec_elf_with_args(path, &[path])
}

pub fn exec_elf_with_args(path: &str, args: &[&str]) -> Result<usize, String> {
    lunix_println!("[ELF] Loading binary '{}' from VFS...", path);
    let bytes = crate::fs::vfs::read_to_vec(path)
        .map_err(|e| alloc::format!("Failed to read '{}': {:?}", path, e))?;

    let prog = load_elf_with_args(&bytes, args)
        .map_err(|e| alloc::format!("ELF loader error: {}", e))?;

    *CURRENT_ELF_EXEC.lock() = Some(prog);

    let pid = crate::task::scheduler::allocate_pid();
    let (cr3_frame, _) = x86_64::registers::control::Cr3::read();
    let ppid = crate::task::scheduler::current_pid();
    let proc = crate::task::process::Process::new_user(pid, ppid, path, cr3_frame.start_address().as_u64());
    crate::task::scheduler::register_process(proc);

    if ppid != pid {
        if let Some(parent_proc) = crate::task::scheduler::get_process(ppid) {
            parent_proc.lock().children.push(pid);
        }
    }

    lunix_println!("[+] Spawned Ring 3 ELF process (PID: {}, PPID: {})", pid, ppid);
    let tid = crate::task::scheduler::spawn_with_pid("elf_process", pid, elf_runner_trampoline, 8);
    Ok(tid)
}



