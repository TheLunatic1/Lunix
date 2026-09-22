use x86_64::structures::paging::PageTableFlags;
use x86_64::VirtAddr;
use alloc::string::String;

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

// The stack is mapped eagerly (no demand growth yet), so it must be big enough for real
// programs: 2 MiB, ending at the same top address as before.
pub const USER_STACK_SIZE: usize = 2 * 1024 * 1024;
pub const USER_STACK_TOP: u64 = 0x0000_7FFF_2001_0000;
pub const USER_STACK_BASE: u64 = USER_STACK_TOP - USER_STACK_SIZE as u64;
pub const INTERP_LOAD_BASE: u64 = 0x0000_7FFF_E000_0000;

#[derive(Debug, Clone, Copy)]
pub struct LoadedProgram {
    pub entry_point: u64,
    pub user_stack_top: u64,
}

static CURRENT_ELF_EXEC: Mutex<Option<LoadedProgram>> = Mutex::new(None);

pub fn load_elf(elf_bytes: &[u8]) -> Result<LoadedProgram, &'static str> {
    let (prog, _pml4) = load_elf_with_args(elf_bytes, &["prog"], None, 1)?;
    Ok(prog)
}

static ELF_EXEC_MAP: Mutex<alloc::collections::BTreeMap<usize, LoadedProgram>> = Mutex::new(alloc::collections::BTreeMap::new());

fn map_and_load_segment(
    pml4_phys: x86_64::PhysAddr,
    vaddr: u64,
    memsz: usize,
    filesz: usize,
    file_bytes: &[u8],
) -> Result<(), &'static str> {
    let page_start = vaddr & !0xFFFu64;
    let page_end = (vaddr + memsz as u64 + 4095) & !0xFFFu64;
    let mut current_page = page_start;

    while current_page < page_end {
        let phys_frame = if let Some(existing) = crate::mm::vmm::get_page_phys_in_pml4(pml4_phys, VirtAddr::new(current_page)) {
            existing
        } else {
            let frame = crate::mm::pmm::alloc_frame().ok_or("Out of memory for user page")?;
            unsafe {
                core::ptr::write_bytes(frame.as_u64() as *mut u8, 0, 4096);
            }
            let flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER_ACCESSIBLE;
            crate::mm::vmm::map_page_in_pml4(pml4_phys, VirtAddr::new(current_page), frame, flags)?;
            frame
        };

        let page_vstart = current_page;
        let page_vend = current_page + 4096;
        let seg_vstart = vaddr;
        let seg_vend = vaddr + filesz as u64;

        if seg_vend > page_vstart && seg_vstart < page_vend && filesz > 0 {
            let copy_vstart = seg_vstart.max(page_vstart);
            let copy_vend = seg_vend.min(page_vend);
            let copy_len = (copy_vend - copy_vstart) as usize;

            let file_offset = (copy_vstart - seg_vstart) as usize;
            let page_offset = (copy_vstart - page_vstart) as usize;

            if file_offset + copy_len <= file_bytes.len() {
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        file_bytes.as_ptr().add(file_offset),
                        (phys_frame.as_u64() + page_offset as u64) as *mut u8,
                        copy_len,
                    );
                }
            }
        }

        current_page += 4096;
    }
    Ok(())
}

