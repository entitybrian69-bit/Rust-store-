// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! Where files live: the replay library, retention, export and rename.
//!
//! The library is a folder of `.vrc` files. Everything the browser shows comes
//! from each file's `metaData.json` (never from decoding the recording), which
//! is what keeps opening a folder of two-hour replays instant.

use std::path::{Path, PathBuf};

use crate::config::StorageSettings;
use crate::hud::format_duration;
use crate::package::{PackageError, ReplayPackage};

/// The folder layout Volt Lapse uses inside the game directory.
#[derive(Clone, Debug, PartialEq)]
pub struct StoragePaths {
    /// Game folder (the one holding `options.txt`).
    pub game_dir: PathBuf,
    /// `config/voltlapse`.
    pub config_dir: PathBuf,
    /// Where replays are written.
    pub replays_dir: PathBuf,
    /// Where exports (`/voltlapse export`) go.
    pub exports_dir: PathBuf,
    /// Crash-recovery journals.
    pub journal_dir: PathBuf,
    /// Cached thumbnails.
    pub cache_dir: PathBuf,
}

impl StoragePaths {
    /// Resolve the layout from the game folder and the storage settings.
    ///
    /// A relative `settings.directory` is joined to the game folder; an absolute
    /// one is used as-is (on Android the game folder is already app-private).
    pub fn resolve(game_dir: &Path, settings: &StorageSettings) -> Self {
        let configured = Path::new(&settings.directory);
        let replays_dir = if configured.is_absolute() {
            configured.to_path_buf()
        } else {
            game_dir.join(configured)
        };
        let config_dir = game_dir.join("config").join("voltlapse");
        Self {
            game_dir: game_dir.to_path_buf(),
            exports_dir: config_dir.join("exports"),
            journal_dir: config_dir.join("journal"),
            cache_dir: config_dir.join("cache"),
            config_dir,
            replays_dir,
        }
    }

    /// The main config document.
    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.json")
    }

    /// The folder holding named profiles.
    pub fn profiles_dir(&self) -> PathBuf {
        self.config_dir.join("profiles")
    }

    /// The folder holding overlay images such as a watermark.
    pub fn overlays_dir(&self) -> PathBuf {
        self.config_dir.join("overlays")
    }

    /// Where a replay's recovery journal goes while it is being recorded.
    pub fn journal_file(&self, session: &str) -> PathBuf {
        self.journal_dir.join(format!("{session}.journal"))
    }

    /// Create every folder the engine writes to.
    pub fn ensure(&self) -> std::io::Result<()> {
        for dir in [&self.replays_dir, &self.config_dir, &self.exports_dir, &self.journal_dir, &self.cache_dir] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    /// Clean a user-supplied name into something safe to use as a file name.
    /// Keeps letters, digits, spaces, dots, dashes and underscores.
    pub fn safe_file_name(name: &str) -> String {
        let cleaned: String = name
            .chars()
            .map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_' | '(' | ')') { c } else { '_' })
            .collect();
        let trimmed = cleaned.trim().trim_matches('.').to_owned();
        if trimmed.is_empty() {
            "replay".to_owned()
        } else {
            trimmed.chars().take(96).collect()
        }
    }
}

/// One row in the replay browser.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplayEntry {
    pub path: PathBuf,
    pub name: String,
    pub size_bytes: u64,
    /// Duration in ticks, from the metadata.
    pub duration_ticks: u64,
    pub tick_rate: u16,
    pub mc_version: String,
    pub date_unix: i64,
    pub players: usize,
    pub has_thumbnail: bool,
    /// Why the file could not be read, when it could not.
    pub error: Option<String>,
}

impl ReplayEntry {
    pub fn duration_text(&self) -> String {
        format_duration(self.duration_ticks, self.tick_rate)
    }

    /// The line the browser shows under the name.
    pub fn subtitle(&self) -> String {
        if let Some(error) = &self.error {
            return error.clone();
        }
        format!(
            "{} · {} · {} · {} player{}",
            self.duration_text(),
            self.mc_version,
            human_size(self.size_bytes),
            self.players,
            if self.players == 1 { "" } else { "s" }
        )
    }
}

/// `12.4 MB`, for the browser.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// The replay folder, as an index the browser can filter and sort.
#[derive(Clone, Debug, Default)]
pub struct ReplayLibrary {
    pub dir: PathBuf,
    entries: Vec<ReplayEntry>,
}

