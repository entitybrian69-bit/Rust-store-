// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! # Volt Lapse engine
//!
//! Replay recording, playback, cinematic camera paths and offline rendering for
//! [Horizon](https://github.com/horizon-rust/horizon), the Rust Minecraft 26.3
//! client, on PC and Android.
//!
//! The crate is deliberately free of game types and of dependencies: it is
//! staged into Horizon's own locked workspace as a bundle crate by
//! `horizon-mixins`, so it may only use what that `Cargo.lock` already pins.
//! Everything here is therefore pure Rust plus `std`, and everything here is
//! testable with `cargo test`.
//!
//! ## Layout
//!
//! | Module | What it owns |
//! |---|---|
//! | [`wire`], [`json`], [`crc`], [`zipstore`], [`png`], [`lz`] | the bytes: encoding, containers, compression, screenshots |
//! | [`event`], [`recording`] | what a tick records and how a recording is stored and seeked |
//! | [`package`], [`storage`], [`bookmarks`], [`recovery`] | replay files, the library, crash recovery |
//! | [`playback`] | reconstructing the world and driving the transport |
//! | [`keyframe`], [`interp`], [`camera`] | camera paths, easing, shake, modes, matrices |
//! | [`render`] | offline render settings, the queue, post-processing |
//! | [`hud`] | HUD and overlay composition, fades, layout |
//! | [`config`], [`keybinds`], [`perf`], [`platform`] | settings, keys, budgets, PC vs Android |
//! | [`lang`] | the mod's own translation table |
//! | [`command`], [`api`], [`sync`] | `/voltlapse`, the hook API, shared sessions |
//!
//! ## The one object the hooks hold
//!
//! [`Engine`] ties those modules together: it is what `runtime.rs` stores for
//! the duration of the game session and what the hook helpers in `hooks/` call.
//!
//! ## Licence
//!
//! LGPL-3.0-or-later, with attribution to the Replay Mod authors
//! (Johni0702 et al.); see `LICENSE` and `THIRD_PARTY_NOTICES.md` beside this
//! mod's sources.

pub mod api;
pub mod bookmarks;
pub mod camera;
pub mod command;
pub mod config;
pub mod crc;
pub mod event;
pub mod hud;
pub mod interp;
pub mod json;
pub mod keybinds;
pub mod keyframe;
pub mod lang;
pub mod lz;
pub mod package;
pub mod perf;
pub mod png;
pub mod platform;
pub mod playback;
pub mod recording;
pub mod recovery;
pub mod render;
pub mod storage;
pub mod sync;
pub mod wire;
pub mod zipstore;

use std::path::{Path, PathBuf};

use api::{ApiEvent, ModApi};
use bookmarks::Bookmark;
use camera::{CameraMode, CameraRig, CameraTarget};
use config::Config;
use event::Event;
use hud::HudOverlay;
use keyframe::Keyframe;
use package::{RecordingOrigin, ReplayMetadata, ReplayPackage};
use perf::Budget;
use platform::Platform;
use playback::Playback;
use recording::{Recorder, Recording, RecordingOptions};
use render::{RenderJob, RenderQueue};
use storage::{ReplayLibrary, StoragePaths};

/// The mod id: settings folder, log name, namespace.
pub const ID: &str = "voltlapse";
/// The display name.
pub const NAME: &str = "Volt Lapse";
/// The version, kept in step with `horizon-native.json`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The description shown on the Mods and settings pages.
pub const DESCRIPTION: &str =
    "Record your world, replay it, fly a cinematic camera path and render it to video. Replay Mod's features, for Horizon on PC and Android.";
/// Prefix of every translation key.
pub const TRANSLATION_PREFIX: &str = "voltlapse.";
/// Size of the engine's own settings area in the game's Mods list.
pub const SETTINGS_PAGE: &str = "voltlapse";

/// The engine: one per game session, held by the mod's runtime module.
pub struct Engine {
    pub config: Config,
    pub platform: Platform,
    pub paths: StoragePaths,
    pub library: ReplayLibrary,
    pub api: ModApi,
    pub rig: CameraRig,
    pub overlay: HudOverlay,
    pub queue: RenderQueue,
    pub budget: Budget,
    /// Bookmarks of the replay being played or edited.
    pub bookmarks: bookmarks::Bookmarks,
    recorder: Option<Recorder>,
    playback: Option<Playback>,
    recording_origin: RecordingOrigin,
    /// The tick the current recording started at, for timestamps.
    recording_started_unix: i64,
    /// Where the current recording will be written.
    recording_target: Option<PathBuf>,
    /// Recovered snapshot waiting for the player's decision.
    pub recovered: Option<ReplayPackage>,
    /// The strings the mod's own screens draw.
    pub catalog: lang::Catalog,
}