pub fn load_elf_with_args(
    elf_bytes: &[u8],
    args: &[&str],
    envp: Option<&[&str]>,
    pid: usize,
) -> Result<(LoadedProgram, x86_64::structures::paging::PhysFrame<x86_64::structures::paging::Size4KiB>), &'static str> {
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

    lunix_strace!("  [ELF64] Valid ELF found (PID {}). Entry: 0x{:X}, PhNum: {}, Type: {}", pid, header.entry, header.phnum, header.elf_type);

    let proc_pml4_frame = crate::mm::vmm::create_process_pml4()?;
    let pml4_phys = proc_pml4_frame.start_address();

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

            lunix_strace!("  [ELF64] PT_LOAD: VAddr=0x{:X}, FileSz={}, MemSz={}", vaddr, filesz, memsz);

            let src_start = phdr.p_offset as usize;
            let src_end = src_start + filesz;
            let seg_bytes = if src_end <= elf_bytes.len() {
                &elf_bytes[src_start..src_end]
            } else {
                &[]
            };

            map_and_load_segment(pml4_phys, vaddr, memsz, filesz, seg_bytes)?;
        }
    }

    // Handle Dynamic Linker (PT_INTERP) if present
    let mut interp_base = 0u64;
    let mut execution_entry = main_base + header.entry;

    if interp_len > 0 {
        let raw_path = core::str::from_utf8(&interp_path_buf[..interp_len])
            .unwrap_or("")
            .trim_matches('\0');
        
        lunix_strace!("  [ELF64] Binary requires dynamic interpreter: '{}'", raw_path);

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

                                let src_start = i_phdr.p_offset as usize;
                                let src_end = src_start + i_filesz;
                                let seg_bytes = if src_end <= interp_bytes.len() {
                                    &interp_bytes[src_start..src_end]
                                } else {
                                    &[]
                                };

                                map_and_load_segment(pml4_phys, i_vaddr, i_memsz, i_filesz, seg_bytes)?;
                            }
                        }
                    }

                    execution_entry = interp_base + interp_header.entry;
                    lunix_strace!("  [ELF64] Dynamic interpreter mapped at 0x{:X}, entry: 0x{:X}", interp_base, execution_entry);
                }
            }
        } else {
            lunix_strace!("  [ELF64] Warning: dynamic interpreter '{}' not found on VFS, falling back to static entry", raw_path);
        }
    }

    // Allocate and map isolated User Stack (64 KiB)
    let user_stack_base = USER_STACK_BASE;
    let num_stack_pages = USER_STACK_SIZE / 4096;
    // Only the top of the stack (where argv/envp/auxv live and the program starts) is mapped
    // now; the rest of the 2 MiB is demand-zero memory that costs nothing until touched.
    const EAGER_STACK_PAGES: usize = 16;
    let lazy_pages = num_stack_pages - EAGER_STACK_PAGES;
    crate::mm::vma::map_in(pml4_phys.as_u64(), user_stack_base, (lazy_pages * 4096) as u64, None);
    for p in lazy_pages..num_stack_pages {
        let page_vaddr = VirtAddr::new(user_stack_base + (p * 4096) as u64);
        let phys_frame = crate::mm::pmm::alloc_frame().ok_or("Out of memory for user stack")?;
        unsafe {
            core::ptr::write_bytes(phys_frame.as_u64() as *mut u8, 0, 4096);
        }

        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::USER_ACCESSIBLE;

        crate::mm::vmm::map_page_in_pml4(pml4_phys, page_vaddr, phys_frame, flags)?;
    }

    // ---- Initial process stack (System V AMD64 ABI) ----
    // Built as one image in kernel memory, then copied to the top of the user stack:
    //   sp -> argc | argv[0..argc] NULL | envp[..] NULL | auxv pairs ... AT_NULL | padding | strings
    let stack_end = user_stack_base + USER_STACK_SIZE as u64;

    let default_env: [&str; 8] = [
        "PATH=/usr/local/bin:/bin:/usr/bin:/sbin:/usr/sbin",
        "HOME=/root",
        "USER=root",
        "LOGNAME=root",
        "TERM=linux",
        "SHELL=/bin/bash",
        "PWD=/",
        "LD_LIBRARY_PATH=/usr/local/lib:/usr/lib:/lib:/lib64:/usr/lib64",
    ];
    let env: &[&str] = envp.unwrap_or(&default_env);

    // String block: AT_RANDOM bytes, AT_EXECFN, argv strings, envp strings.
    let mut strings: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
    let mut random = [0u8; 16];
    for (k, b) in random.iter_mut().enumerate() {
        *b = (0x5A ^ (k as u8)).wrapping_add(0x13);
    }
    strings.extend_from_slice(&random);
    let add_string = |strings: &mut alloc::vec::Vec<u8>, s: &str| -> usize {
        let off = strings.len();
        strings.extend_from_slice(s.as_bytes());
        strings.push(0);
        off
    };
    let execfn_off = add_string(&mut strings, args.first().copied().unwrap_or(""));
    let argv_offs: alloc::vec::Vec<usize> = args.iter().map(|a| add_string(&mut strings, a)).collect();
    let env_offs: alloc::vec::Vec<usize> = env.iter().map(|e| add_string(&mut strings, e)).collect();
    let strings_size = (strings.len() + 15) & !15;
    let strings_start = stack_end - strings_size as u64;

    let phdr_vaddr = main_base
        + if header.phoff < 0x400000 && main_base == 0 { 0x400000 + header.phoff } else { header.phoff };
    let auxv: [(u64, u64); 16] = [
        (3, phdr_vaddr),                             // AT_PHDR
        (4, header.phentsize as u64),                // AT_PHENT
        (5, header.phnum as u64),                    // AT_PHNUM
        (6, 4096),                                   // AT_PAGESZ
        (7, interp_base),                            // AT_BASE (dynamic interpreter)
        (8, 0),                                      // AT_FLAGS
        (9, main_base + header.entry),               // AT_ENTRY
        (11, 0),                                     // AT_UID
        (12, 0),                                     // AT_EUID
        (13, 0),                                     // AT_GID
        (14, 0),                                     // AT_EGID
        (17, 100),                                   // AT_CLKTCK
        (25, strings_start),                         // AT_RANDOM (first 16 bytes of the block)
        (31, strings_start + execfn_off as u64),     // AT_EXECFN
        (33, 0),                                     // AT_SYSINFO_EHDR
        (0, 0),                                      // AT_NULL
    ];

    let words = 1 + (args.len() + 1) + (env.len() + 1) + auxv.len() * 2;
    let sp = (strings_start - (words * 8) as u64) & !0xF;
    let eager_bytes = (EAGER_STACK_PAGES * 4096) as u64;
    if stack_end - sp > eager_bytes - 4096 {
        return Err("Argument list too long");
    }

    let mut image: alloc::vec::Vec<u64> = alloc::vec::Vec::with_capacity(words);
    image.push(args.len() as u64);
    image.extend(argv_offs.iter().map(|&o| strings_start + o as u64));
    image.push(0);
    image.extend(env_offs.iter().map(|&o| strings_start + o as u64));
    image.push(0);
    for &(t, v) in &auxv {
        image.push(t);
        image.push(v);
    }

    // Copy the image (pointer block, then strings) into the mapped stack pages.
    let write_user = |vaddr: u64, bytes: &[u8]| -> Result<(), &'static str> {
        let mut done = 0usize;
        while done < bytes.len() {
            let va = vaddr + done as u64;
            let page = va & !0xFFF;
            let phys = crate::mm::vmm::get_page_phys_in_pml4(pml4_phys, VirtAddr::new(page))
                .ok_or("Initial stack page is not mapped")?;
            let off = (va - page) as usize;
            let n = (4096 - off).min(bytes.len() - done);
            unsafe {
                core::ptr::copy_nonoverlapping(bytes.as_ptr().add(done), (phys.as_u64() + off as u64) as *mut u8, n);
            }
            done += n;
        }
        Ok(())
    };
    let image_bytes: alloc::vec::Vec<u8> = image.iter().flat_map(|w| w.to_le_bytes()).collect();
    write_user(sp, &image_bytes)?;
    write_user(strings_start, &strings)?;
    let user_stack_top = sp;

    Ok((
        LoadedProgram {
            entry_point: execution_entry,
            user_stack_top,
        },
        proc_pml4_frame,
    ))
}