impl ReplayLibrary {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into(), entries: Vec::new() }
    }

    pub fn entries(&self) -> &[ReplayEntry] {
        &self.entries
    }

    /// Rescan the folder, reading each file's metadata only.
    pub fn refresh(&mut self) -> std::io::Result<usize> {
        self.entries.clear();
        if !self.dir.exists() {
            std::fs::create_dir_all(&self.dir)?;
        }
        for entry in std::fs::read_dir(&self.dir)? {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("vrc") {
                continue;
            }
            self.entries.push(read_entry(&path));
        }
        self.entries.sort_by(|a, b| b.date_unix.cmp(&a.date_unix));
        Ok(self.entries.len())
    }

    /// Build an index from files already read (used by tests and by the
    /// browser's "import folder" action).
    pub fn from_entries(dir: impl Into<PathBuf>, mut entries: Vec<ReplayEntry>) -> Self {
        entries.sort_by(|a, b| b.date_unix.cmp(&a.date_unix));
        Self { dir: dir.into(), entries }
    }

    /// Entries whose name, tag or player matches a search string.
    pub fn search(&self, query: &str) -> Vec<&ReplayEntry> {
        let needle = query.trim().to_ascii_lowercase();
        if needle.is_empty() {
            return self.entries.iter().collect();
        }
        self.entries
            .iter()
            .filter(|entry| {
                entry.name.to_ascii_lowercase().contains(&needle)
                    || entry.mc_version.to_ascii_lowercase().contains(&needle)
            })
            .collect()
    }

    /// Sort by duration, longest first.
    pub fn by_duration(&self) -> Vec<&ReplayEntry> {
        let mut entries: Vec<&ReplayEntry> = self.entries.iter().collect();
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.duration_ticks));
        entries
    }

    /// Rename a replay: the display name inside the metadata and, when asked,
    /// the file on disk.
    pub fn rename(&mut self, index: usize, new_name: &str, rename_file: bool) -> Result<PathBuf, StorageError> {
        let entry = self.entries.get(index).ok_or(StorageError::NoEntEntry)?;
        let bytes = std::fs::read(&entry.path)?;
        let mut package = ReplayPackage::open(&bytes).map_err(StorageError::Package)?;
        package.metadata.name = new_name.to_owned();
        let output = if rename_file {
            let target = self.dir.join(format!("{}.vrc", StoragePaths::safe_file_name(new_name)));
            if target.exists() && target != entry.path {
                return Err(StorageError::AlreadyExists(target));
            }
            target
        } else {
            entry.path.clone()
        };
        std::fs::write(&output, package.to_bytes().map_err(StorageError::Package)?)?;
        if rename_file && output != entry.path {
            std::fs::remove_file(&entry.path)?;
        }
        self.refresh()?;
        Ok(output)
    }

    /// Delete a replay from disk.
    pub fn delete(&mut self, index: usize) -> Result<(), StorageError> {
        let entry = self.entries.get(index).ok_or(StorageError::NoEntEntry)?;
        std::fs::remove_file(&entry.path)?;
        self.entries.remove(index);
        Ok(())
    }

    /// Copy a replay into the exports folder under a shareable name.
    pub fn export(&self, index: usize, exports_dir: &Path, name: &str) -> Result<PathBuf, StorageError> {
        let entry = self.entries.get(index).ok_or(StorageError::NoEntEntry)?;
        std::fs::create_dir_all(exports_dir)?;
        let target = exports_dir.join(format!("{}.vrc", StoragePaths::safe_file_name(name)));
        std::fs::copy(&entry.path, &target)?;
        Ok(target)
    }

    /// Apply the retention settings: keep at most `max` replays and delete
    /// those older than `keep_days`. Returns how many were deleted.
    pub fn apply_retention(&mut self, settings: &StorageSettings, now_unix: i64) -> usize {
        let mut deleted = 0;
        if settings.keep_days > 0 {
            let cutoff = now_unix - settings.keep_days as i64 * 86_400;
            let stale: Vec<PathBuf> = self
                .entries
                .iter()
                .filter(|entry| entry.date_unix < cutoff && entry.error.is_none())
                .map(|entry| entry.path.clone())
                .collect();
            for path in stale {
                if std::fs::remove_file(&path).is_ok() {
                    deleted += 1;
                }
            }
        }
        if settings.max_replays > 0 {
            self.refresh().ok();
            // Entries are newest-first, so the tail is the oldest.
            while self.entries.len() > settings.max_replays as usize {
                let path = self.entries.last().map(|entry| entry.path.clone());
                match path {
                    Some(path) => {
                        if std::fs::remove_file(&path).is_ok() {
                            deleted += 1;
                        }
                        self.entries.pop();
                    }
                    None => break,
                }
            }
        }
        if deleted > 0 {
            self.refresh().ok();
        }
        deleted
    }

    /// Total bytes the folder uses.
    pub fn total_bytes(&self) -> u64 {
        self.entries.iter().map(|entry| entry.size_bytes).sum()
    }
}

