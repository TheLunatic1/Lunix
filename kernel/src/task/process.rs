use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

pub const MAX_FD: usize = 64;

#[derive(Clone)]
pub enum FdTarget {
    Stdin,
    Stdout,
    Stderr,
    File {
        path: String,
        offset: usize,
        buffer: Arc<Mutex<Vec<u8>>>,
    },
    Directory {
        path: String,
        entries: Vec<crate::fs::file::DirectoryEntry>,
        current_idx: usize,
    },
    PipeRead(Arc<Mutex<crate::task::pipe::PipeBuffer>>),
    PipeWrite(Arc<Mutex<crate::task::pipe::PipeBuffer>>),
    Socket(usize),
<<<<<<< HEAD
    Epoll(crate::syscall::epoll::EpollSet),
    EventFd(Arc<Mutex<u64>>),
=======
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
    UnixSocket(Arc<Mutex<crate::syscall::unix_socket::UnixSocket>>),
    VfsHandle {
        handle: Arc<Mutex<Box<dyn crate::fs::file::FileHandle>>>,
        path: String,
        inode_id: u64,
    },
}

#[derive(Clone)]
pub struct FileDescriptor {
    pub target: FdTarget,
    pub flags: u32,
}

pub struct Process {
    pub id: usize,
    pub ppid: usize,
    pub name: String,
    pub threads: Vec<usize>,
    pub children: Vec<usize>,
    pub vfork_waiting_parent: Option<usize>,
    pub cr3: u64,
    pub cwd: String,
    pub uid: u32,
    pub gid: u32,
    pub euid: u32,
    pub egid: u32,
    pub suid: u32,
    pub sgid: u32,
    pub fsuid: u32,
    pub fsgid: u32,
    pub umask: u32,
    pub pgid: usize,
    pub sid: usize,
    pub is_alive: bool,
    pub exit_code: Option<i32>,
    pub fds: [Option<Arc<Mutex<FileDescriptor>>>; MAX_FD],
}

impl Process {
    pub fn new_kernel() -> Self {
        const INIT_FD: Option<Arc<Mutex<FileDescriptor>>> = None;
        let (cr3_frame, _) = x86_64::registers::control::Cr3::read();
        
        let mut proc = Self {
            id: 0,
            ppid: 0,
            name: String::from("kernel"),
            threads: Vec::new(),
            children: Vec::new(),
            vfork_waiting_parent: None,
            cr3: cr3_frame.start_address().as_u64(),
            cwd: String::from("/"),
            uid: 0,
            gid: 0,
            euid: 0,
            egid: 0,
            suid: 0,
            sgid: 0,
            fsuid: 0,
            fsgid: 0,
            umask: 0o022,
            pgid: 0,
            sid: 0,
            is_alive: true,
            exit_code: None,
            fds: [INIT_FD; MAX_FD],
        };
        proc.init_std_fds();
        proc
    }

    pub fn new_user(id: usize, ppid: usize, name: &str, cr3: u64) -> Self {
        const INIT_FD: Option<Arc<Mutex<FileDescriptor>>> = None;
        let mut proc = Self {
            id,
            ppid,
            name: String::from(name),
            threads: Vec::new(),
            children: Vec::new(),
            vfork_waiting_parent: None,
            cr3,
            cwd: String::from("/"),
            uid: 0,
            gid: 0,
            euid: 0,
            egid: 0,
            suid: 0,
            sgid: 0,
            fsuid: 0,
            fsgid: 0,
            umask: 0o022,
            pgid: id,
            sid: id,
            is_alive: true,
            exit_code: None,
            fds: [INIT_FD; MAX_FD],
        };
        proc.init_std_fds();
        proc
    }

    pub fn init_std_fds(&mut self) {
        self.fds[0] = Some(Arc::new(Mutex::new(FileDescriptor {
            target: FdTarget::Stdin,
            flags: 0,
        })));
        self.fds[1] = Some(Arc::new(Mutex::new(FileDescriptor {
            target: FdTarget::Stdout,
            flags: 0,
        })));
        self.fds[2] = Some(Arc::new(Mutex::new(FileDescriptor {
            target: FdTarget::Stderr,
            flags: 0,
        })));
    }

