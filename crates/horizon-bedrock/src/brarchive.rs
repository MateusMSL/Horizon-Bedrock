//! The `.brarchive` container: many small files (a pack's JSON) in one, so a pack opens with a
//! handful of reads. Layout, little-endian:
//!
//! ```text
//! 0   u64   magic 0x267052A0B125277D
//! 8   u32   entry count N
//! 12  u32   version (1)
//! 16  N x 256-byte entries: u8 name length, 247 bytes of name, u32 offset, u32 size
//! 16 + 256 N  the data; offsets count from here
//! ```
//!
//! The archive is plain, not encrypted. Where its files belong in a pack is the pack's business
//! ([`crate::pack`]): `a/__brarchive/b/c.brarchive` holds the files of `a/b/c/`.

use crate::{Error, Result};

/// The first eight bytes of every archive.
pub const MAGIC: [u8; 8] = [0x7D, 0x27, 0x25, 0xB1, 0xA0, 0x52, 0x70, 0x26];
const HEADER: usize = 16;
const ENTRY: usize = 256;
const NAME_MAX: usize = 247;

/// One file in an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    /// Offset of the file's data from the start of the data area.
    pub offset: u32,
    pub size: u32,
}

/// An archive's index over its bytes.
#[derive(Debug)]
pub struct Archive<'a> {
    data: &'a [u8],
    base: usize,
    entries: Vec<Entry>,
}

impl<'a> Archive<'a> {
    /// Reads the index. Every entry must lie inside `data`.
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        if data.len() < HEADER || data[..8] != MAGIC {
            return Err(Error::Archive("not a brarchive (bad magic)".into()));
        }
        let count = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
        let base = count
            .checked_mul(ENTRY)
            .and_then(|n| n.checked_add(HEADER))
            .filter(|&b| b <= data.len())
            .ok_or_else(|| Error::Archive(format!("{count} entries do not fit")))?;
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let at = HEADER + i * ENTRY;
            let len = data[at] as usize;
            if len > NAME_MAX {
                return Err(Error::Archive(format!("entry {i}: name length {len}")));
            }
            let name = String::from_utf8_lossy(&data[at + 1..at + 1 + len]).into_owned();
            let word = |o: usize| {
                u32::from_le_bytes([
                    data[at + o],
                    data[at + o + 1],
                    data[at + o + 2],
                    data[at + o + 3],
                ])
            };
            let (offset, size) = (word(248), word(252));
            let end = base as u64 + u64::from(offset) + u64::from(size);
            if end > data.len() as u64 {
                return Err(Error::Archive(format!("entry {name}: data past the end")));
            }
            entries.push(Entry { name, offset, size });
        }
        Ok(Archive {
            data,
            base,
            entries,
        })
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// An entry's bytes.
    pub fn read(&self, entry: &Entry) -> &'a [u8] {
        let start = self.base + entry.offset as usize;
        &self.data[start..start + entry.size as usize]
    }
}

/// An archive holding `files` (name, bytes). Used by tests and tools; names longer than the
/// format's 247 bytes are refused.
pub fn write(files: &[(&str, &[u8])]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&(files.len() as u32).to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    let mut offset = 0u32;
    for (name, bytes) in files {
        if name.len() > NAME_MAX {
            return Err(Error::Archive(format!("name too long: {name}")));
        }
        let mut entry = [0u8; ENTRY];
        entry[0] = name.len() as u8;
        entry[1..1 + name.len()].copy_from_slice(name.as_bytes());
        entry[248..252].copy_from_slice(&offset.to_le_bytes());
        entry[252..256].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&entry);
        offset += bytes.len() as u32;
    }
    for (_, bytes) in files {
        out.extend_from_slice(bytes);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let bytes = write(&[("a.json", b"{}"), ("dir/b.json", b"[1]"), ("empty", b"")]).unwrap();
        let archive = Archive::parse(&bytes).unwrap();
        let names: Vec<_> = archive.entries().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["a.json", "dir/b.json", "empty"]);
        assert_eq!(archive.read(&archive.entries()[1]), b"[1]");
        assert_eq!(archive.read(&archive.entries()[2]), b"");
    }

    #[test]
    fn empty_archive_is_sixteen_bytes() {
        let bytes = write(&[]).unwrap();
        assert_eq!(bytes.len(), 16);
        assert!(Archive::parse(&bytes).unwrap().entries().is_empty());
    }

    #[test]
    fn rejects_bad_input() {
        assert!(Archive::parse(b"nope").is_err());
        let mut bytes = write(&[("a", b"xyz")]).unwrap();
        bytes.truncate(bytes.len() - 1);
        assert!(Archive::parse(&bytes).is_err());
        let mut huge = MAGIC.to_vec();
        huge.extend_from_slice(&u32::MAX.to_le_bytes());
        huge.extend_from_slice(&1u32.to_le_bytes());
        assert!(Archive::parse(&huge).is_err());
    }

    #[test]
    fn long_names_refused() {
        let name = "x".repeat(248);
        assert!(write(&[(name.as_str(), b"")]).is_err());
    }
}
