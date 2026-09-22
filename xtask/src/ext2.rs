//! ext2 filesystem writer (revision 1, 4 KiB blocks, 128-byte inodes, `filetype` feature).
//!
//! Every block group carries a backup superblock + group descriptor table (no
//! `sparse_super`), so a stock `e2fsck`/`mount` reads the result too. File contents are
//! streamed from their sources (tar archives) straight to the image.

use crate::rootfs::{Kind, Source, Tree};
use anyhow::{bail, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

const BLOCK: u64 = 4096;
const BLOCKS_PER_GROUP: u64 = 32768;
const INODE_SIZE: u64 = 128;
const ROOT_INO: u32 = 2;
const FIRST_INO: u32 = 11;
const PTRS: u64 = BLOCK / 4;

const S_IFREG: u16 = 0o100000;
const S_IFDIR: u16 = 0o040000;
const S_IFLNK: u16 = 0o120000;

struct Group {
    start: u64,
    /// Number of blocks in this group (last group may be short).
    len: u64,
    block_bitmap: u64,
    inode_bitmap: u64,
    inode_table: u64,
    /// First block available for file/directory data.
    data_start: u64,
    used: Vec<u8>, // block bitmap, 1 bit per block
    inode_used: Vec<u8>,
    free_blocks: u64,
    free_inodes: u32,
    dirs: u32,
}

pub struct Ext2Writer<'a> {
    out: &'a mut File,
    base: u64, // byte offset of the filesystem inside `out`
    groups: Vec<Group>,
    blocks_count: u64,
    inodes_per_group: u32,
    itable_blocks: u64,
    gdt_blocks: u64,
    cursor_group: usize,
    cursor_block: u64,
    inode_of: Vec<u32>, // tree node index -> inode number
    archives: HashMap<usize, File>,
}

fn set_bit(bm: &mut [u8], i: u64) {
    bm[(i / 8) as usize] |= 1 << (i % 8);
}

fn le32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}
fn le16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

/// Write `tree` as an ext2 filesystem of `size_bytes` at byte offset `base` of `out`.
pub fn write_ext2(out: &mut File, base: u64, size_bytes: u64, tree: &Tree, label: &str) -> Result<()> {
    let blocks_count = size_bytes / BLOCK;
    let ngroups = (blocks_count + BLOCKS_PER_GROUP - 1) / BLOCKS_PER_GROUP;
    let gdt_blocks = (ngroups * 32 + BLOCK - 1) / BLOCK;

    // Inodes: every node plus reserved ones plus 10% slack, spread evenly over the groups.
    let want = tree.nodes.len() as u64 + FIRST_INO as u64 + tree.nodes.len() as u64 / 10 + 64;
    let mut ipg = (want + ngroups - 1) / ngroups;
    ipg = ((ipg + 31) / 32) * 32; // whole inode-table blocks, and a multiple of 8 for the bitmap
    if ipg > BLOCK * 8 {
        bail!("too many inodes for {} groups", ngroups);
    }
    let itable_blocks = ipg * INODE_SIZE / BLOCK;

    let mut groups = Vec::new();
    for g in 0..ngroups {
        let start = g * BLOCKS_PER_GROUP;
        let len = BLOCKS_PER_GROUP.min(blocks_count - start);
        // [superblock backup][GDT][block bitmap][inode bitmap][inode table][data...]
        let block_bitmap = start + 1 + gdt_blocks;
        let inode_bitmap = block_bitmap + 1;
        let inode_table = inode_bitmap + 1;
        let data_start = inode_table + itable_blocks;
        if data_start >= start + len {
            bail!("group {} too small for its metadata", g);
        }
        let mut used = vec![0u8; BLOCK as usize];
        for b in 0..(data_start - start) {
            set_bit(&mut used, b);
        }
        // Bits past the end of a short last group are marked used.
        for b in len..BLOCKS_PER_GROUP {
            set_bit(&mut used, b);
        }
        groups.push(Group {
            start,
            len,
            block_bitmap,
            inode_bitmap,
            inode_table,
            data_start,
            used,
            inode_used: vec![0u8; BLOCK as usize],
            free_blocks: len - (data_start - start),
            free_inodes: ipg as u32,
            dirs: 0,
        });
    }

    let mut w = Ext2Writer {
        out,
        base,
        groups,
        blocks_count,
        inodes_per_group: ipg as u32,
        itable_blocks,
        gdt_blocks,
        cursor_group: 0,
        cursor_block: 0,
        inode_of: vec![0; tree.nodes.len()],
        archives: HashMap::new(),
    };
    w.cursor_block = w.groups[0].data_start;
    w.write_tree(tree, label)
}