fn read_entry(path: &Path) -> ReplayEntry {
    let size_bytes = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    let file_name = path.file_stem().and_then(|name| name.to_str()).unwrap_or("replay").to_owned();
    let date_unix = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    match std::fs::read(path) {
        Ok(bytes) => match ReplayPackage::read_metadata(&bytes) {
            Ok(metadata) => ReplayEntry {
                path: path.to_path_buf(),
                name: if metadata.name.is_empty() { file_name } else { metadata.name },
                size_bytes,
                duration_ticks: metadata.duration_ticks,
                tick_rate: metadata.tick_rate,
                mc_version: metadata.mc_version,
                date_unix: if metadata.date_unix != 0 { metadata.date_unix } else { date_unix },
                players: metadata.players.len(),
                has_thumbnail: bytes.windows(4).any(|window| window == [0x89, b'P', b'N', b'G']),
                error: None,
            },
            Err(error) => broken_entry(path, &file_name, size_bytes, date_unix, error.to_string()),
        },
        Err(error) => broken_entry(path, &file_name, size_bytes, date_unix, error.to_string()),
    }
}

fn broken_entry(path: &Path, name: &str, size_bytes: u64, date_unix: i64, error: String) -> ReplayEntry {
    ReplayEntry {
        path: path.to_path_buf(),
        name: name.to_owned(),
        size_bytes,
        duration_ticks: 0,
        tick_rate: 20,
        mc_version: "?".into(),
        date_unix,
        players: 0,
        has_thumbnail: false,
        error: Some(error),
    }
}

/// Why a library operation failed.
#[derive(Clone, Debug, PartialEq)]
pub enum StorageError {
    Io(String),
    Package(PackageError),
    /// The index has no entry at that position.
    NoEntEntry,
    /// A file with that name is already there.
    AlreadyExists(PathBuf),
}

impl From<std::io::Error> for StorageError {
    fn from(error: std::io::Error) -> Self {
        StorageError::Io(error.to_string())
    }
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::Io(why) => write!(f, "file error: {why}"),
            StorageError::Package(error) => write!(f, "{error}"),
            StorageError::NoEntEntry => f.write_str("no replay at that position"),
            StorageError::AlreadyExists(path) => write!(f, "{} already exists", path.display()),
        }
    }
}

