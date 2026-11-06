// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! The replay file: a `.vrc` package holding `metaData.json` and
//! `recording.tmc`.
//!
//! That is the shape Replay Mod's `.mcpr` files have, so a Volt Lapse replay is
//! a zip anyone can open, a metadata file any tool can read, and a recording
//! stream this engine reads. The metadata carries everything that is not a
//! packet: who recorded it, when, how long, which camera paths and overlays the
//! creator made, the bookmarks and the thumbnail.

use crate::bookmarks::Bookmarks;
use crate::camera::CameraMode;
use crate::hud::HudOverlay;
use crate::json::{self, Json};
use crate::keyframe::CameraPath;
use crate::recording::{Recording, RecordingError};
use crate::zipstore::{self, Entry, Stamp, ZipError};

/// Container version of the metadata document.
pub const METADATA_VERSION: u32 = 1;
/// What `fileFormat` says, matching Replay Mod's own field so shared tooling
/// recognises the folder layout.
pub const FILE_FORMAT: &str = "MCPR";
pub const FILE_FORMAT_VERSION: u32 = 14;
/// Entry names inside the package.
pub const METADATA_ENTRY: &str = "metaData.json";
pub const RECORDING_ENTRY: &str = "recording.tmc";
pub const THUMBNAIL_ENTRY: &str = "thumbnail.png";

/// A player who appears in the recording.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerInfo {
    pub uuid: String,
    pub name: String,
}

/// Everything a replay knows about itself.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplayMetadata {
    pub version: u32,
    /// Display name, which the browser and the rename dialog edit.
    pub name: String,
    pub description: String,
    /// Unix seconds the recording started.
    pub date_unix: i64,
    pub duration_ticks: u64,
    pub tick_rate: u16,
    pub mc_version: String,
    pub protocol: u32,
    pub generator: String,
    /// The recording player's UUID.
    pub self_uuid: String,
    pub players: Vec<PlayerInfo>,
    /// `""` for a single-player world.
    pub server_name: String,
    pub singleplayer: bool,
    /// Tags the browser filters on (`pvp`, `build`, ...).
    pub tags: Vec<String>,
    /// Camera paths saved with the replay.
    pub camera_paths: Vec<CameraPath>,
    /// Which path the editor opens.
    pub active_path: usize,
    pub overlays: HudOverlay,
    pub bookmarks: Bookmarks,
    /// Render settings last used for this replay.
    pub render: Option<crate::render::RenderSettings>,
    /// Free-form notes from the metadata editor.
    pub notes: String,
}

impl Default for ReplayMetadata {
    fn default() -> Self {
        Self {
            version: METADATA_VERSION,
            name: "Replay".into(),
            description: String::new(),
            date_unix: 0,
            duration_ticks: 0,
            tick_rate: 20,
            mc_version: "26.3".into(),
            protocol: 0,
            generator: format!("Volt Lapse {}", env!("CARGO_PKG_VERSION")),
            self_uuid: String::new(),
            players: Vec::new(),
            server_name: String::new(),
            singleplayer: true,
            tags: Vec::new(),
            camera_paths: Vec::new(),
            active_path: 0,
            overlays: HudOverlay::default(),
            bookmarks: Bookmarks::new(),
            render: None,
            notes: String::new(),
        }
    }
}

impl ReplayMetadata {
    /// Fill the fields that come from the recording itself.
    pub fn from_recording(recording: &Recording, name: impl Into<String>, date_unix: i64) -> Self {
        let header = recording.header();
        Self {
            name: name.into(),
            duration_ticks: recording.tick_count(),
            tick_rate: header.tick_rate,
            mc_version: header.mc_version.clone(),
            date_unix,
            ..Self::default()
        }
    }

    /// Duration in seconds.
    pub fn duration_seconds(&self) -> f64 {
        self.duration_ticks as f64 / self.tick_rate.max(1) as f64
    }

    /// The camera path the editor should open.
    pub fn active_camera(&self) -> Option<&CameraPath> {
        self.camera_paths.get(self.active_path).or_else(|| self.camera_paths.first())
    }

