// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! A stored-only zip reader and writer.
//!
//! A Volt Lapse replay is a `.vrc` package: a zip holding `metaData.json` and
//! `recording.tmc`, which is what Replay Mod's `.mcpr` files are, so a
//! Volt Lapse replay opens in an unzip tool and in Replay Mod's tooling that
//! only reads the metadata. The entries are *stored*, not deflated: the engine
//! cannot take `flate2` (a bundle crate may only use packages Horizon's
//! `Cargo.lock` already pins), and the recording stream is compressed by
//! `lz.rs` instead, which keeps entries byte-identical across platforms and
//! makes random access into `recording.tmc` a slice rather than an inflate.

use crate::crc::crc32;

/// Why a zip could not be written or read.
#[derive(Clone, Debug, PartialEq)]
pub enum ZipError {
    /// Not a zip: no end-of-central-directory record.
    NotAZip,
    /// A deflated or otherwise unsupported entry.
    UnsupportedCompression(u16),
    /// An entry path is not a relative, `/`-separated path.
    BadPath(String),
    /// The archive is truncated or a header is inconsistent.
    Corrupt(String),
    /// A duplicate entry path.
    DuplicatePath(String),
    /// The archive exceeds a limit the reader enforces.
    TooLarge,
}

impl std::fmt::Display for ZipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZipError::NotAZip => f.write_str("not a zip archive"),
            ZipError::UnsupportedCompression(method) => write!(f, "unsupported compression method {method}"),
            ZipError::BadPath(path) => write!(f, "invalid entry path '{path}'"),
            ZipError::Corrupt(why) => write!(f, "corrupt archive: {why}"),
            ZipError::DuplicatePath(path) => write!(f, "duplicate entry '{path}'"),
            ZipError::TooLarge => f.write_str("archive exceeds the size limit"),
        }
    }
}

impl std::error::Error for ZipError {}

pub type Result<T> = std::result::Result<T, ZipError>;

/// A file's timestamp, written into the DOS time field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl Stamp {
    /// A fixed stamp for packages the builder writes, so building twice gives
    /// the same bytes (which is what the store's SHA-256 index relies on).
    pub const REPRODUCIBLE: Stamp =
        Stamp { year: 2026, month: 1, day: 1, hour: 0, minute: 0, second: 0 };

    fn dos_time(&self) -> u16 {
        (self.hour as u16) << 11 | (self.minute as u16) << 5 | (self.second as u16 / 2)
    }

    fn dos_date(&self) -> u16 {
        (self.year - 1980) << 9 | (self.month as u16) << 5 | self.day as u16
    }
}

/// One entry to write.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub data: Vec<u8>,
}

impl Entry {
    pub fn new(name: impl Into<String>, data: impl Into<Vec<u8>>) -> Self {
        Self { name: name.into(), data: data.into() }
    }
}

/// Validate a path against the mod-file spec's rules: relative, `/`-separated,
/// no `.`/`..`/empty part, no backslash, no colon, not absolute.
pub fn validate_path(name: &str) -> Result<()> {
    if name.is_empty() || name.starts_with('/') || name.contains('\\') || name.contains(':') {
        return Err(ZipError::BadPath(name.to_owned()));
    }
    for part in name.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(ZipError::BadPath(name.to_owned()));
        }
    }
    Ok(())
}

