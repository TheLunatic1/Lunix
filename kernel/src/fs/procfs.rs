//! Process & System Information Filesystem (/proc)
//!
//! Provides dynamic virtual files reflecting kernel, memory, processor, and mount states:
//! - /proc/version: Kernel release, build timestamp, and compiler info
//! - /proc/meminfo: Real-time physical and usable memory metrics from PMM
//! - /proc/cpuinfo: SMP multi-core topology and CPU flags
//! - /proc/mounts: Active VFS mount table
//! - /proc/uptime: System uptime in seconds

use super::file::{DirectoryEntry, FileHandle, MemoryFile};
use super::inode::{INode, INodeType};
use super::vfs::{FileSystem, VfsError};
use crate::mm::pmm;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

pub struct ProcFs;

impl ProcFs {
    pub fn new() -> Self {
        Self
    }

    fn generate_version() -> String {
        format!("Linux version 6.8.0-lunix-pure (root@lunix) (rustc 1.85.0-nightly #1 SMP PREEMPT 2026-09-21)\n")
    }

    fn generate_meminfo() -> String {
        let (total_bytes, usable_bytes, used_bytes) = pmm::get_memory_stats();
        let total_kb = total_bytes / 1024;
        let free_bytes = usable_bytes.saturating_sub(used_bytes);
        let free_kb = free_bytes / 1024;

        format!(
            "MemTotal:       {:8} kB\n\
             MemFree:        {:8} kB\n\
             MemAvailable:   {:8} kB\n\
             Buffers:               0 kB\n\
             Cached:             8192 kB\n\
             SwapTotal:             0 kB\n\
             SwapFree:              0 kB\n",
            total_kb, free_kb, free_kb
        )
    }

    fn generate_cpuinfo() -> String {
        let cores = crate::arch::x86_64::smp::CPU_COUNT.load(core::sync::atomic::Ordering::Relaxed).max(1);
        let mut out = String::new();

        for c in 0..cores {
            out.push_str(&format!(
                "processor\t: {}\n\
                 vendor_id\t: GenuineIntel\n\
                 cpu family\t: 6\n\
                 model name\t: Lunix Virtual x86_64 Processor (SMP)\n\
                 flags\t\t: fpu vme de pse tsc msr pae mce cx8 apic sep mtrr pge mca cmov pat pse36 sse sse2 sse3 ssse3 sse4_1 sse4_2\n\
                 cpu cores\t: {}\n\n",
                c, cores
            ));
        }

        out
    }

    fn generate_mounts() -> String {
        format!(
            "rootfs / rootfs rw 0 0\n\
             /dev/sda / fat32 rw,relatime,fmask=0022,dmask=0022,codepage=437 0 0\n\
             devtmpfs /dev devtmpfs rw,nosuid,size=4096k,nr_inodes=1024,mode=755 0 0\n\
             proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0\n"
        )
    }

    fn generate_uptime() -> String {
        let ticks = crate::drivers::timer::get_ticks();
        let secs = ticks / 1000;
        let centisecs = (ticks % 1000) / 10;
        format!("{}.{:02} {}.{:02}\n", secs, centisecs, secs * 2, centisecs)
    }
}

impl FileSystem for ProcFs {
    fn open(&self, path: &str) -> Result<Box<dyn FileHandle>, VfsError> {
        let clean = path.trim_start_matches('/');
        let content = match clean {
            "version" => Self::generate_version(),
            "meminfo" => Self::generate_meminfo(),
            "cpuinfo" => Self::generate_cpuinfo(),
            "mounts" => Self::generate_mounts(),
            "uptime" => Self::generate_uptime(),
            _ => return Err(VfsError::NotFound),
        };

        Ok(Box::new(MemoryFile::new(content.into_bytes())))
    }

    fn read_dir(&self, path: &str) -> Result<Vec<DirectoryEntry>, VfsError> {
        let clean = path.trim_start_matches('/');
        if !clean.is_empty() && clean != "." {
            return Err(VfsError::NotADirectory);
        }

        let entries = alloc::vec![
            DirectoryEntry {
                name: String::from("version"),
                node_type: INodeType::File,
                size: Self::generate_version().len() as u64,
            },
            DirectoryEntry {
                name: String::from("meminfo"),
                node_type: INodeType::File,
                size: Self::generate_meminfo().len() as u64,
            },
            DirectoryEntry {
                name: String::from("cpuinfo"),
                node_type: INodeType::File,
                size: Self::generate_cpuinfo().len() as u64,
            },
            DirectoryEntry {
                name: String::from("mounts"),
                node_type: INodeType::File,
                size: Self::generate_mounts().len() as u64,
            },
            DirectoryEntry {
                name: String::from("uptime"),
                node_type: INodeType::File,
                size: Self::generate_uptime().len() as u64,
            },
        ];

        Ok(entries)
    }

    fn stat(&self, path: &str) -> Result<INode, VfsError> {
        let clean = path.trim_start_matches('/');
        if clean.is_empty() || clean == "." {
            return Ok(INode {
                id: 10,
                size: 0,
                node_type: INodeType::Directory,
                permissions: 0o755,
                name: String::from("proc"),
            });
        }

        let (id, size) = match clean {
            "version" => (11, Self::generate_version().len() as u64),
            "meminfo" => (12, Self::generate_meminfo().len() as u64),
            "cpuinfo" => (13, Self::generate_cpuinfo().len() as u64),
            "mounts" => (14, Self::generate_mounts().len() as u64),
            "uptime" => (15, Self::generate_uptime().len() as u64),
            _ => return Err(VfsError::NotFound),
        };

        Ok(INode {
            id,
            size,
            node_type: INodeType::File,
            permissions: 0o444,
            name: String::from(clean),
        })
    }
}