    pub fn to_json(&self) -> Json {
        Json::Object(vec![
            ("version".into(), Json::int(self.version as i64)),
            ("fileFormat".into(), Json::String(FILE_FORMAT.into())),
            ("fileFormatVersion".into(), Json::int(FILE_FORMAT_VERSION as i64)),
            ("name".into(), Json::String(self.name.clone())),
            ("description".into(), Json::String(self.description.clone())),
            ("date".into(), Json::int(self.date_unix)),
            ("duration".into(), Json::int(self.duration_ticks as i64)),
            ("tickRate".into(), Json::int(self.tick_rate as i64)),
            ("mcversion".into(), Json::String(self.mc_version.clone())),
            ("protocol".into(), Json::int(self.protocol as i64)),
            ("generator".into(), Json::String(self.generator.clone())),
            ("selfId".into(), Json::String(self.self_uuid.clone())),
            (
                "players".into(),
                Json::Array(
                    self.players
                        .iter()
                        .map(|p| Json::Object(vec![("uuid".into(), Json::String(p.uuid.clone())), ("name".into(), Json::String(p.name.clone()))]))
                        .collect(),
                ),
            ),
            ("serverName".into(), Json::String(self.server_name.clone())),
            ("singleplayer".into(), Json::Bool(self.singleplayer)),
            ("tags".into(), Json::Array(self.tags.iter().cloned().map(Json::String).collect())),
            (
                "cameraPaths".into(),
                Json::Array(self.camera_paths.iter().map(CameraPath::to_json).collect()),
            ),
            ("activePath".into(), Json::int(self.active_path as i64)),
            ("overlays".into(), self.overlays.to_json()),
            ("bookmarks".into(), self.bookmarks.to_json()),
            (
                "render".into(),
                self.render.as_ref().map(|render| render.to_json()).unwrap_or(Json::Null),
            ),
            ("notes".into(), Json::String(self.notes.clone())),
        ])
    }

    pub fn from_json(json: &Json) -> Result<Self, PackageError> {
        let version = json.get("version").and_then(Json::as_u64).unwrap_or(1) as u32;
        if version > METADATA_VERSION {
            return Err(PackageError::NewerMetadata(version));
        }
        let mut metadata = ReplayMetadata {
            version: METADATA_VERSION,
            name: json.get("name").and_then(Json::as_str).unwrap_or("Replay").to_owned(),
            description: json.get("description").and_then(Json::as_str).unwrap_or("").to_owned(),
            date_unix: json.get("date").and_then(Json::as_i64).unwrap_or(0),
            duration_ticks: json.get("duration").and_then(Json::as_u64).unwrap_or(0),
            tick_rate: json.get("tickRate").and_then(Json::as_u64).unwrap_or(20) as u16,
            mc_version: json.get("mcversion").and_then(Json::as_str).unwrap_or("26.3").to_owned(),
            protocol: json.get("protocol").and_then(Json::as_u64).unwrap_or(0) as u32,
            generator: json.get("generator").and_then(Json::as_str).unwrap_or("Volt Lapse").to_owned(),
            self_uuid: json.get("selfId").and_then(Json::as_str).unwrap_or("").to_owned(),
            server_name: json.get("serverName").and_then(Json::as_str).unwrap_or("").to_owned(),
            singleplayer: json.get("singleplayer").and_then(Json::as_bool).unwrap_or(true),
            notes: json.get("notes").and_then(Json::as_str).unwrap_or("").to_owned(),
            ..ReplayMetadata::default()
        };
        if let Some(players) = json.get("players").and_then(Json::as_array) {
            for player in players {
                if let Some(uuid) = player.get("uuid").and_then(Json::as_str) {
                    metadata.players.push(PlayerInfo {
                        uuid: uuid.to_owned(),
                        name: player.get("name").and_then(Json::as_str).unwrap_or(uuid).to_owned(),
                    });
                }
            }
        }
        if let Some(tags) = json.get("tags").and_then(Json::as_array) {
            metadata.tags = tags.iter().filter_map(Json::as_str).map(str::to_owned).collect();
        }
        if let Some(paths) = json.get("cameraPaths").and_then(Json::as_array) {
            for path in paths {
                if let Some(path) = CameraPath::from_json(path) {
                    metadata.camera_paths.push(path);
                }
            }
        }
        metadata.active_path = json.get("activePath").and_then(Json::as_u64).unwrap_or(0) as usize;
        if metadata.active_path >= metadata.camera_paths.len() {
            metadata.active_path = 0;
        }
        if let Some(overlays) = json.get("overlays") {
            metadata.overlays = HudOverlay::from_json(overlays);
        }
        if let Some(bookmarks) = json.get("bookmarks") {
            metadata.bookmarks = Bookmarks::from_json(bookmarks);
        }
        if let Some(render) = json.get("render").filter(|r| **r != Json::Null) {
            metadata.render = Some(crate::render::RenderSettings::from_json(render));
        }
        Ok(metadata)
    }

