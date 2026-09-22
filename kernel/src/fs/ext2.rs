//! Read-only ext2 driver (block sizes 1-64 KiB, 128/256-byte inodes, `filetype` feature).
//!
//! Mounted as the root filesystem: unlike FAT32 it is case-sensitive and has symlinks,
//! hard links and Unix permissions, which a real Linux distribution needs. Path
//! resolution follows symlinks (Linux `stat` vs `lstat` semantics) inside the filesystem.

use super::block::BlockDevice;
use super::file::{DirectoryEntry, FileHandle, SeekFrom};
use super::inode::{INode, INodeType};
use super::vfs::{FileSystem, VfsError};
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use spin::Mutex;

const EXT2_MAGIC: u16 = 0xEF53;
const ROOT_INO: u32 = 2;
const MAX_SYMLINKS: u32 = 40;
const CACHE_BLOCKS: usize = 256;

const S_IFMT: u16 = 0o170000;
const S_IFREG: u16 = 0o100000;
const S_IFDIR: u16 = 0o040000;
const S_IFLNK: u16 = 0o120000;
const S_IFCHR: u16 = 0o020000;
const S_IFBLK: u16 = 0o060000;

#[derive(Clone)]
struct RawInode {
    mode: u16,
    size: u64,
    block: [u32; 15],
}

impl RawInode {
    fn is_dir(&self) -> bool {
        self.mode & S_IFMT == S_IFDIR
    }
    fn is_symlink(&self) -> bool {
        self.mode & S_IFMT == S_IFLNK
    }
    fn node_type(&self) -> INodeType {
        match self.mode & S_IFMT {
            S_IFDIR => INodeType::Directory,
            S_IFLNK => INodeType::SymLink,
            S_IFCHR => INodeType::CharDevice,
            S_IFBLK => INodeType::BlockDevice,
            _ => INodeType::File,
        }
    }
}

struct Inner {
    dev: Arc<dyn BlockDevice>,
    /// First 512-byte sector of the filesystem on `dev`.
    start_lba: u64,
    block_size: usize,
    inodes_per_group: u32,
    inode_size: usize,
    /// Inode table start block of each group.
    inode_tables: Vec<u32>,
    cache: Mutex<VecDeque<(u64, Arc<Vec<u8>>)>>,
}

pub struct Ext2FileSystem {
    inner: Arc<Inner>,
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// Find an ext2 filesystem on `dev`: scan the GPT for a partition whose superblock has the
/// ext2 magic, or fall back to an unpartitioned device. Returns the start sector.
pub fn find_ext2(dev: &Arc<dyn BlockDevice>) -> Option<u64> {
    let has_magic = |lba: u64| -> bool {
        let mut sb = [0u8; 1024];
        // Superblock is 1024 bytes into the filesystem = sector 2.
        dev.read_blocks(lba + 2, 2, &mut sb).is_ok() && u16_at(&sb, 56) == EXT2_MAGIC
    };

    let mut hdr = [0u8; 512];
    if dev.read_blocks(1, 1, &mut hdr).is_ok() && &hdr[0..8] == b"EFI PART" {
        let entries_lba = u64::from_le_bytes(hdr[72..80].try_into().ok()?);
        let count = u32_at(&hdr, 80).min(128) as usize;
        let entry_size = u32_at(&hdr, 84) as usize;
        if entry_size >= 128 && count > 0 {
            let sectors = (count * entry_size + 511) / 512;
            let mut table = vec![0u8; sectors * 512];
            if dev.read_blocks(entries_lba, sectors, &mut table).is_ok() {
                for i in 0..count {
                    let e = &table[i * entry_size..(i + 1) * entry_size];
                    if e[0..16].iter().all(|&b| b == 0) {
                        continue;
                    }
                    let start = u64::from_le_bytes(e[32..40].try_into().ok()?);
                    if start > 0 && has_magic(start) {
                        return Some(start);
                    }
                }
            }
        }
    }
    if has_magic(0) {
        return Some(0);
    }
    None
}

impl Ext2FileSystem {
    pub fn new(dev: Arc<dyn BlockDevice>, start_lba: u64) -> Option<Self> {
        let mut sb = [0u8; 1024];
        dev.read_blocks(start_lba + 2, 2, &mut sb).ok()?;
        if u16_at(&sb, 56) != EXT2_MAGIC {
            return None;
        }
        let block_size = 1024usize << u32_at(&sb, 24);
        if block_size < 1024 || block_size > 65536 {
            return None;
        }
        let blocks_count = u32_at(&sb, 4) as u64;
        let blocks_per_group = u32_at(&sb, 32) as u64;
        let inodes_per_group = u32_at(&sb, 40);
        let first_data_block = u32_at(&sb, 20) as u64;
        let rev = u32_at(&sb, 76);
        let inode_size = if rev >= 1 { u16_at(&sb, 88) as usize } else { 128 };
        let incompat = u32_at(&sb, 96);
        // Unsupported incompatible features (compression, journal replay needed, extents, ...).
        if incompat & !0x0002 != 0 {
            crate::lunix_serial_println!("  [EXT2] unsupported incompat features 0x{:X}", incompat);
            return None;
        }
        let groups = ((blocks_count - first_data_block + blocks_per_group - 1) / blocks_per_group) as usize;

        // Group descriptor table starts in the block after the superblock's.
        let gdt_block = first_data_block + 1;
        let gdt_bytes = groups * 32;
        let gdt_blocks = (gdt_bytes + block_size - 1) / block_size;
        let sectors_per_block = block_size / 512;
        let mut gdt = vec![0u8; gdt_blocks * block_size];
        dev.read_blocks(start_lba + gdt_block * sectors_per_block as u64, gdt_blocks * sectors_per_block, &mut gdt).ok()?;
        let inode_tables = (0..groups).map(|g| u32_at(&gdt, g * 32 + 8)).collect();

        crate::lunix_serial_println!(
            "  [EXT2] Mounted ext2 on '{}' (start LBA {}, {} B blocks, {} groups, {} inodes/group)",
            dev.name(), start_lba, block_size, groups, inodes_per_group
        );
        Some(Self {
            inner: Arc::new(Inner {
                dev,
                start_lba,
                block_size,
                inodes_per_group,
                inode_size,
                inode_tables,
                cache: Mutex::new(VecDeque::new()),
            }),
        })
    }
}

impl Inner {
    fn sectors_per_block(&self) -> usize {
        self.block_size / 512
    }