impl<'a> Ext2Writer<'a> {
    fn seek_block(&mut self, block: u64) -> Result<()> {
        self.out.seek(SeekFrom::Start(self.base + block * BLOCK))?;
        Ok(())
    }

    fn write_block(&mut self, block: u64, data: &[u8]) -> Result<()> {
        self.seek_block(block)?;
        self.out.write_all(data)?;
        Ok(())
    }

    fn alloc_block(&mut self) -> Result<u64> {
        loop {
            let g = &mut self.groups[self.cursor_group];
            if self.cursor_block < g.start + g.len {
                let b = self.cursor_block;
                self.cursor_block += 1;
                let bit = b - g.start;
                set_bit(&mut g.used, bit);
                g.free_blocks -= 1;
                return Ok(b);
            }
            if self.cursor_group + 1 >= self.groups.len() {
                bail!("ext2 image is full (increase the rootfs partition size)");
            }
            self.cursor_group += 1;
            self.cursor_block = self.groups[self.cursor_group].data_start;
        }
    }

    fn alloc_inode(&mut self, is_dir: bool) -> Result<u32> {
        // Inodes 1..FIRST_INO are reserved (group 0); regular ones are handed out sequentially.
        for (gi, g) in self.groups.iter_mut().enumerate() {
            if g.free_inodes > 0 {
                let first_free = (0..self.inodes_per_group as u64)
                    .find(|&i| g.inode_used[(i / 8) as usize] & (1 << (i % 8)) == 0)
                    .unwrap();
                set_bit(&mut g.inode_used, first_free);
                g.free_inodes -= 1;
                if is_dir {
                    g.dirs += 1;
                }
                return Ok(gi as u32 * self.inodes_per_group + first_free as u32 + 1);
            }
        }
        bail!("out of inodes")
    }

    fn write_inode(&mut self, ino: u32, raw: &[u8; 128]) -> Result<()> {
        let g = ((ino - 1) / self.inodes_per_group) as usize;
        let idx = ((ino - 1) % self.inodes_per_group) as u64;
        let off = self.groups[g].inode_table * BLOCK + idx * INODE_SIZE;
        self.out.seek(SeekFrom::Start(self.base + off))?;
        self.out.write_all(raw)?;
        Ok(())
    }

    /// Store `data_blocks` (already-written block numbers) in the inode's block map,
    /// allocating indirect blocks as needed. Returns (i_block[15], extra blocks used).
    fn build_block_map(&mut self, data_blocks: &[u32]) -> Result<([u32; 15], u64)> {
        let mut map = [0u32; 15];
        let mut extra = 0u64;
        for (i, &b) in data_blocks.iter().take(12).enumerate() {
            map[i] = b;
        }
        let rest = if data_blocks.len() > 12 { &data_blocks[12..] } else { &[][..] };
        let mut write_ptr_block = |this: &mut Self, ptrs: &[u32]| -> Result<u32> {
            let blk = this.alloc_block()?;
            let mut buf = vec![0u8; BLOCK as usize];
            for (i, p) in ptrs.iter().enumerate() {
                le32(&mut buf, i * 4, *p);
            }
            this.write_block(blk, &buf)?;
            Ok(blk as u32)
        };
        if !rest.is_empty() {
            let single = &rest[..rest.len().min(PTRS as usize)];
            map[12] = write_ptr_block(self, single)?;
            extra += 1;
            let rest2 = &rest[single.len()..];
            if rest2.len() > (PTRS * PTRS) as usize {
                bail!("file too large for double indirect blocks");
            }
            if !rest2.is_empty() {
                // Double indirect: a block of pointers to blocks of pointers.
                let mut l1: Vec<u32> = Vec::new();
                for chunk in rest2.chunks(PTRS as usize).take(PTRS as usize) {
                    l1.push(write_ptr_block(self, chunk)?);
                    extra += 1;
                }
                map[13] = write_ptr_block(self, &l1)?;
                extra += 1;
            }
        }
        Ok((map, extra))
    }