    /// A one-line summary for the browser and the log.
    pub fn summary(&self) -> String {
        format!(
            "{} — {} — {} — {} player{}",
            self.name,
            crate::hud::format_duration(self.duration_ticks, self.tick_rate),
            self.mc_version,
            self.players.len(),
            if self.players.len() == 1 { "" } else { "s" }
        )
    }
}

/// A loaded replay file.
#[derive(Clone, Debug)]
pub struct ReplayPackage {
    pub metadata: ReplayMetadata,
    pub recording: Recording,
    /// Thumbnail bytes, when the file has one.
    pub thumbnail: Option<Vec<u8>>,
}

impl ReplayPackage {
    /// Serialise to `.vrc` bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, PackageError> {
        let mut entries = vec![
            Entry::new(METADATA_ENTRY, self.metadata.to_json().to_pretty().into_bytes()),
            Entry::new(RECORDING_ENTRY, self.recording.bytes().to_vec()),
        ];
        if let Some(thumbnail) = &self.thumbnail {
            entries.push(Entry::new(THUMBNAIL_ENTRY, thumbnail.clone()));
        }
        zipstore::write(&entries, Stamp::REPRODUCIBLE).map_err(PackageError::Zip)
    }

    /// Parse a `.vrc` file.
    pub fn open(bytes: &[u8]) -> Result<Self, PackageError> {
        let entries = zipstore::read(bytes).map_err(PackageError::Zip)?;
        let metadata_entry = entries
            .iter()
            .find(|entry| entry.name == METADATA_ENTRY)
            .ok_or_else(|| PackageError::MissingEntry(METADATA_ENTRY.into()))?;
        let recording_entry = entries
            .iter()
            .find(|entry| entry.name == RECORDING_ENTRY)
            .ok_or_else(|| PackageError::MissingEntry(RECORDING_ENTRY.into()))?;
        let text = std::str::from_utf8(metadata_entry.data).map_err(|_| PackageError::BadMetadata("not UTF-8".into()))?;
        let json = json::parse(text).map_err(|error| PackageError::BadMetadata(error.to_string()))?;
        let metadata = ReplayMetadata::from_json(&json)?;
        let recording = Recording::parse(recording_entry.data.to_vec()).map_err(PackageError::Recording)?;
        let thumbnail = entries
            .iter()
            .find(|entry| entry.name == THUMBNAIL_ENTRY)
            .map(|entry| entry.data.to_vec());
        Ok(Self { metadata, recording, thumbnail })
    }

    /// Read only the metadata, without decoding the recording. The browser uses
    /// this to list a folder of large replays quickly.
    pub fn read_metadata(bytes: &[u8]) -> Result<ReplayMetadata, PackageError> {
        let entries = zipstore::read(bytes).map_err(PackageError::Zip)?;
        let metadata_entry = entries
            .iter()
            .find(|entry| entry.name == METADATA_ENTRY)
            .ok_or_else(|| PackageError::MissingEntry(METADATA_ENTRY.into()))?;
        let text = std::str::from_utf8(metadata_entry.data).map_err(|_| PackageError::BadMetadata("not UTF-8".into()))?;
        let json = json::parse(text).map_err(|error| PackageError::BadMetadata(error.to_string()))?;
        ReplayMetadata::from_json(&json)
    }
}

/// Why a replay file could not be read or written.
#[derive(Clone, Debug, PartialEq)]
pub enum PackageError {
    Zip(ZipError),
    Recording(RecordingError),
    MissingEntry(String),
    BadMetadata(String),
    NewerMetadata(u32),
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageError::Zip(error) => write!(f, "{error}"),
            PackageError::Recording(error) => write!(f, "{error}"),
            PackageError::MissingEntry(name) => write!(f, "replay is missing '{name}'"),
            PackageError::BadMetadata(why) => write!(f, "replay metadata is unreadable: {why}"),
            PackageError::NewerMetadata(version) => write!(f, "replay metadata version {version} is newer than this build"),
        }
    }
}

impl std::error::Error for PackageError {}