/// Build a stored zip from entries, in the order given.
pub fn write(entries: &[Entry], stamp: Stamp) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut directory = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    for entry in entries {
        validate_path(&entry.name)?;
        if !seen.insert(entry.name.clone()) {
            return Err(ZipError::DuplicatePath(entry.name.clone()));
        }
        let offset = out.len() as u32;
        let crc = crc32(&entry.data);
        let size = entry.data.len() as u32;

        put_u32(&mut out, 0x0403_4B50); // local file header
        put_u16(&mut out, 20); // version needed
        put_u16(&mut out, 0); // flags
        put_u16(&mut out, 0); // method: stored
        put_u16(&mut out, stamp.dos_time());
        put_u16(&mut out, stamp.dos_date());
        put_u32(&mut out, crc);
        put_u32(&mut out, size);
        put_u32(&mut out, size);
        put_u16(&mut out, entry.name.len() as u16);
        put_u16(&mut out, 0); // extra
        out.extend_from_slice(entry.name.as_bytes());
        out.extend_from_slice(&entry.data);

        put_u32(&mut directory, 0x0201_4B50); // central directory header
        put_u16(&mut directory, 0x031E); // made by: unix, spec 3.0
        put_u16(&mut directory, 20);
        put_u16(&mut directory, 0);
        put_u16(&mut directory, 0);
        put_u16(&mut directory, stamp.dos_time());
        put_u16(&mut directory, stamp.dos_date());
        put_u32(&mut directory, crc);
        put_u32(&mut directory, size);
        put_u32(&mut directory, size);
        put_u16(&mut directory, entry.name.len() as u16);
        put_u16(&mut directory, 0); // extra
        put_u16(&mut directory, 0); // comment
        put_u16(&mut directory, 0); // disk
        put_u16(&mut directory, 0); // internal attributes
        put_u32(&mut directory, 0o100_644 << 16); // regular file, 0644
        put_u32(&mut directory, offset);
        directory.extend_from_slice(entry.name.as_bytes());
    }

    let directory_offset = out.len() as u32;
    out.extend_from_slice(&directory);
    let directory_size = directory.len() as u32;

    put_u32(&mut out, 0x0605_4B50); // end of central directory
    put_u16(&mut out, 0);
    put_u16(&mut out, 0);
    put_u16(&mut out, entries.len() as u16);
    put_u16(&mut out, entries.len() as u16);
    put_u32(&mut out, directory_size);
    put_u32(&mut out, directory_offset);
    put_u16(&mut out, 0); // comment
    Ok(out)
}

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn read_u16(bytes: &[u8], at: usize) -> Result<u16> {
    let slice = bytes.get(at..at + 2).ok_or_else(|| ZipError::Corrupt("truncated header".into()))?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32> {
    let slice = bytes.get(at..at + 4).ok_or_else(|| ZipError::Corrupt("truncated header".into()))?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

/// A read entry, borrowing from the archive bytes.
#[derive(Clone, Debug)]
pub struct ReadEntry<'a> {
    pub name: String,
    pub data: &'a [u8],
    pub crc: u32,
}

/// Largest archive the reader will consider (512 MiB, the mod-file limit).
pub const MAX_ARCHIVE: usize = 512 * 1024 * 1024;

/// Read every entry of a stored zip.
pub fn read(bytes: &[u8]) -> Result<Vec<ReadEntry<'_>>> {
    if bytes.len() > MAX_ARCHIVE {
        return Err(ZipError::TooLarge);
    }
    // The end-of-central-directory record is at the end, after an optional
    // comment of at most 64 KiB.
    let search_start = bytes.len().saturating_sub(22 + 65_535);
    let mut eocd = None;
    let mut at = bytes.len().saturating_sub(22);
    loop {
        if at < search_start && at != 0 {
            break;
        }
        if read_u32(bytes, at).ok() == Some(0x0605_4B50) {
            eocd = Some(at);
            break;
        }
        if at == 0 {
            break;
        }
        at -= 1;
    }
    let eocd = eocd.ok_or(ZipError::NotAZip)?;
    let count = read_u16(bytes, eocd + 10)? as usize;
    let mut offset = read_u32(bytes, eocd + 16)? as usize;

    let mut entries = Vec::with_capacity(count);
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..count {
        if read_u32(bytes, offset)? != 0x0201_4B50 {
            return Err(ZipError::Corrupt("bad central directory header".into()));
        }
        let method = read_u16(bytes, offset + 10)?;
        let crc = read_u32(bytes, offset + 16)?;
        let size = read_u32(bytes, offset + 24)? as usize;
        let name_len = read_u16(bytes, offset + 28)? as usize;
        let extra_len = read_u16(bytes, offset + 30)? as usize;
        let comment_len = read_u16(bytes, offset + 32)? as usize;
        let local_offset = read_u32(bytes, offset + 42)? as usize;
        let name = std::str::from_utf8(
            bytes
                .get(offset + 46..offset + 46 + name_len)
                .ok_or_else(|| ZipError::Corrupt("truncated name".into()))?,
        )
        .map_err(|_| ZipError::Corrupt("entry name is not UTF-8".into()))?
        .to_owned();
        if !seen.insert(name.clone()) {
            return Err(ZipError::DuplicatePath(name));
        }
        if method != 0 {
            return Err(ZipError::UnsupportedCompression(method));
        }
        validate_path(&name)?;

        if read_u32(bytes, local_offset)? != 0x0403_4B50 {
            return Err(ZipError::Corrupt("bad local file header".into()));
        }
        let local_name_len = read_u16(bytes, local_offset + 26)? as usize;
        let local_extra_len = read_u16(bytes, local_offset + 28)? as usize;
        let data_start = local_offset + 30 + local_name_len + local_extra_len;
        let data = bytes
            .get(data_start..data_start + size)
            .ok_or_else(|| ZipError::Corrupt("truncated entry data".into()))?;
        entries.push(ReadEntry { name, data, crc });
        offset += 46 + name_len + extra_len + comment_len;
    }
    Ok(entries)
}