    /// Read one block through the cache.
    fn block(&self, n: u64) -> Result<Arc<Vec<u8>>, VfsError> {
        {
            let cache = self.cache.lock();
            if let Some((_, data)) = cache.iter().find(|(b, _)| *b == n) {
                return Ok(data.clone());
            }
        }
        let mut buf = vec![0u8; self.block_size];
        self.dev
            .read_blocks(self.start_lba + n * self.sectors_per_block() as u64, self.sectors_per_block(), &mut buf)
            .map_err(|_| VfsError::DeviceError)?;
        let data = Arc::new(buf);
        let mut cache = self.cache.lock();
        if cache.len() >= CACHE_BLOCKS {
            cache.pop_front();
        }
        cache.push_back((n, data.clone()));
        Ok(data)
    }

    fn read_inode(&self, ino: u32) -> Result<RawInode, VfsError> {
        if ino == 0 {
            return Err(VfsError::NotFound);
        }
        let group = ((ino - 1) / self.inodes_per_group) as usize;
        let index = ((ino - 1) % self.inodes_per_group) as usize;
        let table = *self.inode_tables.get(group).ok_or(VfsError::NotFound)? as u64;
        let byte = index * self.inode_size;
        let blk = self.block(table + (byte / self.block_size) as u64)?;
        let o = byte % self.block_size;
        let raw = &blk[o..o + self.inode_size.min(128)];
        let mode = u16_at(raw, 0);
        let mut size = u32_at(raw, 4) as u64;
        if mode & S_IFMT == S_IFREG {
            size |= (u32_at(raw, 108) as u64) << 32;
        }
        let mut block = [0u32; 15];
        for (i, b) in block.iter_mut().enumerate() {
            *b = u32_at(raw, 40 + i * 4);
        }
        Ok(RawInode { mode, size, block })
    }

    /// Physical block of logical block `idx` of a file (0 = hole).
    fn bmap(&self, inode: &RawInode, idx: u64) -> Result<u32, VfsError> {
        let per = (self.block_size / 4) as u64;
        if idx < 12 {
            return Ok(inode.block[idx as usize]);
        }
        let mut i = idx - 12;
        let ptr = |blk: u32, n: u64| -> Result<u32, VfsError> {
            if blk == 0 {
                return Ok(0);
            }
            let b = self.block(blk as u64)?;
            Ok(u32_at(&b, (n * 4) as usize))
        };
        if i < per {
            return ptr(inode.block[12], i);
        }
        i -= per;
        if i < per * per {
            let l1 = ptr(inode.block[13], i / per)?;
            return ptr(l1, i % per);
        }
        i -= per * per;
        let l1 = ptr(inode.block[14], i / (per * per))?;
        let l2 = ptr(l1, (i / per) % per)?;
        ptr(l2, i % per)
    }

