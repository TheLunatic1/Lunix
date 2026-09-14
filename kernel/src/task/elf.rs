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
pub const PT_DYNAMIC: u32 = 2;
pub const PT_INTERP: u32 = 3;
pub const PT_NOTE: u32 = 4;
pub const PT_PHDR: u32 = 6;
pub const PT_TLS: u32 = 7;

pub const USER_STACK_BASE: u64 = 0x0000_7FFF_2000_0000;
pub const USER_STACK_SIZE: usize = 64 * 1024; // 64 KiB
pub const INTERP_LOAD_BASE: u64 = 0x0000_7FFF_E000_0000;

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

    lunix_serial_println!("  [ELF64] Valid ELF found. Entry: 0x{:X}, PhNum: {}, Type: {}", header.entry, header.phnum, header.elf_type);

    let phdr_size = header.phentsize as usize;
    let mut interp_path_buf = [0u8; 128];
    let mut interp_len = 0;

    // Check for PT_INTERP program header
    for i in 0..header.phnum as usize {
        let phdr_offset = header.phoff as usize + (i * phdr_size);
        if phdr_offset + phdr_size <= elf_bytes.len() {
            let phdr = unsafe { &*(elf_bytes.as_ptr().add(phdr_offset) as *const Elf64Phdr) };
            if phdr.p_type == PT_INTERP {
                let off = phdr.p_offset as usize;
                let sz = (phdr.p_filesz as usize).min(127);
                if off + sz <= elf_bytes.len() {
                    interp_path_buf[..sz].copy_from_slice(&elf_bytes[off..off + sz]);
                    interp_len = sz;
                }
            }
        }
    }

    let mut main_base = 0u64;
    if header.elf_type == 3 && header.entry < 0x400000 {
        main_base = 0x400000;
    }

    // Iterate program headers and map PT_LOAD segments for main binary
    for i in 0..header.phnum as usize {
        let phdr_offset = header.phoff as usize + (i * phdr_size);
        if phdr_offset + phdr_size > elf_bytes.len() {
            return Err("Invalid program header offset");
        }

        let phdr = unsafe { &*(elf_bytes.as_ptr().add(phdr_offset) as *const Elf64Phdr) };

        if phdr.p_type == PT_LOAD {
            let vaddr = main_base + phdr.p_vaddr;
            let memsz = phdr.p_memsz as usize;
            let filesz = phdr.p_filesz as usize;

            lunix_serial_println!("  [ELF64] PT_LOAD: VAddr=0x{:X}, FileSz={}, MemSz={}", vaddr, filesz, memsz);

            let num_pages = (memsz + 4095) / 4096;
            for p in 0..num_pages {
                let page_vaddr = VirtAddr::new(vaddr + (p * 4096) as u64);
                let phys_frame = crate::mm::pmm::alloc_frame().ok_or("Out of memory for user page")?;

                let flags = PageTableFlags::PRESENT
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::USER_ACCESSIBLE;

                let _ = crate::mm::vmm::map_page(page_vaddr, phys_frame, flags);

                unsafe {
                    core::ptr::write_bytes(phys_frame.as_u64() as *mut u8, 0, 4096);
                }
            }

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

    // Handle Dynamic Linker (PT_INTERP) if present
    let mut interp_base = 0u64;
    let mut execution_entry = main_base + header.entry;

    if interp_len > 0 {
        let raw_path = core::str::from_utf8(&interp_path_buf[..interp_len])
            .unwrap_or("")
            .trim_matches('\0');
        
        lunix_serial_println!("  [ELF64] Binary requires dynamic interpreter: '{}'", raw_path);

        if let Ok(interp_bytes) = crate::fs::vfs::read_to_vec(raw_path) {
            if interp_bytes.len() >= core::mem::size_of::<Elf64Header>() {
                let interp_header = unsafe { &*(interp_bytes.as_ptr() as *const Elf64Header) };
                if &interp_header.ident[0..4] == b"\x7fELF" {
                    interp_base = INTERP_LOAD_BASE;
                    let interp_phdr_size = interp_header.phentsize as usize;

                    for j in 0..interp_header.phnum as usize {
                        let i_off = interp_header.phoff as usize + (j * interp_phdr_size);
                        if i_off + interp_phdr_size <= interp_bytes.len() {
                            let i_phdr = unsafe { &*(interp_bytes.as_ptr().add(i_off) as *const Elf64Phdr) };
                            if i_phdr.p_type == PT_LOAD {
                                let i_vaddr = interp_base + i_phdr.p_vaddr;
                                let i_memsz = i_phdr.p_memsz as usize;
                                let i_filesz = i_phdr.p_filesz as usize;

                                let num_pages = (i_memsz + 4095) / 4096;
                                for p in 0..num_pages {
                                    let page_vaddr = VirtAddr::new(i_vaddr + (p * 4096) as u64);
                                    if let Some(phys_frame) = crate::mm::pmm::alloc_frame() {
                                        let flags = PageTableFlags::PRESENT
                                            | PageTableFlags::WRITABLE
                                            | PageTableFlags::USER_ACCESSIBLE;
                                        let _ = crate::mm::vmm::map_page(page_vaddr, phys_frame, flags);
                                        unsafe {
                                            core::ptr::write_bytes(phys_frame.as_u64() as *mut u8, 0, 4096);
                                        }
                                    }
                                }

                                let src_start = i_phdr.p_offset as usize;
                                let src_end = src_start + i_filesz;
                                if src_end <= interp_bytes.len() {
                                    unsafe {
                                        core::ptr::copy_nonoverlapping(
                                            interp_bytes.as_ptr().add(src_start),
                                            i_vaddr as *mut u8,
                                            i_filesz,
                                        );
                                    }
                                }
                            }
                        }
                    }

                    execution_entry = interp_base + interp_header.entry;
                    lunix_serial_println!("  [ELF64] Dynamic interpreter mapped at 0x{:X}, entry: 0x{:X}", interp_base, execution_entry);
                }
            }
        } else {
            lunix_serial_println!("  [ELF64] Warning: dynamic interpreter '{}' not found on VFS, falling back to static entry", raw_path);
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

    let user_stack_top = USER_STACK_BASE + (USER_STACK_SIZE as u64) - 512;

    // Set up standard Linux System V AMD64 initial user stack frame:
    // [High Address]
    // String Table (argv strings, envp strings, 16 random bytes for AT_RANDOM)
    // ------------------------------------------------------------------------
    // auxv pairs: AT_RANDOM, AT_ENTRY, AT_PAGESZ, AT_PHDR, AT_PHENT, AT_PHNUM, AT_UID, AT_GID, AT_NULL
    // NULL (envp terminator)
    // envp[N]... envp[0]
    // NULL (argv terminator)
    // argv[argc-1]... argv[0]
    // argc
    // [user_stack_top -> [rsp] = argc]
    unsafe {
        let string_base = (stack_top_frame + 4096 - 256) as *mut u8;
        let stack_ptr = (stack_top_frame + 4096 - 512) as *mut u64;

        let mut str_offset = 0usize;
        let mut argv_addrs = [0u64; 16];

        for (i, &arg) in args.iter().enumerate() {
            if i < 16 && str_offset + arg.len() + 1 <= 128 {
                let dst = string_base.add(str_offset);
                core::ptr::copy_nonoverlapping(arg.as_ptr(), dst, arg.len());
                *dst.add(arg.len()) = 0;
                let virt_arg = (USER_STACK_BASE + (USER_STACK_SIZE as u64) - 256) + str_offset as u64;
                argv_addrs[i] = virt_arg;
                str_offset += arg.len() + 1;
            }
        }

        // Environment variables
        let default_envs: &[&[u8]] = &[
            b"PATH=/bin:/usr/bin:/sbin:/usr/sbin\0",
            b"HOME=/home/tc\0",
            b"USER=tc\0",
            b"LOGNAME=tc\0",
            b"TERM=linux\0",
            b"SHELL=/bin/sh\0",
            b"PWD=/\0",
        ];
        let mut env_addrs = [0u64; 8];
        let mut env_count = 0;

        for (i, &env_bytes) in default_envs.iter().enumerate() {
            if i < 8 && str_offset + env_bytes.len() <= 240 {
                let dst = string_base.add(str_offset);
                core::ptr::copy_nonoverlapping(env_bytes.as_ptr(), dst, env_bytes.len());
                let virt_env = (USER_STACK_BASE + (USER_STACK_SIZE as u64) - 256) + str_offset as u64;
                env_addrs[i] = virt_env;
                str_offset += env_bytes.len();
                env_count += 1;
            }
        }

        // 16 random bytes for AT_RANDOM
        let random_ptr_offset = str_offset;
        let random_dst = string_base.add(random_ptr_offset);
        for k in 0..16 {
            *random_dst.add(k) = (0x5A ^ (k as u8)).wrapping_add(0x13);
        }
        let virt_random_ptr = (USER_STACK_BASE + (USER_STACK_SIZE as u64) - 256) + random_ptr_offset as u64;

        let argc = args.len().min(16);
        let mut idx = 0;

        // 1. argc
        *stack_ptr.add(idx) = argc as u64;
        idx += 1;

        // 2. argv[0..argc] + NULL
        for i in 0..argc {
            *stack_ptr.add(idx) = argv_addrs[i];
            idx += 1;
        }
        *stack_ptr.add(idx) = 0; // NULL
        idx += 1;

        // 3. envp[0..env_count] + NULL
        for i in 0..env_count {
            *stack_ptr.add(idx) = env_addrs[i];
            idx += 1;
        }
        *stack_ptr.add(idx) = 0; // NULL
        idx += 1;

        // 4. Auxiliary Vectors (auxv)
        let phdr_vaddr = main_base + if header.phoff < 0x400000 && main_base == 0 {
            0x400000 + header.phoff
        } else {
            header.phoff
        };

        let auxv: &[(u64, u64)] = &[
            (3, phdr_vaddr),                 // AT_PHDR
            (4, header.phentsize as u64),    // AT_PHENT
            (5, header.phnum as u64),        // AT_PHNUM
            (6, 4096),                       // AT_PAGESZ
            (7, interp_base),                // AT_BASE (Dynamic interpreter base)
            (8, 0),                          // AT_FLAGS
            (9, main_base + header.entry),   // AT_ENTRY (Application main entry point)
            (11, 0),                         // AT_UID
            (12, 0),                         // AT_EUID
            (13, 0),                         // AT_GID
            (14, 0),                         // AT_EGID
            (17, 100),                       // AT_CLKTCK
            (25, virt_random_ptr),           // AT_RANDOM
            (31, argv_addrs[0]),             // AT_EXECFN
            (33, 0),                         // AT_SYSINFO_EHDR
            (0, 0),                          // AT_NULL
        ];

        for &(a_type, a_val) in auxv {
            *stack_ptr.add(idx) = a_type;
            *stack_ptr.add(idx + 1) = a_val;
            idx += 2;
        }
    }

    Ok(LoadedProgram {
        entry_point: execution_entry,
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