impl std::error::Error for StorageError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::package::ReplayMetadata;
    use crate::recording::{Recorder, RecordingOptions};
    use crate::wire::Vec3;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let unique = format!(
                "voltlapse-test-{tag}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
            );
            let path = std::env::temp_dir().join(unique);
            std::fs::create_dir_all(&path).expect("temp dir");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write_replay(dir: &Path, name: &str, ticks: u64, date: i64) {
        let mut recorder = Recorder::new(RecordingOptions::default(), "26.3", "test");
        for tick in 0..ticks {
            recorder.record(Event::EntityMove { id: 1, pos: Vec3::new(tick as f64, 0.0, 0.0), yaw: 0.0, pitch: 0.0 });
            recorder.end_tick();
        }
        let (bytes, _) = recorder.finish();
        let recording = crate::recording::Recording::parse(bytes).unwrap();
        let mut metadata = ReplayMetadata::from_recording(&recording, name, date);
        metadata.players.push(crate::package::PlayerInfo { uuid: "u".into(), name: "Alex".into() });
        let package = ReplayPackage { metadata, recording, thumbnail: Some(vec![0x89, b'P', b'N', b'G']) };
        std::fs::write(dir.join(format!("{}.vrc", StoragePaths::safe_file_name(name))), package.to_bytes().unwrap()).unwrap();
    }

    #[test]
    fn paths_resolve_relative_and_absolute() {
        let game = Path::new("/games/horizon");
        let paths = StoragePaths::resolve(game, &StorageSettings::default());
        assert_eq!(paths.replays_dir, game.join("voltlapse/replays"));
        assert_eq!(paths.config_file(), game.join("config/voltlapse/config.json"));
        assert_eq!(paths.profiles_dir(), game.join("config/voltlapse/profiles"));

        let absolute = StorageSettings { directory: "/sdcard/voltlapse".into(), ..StorageSettings::default() };
        let paths = StoragePaths::resolve(game, &absolute);
        assert_eq!(paths.replays_dir, PathBuf::from("/sdcard/voltlapse"));
    }

    #[test]
    fn file_names_are_cleaned() {
        assert_eq!(StoragePaths::safe_file_name("Clutch!! 1v3?.vrc"), "Clutch__ 1v3_.vrc");
        assert_eq!(StoragePaths::safe_file_name("   "), "replay");
        assert_eq!(StoragePaths::safe_file_name("..."), "replay");
        assert!(StoragePaths::safe_file_name(&"x".repeat(200)).len() <= 96);
    }

    #[test]
    fn a_folder_of_replays_indexes_with_metadata() {
        let temp = TempDir::new("index");
        write_replay(&temp.0, "First", 20, 1_700_000_000);
        write_replay(&temp.0, "Second", 60, 1_700_000_500);
        std::fs::write(temp.0.join("broken.vrc"), b"garbage").unwrap();
        std::fs::write(temp.0.join("notes.txt"), b"ignore me").unwrap();

        let mut library = ReplayLibrary::new(&temp.0);
        assert_eq!(library.refresh().unwrap(), 3, "two replays and one broken file");
        assert_eq!(library.entries()[0].name, "Second", "newest first");
        assert_eq!(library.entries()[0].duration_text(), "00:03.00");
        assert!(library.entries()[0].subtitle().contains("1 player"));
        let broken = library.entries().iter().find(|entry| entry.name == "broken").unwrap();
        assert!(broken.error.is_some());
        assert!(broken.subtitle().contains("zip") || broken.subtitle().contains("not a"));

        assert_eq!(library.search("second").len(), 1);
        assert_eq!(library.search("").len(), 3);
        assert_eq!(library.by_duration()[0].name, "Second");
        assert!(library.total_bytes() > 0);
    }

    #[test]
    fn rename_and_delete_work_on_disk() {
        let temp = TempDir::new("rename");
        write_replay(&temp.0, "Old name", 20, 1_700_000_000);
        let mut library = ReplayLibrary::new(&temp.0);
        library.refresh().unwrap();

        let path = library.rename(0, "New name", true).unwrap();
        assert!(path.ends_with("New name.vrc"));
        assert!(path.exists());
        library.refresh().unwrap();
        assert_eq!(library.entries()[0].name, "New name");

        library.delete(0).unwrap();
        assert!(library.entries().is_empty());
        assert!(!path.exists());
        assert_eq!(library.delete(5), Err(StorageError::NoEntEntry));
    }

    #[test]
    fn renaming_onto_an_existing_file_is_refused() {
        let temp = TempDir::new("collide");
        write_replay(&temp.0, "A", 20, 1_700_000_000);
        write_replay(&temp.0, "B", 20, 1_700_000_100);
        let mut library = ReplayLibrary::new(&temp.0);
        library.refresh().unwrap();
        let result = library.rename(0, "A", true);
        assert!(matches!(result, Err(StorageError::AlreadyExists(_))));
    }

    #[test]
    fn export_copies_into_the_exports_folder() {
        let temp = TempDir::new("export");
        write_replay(&temp.0, "Keep", 20, 1_700_000_000);
        let mut library = ReplayLibrary::new(&temp.0);
        library.refresh().unwrap();
        let exports = temp.0.join("exports");
        let copied = library.export(0, &exports, "shared replay").unwrap();
        assert!(copied.exists());
        assert!(copied.ends_with("shared replay.vrc"));
    }

    #[test]
    fn retention_prunes_the_oldest_replays() {
        let temp = TempDir::new("retention");
        for (index, name) in ["one", "two", "three"].iter().enumerate() {
            write_replay(&temp.0, name, 20, 1_700_000_000 + index as i64 * 1_000);
        }
        let mut library = ReplayLibrary::new(&temp.0);
        library.refresh().unwrap();
        let settings = StorageSettings { max_replays: 2, ..StorageSettings::default() };
        let deleted = library.apply_retention(&settings, 1_700_000_000);
        assert_eq!(deleted, 1);
        assert_eq!(library.entries().len(), 2);
        assert!(library.entries().iter().all(|entry| entry.name != "one"));
    }

    #[test]
    fn age_based_retention_removes_old_files() {
        let temp = TempDir::new("age");
        write_replay(&temp.0, "ancient", 20, 1_000_000_000);
        write_replay(&temp.0, "fresh", 20, 1_700_000_000);
        let mut library = ReplayLibrary::new(&temp.0);
        library.refresh().unwrap();
        let settings = StorageSettings { keep_days: 30, ..StorageSettings::default() };
        let deleted = library.apply_retention(&settings, 1_700_000_000);
        assert_eq!(deleted, 1);
        assert_eq!(library.entries().len(), 1);
        assert_eq!(library.entries()[0].name, "fresh");
    }

    #[test]
    fn human_sizes_read_naturally() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1_536), "1.5 KB");
        assert_eq!(human_size(12_400_000), "11.8 MB");
    }
}