    /// Read file data at `offset` into `buf`. Runs of physically contiguous blocks are
    /// fetched with a single device read.
    fn read_data(&self, inode: &RawInode, offset: u64, buf: &mut [u8]) -> Result<usize, VfsError> {
        if offset >= inode.size {
            return Ok(0);
        }
        let want = (buf.len() as u64).min(inode.size - offset) as usize;
        let bs = self.block_size as u64;
        let mut done = 0usize;
        while done < want {
            let pos = offset + done as u64;
            let idx = pos / bs;
            let within = (pos % bs) as usize;
            let phys = self.bmap(inode, idx)?;

            if within == 0 && want - done >= self.block_size && phys != 0 {
                // Whole blocks: extend over contiguous physical blocks.
                let mut run = 1u64;
                let max_run = ((want - done) as u64 / bs).min(256);
                while run < max_run && self.bmap(inode, idx + run)? as u64 == phys as u64 + run {
                    run += 1;
                }
                let bytes = (run * bs) as usize;
                self.dev
                    .read_blocks(self.start_lba + phys as u64 * self.sectors_per_block() as u64, (run as usize) * self.sectors_per_block(), &mut buf[done..done + bytes])
                    .map_err(|_| VfsError::DeviceError)?;
                done += bytes;
            } else {
                let n = (self.block_size - within).min(want - done);
                if phys == 0 {
                    buf[done..done + n].fill(0); // sparse hole
                } else {
                    let b = self.block(phys as u64)?;
                    buf[done..done + n].copy_from_slice(&b[within..within + n]);
                }
                done += n;
            }
        }
        Ok(done)
    }

    fn read_all(&self, inode: &RawInode) -> Result<Vec<u8>, VfsError> {
        let mut v = vec![0u8; inode.size as usize];
        let n = self.read_data(inode, 0, &mut v)?;
        v.truncate(n);
        Ok(v)
    }

    fn read_link(&self, inode: &RawInode) -> Result<String, VfsError> {
        let bytes = if inode.size < 60 {
            // Fast symlink: the target is stored in the block pointer area.
            let mut raw = [0u8; 60];
            for (i, b) in inode.block.iter().enumerate() {
                raw[i * 4..i * 4 + 4].copy_from_slice(&b.to_le_bytes());
            }
            raw[..inode.size as usize].to_vec()
        } else {
            self.read_all(inode)?
        };
        String::from_utf8(bytes).map_err(|_| VfsError::DeviceError)
    }

    /// Iterate a directory: (name, inode, file_type).
    fn dir_entries(&self, dir: &RawInode) -> Result<Vec<(String, u32, u8)>, VfsError> {
        let data = self.read_all(dir)?;
        let mut out = Vec::new();
        let mut o = 0usize;
        while o + 8 <= data.len() {
            let ino = u32_at(&data, o);
            let rec_len = u16_at(&data, o + 4) as usize;
            let name_len = data[o + 6] as usize;
            let ftype = data[o + 7];
            if rec_len < 8 || o + rec_len > data.len() {
                break;
            }
            if ino != 0 && o + 8 + name_len <= data.len() {
                if let Ok(name) = core::str::from_utf8(&data[o + 8..o + 8 + name_len]) {
                    out.push((String::from(name), ino, ftype));
                }
            }
            o += rec_len;
        }
        Ok(out)
    }

    fn lookup(&self, dir: &RawInode, name: &str) -> Result<u32, VfsError> {
        if !dir.is_dir() {
            return Err(VfsError::NotADirectory);
        }
        // Scan block by block so large directories don't need to be read whole.
        let bs = self.block_size as u64;
        let nblocks = (dir.size + bs - 1) / bs;
        for b in 0..nblocks {
            let phys = self.bmap(dir, b)?;
            if phys == 0 {
                continue;
            }
            let data = self.block(phys as u64)?;
            let mut o = 0usize;
            while o + 8 <= data.len() {
                let ino = u32_at(&data, o);
                let rec_len = u16_at(&data, o + 4) as usize;
                let name_len = data[o + 6] as usize;
                if rec_len < 8 || o + rec_len > data.len() {
                    break;
                }
                if ino != 0 && &data[o + 8..o + 8 + name_len] == name.as_bytes() {
                    return Ok(ino);
                }
                o += rec_len;
            }
        }
        Err(VfsError::NotFound)
    }

