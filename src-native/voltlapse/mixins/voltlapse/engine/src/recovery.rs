// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! Crash recovery.
//!
//! A journal is a file of snapshots appended while recording. Each record is
//! `magic len crc32 tick payload`, so a crash mid-write leaves a truncated tail
//! that the reader skips instead of a file that cannot be opened. On the next
//! launch the newest complete record becomes a replay with the title
//! "(recovered)", which is what a player expects after the game dies.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::crc::crc32;

/// Record magic.
pub const RECORD_MAGIC: [u8; 4] = *b"VLJ1";
/// Largest snapshot the reader accepts (64 MiB).
pub const MAX_RECORD: usize = 64 * 1024 * 1024;

/// Appends snapshots to a journal while recording.
#[derive(Debug)]
pub struct Journal {
    path: PathBuf,
    file: File,
    /// Snapshots written so far.
    pub written: u64,
}

impl Journal {
    /// Open (or create) a journal for a session.
    pub fn open(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self { path, file, written: 0 })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one snapshot. Errors are returned rather than ignored: the caller
    /// shows "recovery unavailable" instead of silently promising safety.
    pub fn write_snapshot(&mut self, tick: u64, bytes: &[u8]) -> std::io::Result<()> {
        let mut record = Vec::with_capacity(bytes.len() + 20);
        record.extend_from_slice(&RECORD_MAGIC);
        record.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        record.extend_from_slice(&crc32(bytes).to_le_bytes());
        record.extend_from_slice(&tick.to_le_bytes());
        record.extend_from_slice(bytes);
        self.file.write_all(&record)?;
        self.file.flush()?;
        self.written += 1;
        Ok(())
    }

    /// Delete the journal once the replay is safely saved.
    pub fn finish(self) -> std::io::Result<()> {
        let path = self.path.clone();
        drop(self.file);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}

/// The newest complete snapshot in a journal.
#[derive(Clone, Debug, PartialEq)]
pub struct RecoveredRecording {
    pub tick: u64,
    pub bytes: Vec<u8>,
}

/// Read a journal, returning the last intact snapshot.
pub fn recover(path: &Path) -> std::io::Result<Option<RecoveredRecording>> {
    if !path.exists() {
        return Ok(None);
    }
    let mut file = File::open(path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    let mut best: Option<RecoveredRecording> = None;
    let mut at = 0usize;
    while at + 20 <= data.len() {
        if data[at..at + 4] != RECORD_MAGIC {
            // A torn record: stop, keeping what was whole before it.
            break;
        }
        let len = u32::from_le_bytes([data[at + 4], data[at + 5], data[at + 6], data[at + 7]]) as usize;
        if len > MAX_RECORD {
            break;
        }
        let crc = u32::from_le_bytes([data[at + 8], data[at + 9], data[at + 10], data[at + 11]]);
        let tick = u64::from_le_bytes([
            data[at + 12],
            data[at + 13],
            data[at + 14],
            data[at + 15],
            data[at + 16],
            data[at + 17],
            data[at + 18],
            data[at + 19],
        ]);
        let start = at + 20;
        if start + len > data.len() {
            break;
        }
        let payload = &data[start..start + len];
        if crc32(payload) == crc {
            best = Some(RecoveredRecording { tick, bytes: payload.to_vec() });
        }
        at = start + len;
    }
    Ok(best)
}

/// Journals left behind by a previous run.
pub fn pending_journals(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("journal"))
        .collect();
    paths.sort();
    paths
}

/// Truncate a journal to its last complete record, so a resumed session keeps
/// appending after a torn write.
pub fn repair(path: &Path) -> std::io::Result<u64> {
    let Some(_last) = recover(path)? else {
        return Ok(0);
    };
    let mut file = OpenOptions::new().write(true).open(path)?;
    let total = file.metadata()?.len();
    // Find the offset of the last good record by scanning again: records are
    // self-describing, so the last one starts where its length says it does.
    let mut data = Vec::new();
    File::open(path)?.read_to_end(&mut data)?;
    let mut at = 0usize;
    let mut last_start = 0usize;
    while at + 20 <= data.len() && data[at..at + 4] == RECORD_MAGIC {
        let len = u32::from_le_bytes([data[at + 4], data[at + 5], data[at + 6], data[at + 7]]) as usize;
        let crc = u32::from_le_bytes([data[at + 8], data[at + 9], data[at + 10], data[at + 11]]);
        let start = at + 20;
        if start + len > data.len() || crc32(&data[start..start + len]) != crc {
            break;
        }
        last_start = at;
        at = start + len;
    }
    let _ = last_start;
    let end = at as u64;
    if end < total {
        file.set_len(end)?;
        file.seek(SeekFrom::Start(end))?;
    }
    Ok(_last.tick)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let unique = format!(
                "voltlapse-journal-{tag}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
            );
            let path = std::env::temp_dir().join(unique);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_newest_snapshot_wins() {
        let temp = TempDir::new("newest");
        let path = temp.0.join("session.journal");
        let mut journal = Journal::open(&path).unwrap();
        journal.write_snapshot(100, b"first").unwrap();
        journal.write_snapshot(200, b"second").unwrap();
        assert_eq!(journal.written, 2);
        let recovered = recover(&path).unwrap().unwrap();
        assert_eq!(recovered.tick, 200);
        assert_eq!(recovered.bytes, b"second");
        journal.finish().unwrap();
        assert!(!path.exists(), "a finished journal is removed");
        assert!(recover(&path).unwrap().is_none());
    }

    #[test]
    fn a_torn_tail_is_skipped() {
        let temp = TempDir::new("torn");
        let path = temp.0.join("session.journal");
        let mut journal = Journal::open(&path).unwrap();
        journal.write_snapshot(50, b"good snapshot").unwrap();
        journal.write_snapshot(60, b"will be torn").unwrap();
        drop(journal);

        let bytes = std::fs::read(&path).unwrap();
        std::fs::write(&path, &bytes[..bytes.len() - 5]).unwrap();
        let recovered = recover(&path).unwrap().unwrap();
        assert_eq!(recovered.tick, 50);
        assert_eq!(recovered.bytes, b"good snapshot");

        // Repair truncates the torn record and reports the last good tick.
        assert_eq!(repair(&path).unwrap(), 50);
        let repaired = std::fs::read(&path).unwrap();
        assert_eq!(repaired.len(), 20 + "good snapshot".len(), "only the whole record is kept");
        let mut journal = Journal::open(&path).unwrap();
        journal.write_snapshot(70, b"third").unwrap();
        assert_eq!(recover(&path).unwrap().unwrap().tick, 70);
    }

    #[test]
    fn corrupt_payloads_are_not_recovered() {
        let temp = TempDir::new("corrupt");
        let path = temp.0.join("session.journal");
        let mut journal = Journal::open(&path).unwrap();
        journal.write_snapshot(10, b"payload").unwrap();
        drop(journal);
        let mut bytes = std::fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        std::fs::write(&path, &bytes).unwrap();
        assert!(recover(&path).unwrap().is_none());
    }

    #[test]
    fn pending_journals_are_listed_sorted() {
        let temp = TempDir::new("pending");
        std::fs::write(temp.0.join("b.journal"), b"").unwrap();
        std::fs::write(temp.0.join("a.journal"), b"").unwrap();
        std::fs::write(temp.0.join("notes.txt"), b"").unwrap();
        let found = pending_journals(&temp.0);
        assert_eq!(found.len(), 2);
        assert!(found[0].ends_with("a.journal"));
        assert!(pending_journals(&temp.0.join("missing")).is_empty());
    }
}