fn elf_runner_trampoline() {
    let tid = crate::task::scheduler::current_tid();
    let prog = {
        let lock = ELF_EXEC_MAP.lock();
        lock.get(&tid).copied().expect("No ELF program loaded for current thread")
    };

    lunix_strace!("  [USER] Transitioning CPU to Ring 3 at entry point 0x{:X}, stack 0x{:X}...", prog.entry_point, prog.user_stack_top);
    unsafe {
        crate::task::user::enter_user_mode(prog.entry_point, prog.user_stack_top);
    }
}

pub fn exec_elf(path: &str) -> Result<usize, String> {
    exec_elf_with_args(path, &[path])
}

pub fn exec_elf_with_args(path: &str, args: &[&str]) -> Result<usize, String> {
    lunix_strace!("[ELF] Loading binary '{}' from VFS...", path);
    let bytes = crate::fs::vfs::read_to_vec(path)
        .map_err(|e| alloc::format!("Failed to read '{}': {:?}", path, e))?;

    if bytes.starts_with(b"#!") {
        let mut line_end = 2;
        while line_end < bytes.len() && bytes[line_end] != b'\n' && bytes[line_end] != b'\r' {
            line_end += 1;
        }
        let shebang_line = core::str::from_utf8(&bytes[2..line_end]).unwrap_or("/bin/sh").trim();
        let interp = shebang_line.split_whitespace().next().unwrap_or("/bin/sh");
        lunix_strace!("  [SHEBANG] Executing script '{}' via interpreter '{}'...", path, interp);
        return exec_elf_with_args(interp, &[interp, path]);
    }

    let is_init = path == "/sbin/init" || args.first().map(|s| *s == "init").unwrap_or(false);
    let pid = if is_init { 1 } else { crate::task::scheduler::allocate_pid() };

    let (prog, proc_pml4_frame) = load_elf_with_args(&bytes, args, None, pid)
        .map_err(|e| alloc::format!("ELF loader error: {}", e))?;

    let ppid = if is_init { 0 } else { crate::task::scheduler::current_pid() };
    let proc = crate::task::process::Process::new_user(pid, ppid, path, proc_pml4_frame.start_address().as_u64());
    crate::task::scheduler::register_process(proc);

    if ppid != pid && ppid > 0 {
        if let Some(parent_proc) = crate::task::scheduler::get_process(ppid) {
            parent_proc.lock().children.push(pid);
        }
    }

    lunix_strace!("[+] Spawned Ring 3 ELF process (PID: {}, PPID: {})", pid, ppid);
    let tid = crate::task::scheduler::spawn_with_pid("elf_process", pid, elf_runner_trampoline, 8);
    ELF_EXEC_MAP.lock().insert(tid, prog);
    Ok(tid)
}