    fn stream_file(&mut self, src: &Source, size: u64, tree: &Tree) -> Result<Vec<u32>> {
        let nblocks = (size + BLOCK - 1) / BLOCK;
        let mut blocks = Vec::with_capacity(nblocks as usize);
        let mut mem_off = 0usize;
        // Take the archive handle out of the cache while streaming (put back below).
        let mut taken: Option<(usize, File)> = None;
        if let Source::Tar(ai, off) = src {
            let mut f = match self.archives.remove(ai) {
                Some(f) => f,
                None => File::open(&tree.archives[*ai])?,
            };
            f.seek(SeekFrom::Start(*off))?;
            taken = Some((*ai, f));
        }
        let mut buf = vec![0u8; BLOCK as usize];
        let mut remaining = size;
        for _ in 0..nblocks {
            let n = remaining.min(BLOCK) as usize;
            buf.iter_mut().for_each(|b| *b = 0);
            match src {
                Source::Mem(data) => {
                    buf[..n].copy_from_slice(&data[mem_off..mem_off + n]);
                    mem_off += n;
                }
                Source::Tar(..) => taken.as_mut().unwrap().1.read_exact(&mut buf[..n])?,
            }
            remaining -= n as u64;
            let blk = self.alloc_block()?;
            self.write_block(blk, &buf)?;
            blocks.push(blk as u32);
        }
        if let Some((ai, f)) = taken {
            self.archives.insert(ai, f);
        }
        Ok(blocks)
    }

    fn dir_blocks(&mut self, entries: &[(String, u32, u8)]) -> Result<Vec<u32>> {
        // entries include "." and "..". Pack into 4 KiB blocks; the last entry of a
        // block owns the remaining space.
        let mut blocks_data: Vec<Vec<u8>> = vec![Vec::new()];
        let mut last_off: Vec<usize> = vec![usize::MAX];
        for (name, ino, ftype) in entries {
            let name_b = name.as_bytes();
            let rec = (8 + name_b.len() + 3) & !3;
            let cur = blocks_data.last().unwrap().len();
            if cur + rec > BLOCK as usize {
                blocks_data.push(Vec::new());
                last_off.push(usize::MAX);
            }
            let bi = blocks_data.len() - 1;
            let d = &mut blocks_data[bi];
            let at = d.len();
            d.extend_from_slice(&ino.to_le_bytes());
            d.extend_from_slice(&(rec as u16).to_le_bytes());
            d.push(name_b.len() as u8);
            d.push(*ftype);
            d.extend_from_slice(name_b);
            d.resize(at + rec, 0);
            last_off[bi] = at;
        }
        let mut out = Vec::new();
        for (bi, mut d) in blocks_data.into_iter().enumerate() {
            let at = last_off[bi];
            let pad = BLOCK as usize - d.len();
            if at != usize::MAX {
                let cur = u16::from_le_bytes([d[at + 4], d[at + 5]]) as usize;
                d[at + 4..at + 6].copy_from_slice(&((cur + pad) as u16).to_le_bytes());
            }
            d.resize(BLOCK as usize, 0);
            let blk = self.alloc_block()?;
            self.write_block(blk, &d)?;
            out.push(blk as u32);
        }
        Ok(out)
    }