impl Engine {
    /// Build the engine for a game folder.
    pub fn new(config: Config, game_dir: &Path) -> Self {
        let platform = Platform::detect();
        let paths = StoragePaths::resolve(game_dir, &config.storage);
        let budget = Budget::new(config.performance.clone());
        Self {
            library: ReplayLibrary::new(paths.replays_dir.clone()),
            overlay: HudOverlay { settings: config.hud, ..HudOverlay::default() },
            rig: CameraRig {
                mode: config.camera.mode.clone(),
                path: None,
                shake: config.camera.shake,
                base_fov: config.camera.fov,
                ..CameraRig::default()
            },
            api: ModApi::new(),
            queue: RenderQueue::new(),
            bookmarks: bookmarks::Bookmarks::new(),
            paths,
            platform,
            config,
            budget,
            recorder: None,
            playback: None,
            recording_origin: RecordingOrigin::default(),
            recording_started_unix: 0,
            recording_target: None,
            recovered: None,
            catalog: lang::Catalog::english(),
        }
    }

    /// Reload the config from disk (or write the defaults when it is missing).
    pub fn load_config(game_dir: &Path) -> (Config, Vec<String>) {
        let paths = StoragePaths::resolve(game_dir, &config::StorageSettings::default());
        let mut repairs = Vec::new();
        let mut config = match std::fs::read_to_string(paths.config_file()) {
            Ok(text) => match Config::parse(&text) {
                Ok((config, notes)) => {
                    repairs.extend(notes);
                    config
                }
                Err(error) => {
                    repairs.push(format!("config could not be read ({error}); defaults written"));
                    Config::default()
                }
            },
            Err(_) => Config::default(),
        };
        Platform::detect().apply_defaults(&mut config);
        repairs.extend(config.validate());
        if let Err(error) = paths.ensure().and_then(|()| {
            std::fs::write(paths.config_file(), config.to_pretty()).map_err(std::io::Error::from)
        }) {
            repairs.push(format!("config could not be saved: {error}"));
        }
        (config, repairs)
    }

    /// Load the language the config selects.
    ///
    /// The mod ships English in the binary. A translation is a flat
    /// `key: text` JSON file at `config/voltlapse/lang/<locale>.json`, which a
    /// player or a translator can drop in without rebuilding the mod; an absent
    /// or broken file leaves English in place and reports why.
    pub fn load_language(&mut self, config_dir: &Path) -> Option<String> {
        let locale = lang::locale_for(&self.config.ui.language);
        if locale == "en_us" {
            self.catalog = lang::Catalog::english();
            return None;
        }
        let path = config_dir.join("lang").join(format!("{locale}.json"));
        match std::fs::read_to_string(&path) {
            Ok(text) => match lang::Catalog::parse(locale, &text) {
                Ok(catalog) => {
                    self.catalog = catalog;
                    None
                }
                Err(error) => {
                    self.catalog = lang::Catalog::english();
                    Some(format!("{} is not valid JSON ({error}); English is used", path.display()))
                }
            },
            Err(error) => {
                self.catalog = lang::Catalog::english();
                Some(format!("{} could not be read ({error}); English is used", path.display()))
            }
        }
    }

    /// Save the current config.
    pub fn save_config(&self) -> std::io::Result<()> {
        self.paths.ensure()?;
        std::fs::write(self.paths.config_file(), self.config.to_pretty())
    }

    pub fn is_recording(&self) -> bool {
        self.recorder.is_some()
    }

    pub fn recorder(&self) -> Option<&Recorder> {
        self.recorder.as_ref()
    }

    pub fn playback(&self) -> Option<&Playback> {
        self.playback.as_ref()
    }

    pub fn playback_mut(&mut self) -> Option<&mut Playback> {
        self.playback.as_mut()
    }