    /// Resolve `path` to an inode number, following symlinks in intermediate components
    /// and, when `follow_last`, in the final one too.
    fn resolve(&self, path: &str, follow_last: bool) -> Result<u32, VfsError> {
        let mut stack: VecDeque<String> = path.split('/').filter(|c| !c.is_empty()).map(String::from).collect();
        let mut cur = ROOT_INO;
        let mut links = 0u32;
        while let Some(comp) = stack.pop_front() {
            if comp == "." {
                continue;
            }
            let dir = self.read_inode(cur)?;
            let child = self.lookup(&dir, &comp)?;
            let node = self.read_inode(child)?;
            if node.is_symlink() && (follow_last || !stack.is_empty()) {
                links += 1;
                if links > MAX_SYMLINKS {
                    return Err(VfsError::NotFound); // ELOOP
                }
                let target = self.read_link(&node)?;
                if target.starts_with('/') {
                    cur = ROOT_INO;
                }
                for c in target.split('/').filter(|c| !c.is_empty()).rev() {
                    stack.push_front(String::from(c));
                }
                continue;
            }
            cur = child;
        }
        Ok(cur)
    }

    fn make_inode(&self, ino: u32, raw: &RawInode, name: &str) -> INode {
        INode {
            id: ino as u64,
            size: raw.size,
            node_type: raw.node_type(),
            permissions: raw.mode & 0o7777,
            name: String::from(name),
        }
    }
}

struct Ext2File {
    inner: Arc<Inner>,
    inode: RawInode,
    position: u64,
}

impl FileHandle for Ext2File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        let n = self.inner.read_data(&self.inode, self.position, buf).map_err(|_| "ext2 read error")?;
        self.position += n as u64;
        Ok(n)
    }

    fn write(&mut self, _buf: &[u8]) -> Result<usize, &'static str> {
        Err("ext2 root filesystem is mounted read-only")
    }

    fn seek(&mut self, pos: SeekFrom) -> Result<u64, &'static str> {
        let new = match pos {
            SeekFrom::Start(o) => o as i64,
            SeekFrom::Current(o) => self.position as i64 + o,
            SeekFrom::End(o) => self.inode.size as i64 + o,
        };
        if new < 0 {
            return Err("Invalid seek position");
        }
        self.position = new as u64;
        Ok(self.position)
    }

    fn size(&self) -> u64 {
        self.inode.size
    }
}

impl FileSystem for Ext2FileSystem {
    fn open(&self, path: &str) -> Result<Box<dyn FileHandle>, VfsError> {
        let ino = self.inner.resolve(path, true)?;
        let inode = self.inner.read_inode(ino)?;
        if inode.is_dir() {
            return Err(VfsError::NotAFile);
        }
        Ok(Box::new(Ext2File { inner: self.inner.clone(), inode, position: 0 }))
    }

    fn read_dir(&self, path: &str) -> Result<Vec<DirectoryEntry>, VfsError> {
        let ino = self.inner.resolve(path, true)?;
        let dir = self.inner.read_inode(ino)?;
        if !dir.is_dir() {
            return Err(VfsError::NotADirectory);
        }
        let mut out = Vec::new();
        for (name, _ino, ftype) in self.inner.dir_entries(&dir)? {
            if name == "." || name == ".." {
                continue;
            }
            let node_type = match ftype {
                2 => INodeType::Directory,
                7 => INodeType::SymLink,
                3 => INodeType::CharDevice,
                4 => INodeType::BlockDevice,
                _ => INodeType::File,
            };
            out.push(DirectoryEntry { name, node_type, size: 0 });
        }
        Ok(out)
    }

    fn stat(&self, path: &str) -> Result<INode, VfsError> {
        let ino = self.inner.resolve(path, true)?;
        let raw = self.inner.read_inode(ino)?;
        let name = path.rsplit('/').next().unwrap_or("");
        Ok(self.inner.make_inode(ino, &raw, name))
    }

    fn lstat(&self, path: &str) -> Result<INode, VfsError> {
        let ino = self.inner.resolve(path, false)?;
        let raw = self.inner.read_inode(ino)?;
        let name = path.rsplit('/').next().unwrap_or("");
        Ok(self.inner.make_inode(ino, &raw, name))
    }

    fn readlink(&self, path: &str) -> Result<String, VfsError> {
        let ino = self.inner.resolve(path, false)?;
        let raw = self.inner.read_inode(ino)?;
        if !raw.is_symlink() {
            return Err(VfsError::UnsupportedOperation);
        }
        self.inner.read_link(&raw)
    }
}