    fn write_tree(&mut self, tree: &Tree, label: &str) -> Result<()> {
        // Inode numbers: root is 2; everything else follows the reserved range.
        // Reserve inodes 1..=10 up front.
        for i in 0..(FIRST_INO as u64 - 1) {
            set_bit(&mut self.groups[0].inode_used, i);
        }
        self.groups[0].free_inodes -= FIRST_INO - 1;
        // Root must be inode 2 (already reserved): mark by hand.
        self.inode_of[0] = ROOT_INO;
        self.groups[0].dirs += 1;

        // Allocate inode numbers for all other nodes.
        for i in 1..tree.nodes.len() {
            let is_dir = matches!(tree.nodes[i].kind, Kind::Dir(_));
            self.inode_of[i] = self.alloc_inode(is_dir)?;
        }

        // Parent lookup for ".." entries.
        let mut parent = vec![0usize; tree.nodes.len()];
        for (i, n) in tree.nodes.iter().enumerate() {
            if let Kind::Dir(children) = &n.kind {
                for &c in children.values() {
                    parent[c] = i;
                }
            }
        }
        let mut link_count = vec![1u16; tree.nodes.len()];
        for (i, n) in tree.nodes.iter().enumerate() {
            if let Kind::Dir(children) = &n.kind {
                link_count[i] = 2 + children.values().filter(|&&c| matches!(tree.nodes[c].kind, Kind::Dir(_))).count() as u16;
            }
        }

        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() as u32;
        for i in 0..tree.nodes.len() {
            let n = &tree.nodes[i];
            let ino = self.inode_of[i];
            let mut raw = [0u8; 128];
            let (type_bits, size, blocks512, map): (u16, u64, u64, [u32; 15]) = match &n.kind {
                Kind::Dir(children) => {
                    let mut entries: Vec<(String, u32, u8)> = Vec::new();
                    entries.push((".".into(), ino, 2));
                    entries.push(("..".into(), self.inode_of[parent[i]], 2));
                    for (name, &c) in children {
                        let ft = match tree.nodes[c].kind {
                            Kind::Dir(_) => 2,
                            Kind::File { .. } => 1,
                            Kind::Symlink(_) => 7,
                        };
                        entries.push((name.clone(), self.inode_of[c], ft));
                    }
                    let blocks = self.dir_blocks(&entries)?;
                    let (map, extra) = self.build_block_map(&blocks)?;
                    (S_IFDIR, blocks.len() as u64 * BLOCK, (blocks.len() as u64 + extra) * 8, map)
                }
                Kind::File { src, size } => {
                    let blocks = self.stream_file(src, *size, tree)?;
                    let (map, extra) = self.build_block_map(&blocks)?;
                    (S_IFREG, *size, (blocks.len() as u64 + extra) * 8, map)
                }
                Kind::Symlink(target) => {
                    let t = target.as_bytes();
                    let mut map = [0u32; 15];
                    if t.len() < 60 {
                        // Fast symlink: the target lives inside i_block.
                        let mut bytes = [0u8; 60];
                        bytes[..t.len()].copy_from_slice(t);
                        for k in 0..15 {
                            map[k] = u32::from_le_bytes([bytes[k * 4], bytes[k * 4 + 1], bytes[k * 4 + 2], bytes[k * 4 + 3]]);
                        }
                        (S_IFLNK, t.len() as u64, 0, map)
                    } else {
                        let blk = self.alloc_block()?;
                        let mut buf = vec![0u8; BLOCK as usize];
                        buf[..t.len()].copy_from_slice(t);
                        self.write_block(blk, &buf)?;
                        map[0] = blk as u32;
                        (S_IFLNK, t.len() as u64, 8, map)
                    }
                }
            };
            le16(&mut raw, 0, type_bits | n.mode);
            le16(&mut raw, 2, n.uid as u16);
            le32(&mut raw, 4, size as u32);
            let t = if n.mtime != 0 { n.mtime } else { now };
            le32(&mut raw, 8, t);
            le32(&mut raw, 12, t);
            le32(&mut raw, 16, t);
            le16(&mut raw, 24, n.gid as u16);
            le16(&mut raw, 26, link_count[i]);
            le32(&mut raw, 28, blocks512 as u32);
            for (k, v) in map.iter().enumerate() {
                le32(&mut raw, 40 + k * 4, *v);
            }
            self.write_inode(ino, &raw)?;
        }

        self.write_metadata(label, now)
    }

