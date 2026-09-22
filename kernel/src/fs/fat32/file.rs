//! FAT32 File Handle

use super::cluster::ClusterManager;
use crate::fs::file::{FileHandle, SeekFrom};
use alloc::sync::Arc;
use alloc::vec::Vec;

pub struct Fat32File {
    cluster_mgr: Arc<ClusterManager>,
    start_cluster: u32,
    size: u64,
    position: u64,
    /// Chain position of the last cluster visited, so sequential reads don't rewalk the FAT.
    cursor_idx: u64,
    cursor_cluster: u32,
    /// One-cluster read cache: (chain index, contents).
    cache_idx: Option<u64>,
    cache: Vec<u8>,
}

impl Fat32File {
    pub fn new(cluster_mgr: Arc<ClusterManager>, start_cluster: u32, size: u64) -> Self {
        Self {
            cluster_mgr,
            start_cluster,
            size,
            position: 0,
            cursor_idx: 0,
            cursor_cluster: start_cluster,
            cache_idx: None,
            cache: Vec::new(),
        }
    }

    /// Make the cache hold cluster number `idx` of the file's chain.
    fn load_cluster(&mut self, idx: u64) -> Result<(), &'static str> {
        if self.cache_idx == Some(idx) {
            return Ok(());
        }
        if idx < self.cursor_idx {
            self.cursor_idx = 0;
            self.cursor_cluster = self.start_cluster;
        }
        while self.cursor_idx < idx {
            self.cursor_cluster = self.cluster_mgr.next_cluster(self.cursor_cluster).ok_or("FAT32 chain ended early")?;
            self.cursor_idx += 1;
        }
        if self.cache.len() != self.cluster_mgr.cluster_bytes() {
            self.cache = alloc::vec![0u8; self.cluster_mgr.cluster_bytes()];
        }
        self.cluster_mgr.read_cluster(self.cursor_cluster, &mut self.cache)?;
        self.cache_idx = Some(idx);
        Ok(())
    }
}

impl FileHandle for Fat32File {
    /// Streams from the cluster chain; files are never loaded whole into kernel memory.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        if self.position >= self.size {
            return Ok(0); // EOF
        }
        let cluster_bytes = self.cluster_mgr.cluster_bytes() as u64;
        let to_read = (buf.len() as u64).min(self.size - self.position) as usize;
        let mut done = 0;
        while done < to_read {
            let idx = self.position / cluster_bytes;
            let within = (self.position % cluster_bytes) as usize;
            self.load_cluster(idx)?;
            let n = (to_read - done).min(cluster_bytes as usize - within);
            buf[done..done + n].copy_from_slice(&self.cache[within..within + n]);
            done += n;
            self.position += n as u64;
        }
        Ok(done)
    }

    fn write(&mut self, _buf: &[u8]) -> Result<usize, &'static str> {
        Err("FAT32 file writing not yet supported on read-only mount")
    }

    fn seek(&mut self, pos: SeekFrom) -> Result<u64, &'static str> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::Current(offset) => self.position as i64 + offset,
            SeekFrom::End(offset) => self.size as i64 + offset,
        };

        if new_pos < 0 {
            return Err("Invalid seek position");
        }

        self.position = new_pos as u64;
        Ok(self.position)
    }

    fn size(&self) -> u64 {
        self.size
    }
}