    pub fn clone_process(&self, new_pid: usize, is_vfork: bool) -> Self {
        let child_cr3 = if is_vfork {
            self.cr3
        } else {
            if let Ok(frame) = crate::mm::vmm::clone_process_pml4(x86_64::PhysAddr::new(self.cr3)) {
                let cr3 = frame.start_address().as_u64();
                // The child inherits the parent's demand-paged mappings.
                crate::mm::vma::fork(self.cr3, cr3);
                cr3
            } else {
                self.cr3
            }
        };

        let mut new_proc = Self::new_user(new_pid, self.id, &self.name, child_cr3);
        new_proc.cwd = self.cwd.clone();
        new_proc.uid = self.uid;
        new_proc.gid = self.gid;
        new_proc.euid = self.euid;
        new_proc.egid = self.egid;
        new_proc.suid = self.suid;
        new_proc.sgid = self.sgid;
        new_proc.fsuid = self.fsuid;
        new_proc.fsgid = self.fsgid;
        new_proc.umask = self.umask;
        new_proc.pgid = self.pgid;
        new_proc.sid = self.sid;
        for i in 0..MAX_FD {
            if let Some(ref fd) = self.fds[i] {
                let lock = fd.lock();
                // Increment pipe reader/writer counts if cloning a pipe FD
                match lock.target {
                    FdTarget::PipeRead(ref pipe) => {
                        pipe.lock().add_reader();
                    }
                    FdTarget::PipeWrite(ref pipe) => {
                        pipe.lock().add_writer();
                    }
                    FdTarget::UnixSocket(ref sock) => {
                        sock.lock().open_fds += 1;
                    }
                    _ => {}
                }
                new_proc.fds[i] = Some(Arc::new(Mutex::new(lock.clone())));
            }
        }
        new_proc
    }

    /// Close every descriptor (process exit): pipe ends and sockets get their peers notified.
    pub fn close_all_fds(&mut self) {
        for fd in 0..MAX_FD {
            self.close_fd(fd);
        }
    }

    pub fn allocate_fd(&mut self, desc: FileDescriptor) -> Option<usize> {
        for (i, slot) in self.fds.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(Arc::new(Mutex::new(desc)));
                return Some(i);
            }
        }
        None
    }

    pub fn allocate_fd_at(&mut self, fd: usize, desc: FileDescriptor) -> bool {
        if fd >= MAX_FD {
            return false;
        }
        self.close_fd(fd);
        self.fds[fd] = Some(Arc::new(Mutex::new(desc)));
        true
    }

    pub fn get_fd(&self, fd: usize) -> Option<Arc<Mutex<FileDescriptor>>> {
        if fd < MAX_FD {
            self.fds[fd].clone()
        } else {
            None
        }
    }

    pub fn close_fd(&mut self, fd: usize) -> bool {
        if fd < MAX_FD && self.fds[fd].is_some() {
            if let Some(ref desc_arc) = self.fds[fd] {
                let desc = desc_arc.lock();
                match desc.target {
                    FdTarget::PipeRead(ref pipe) => {
                        pipe.lock().remove_reader();
                    }
                    FdTarget::PipeWrite(ref pipe) => {
                        pipe.lock().remove_writer();
                    }
                    FdTarget::UnixSocket(ref sock) => {
                        sock.lock().close_one();
                    }
                    _ => {}
                }
            }
            self.fds[fd] = None;
            true
        } else {
            false
        }
    }

    pub fn dup_fd(&mut self, oldfd: usize, newfd: usize) -> Option<usize> {
        if oldfd >= MAX_FD || newfd >= MAX_FD {
            return None;
        }
        let src_opt = if let Some(ref src) = self.fds[oldfd] {
            let lock = src.lock();
            match lock.target {
                FdTarget::PipeRead(ref pipe) => {
                    pipe.lock().add_reader();
                }
                FdTarget::PipeWrite(ref pipe) => {
                    pipe.lock().add_writer();
                }
                FdTarget::UnixSocket(ref sock) => {
                    sock.lock().open_fds += 1;
                }
                _ => {}
            }
            Some(lock.clone())
        } else {
            None
        };

        if let Some(desc) = src_opt {
            self.close_fd(newfd);
            self.fds[newfd] = Some(Arc::new(Mutex::new(desc)));
            Some(newfd)
        } else {
            None
        }
    }

    pub fn dup_lowest_fd(&mut self, oldfd: usize, min_fd: usize) -> Option<usize> {
        if oldfd >= MAX_FD || min_fd >= MAX_FD {
            return None;
        }
        let src_opt = if let Some(ref src) = self.fds[oldfd] {
            let lock = src.lock();
            match lock.target {
                FdTarget::PipeRead(ref pipe) => {
                    pipe.lock().add_reader();
                }
                FdTarget::PipeWrite(ref pipe) => {
                    pipe.lock().add_writer();
                }
                FdTarget::UnixSocket(ref sock) => {
                    sock.lock().open_fds += 1;
                }
                _ => {}
            }
            Some(lock.clone())
        } else {
            None
        };

        if let Some(desc) = src_opt {
            for target_fd in min_fd..MAX_FD {
                if self.fds[target_fd].is_none() {
                    self.fds[target_fd] = Some(Arc::new(Mutex::new(desc)));
                    return Some(target_fd);
                }
            }
        }
        None
    }
}
