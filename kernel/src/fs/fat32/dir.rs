use alloc::string::String;
use alloc::vec::Vec;

pub const ATTR_READ_ONLY: u8 = 0x01;
pub const ATTR_HIDDEN: u8 = 0x02;
pub const ATTR_SYSTEM: u8 = 0x04;
pub const ATTR_VOLUME_ID: u8 = 0x08;
pub const ATTR_DIRECTORY: u8 = 0x10;
pub const ATTR_ARCHIVE: u8 = 0x20;
pub const ATTR_LFN: u8 = 0x0F;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RawDirEntry {
    pub name: [u8; 11],
    pub attr: u8,
    pub ntres: u8,
    pub create_time_tenth: u8,
    pub create_time: u16,
    pub create_date: u16,
    pub last_access_date: u16,
    pub cluster_high: u16,
    pub write_time: u16,
    pub write_date: u16,
    pub cluster_low: u16,
    pub file_size: u32,
}

#[derive(Debug, Clone)]
pub struct ParsedFatEntry {
    pub name: String,
    pub is_dir: bool,
    pub start_cluster: u32,
    pub file_size: u32,
}

pub fn parse_directory(data: &[u8]) -> Vec<ParsedFatEntry> {
    let mut entries = Vec::new();
    let entry_size = 32;
    let count = data.len() / entry_size;

    let mut lfn_parts: Vec<(u8, String)> = Vec::new();

    for i in 0..count {
        let offset = i * entry_size;
        let first_byte = data[offset];

        if first_byte == 0x00 {
            // No more entries
            break;
        }

        if first_byte == 0xE5 {
            // Deleted entry
            lfn_parts.clear();
            continue;
        }

        let attr = data[offset + 11];

        if attr == ATTR_LFN {
            // Long File Name entry
            let seq = first_byte & 0x1F;
            let mut name_buf = Vec::new();

            // Extract UTF-16 characters
            let chars_pos = [
                (1, 10),  // 5 chars
                (14, 12), // 6 chars
                (28, 4),  // 2 chars
            ];

            for (start, len) in chars_pos {
                for c_idx in (start..start + len).step_by(2) {
                    let ch = (data[offset + c_idx] as u16) | ((data[offset + c_idx + 1] as u16) << 8);
                    if ch == 0 || ch == 0xFFFF {
                        break;
                    }
                    if let Some(c) = char::from_u32(ch as u32) {
                        name_buf.push(c);
                    }
                }
            }

            let part_str: String = name_buf.into_iter().collect();
            lfn_parts.push((seq, part_str));
            continue;
        }

        if (attr & ATTR_VOLUME_ID) != 0 {
            // Skip volume labels
            lfn_parts.clear();
            continue;
        }

        // Standard 8.3 Directory Entry
        let raw = unsafe { &*(data[offset..].as_ptr() as *const RawDirEntry) };

        let name = if !lfn_parts.is_empty() {
            // Assemble LFN in reverse sequence order
            lfn_parts.sort_by(|a, b| b.0.cmp(&a.0));
            let mut full_name = String::new();
            for (_, part) in lfn_parts.iter() {
                full_name.push_str(part);
            }
            lfn_parts.clear();
            full_name
        } else {
            // Parse 8.3 Short Name
            let base = core::str::from_utf8(&raw.name[0..8]).unwrap_or("").trim();
            let ext = core::str::from_utf8(&raw.name[8..11]).unwrap_or("").trim();
            if ext.is_empty() {
                String::from(base)
            } else {
                alloc::format!("{}.{}", base, ext)
            }
        };

        if name == "." || name == ".." {
            continue;
        }

        let is_dir = (attr & ATTR_DIRECTORY) != 0;
        let start_cluster = ((raw.cluster_high as u32) << 16) | (raw.cluster_low as u32);
        let file_size = raw.file_size;

        entries.push(ParsedFatEntry {
            name,
            is_dir,
            start_cluster,
            file_size,
        });
    }

    entries
}