pub fn exec_elf_replace(path: &str, args: &[&str], envp: Option<&[&str]>) -> Result<(), String> {
    lunix_strace!("[ELF] execve replacing image with binary '{}' from VFS...", path);
    let bytes = crate::fs::vfs::read_to_vec(path)
        .map_err(|e| alloc::format!("Failed to read '{}': {:?}", path, e))?;

    if bytes.starts_with(b"#!") {
        let mut line_end = 2;
        while line_end < bytes.len() && bytes[line_end] != b'\n' && bytes[line_end] != b'\r' {
            line_end += 1;
        }
        let shebang_line = core::str::from_utf8(&bytes[2..line_end]).unwrap_or("/bin/sh").trim();
        let interp = shebang_line.split_whitespace().next().unwrap_or("/bin/sh");
        lunix_strace!("  [SHEBANG] Executing script '{}' via interpreter '{}'...", path, interp);
        return exec_elf_replace(interp, &[interp, path], envp);
    }

    let current_pid = crate::task::scheduler::current_pid();
    let (prog, proc_pml4_frame) = load_elf_with_args(&bytes, args, envp, current_pid)
        .map_err(|e| alloc::format!("ELF loader error: {}", e))?;

    // Update current process PML4 CR3 and wake up any waiting vfork parent
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut proc = proc_arc.lock();
        proc.cr3 = proc_pml4_frame.start_address().as_u64();
        proc.name = alloc::string::String::from(path);
        crate::arch::x86_64::fpu::reset_user_state();
        // Descriptors marked close-on-exec do not survive into the new program (this is
        // what lets pipe readers see EOF once the writers have exited).
        for fd in 0..crate::task::process::MAX_FD {
            let cloexec = proc
                .get_fd(fd)
                .map(|d| d.lock().flags & crate::syscall::O_CLOEXEC != 0)
                .unwrap_or(false);
            if cloexec {
                proc.close_fd(fd);
            }
        }
        if let Some(parent_tid) = proc.vfork_waiting_parent.take() {
            crate::task::scheduler::unblock_thread(parent_tid);
        }
    }

    // Switch active CR3 page table to new process image
    let old_cr3 = x86_64::registers::control::Cr3::read().0.start_address().as_u64();
    unsafe {
        x86_64::registers::control::Cr3::write(
            proc_pml4_frame,
            x86_64::registers::control::Cr3Flags::empty(),
        );
        // Reset FS_BASE to 0
        crate::arch::x86_64::io::wrmsr(0xC000_0100, 0);
    }
    crate::task::scheduler::set_current_thread_fs_base(0);

    // The old image is gone: return its memory, unless a vfork parent still runs on it.
    if old_cr3 != 0 && old_cr3 != proc_pml4_frame.start_address().as_u64()
        && !crate::task::scheduler::address_space_shared(old_cr3, current_pid)
    {
        crate::mm::vmm::free_user_frames(x86_64::PhysAddr::new(old_cr3));
    }

    lunix_strace!("[+] execve image replaced successfully (PID {})! Transitioning to entry 0x{:X}...", current_pid, prog.entry_point);
    unsafe {
        crate::task::user::enter_user_mode(prog.entry_point, prog.user_stack_top);
    }
}



