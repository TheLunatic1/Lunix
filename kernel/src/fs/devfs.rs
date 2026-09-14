//! Device Filesystem (/dev)
//!
//! Provides Linux-compatible pseudo character and block device nodes:
//! - /dev/null: Discards all writes, reads return 0 (EOF)
//! - /dev/zero: Discards all writes, reads fill buffer with 0x00
//! - /dev/urandom & /dev/random: Hardware/PRNG entropy stream
//! - /dev/tty & /dev/console: Console terminal stream
//! - /dev/sda: Block storage device node

use super::file::{DirectoryEntry, FileHandle, SeekFrom};
use super::inode::{INode, INodeType};
use super::vfs::{FileSystem, VfsError};
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

static PRNG_STATE: AtomicU64 = AtomicU64::new(0x853C49E6748FEA9B);

fn next_random_u64() -> u64 {
    let mut state = PRNG_STATE.load(Ordering::Relaxed);
    let tsc: u64 = unsafe {
        let low: u32;
        let high: u32;
        core::arch::asm!("rdtsc", out("eax") low, out("edx") high, options(nostack, nomem));
        ((high as u64) << 32) | (low as u64)
    };
    state = state.wrapping_add(tsc).wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    PRNG_STATE.store(state, Ordering::Relaxed);
    state
}

// -----------------------------------------------------------------------------
// Device File Handles
// -----------------------------------------------------------------------------

pub struct NullHandle;
impl FileHandle for NullHandle {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, &'static str> {
        Ok(0) // Immediate EOF
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        Ok(buf.len()) // Discard all input
    }

    fn seek(&mut self, _pos: SeekFrom) -> Result<u64, &'static str> {
        Ok(0)
    }

    fn size(&self) -> u64 {
        0
    }
}

pub struct ZeroHandle;
impl FileHandle for ZeroHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        buf.fill(0);
        Ok(buf.len())
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        Ok(buf.len()) // Discard all input
    }

    fn seek(&mut self, _pos: SeekFrom) -> Result<u64, &'static str> {
        Ok(0)
    }

    fn size(&self) -> u64 {
        0
    }
}

pub struct RandomHandle;
impl FileHandle for RandomHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        let mut chunks = buf.chunks_exact_mut(8);
        for chunk in &mut chunks {
            let rand_val = next_random_u64();
            chunk.copy_from_slice(&rand_val.to_ne_bytes());
        }
        let rem = chunks.into_remainder();
        if !rem.is_empty() {
            let rand_val = next_random_u64();
            let bytes = rand_val.to_ne_bytes();
            rem.copy_from_slice(&bytes[..rem.len()]);
        }
        Ok(buf.len())
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        // Accept user entropy into PRNG state
        let mut seed: u64 = 0;
        for (i, &b) in buf.iter().take(8).enumerate() {
            seed |= (b as u64) << (i * 8);
        }
        if seed != 0 {
            PRNG_STATE.fetch_xor(seed, Ordering::Relaxed);
        }
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> Result<u64, &'static str> {
        Ok(0)
    }

    fn size(&self) -> u64 {
        0
    }
}

pub struct TtyHandle;
impl FileHandle for TtyHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        if buf.is_empty() {
            return Ok(0);
        }
        // Return 0 or single newline if idle
        buf[0] = b'\n';
        Ok(1)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        if let Ok(s) = core::str::from_utf8(buf) {
            crate::lunix_print!("{}", s);
        } else {
            for &b in buf {
                crate::lunix_print!("{}", b as char);
            }
        }
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> Result<u64, &'static str> {
        Ok(0)
    }

    fn size(&self) -> u64 {
        0
    }
}

pub struct BlockDevHandle {
    position: u64,
    size: u64,
}
impl FileHandle for BlockDevHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        buf.fill(0);
        self.position = self.position.saturating_add(buf.len() as u64);
        Ok(buf.len())
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        self.position = self.position.saturating_add(buf.len() as u64);
        Ok(buf.len())
    }

    fn seek(&mut self, pos: SeekFrom) -> Result<u64, &'static str> {
        let new_pos = match pos {
            SeekFrom::Start(off) => off as i64,
            SeekFrom::Current(off) => self.position as i64 + off,
            SeekFrom::End(off) => self.size as i64 + off,
        };
        if new_pos < 0 {
            return Err("Invalid negative seek");
        }
        self.position = new_pos as u64;
        Ok(self.position)
    }

    fn size(&self) -> u64 {
        self.size
    }
}