    /// Begin recording. The origin's details are stamped into the metadata.
    pub fn start_recording(&mut self, origin: RecordingOrigin, now_unix: i64) -> PathBuf {
        let options = RecordingOptions {
            auto_save_ticks: self.config.storage.auto_save_ticks,
            block_ticks: self.budget.block_ticks(),
            memory_budget_bytes: self.budget.buffer_bytes(),
            ..self.config.recording.clone()
        };
        let recorder = Recorder::new(options, origin.mc_version.clone(), format!("{NAME} {VERSION}"));
        let name = format!("Replay {}", format_stamp(now_unix));
        let target = self.paths.replays_dir.join(format!("{}.vrc", StoragePaths::safe_file_name(&name)));
        self.recorder = Some(recorder);
        self.recording_origin = origin;
        self.recording_started_unix = now_unix;
        self.recording_target = Some(target.clone());
        let _ = self.paths.ensure();
        self.api.emit(&ApiEvent::RecordingStarted {
            tick_rate: self.config.recording.tick_rate,
            mc_version: self.recording_origin.mc_version.clone(),
        });
        target
    }

    /// Capture one tick's events. Returns true when a block was flushed.
    pub fn record_tick(&mut self, events: Vec<Event>) -> bool {
        let Some(recorder) = self.recorder.as_mut() else {
            return false;
        };
        let under_pressure = self.budget.under_pressure();
        let mut captured = 0usize;
        for event in events {
            if self.budget.may_capture(&event, under_pressure) {
                recorder.record(event);
                captured += 1;
            } else {
                self.budget.dropped += 1;
            }
        }
        let tick = recorder.tick();
        let flushed = recorder.end_tick();
        if captured > 0 {
            self.api.emit(&ApiEvent::TickRecorded { tick, events: captured });
        }
        flushed
    }

    pub fn pause_recording(&mut self) {
        if let Some(recorder) = self.recorder.as_mut() {
            recorder.pause();
            let tick = recorder.tick();
            self.api.emit(&ApiEvent::RecordingPaused { tick });
        }
    }

    pub fn resume_recording(&mut self) {
        if let Some(recorder) = self.recorder.as_mut() {
            recorder.resume();
            let tick = recorder.tick();
            self.api.emit(&ApiEvent::RecordingResumed { tick });
        }
    }

    /// A crash-recovery snapshot of the recording in progress.
    pub fn snapshot_recording(&mut self) -> Option<Vec<u8>> {
        self.recorder.as_mut().map(Recorder::snapshot)
    }

    /// Stop recording, write the replay package and add it to the library.
    pub fn stop_recording(&mut self, now_unix: i64) -> Option<Result<PathBuf, String>> {
        let recorder = self.recorder.take()?;
        let target = self.recording_target.take()?;
        let (bytes, stats) = recorder.finish();
        let recording = match Recording::parse(bytes) {
            Ok(recording) => recording,
            Err(error) => return Some(Err(error.to_string())),
        };
        let mut metadata = ReplayMetadata::from_recording(&recording, file_stem(&target), self.recording_started_unix);
        metadata.self_uuid = self.recording_origin.self_uuid.clone();
        metadata.server_name = self.recording_origin.server_name.clone();
        metadata.singleplayer = self.recording_origin.singleplayer;
        metadata.protocol = self.recording_origin.protocol;
        if !self.recording_origin.self_uuid.is_empty() {
            metadata.players.push(package::PlayerInfo {
                uuid: self.recording_origin.self_uuid.clone(),
                name: self.recording_origin.self_name.clone(),
            });
        }
        metadata.render = Some(self.config.rendering.clone());
        let package = ReplayPackage { metadata, recording, thumbnail: None };
        let encoded = match package.to_bytes() {
            Ok(bytes) => bytes,
            Err(error) => return Some(Err(error.to_string())),
        };
        if let Err(error) = std::fs::create_dir_all(&self.paths.replays_dir) {
            return Some(Err(error.to_string()));
        }
        if let Err(error) = std::fs::write(&target, &encoded) {
            return Some(Err(error.to_string()));
        }
        let _ = self.library.refresh();
        self.api.emit(&ApiEvent::RecordingStopped {
            ticks: stats.ticks,
            path: target.display().to_string(),
        });
        Some(Ok(target))
    }

