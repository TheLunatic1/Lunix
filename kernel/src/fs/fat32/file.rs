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
    cached_data: Option<Vec<u8>>,
}

impl Fat32File {
    pub fn new(cluster_mgr: Arc<ClusterManager>, start_cluster: u32, size: u64) -> Self {
        Self {
            cluster_mgr,
            start_cluster,
            size,
            position: 0,
            cached_data: None,
        }
    }

    fn ensure_loaded(&mut self) -> Result<&[u8], &'static str> {
        if self.cached_data.is_none() {
            let data = self.cluster_mgr.read_all_clusters(self.start_cluster)?;
            let actual_len = data.len().min(self.size as usize);
            self.cached_data = Some(data[..actual_len].to_vec());
        }
        Ok(self.cached_data.as_ref().unwrap())
    }
}

impl FileHandle for Fat32File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        let pos = self.position as usize;
        let file_data = self.ensure_loaded()?;

        if pos >= file_data.len() {
            return Ok(0); // EOF
        }

        let remaining = file_data.len() - pos;
        let to_read = buf.len().min(remaining);

        buf[..to_read].copy_from_slice(&file_data[pos..pos + to_read]);
        self.position = (pos + to_read) as u64;
        Ok(to_read)
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