/// Where a replay came from, for the recorder to stamp the metadata.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecordingOrigin {
    pub server_name: String,
    pub singleplayer: bool,
    pub self_uuid: String,
    pub self_name: String,
    pub mc_version: String,
    pub protocol: u32,
    pub camera_mode: Option<CameraMode>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::recording::{Recorder, RecordingOptions};
    use crate::wire::Vec3;

    fn package() -> ReplayPackage {
        let mut recorder = Recorder::new(RecordingOptions::default(), "26.3", "test");
        for tick in 0..40 {
            recorder.record(Event::EntityMove { id: 1, pos: Vec3::new(tick as f64, 64.0, 0.0), yaw: 0.0, pitch: 0.0 });
            recorder.end_tick();
        }
        let (bytes, _) = recorder.finish();
        let recording = Recording::parse(bytes).expect("parses");
        let mut metadata = ReplayMetadata::from_recording(&recording, "Bedwars clutch", 1_700_000_000);
        metadata.players.push(PlayerInfo { uuid: "uuid-alex".into(), name: "Alex".into() });
        metadata.self_uuid = "uuid-alex".into();
        metadata.tags.push("pvp".into());
        let mut path = CameraPath::new("orbit");
        path.add(crate::keyframe::Keyframe::new(0, Vec3::new(0.0, 70.0, 0.0), 0.0, 0.0));
        path.add(crate::keyframe::Keyframe::new(40, Vec3::new(20.0, 70.0, 0.0), 90.0, 0.0));
        metadata.camera_paths.push(path);
        metadata.bookmarks.add(crate::bookmarks::Bookmark::new(20, "fight"));
        metadata.overlays.add_card(crate::hud::TextCard { id: "title".into(), text: "Clutch".into(), ..crate::hud::TextCard::default() });
        ReplayPackage { metadata, recording, thumbnail: Some(vec![0x89, b'P', b'N', b'G']) }
    }

    #[test]
    fn a_package_round_trips() {
        let original = package();
        let bytes = original.to_bytes().expect("writes");
        let opened = ReplayPackage::open(&bytes).expect("opens");
        assert_eq!(opened.metadata, original.metadata);
        assert_eq!(opened.recording.tick_count(), 40);
        assert!(opened.thumbnail.is_some());
        assert_eq!(opened.metadata.name, "Bedwars clutch");
        assert_eq!(opened.metadata.camera_paths.len(), 1);
        assert!(opened.metadata.active_camera().is_some());
        assert_eq!(opened.metadata.bookmarks.len(), 1);
        assert_eq!(opened.metadata.overlays.cards.len(), 1);
        assert_eq!(opened.metadata.duration_seconds(), 2.0);
        assert!(opened.metadata.summary().contains("00:02.00"));
    }

    #[test]
    fn metadata_can_be_read_without_the_recording() {
        let bytes = package().to_bytes().expect("writes");
        let metadata = ReplayPackage::read_metadata(&bytes).expect("reads");
        assert_eq!(metadata.players.len(), 1);
        assert_eq!(metadata.tags, vec!["pvp".to_owned()]);
    }

    #[test]
    fn a_file_missing_its_parts_is_reported() {
        let only_metadata = zipstore::write(
            &[Entry::new(METADATA_ENTRY, ReplayMetadata::default().to_json().to_compact().into_bytes())],
            Stamp::REPRODUCIBLE,
        )
        .unwrap();
        assert!(matches!(
            ReplayPackage::open(&only_metadata),
            Err(PackageError::MissingEntry(name)) if name == RECORDING_ENTRY
        ));
        assert!(matches!(ReplayPackage::open(b"not a zip"), Err(PackageError::Zip(_))));
    }

    #[test]
    fn newer_metadata_is_refused_with_a_version() {
        let mut json = ReplayMetadata::default().to_json();
        json.set("version", Json::int(99));
        assert_eq!(ReplayMetadata::from_json(&json), Err(PackageError::NewerMetadata(99)));
    }

    #[test]
    fn metadata_survives_a_partial_document() {
        let json = json::parse(r#"{"name":"hand made","duration":200,"tickRate":10}"#).unwrap();
        let metadata = ReplayMetadata::from_json(&json).expect("reads");
        assert_eq!(metadata.name, "hand made");
        assert_eq!(metadata.tick_rate, 10);
        assert_eq!(metadata.duration_seconds(), 20.0);
        assert!(metadata.camera_paths.is_empty());
        assert!(metadata.active_camera().is_none());
    }

    #[test]
    fn writing_twice_gives_identical_bytes() {
        let package = package();
        assert_eq!(package.to_bytes().unwrap(), package.to_bytes().unwrap());
    }
}