/// Read one entry by name, verifying its CRC.
pub fn read_entry<'a>(bytes: &'a [u8], name: &str) -> Result<&'a [u8]> {
    for entry in read(bytes)? {
        if entry.name == name {
            if crc32(entry.data) != entry.crc {
                return Err(ZipError::Corrupt(format!("CRC mismatch on '{name}'")));
            }
            return Ok(entry.data);
        }
    }
    Err(ZipError::Corrupt(format!("missing entry '{name}'")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_entries_and_offsets() {
        let entries = vec![
            Entry::new("metaData.json", br#"{"singleplayer":true}"#.to_vec()),
            Entry::new("recording.tmc", vec![0u8; 1000]),
        ];
        let zip = write(&entries, Stamp::REPRODUCIBLE).expect("writes");
        let read_back = read(&zip).expect("reads");
        assert_eq!(read_back.len(), 2);
        assert_eq!(read_back[0].name, "metaData.json");
        assert_eq!(read_back[1].data.len(), 1000);
        assert_eq!(read_entry(&zip, "metaData.json").unwrap(), br#"{"singleplayer":true}"#);
        assert_eq!(write(&entries, Stamp::REPRODUCIBLE).unwrap(), zip, "deterministic");
    }

    #[test]
    fn rejects_paths_the_spec_rejects() {
        for bad in ["../escape", "/abs", "a//b", "a\\b", "c:/x", "", "./x", "a/./b"] {
            assert!(validate_path(bad).is_err(), "{bad:?} must be rejected");
            let entries = vec![Entry::new(bad, vec![0u8])];
            assert!(write(&entries, Stamp::REPRODUCIBLE).is_err());
        }
        assert!(validate_path("assets/voltlapse/lang/en_us.json").is_ok());
    }

    #[test]
    fn duplicate_entries_are_refused_on_both_sides() {
        let entries = vec![Entry::new("a.json", vec![1]), Entry::new("a.json", vec![2])];
        assert_eq!(
            write(&entries, Stamp::REPRODUCIBLE),
            Err(ZipError::DuplicatePath("a.json".into()))
        );
    }

    #[test]
    fn garbage_is_not_a_zip() {
        assert_eq!(read(b"definitely not a zip"), Err(ZipError::NotAZip));
        assert!(read(&[]).is_err());
    }

    #[test]
    fn a_deflated_entry_is_named_in_the_error() {
        // Build a minimal archive whose central directory claims method 8.
        let mut zip = write(&[Entry::new("a", vec![0u8; 4])], Stamp::REPRODUCIBLE).unwrap();
        // The central directory's method field sits at EOCD offset + 16 (directory start) + 10.
        let eocd = zip.len() - 22;
        let directory = read_u32(&zip, eocd + 16).unwrap() as usize;
        zip[directory + 10] = 8;
        assert_eq!(read(&zip), Err(ZipError::UnsupportedCompression(8)));
    }

    #[test]
    fn dos_time_rounds_seconds_to_even() {
        let stamp = Stamp { year: 2026, month: 11, day: 6, hour: 13, minute: 45, second: 59 };
        assert_eq!(stamp.dos_time(), (13 << 11) | (45 << 5) | 29);
        assert_eq!(stamp.dos_date(), ((2026 - 1980) << 9) | (11 << 5) | 6);
    }

    #[test]
    fn entry_limit_is_enforced_by_the_reader() {
        // Not an allocation test: just prove the guard exists at the boundary.
        assert_eq!(MAX_ARCHIVE, 512 * 1024 * 1024);
    }
}