// -----------------------------------------------------------------------------
// DevFs FileSystem Implementation
// -----------------------------------------------------------------------------

pub struct DevFs;

impl DevFs {
    pub fn new() -> Self {
        Self
    }
}

impl FileSystem for DevFs {
    fn open(&self, path: &str) -> Result<Box<dyn FileHandle>, VfsError> {
        let clean = path.trim_start_matches('/');
        match clean {
            "null" => Ok(Box::new(NullHandle)),
            "zero" => Ok(Box::new(ZeroHandle)),
            "urandom" | "random" => Ok(Box::new(RandomHandle)),
            "tty" | "console" => Ok(Box::new(TtyHandle)),
            "sda" => Ok(Box::new(BlockDevHandle {
                position: 0,
                size: 64 * 1024 * 1024,
            })),
            _ => Err(VfsError::NotFound),
        }
    }

    fn read_dir(&self, path: &str) -> Result<Vec<DirectoryEntry>, VfsError> {
        let clean = path.trim_start_matches('/');
        if !clean.is_empty() && clean != "." {
            return Err(VfsError::NotADirectory);
        }

        let entries = alloc::vec![
            DirectoryEntry {
                name: String::from("null"),
                node_type: INodeType::CharDevice,
                size: 0,
            },
            DirectoryEntry {
                name: String::from("zero"),
                node_type: INodeType::CharDevice,
                size: 0,
            },
            DirectoryEntry {
                name: String::from("urandom"),
                node_type: INodeType::CharDevice,
                size: 0,
            },
            DirectoryEntry {
                name: String::from("random"),
                node_type: INodeType::CharDevice,
                size: 0,
            },
            DirectoryEntry {
                name: String::from("tty"),
                node_type: INodeType::CharDevice,
                size: 0,
            },
            DirectoryEntry {
                name: String::from("console"),
                node_type: INodeType::CharDevice,
                size: 0,
            },
            DirectoryEntry {
                name: String::from("sda"),
                node_type: INodeType::BlockDevice,
                size: 64 * 1024 * 1024,
            },
        ];

        Ok(entries)
    }

    fn stat(&self, path: &str) -> Result<INode, VfsError> {
        let clean = path.trim_start_matches('/');
        if clean.is_empty() || clean == "." {
            return Ok(INode {
                id: 1,
                size: 0,
                node_type: INodeType::Directory,
                permissions: 0o755,
                name: String::from("dev"),
            });
        }

        match clean {
            "null" => Ok(INode {
                id: 2,
                size: 0,
                node_type: INodeType::CharDevice,
                permissions: 0o666,
                name: String::from("null"),
            }),
            "zero" => Ok(INode {
                id: 3,
                size: 0,
                node_type: INodeType::CharDevice,
                permissions: 0o666,
                name: String::from("zero"),
            }),
            "urandom" => Ok(INode {
                id: 4,
                size: 0,
                node_type: INodeType::CharDevice,
                permissions: 0o666,
                name: String::from("urandom"),
            }),
            "random" => Ok(INode {
                id: 5,
                size: 0,
                node_type: INodeType::CharDevice,
                permissions: 0o666,
                name: String::from("random"),
            }),
            "tty" => Ok(INode {
                id: 6,
                size: 0,
                node_type: INodeType::CharDevice,
                permissions: 0o666,
                name: String::from("tty"),
            }),
            "console" => Ok(INode {
                id: 7,
                size: 0,
                node_type: INodeType::CharDevice,
                permissions: 0o666,
                name: String::from("console"),
            }),
            "sda" => Ok(INode {
                id: 8,
                size: 64 * 1024 * 1024,
                node_type: INodeType::BlockDevice,
                permissions: 0o660,
                name: String::from("sda"),
            }),
            _ => Err(VfsError::NotFound),
        }
    }
}