    fn write_metadata(&mut self, label: &str, now: u32) -> Result<()> {
        let total_inodes = self.inodes_per_group as u64 * self.groups.len() as u64;
        let free_blocks: u64 = self.groups.iter().map(|g| g.free_blocks).sum();
        let free_inodes: u64 = self.groups.iter().map(|g| g.free_inodes as u64).sum();

        // Group descriptor table
        let mut gdt = vec![0u8; (self.gdt_blocks * BLOCK) as usize];
        for (i, g) in self.groups.iter().enumerate() {
            let o = i * 32;
            le32(&mut gdt, o, g.block_bitmap as u32);
            le32(&mut gdt, o + 4, g.inode_bitmap as u32);
            le32(&mut gdt, o + 8, g.inode_table as u32);
            le16(&mut gdt, o + 12, g.free_blocks as u16);
            le16(&mut gdt, o + 14, g.free_inodes as u16);
            le16(&mut gdt, o + 16, g.dirs as u16);
        }

        for gi in 0..self.groups.len() {
            // Superblock backup (group 0's lives at byte offset 1024 of block 0).
            let mut sb = vec![0u8; 1024];
            le32(&mut sb, 0, total_inodes as u32);
            le32(&mut sb, 4, self.blocks_count as u32);
            le32(&mut sb, 8, 0);
            le32(&mut sb, 12, free_blocks as u32);
            le32(&mut sb, 16, free_inodes as u32);
            le32(&mut sb, 20, 0); // first data block
            le32(&mut sb, 24, 2); // log block size: 1024 << 2 = 4096
            le32(&mut sb, 28, 2);
            le32(&mut sb, 32, BLOCKS_PER_GROUP as u32);
            le32(&mut sb, 36, BLOCKS_PER_GROUP as u32);
            le32(&mut sb, 40, self.inodes_per_group);
            le32(&mut sb, 44, now);
            le32(&mut sb, 48, now);
            le16(&mut sb, 52, 0);
            le16(&mut sb, 54, 0xFFFF);
            le16(&mut sb, 56, 0xEF53);
            le16(&mut sb, 58, 1); // clean
            le16(&mut sb, 60, 1);
            le32(&mut sb, 64, now);
            le32(&mut sb, 76, 1); // rev 1
            le32(&mut sb, 84, FIRST_INO);
            le16(&mut sb, 88, INODE_SIZE as u16);
            le16(&mut sb, 90, gi as u16);
            le32(&mut sb, 96, 0x0002); // incompat: filetype
            sb[104..120].copy_from_slice(&[0x4C, 0x55, 0x4E, 0x49, 0x58, 0x52, 0x4F, 0x4F, 0x54, 0x46, 0x53, 0x00, 0x01, 0x02, 0x03, 0x04]);
            let lb = label.as_bytes();
            sb[120..120 + lb.len().min(16)].copy_from_slice(&lb[..lb.len().min(16)]);

            let g = &self.groups[gi];
            let (start, bb, ib, used, inode_used) = (g.start, g.block_bitmap, g.inode_bitmap, g.used.clone(), g.inode_used.clone());
            let mut blk0 = vec![0u8; BLOCK as usize];
            if gi == 0 {
                blk0[1024..2048].copy_from_slice(&sb);
            } else {
                blk0[..1024].copy_from_slice(&sb);
            }
            self.write_block(start, &blk0)?;
            self.write_block(start + 1, &gdt)?;
            self.write_block(bb, &used)?;
            let mut ibm = inode_used;
            // Inode bitmap bits past inodes_per_group are marked used.
            for b in self.inodes_per_group as u64..BLOCK * 8 {
                set_bit(&mut ibm, b);
            }
            self.write_block(ib, &ibm)?;
        }
        self.out.flush()?;
        let _ = self.itable_blocks;
        Ok(())
    }
}