    /// Open a replay for playback: its overlays, camera paths and bookmarks
    /// become the editor's.
    pub fn open_replay(&mut self, package: ReplayPackage, name: &str) {
        self.overlay = package.metadata.overlays.clone();
        self.bookmarks = package.metadata.bookmarks.clone();
        self.rig.path = package.metadata.active_camera().cloned();
        let duration_ticks = package.recording.tick_count();
        let playback = Playback::new(package.recording);
        self.playback = Some(playback);
        self.api.emit(&ApiEvent::PlaybackOpened { replay: name.to_owned(), duration_ticks });
    }

    /// The camera pose for the current frame.
    pub fn camera_pose(&mut self, target: Option<CameraTarget>, delta_seconds: f64) -> keyframe::CameraPose {
        let tick = self.playback.as_ref().map(Playback::tick).unwrap_or(0) as f64;
        self.rig.pose(tick, target, delta_seconds)
    }

    /// Add a keyframe at the playback cursor from the current camera pose.
    pub fn add_keyframe(&mut self) -> Option<usize> {
        let pose = self.camera_pose(None, 0.05);
        let tick = self.playback.as_ref().map(Playback::tick).unwrap_or(0);
        let path = self.rig.path.get_or_insert_with(|| keyframe::CameraPath::new("Camera path"));
        let index = path.add(Keyframe {
            tick,
            pos: pose.pos,
            yaw: pose.yaw,
            pitch: pose.pitch,
            roll: pose.roll,
            fov: pose.fov,
            shake: pose.shake,
            ..Keyframe::default()
        });
        self.api.emit(&ApiEvent::KeyframesChanged { count: path.len() });
        Some(index)
    }

    /// Add a bookmark at the playback cursor.
    pub fn add_bookmark(&mut self, label: &str) -> Option<u64> {
        let tick = self.playback.as_ref()?.tick();
        self.bookmarks.add(Bookmark::new(tick, label));
        self.api.emit(&ApiEvent::BookmarkAdded { tick, label: label.to_owned() });
        Some(tick)
    }

    /// Queue a render of the active camera path over the current replay.
    pub fn queue_render(&mut self, name: &str, output: &str) -> Option<u64> {
        let playback = self.playback.as_ref()?;
        let settings = render::RenderSettings { output: output.to_owned(), ..self.config.rendering.clone() }.sanitized();
        let from_tick = 0;
        let to_tick = playback.duration_ticks();
        let total_frames = settings.frame_count(to_tick, playback.recording().header().tick_rate);
        let camera = self
            .rig
            .path
            .as_ref()
            .map(|path| path.to_json().to_compact())
            .unwrap_or_default();
        Some(self.queue.push(RenderJob {
            id: 0,
            name: name.to_owned(),
            replay: String::new(),
            camera,
            settings,
            from_tick,
            to_tick,
            rendered_frames: 0,
            total_frames,
        }))
    }
}

fn file_stem(path: &Path) -> String {
    path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("Replay").to_owned()
}

/// `2026-11-06 14-05` from Unix seconds, for a default replay name. UTC, so the
/// name does not change with the machine's clock settings mid-session.
pub fn format_stamp(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let seconds_of_day = unix.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}-{:02}",
        seconds_of_day / 3600,
        (seconds_of_day % 3600) / 60
    )
}

/// Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::CameraMode;
    use crate::event::Event;
    use crate::wire::Vec3;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let unique = format!(
                "voltlapse-engine-{tag}-{}-{}",
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

    fn origin() -> RecordingOrigin {
        RecordingOrigin {
            server_name: "play.example.net".into(),
            singleplayer: false,
            self_uuid: "uuid-alex".into(),
            self_name: "Alex".into(),
            mc_version: "26.3".into(),
            protocol: 770,
            camera_mode: Some(CameraMode::Free),
        }
    }

    #[test]
    fn stamps_format_like_the_browser_expects() {
        assert_eq!(format_stamp(0), "1970-01-01 00-00");
        assert_eq!(format_stamp(1_700_000_000), "2023-11-14 22-13");
        assert_eq!(format_stamp(-86_400), "1969-12-31 00-00");
    }

    #[test]
    fn a_whole_session_records_saves_and_plays_back() {
        let temp = TempDir::new("session");
        let mut engine = Engine::new(Config::default(), &temp.0);
        let target = engine.start_recording(origin(), 1_700_000_000);
        assert!(engine.is_recording());
        for tick in 0..100u64 {
            engine.record_tick(vec![
                Event::PlayerState {
                    uuid: "uuid-alex".into(),
                    pos: Vec3::new(tick as f64 * 0.1, 64.0, 0.0),
                    yaw: 0.0,
                    pitch: 0.0,
                    on_ground: true,
                    sprinting: false,
                    sneaking: false,
                    flying: false,
                },
                Event::Chat(crate::event::ChatLine { text: format!("tick {tick}"), action_bar: false, sender: "Alex".into() }),
            ]);
        }
        let saved = engine.stop_recording(1_700_000_100).expect("stops").expect("saves");
        assert_eq!(saved, target);
        assert!(!engine.is_recording());
        assert!(target.exists());

        let bytes = std::fs::read(&target).unwrap();
        let package = ReplayPackage::open(&bytes).expect("opens");
        assert_eq!(package.recording.tick_count(), 100);
        assert_eq!(package.metadata.server_name, "play.example.net");
        assert_eq!(package.metadata.players[0].name, "Alex");

        engine.open_replay(package, "session");
        let playback = engine.playback().unwrap();
        assert_eq!(playback.duration_seconds(), 5.0);
        assert!(playback.world().chat.len() > 0);
    }

    #[test]
    fn pausing_and_resuming_are_visible_in_the_recording() {
        let temp = TempDir::new("pause");
        let mut engine = Engine::new(Config::default(), &temp.0);
        engine.start_recording(origin(), 1_700_000_000);
        for _ in 0..10 {
            engine.record_tick(vec![Event::Marker { label: "a".into(), data: String::new() }]);
        }
        engine.pause_recording();
        assert!(engine.recorder().unwrap().is_paused());
        for _ in 0..10 {
            engine.record_tick(vec![Event::Marker { label: "ignored".into(), data: String::new() }]);
        }
        engine.resume_recording();
        assert!(!engine.recorder().unwrap().is_paused());
        for _ in 0..10 {
            engine.record_tick(vec![Event::Marker { label: "b".into(), data: String::new() }]);
        }
        let path = engine.stop_recording(1_700_000_100).unwrap().unwrap();
        let package = ReplayPackage::open(&std::fs::read(path).unwrap()).unwrap();
        let recording = &package.recording;
        assert_eq!(recording.tick_count(), 30, "the clock kept running while paused");
        assert_eq!(recording.events_in_tick(0).unwrap().len(), 1);
        assert!(recording.events_in_tick(15).unwrap().is_empty(), "nothing captured while paused");
        assert_eq!(recording.events_in_tick(25).unwrap().len(), 1);
    }

    #[test]
    fn a_snapshot_mid_recording_can_be_opened() {
        let temp = TempDir::new("snapshot");
        let mut engine = Engine::new(Config::default(), &temp.0);
        engine.start_recording(origin(), 1_700_000_000);
        for _ in 0..25 {
            engine.record_tick(vec![Event::Marker { label: "x".into(), data: String::new() }]);
        }
        let snapshot = engine.snapshot_recording().expect("recording");
        let recording = Recording::parse(snapshot).expect("snapshot is a recording");
        assert_eq!(recording.tick_count(), 25);
        assert!(engine.is_recording(), "a snapshot does not stop the recording");
    }

    #[test]
    fn camera_keyframes_and_renders_come_from_the_playback() {
        let temp = TempDir::new("camera");
        let mut engine = Engine::new(Config::default(), &temp.0);
        engine.start_recording(origin(), 1_700_000_000);
        for tick in 0..60u64 {
            engine.record_tick(vec![Event::PlayerState {
                uuid: "uuid-alex".into(),
                pos: Vec3::new(tick as f64, 70.0, 0.0),
                yaw: (tick * 3) as f32,
                pitch: 0.0,
                on_ground: true,
                sprinting: false,
                sneaking: false,
                flying: false,
            }]);
        }
        let path = engine.stop_recording(1_700_000_100).unwrap().unwrap();
        let package = ReplayPackage::open(&std::fs::read(path).unwrap()).unwrap();
        engine.open_replay(package, "camera");

        engine.rig.mode = CameraMode::FirstPerson { player: "uuid-alex".into() };
        engine.playback_mut().unwrap().seek_tick(10);
        let target = engine.playback().unwrap().camera_target("uuid-alex", 1.62);
        let pose = engine.camera_pose(target, 0.05);
        assert!((pose.pos.y - 71.62).abs() < 1e-5);

        assert_eq!(engine.add_keyframe(), Some(0));
        engine.playback_mut().unwrap().seek_tick(30);
        assert_eq!(engine.add_keyframe(), Some(1));
        assert_eq!(engine.rig.path.as_ref().unwrap().len(), 2);
        assert!(engine.rig.path.as_ref().unwrap().problems().is_empty());

        let job = engine.queue_render("camera", "out.mp4").expect("queued");
        assert_eq!(job, 1);
        let job = &engine.queue.jobs()[0];
        assert_eq!(job.total_frames, 180, "3 s at 60 fps");
        assert_eq!(job.to_tick, 60);
        assert!(!job.camera.is_empty(), "the path travels with the job");
    }

    #[test]
    fn config_is_written_and_read_back() {
        let temp = TempDir::new("config");
        let (config, repairs) = Engine::load_config(&temp.0);
        assert!(repairs.is_empty(), "{repairs:?}");
        let engine = Engine::new(config.clone(), &temp.0);
        engine.save_config().unwrap();
        let written = std::fs::read_to_string(engine.paths.config_file()).unwrap();
        assert!(written.contains("\"voltlapse-config\""));
        let (reloaded, repairs) = Engine::load_config(&temp.0);
        assert!(repairs.is_empty(), "{repairs:?}");
        assert_eq!(reloaded, config);
    }

    #[test]
    fn a_broken_config_falls_back_to_defaults_with_a_note() {
        let temp = TempDir::new("brokenconfig");
        let paths = StoragePaths::resolve(&temp.0, &config::StorageSettings::default());
        paths.ensure().unwrap();
        std::fs::write(paths.config_file(), "{ not json").unwrap();
        let (config, repairs) = Engine::load_config(&temp.0);
        assert_eq!(config.version, config::CONFIG_VERSION);
        assert!(repairs.iter().any(|note| note.contains("could not be read")));
    }

    #[test]
    fn the_api_sees_the_recording_lifecycle() {
        let temp = TempDir::new("api");
        let mut engine = Engine::new(Config::default(), &temp.0);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorder = seen.clone();
        engine.api.subscribe("test", crate::api::ApiFilter::Recording, 0, move |event| {
            recorder.lock().unwrap().push(event.name().to_owned());
        });
        engine.start_recording(origin(), 1_700_000_000);
        engine.record_tick(vec![Event::Marker { label: "x".into(), data: String::new() }]);
        engine.stop_recording(1_700_000_100);
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen[0], "recording_started");
        assert!(seen.contains(&"tick_recorded".to_owned()));
        assert_eq!(seen.last().unwrap(), "recording_stopped");
    }

    #[test]
    fn constants_describe_the_mod() {
        assert_eq!(ID, "voltlapse");
        assert!(NAME.contains("Volt Lapse"));
        assert!(!VERSION.is_empty());
        assert!(DESCRIPTION.contains("Horizon"));
        assert_eq!(TRANSLATION_PREFIX, "voltlapse.");
    }
    #[test]
    fn a_translation_file_is_loaded_and_a_broken_one_falls_back() {
        let temp = TempDir::new("lang");
        let mut engine = Engine::new(Config::default(), &temp.0);
        engine.config.ui.language = "de_de".to_owned();
        assert!(engine.load_language(&engine.paths.config_dir).is_some(), "no file yet");
        assert_eq!(engine.catalog.text("ui.title"), "Volt Lapse");

        let lang_dir = engine.paths.config_dir.join("lang");
        std::fs::create_dir_all(&lang_dir).unwrap();
        std::fs::write(lang_dir.join("de_de.json"), r#"{"ui.title": "Volt Lapse (DE)"}"#).unwrap();
        assert!(engine.load_language(&engine.paths.config_dir).is_none());
        assert_eq!(engine.catalog.text("ui.title"), "Volt Lapse (DE)");
        assert_eq!(engine.catalog.text("ui.replay.play"), "Play", "untranslated keys stay English");

        std::fs::write(lang_dir.join("de_de.json"), "{broken").unwrap();
        assert!(engine.load_language(&engine.paths.config_dir).unwrap().contains("not valid JSON"));
        assert_eq!(engine.catalog.text("ui.title"), "Volt Lapse");
    }

}
