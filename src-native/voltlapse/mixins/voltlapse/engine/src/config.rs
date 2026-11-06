// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! Configuration: one document, every section, profiles and migration.
//!
//! Settings live in `config/voltlapse/config.json` (a JSON document, because it
//! also has to round-trip through the game's native settings storage), with
//! named profiles beside it (`profiles/<name>.json`). Migration is explicit:
//! a document's `version` says what shape it has, [`migrate`] rewrites an older
//! shape into the current one keeping every value it understands, and a
//! document from a *newer* build is refused rather than silently mangled.

use crate::camera::{CameraMode, ShakeSettings};
use crate::hud::{Anchor, HudSettings};
use crate::json::{self, Json};
use crate::keybinds::Keybinds;
use crate::perf::PerformanceSettings;
use crate::recording::RecordingOptions;
use crate::render::RenderSettings;

/// Format version this build writes.
pub const CONFIG_VERSION: u32 = 3;
/// Marker so a stray JSON file is not mistaken for a config.
pub const CONFIG_MARKER: &str = "voltlapse-config";

/// Playback behaviour.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackSettings {
    pub default_speed: f64,
    pub auto_play: bool,
    /// Start a playback looping.
    pub loop_mode: bool,
    pub hud_hidden: bool,
    /// Render the replay at a fixed resolution instead of the window's.
    pub resolution_override: Option<(u32, u32)>,
    /// Chunks drawn while playing back.
    pub render_distance: u8,
    /// Interpolate entity positions between ticks.
    pub interpolate: bool,
    /// Restore the camera mode used last time.
    pub remember_camera: bool,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            default_speed: 1.0,
            auto_play: true,
            loop_mode: false,
            hud_hidden: false,
            resolution_override: None,
            render_distance: 12,
            interpolate: true,
            remember_camera: true,
        }
    }
}

/// Camera defaults for a new path.
#[derive(Clone, Debug, PartialEq)]
pub struct CameraSettings {
    pub mode: CameraMode,
    pub fov: f32,
    pub shake: ShakeSettings,
    pub smoothing: bool,
    /// Ticks the editor snaps to (`0` disables snapping).
    pub snap_ticks: u64,
    /// Draw the camera path in the world while editing.
    pub show_path: bool,
    /// Default interpolation for a new keyframe.
    pub default_interpolation: String,
    pub third_person_distance: f32,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            mode: CameraMode::Free,
            fov: 70.0,
            shake: ShakeSettings::default(),
            smoothing: true,
            snap_ticks: 10,
            show_path: true,
            default_interpolation: "linear".into(),
            third_person_distance: 4.0,
        }
    }
}

/// Where replays live and how they are kept.
#[derive(Clone, Debug, PartialEq)]
pub struct StorageSettings {
    /// Relative to the game folder, or absolute.
    pub directory: String,
    /// Ticks between crash-recovery checkpoints (`0` disables them).
    pub auto_save_ticks: u64,
    /// Delete the oldest replays beyond this count (`0` keeps everything).
    pub max_replays: u32,
    /// Delete replays older than this many days (`0` keeps them).
    pub keep_days: u32,
    /// Write a thumbnail next to each replay.
    pub thumbnails: bool,
    /// Keep recordings that were interrupted (crash recovery).
    pub keep_recovered: bool,
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            directory: "voltlapse/replays".into(),
            auto_save_ticks: 1_200,
            max_replays: 0,
            keep_days: 0,
            thumbnails: true,
            keep_recovered: true,
        }
    }
}

/// Interface preferences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
    /// Follow the system / game setting.
    Auto,
}

impl Theme {
    pub fn id(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
            Theme::Auto => "auto",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id.trim().to_ascii_lowercase().as_str() {
            "dark" => Theme::Dark,
            "light" => Theme::Light,
            "auto" | "system" => Theme::Auto,
            _ => return None,
        })
    }

    pub fn is_dark(self) -> bool {
        matches!(self, Theme::Dark)
    }
}

/// Interface preferences.
#[derive(Clone, Debug, PartialEq)]
pub struct UiSettings {
    pub theme: Theme,
    /// Language code; empty follows the game.
    pub language: String,
    /// The tutorial has been seen (so it does not open again).
    pub onboarding_done: bool,
    /// Where toasts appear.
    pub toast_anchor: Anchor,
    pub toasts: bool,
    /// Show the replay browser as a grid of thumbnails (false is a list).
    pub grid_browser: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            language: String::new(),
            onboarding_done: false,
            toast_anchor: Anchor::TopRight,
            toasts: true,
            grid_browser: true,
        }
    }
}

/// Which section a reset applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Recording,
    Playback,
    Rendering,
    Camera,
    Hud,
    Keybinds,
    Storage,
    Performance,
    Ui,
    All,
}

impl Section {
    pub fn id(self) -> &'static str {
        match self {
            Section::Recording => "recording",
            Section::Playback => "playback",
            Section::Rendering => "rendering",
            Section::Camera => "camera",
            Section::Hud => "hud",
            Section::Keybinds => "keybinds",
            Section::Storage => "storage",
            Section::Performance => "performance",
            Section::Ui => "ui",
            Section::All => "all",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "recording" => Section::Recording,
            "playback" => Section::Playback,
            "rendering" | "render" => Section::Rendering,
            "camera" => Section::Camera,
            "hud" => Section::Hud,
            "keybinds" | "keys" => Section::Keybinds,
            "storage" => Section::Storage,
            "performance" | "perf" => Section::Performance,
            "ui" => Section::Ui,
            "all" | "defaults" => Section::All,
            _ => return None,
        })
    }
}

/// The whole configuration document.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub version: u32,
    /// Name of the profile this document belongs to.
    pub profile: String,
    pub recording: RecordingOptions,
    pub playback: PlaybackSettings,
    pub rendering: RenderSettings,
    pub camera: CameraSettings,
    pub hud: HudSettings,
    pub keybinds: Keybinds,
    pub storage: StorageSettings,
    pub performance: PerformanceSettings,
    pub ui: UiSettings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            profile: "default".into(),
            recording: RecordingOptions::default(),
            playback: PlaybackSettings::default(),
            rendering: RenderSettings::default(),
            camera: CameraSettings::default(),
            hud: HudSettings::default(),
            keybinds: Keybinds::default(),
            storage: StorageSettings::default(),
            performance: PerformanceSettings::default(),
            ui: UiSettings::default(),
        }
    }
}

impl Config {
    /// Reset one section (or everything) to defaults.
    pub fn reset(&mut self, section: Section) {
        let defaults = Config::default();
        match section {
            Section::Recording => self.recording = defaults.recording,
            Section::Playback => self.playback = defaults.playback,
            Section::Rendering => self.rendering = defaults.rendering,
            Section::Camera => self.camera = defaults.camera,
            Section::Hud => self.hud = defaults.hud,
            Section::Keybinds => self.keybinds = defaults.keybinds,
            Section::Storage => self.storage = defaults.storage,
            Section::Performance => self.performance = defaults.performance,
            Section::Ui => self.ui = defaults.ui,
            Section::All => {
                let profile = self.profile.clone();
                *self = defaults;
                self.profile = profile;
            }
        }
    }

    /// Read one setting as text, for `/voltlapse config` and the settings page.
    ///
    /// The keys are the ones the command's help lists; an unknown key returns
    /// `None` so the caller can name it in the error.
    pub fn get_key(&self, section: Section, key: &str) -> Option<String> {
        let boolean = |value: bool| Some(if value { "on".to_owned() } else { "off".to_owned() });
        match (section, key) {
            (Section::Recording, "tick_rate") => Some(self.recording.tick_rate.to_string()),
            (Section::Recording, "block_ticks") => Some(self.recording.block_ticks.to_string()),
            (Section::Recording, "compress") => boolean(self.recording.compress),
            (Section::Recording, "auto_save_ticks") => Some(self.recording.auto_save_ticks.to_string()),
            (Section::Recording, "capture_raw_packets") => boolean(self.recording.capture_raw_packets),
            (Section::Playback, "default_speed") => Some(format!("{:.2}", self.playback.default_speed)),
            (Section::Playback, "auto_play") => boolean(self.playback.auto_play),
            (Section::Playback, "loop_mode") => boolean(self.playback.loop_mode),
            (Section::Playback, "render_distance") => Some(self.playback.render_distance.to_string()),
            (Section::Playback, "interpolate") => boolean(self.playback.interpolate),
            (Section::Rendering, "width") => Some(self.rendering.width.to_string()),
            (Section::Rendering, "height") => Some(self.rendering.height.to_string()),
            (Section::Rendering, "fps") => Some(self.rendering.fps.to_string()),
            (Section::Rendering, "format") => Some(self.rendering.format.id().to_owned()),
            (Section::Rendering, "bitrate") => Some(self.rendering.bitrate.to_string()),
            (Section::Rendering, "anti_aliasing") => Some(self.rendering.anti_aliasing.id()),
            (Section::Rendering, "alpha") => boolean(self.rendering.alpha),
            (Section::Rendering, "multi_pass") => boolean(self.rendering.multi_pass),
            (Section::Rendering, "render_distance") => Some(self.rendering.render_distance.to_string()),
            (Section::Camera, "fov") => Some(format!("{:.1}", self.camera.fov)),
            (Section::Camera, "smoothing") => boolean(self.camera.smoothing),
            (Section::Camera, "show_path") => boolean(self.camera.show_path),
            (Section::Camera, "third_person_distance") => Some(format!("{:.1}", self.camera.third_person_distance)),
            (Section::Hud, "master_visible") => boolean(self.hud.master_visible),
            (Section::Hud, "transport") => boolean(self.hud.transport),
            (Section::Hud, "recording_indicator") => boolean(self.hud.recording_indicator),
            (Section::Hud, "timer") => boolean(self.hud.timer),
            (Section::Hud, "debug") => boolean(self.hud.debug),
            (Section::Storage, "directory") => Some(self.storage.directory.clone()),
            (Section::Storage, "auto_save_ticks") => Some(self.storage.auto_save_ticks.to_string()),
            (Section::Storage, "max_replays") => Some(self.storage.max_replays.to_string()),
            (Section::Storage, "keep_days") => Some(self.storage.keep_days.to_string()),
            (Section::Performance, "buffer_mb") => Some(self.performance.buffer_mb.to_string()),
            (Section::Performance, "low_memory") => boolean(self.performance.low_memory),
            (Section::Performance, "chunk_cache_mb") => Some(self.performance.chunk_cache_mb.to_string()),
            (Section::Performance, "render_threads") => Some(self.performance.render_threads.to_string()),
            (Section::Ui, "theme") => Some(self.ui.theme.id().to_owned()),
            (Section::Ui, "language") => Some(self.ui.language.clone()),
            (Section::Ui, "toasts") => boolean(self.ui.toasts),
            (Section::Ui, "grid_browser") => boolean(self.ui.grid_browser),
            _ => None,
        }
    }

    /// Write one setting from text, then repair anything the game cannot honour.
    ///
    /// The same keys [`Config::get_key`] reads. `on`/`off`, `true`/`false` and
    /// `1`/`0` all parse as a boolean so a chat command stays forgiving.
    pub fn set_key(&mut self, section: Section, key: &str, value: &str) -> Result<(), ConfigError> {
        fn flag(value: &str) -> Option<bool> {
            match value.trim().to_ascii_lowercase().as_str() {
                "on" | "true" | "yes" | "1" => Some(true),
                "off" | "false" | "no" | "0" => Some(false),
                _ => None,
            }
        }
        fn number(value: &str) -> Option<f64> {
            value.trim().parse::<f64>().ok()
        }
        fn whole(value: &str) -> Option<u64> {
            value.trim().parse::<u64>().ok()
        }
        let bad = || ConfigError::BadValue { section: section.id().to_owned(), key: key.to_owned(), value: value.to_owned() };
        match (section, key) {
            (Section::Recording, "tick_rate") => self.recording.tick_rate = whole(value).ok_or_else(bad)? as u16,
            (Section::Recording, "block_ticks") => self.recording.block_ticks = whole(value).ok_or_else(bad)? as u16,
            (Section::Recording, "compress") => self.recording.compress = flag(value).ok_or_else(bad)?,
            (Section::Recording, "auto_save_ticks") => self.recording.auto_save_ticks = whole(value).ok_or_else(bad)?,
            (Section::Recording, "capture_raw_packets") => {
                self.recording.capture_raw_packets = flag(value).ok_or_else(bad)?
            }
            (Section::Playback, "default_speed") => {
                self.playback.default_speed = number(value).ok_or_else(bad)?.clamp(0.1, 16.0)
            }
            (Section::Playback, "auto_play") => self.playback.auto_play = flag(value).ok_or_else(bad)?,
            (Section::Playback, "loop_mode") => self.playback.loop_mode = flag(value).ok_or_else(bad)?,
            (Section::Playback, "render_distance") => {
                self.playback.render_distance = whole(value).ok_or_else(bad)? as u8
            }
            (Section::Playback, "interpolate") => self.playback.interpolate = flag(value).ok_or_else(bad)?,
            (Section::Rendering, "width") => self.rendering.width = whole(value).ok_or_else(bad)? as u32,
            (Section::Rendering, "height") => self.rendering.height = whole(value).ok_or_else(bad)? as u32,
            (Section::Rendering, "fps") => self.rendering.fps = whole(value).ok_or_else(bad)? as u32,
            (Section::Rendering, "format") => {
                self.rendering.format = crate::render::VideoFormat::from_id(value).ok_or_else(bad)?
            }
            (Section::Rendering, "bitrate") => self.rendering.bitrate = whole(value).ok_or_else(bad)? as u32,
            (Section::Rendering, "anti_aliasing") => {
                self.rendering.anti_aliasing = crate::render::AntiAliasing::from_id(value).ok_or_else(bad)?
            }
            (Section::Rendering, "alpha") => self.rendering.alpha = flag(value).ok_or_else(bad)?,
            (Section::Rendering, "multi_pass") => self.rendering.multi_pass = flag(value).ok_or_else(bad)?,
            (Section::Rendering, "render_distance") => {
                self.rendering.render_distance = whole(value).ok_or_else(bad)? as u8
            }
            (Section::Camera, "fov") => self.camera.fov = number(value).ok_or_else(bad)? as f32,
            (Section::Camera, "smoothing") => self.camera.smoothing = flag(value).ok_or_else(bad)?,
            (Section::Camera, "show_path") => self.camera.show_path = flag(value).ok_or_else(bad)?,
            (Section::Camera, "third_person_distance") => {
                self.camera.third_person_distance = number(value).ok_or_else(bad)? as f32
            }
            (Section::Hud, "master_visible") => self.hud.master_visible = flag(value).ok_or_else(bad)?,
            (Section::Hud, "transport") => self.hud.transport = flag(value).ok_or_else(bad)?,
            (Section::Hud, "recording_indicator") => self.hud.recording_indicator = flag(value).ok_or_else(bad)?,
            (Section::Hud, "timer") => self.hud.timer = flag(value).ok_or_else(bad)?,
            (Section::Hud, "debug") => self.hud.debug = flag(value).ok_or_else(bad)?,
            (Section::Storage, "directory") => self.storage.directory = value.trim().to_owned(),
            (Section::Storage, "auto_save_ticks") => self.storage.auto_save_ticks = whole(value).ok_or_else(bad)?,
            (Section::Storage, "max_replays") => self.storage.max_replays = whole(value).ok_or_else(bad)? as u32,
            (Section::Storage, "keep_days") => self.storage.keep_days = whole(value).ok_or_else(bad)? as u32,
            (Section::Performance, "buffer_mb") => self.performance.buffer_mb = whole(value).ok_or_else(bad)? as u32,
            (Section::Performance, "low_memory") => self.performance.low_memory = flag(value).ok_or_else(bad)?,
            (Section::Performance, "chunk_cache_mb") => {
                self.performance.chunk_cache_mb = whole(value).ok_or_else(bad)? as u32
            }
            (Section::Performance, "render_threads") => {
                self.performance.render_threads = whole(value).ok_or_else(bad)? as usize
            }
            (Section::Ui, "theme") => self.ui.theme = Theme::from_id(value).ok_or_else(bad)?,
            (Section::Ui, "language") => self.ui.language = value.trim().to_owned(),
            (Section::Ui, "toasts") => self.ui.toasts = flag(value).ok_or_else(bad)?,
            (Section::Ui, "grid_browser") => self.ui.grid_browser = flag(value).ok_or_else(bad)?,
            _ => {
                return Err(ConfigError::UnknownKey { section: section.id().to_owned(), key: key.to_owned() })
            }
        }
        self.validate();
        Ok(())
    }

    /// Clamp and repair everything, returning the repairs it made (which the
    /// settings screen shows as a warning).
    pub fn validate(&mut self) -> Vec<String> {
        let mut repairs = Vec::new();
        if self.recording.tick_rate == 0 || self.recording.tick_rate > 120 {
            repairs.push(format!("recording tick rate {} clamped to 20", self.recording.tick_rate));
            self.recording.tick_rate = 20;
        }
        self.recording = self.recording.clone().sanitized();
        if !(0.1..=16.0).contains(&self.playback.default_speed) {
            repairs.push(format!("playback speed {} clamped", self.playback.default_speed));
            self.playback.default_speed = 1.0;
        }
        if self.playback.render_distance == 0 {
            self.playback.render_distance = 12;
        } else if self.playback.render_distance > 32 {
            repairs.push("playback render distance clamped to 32".into());
            self.playback.render_distance = 32;
        }
        if !(1.0..=179.0).contains(&self.camera.fov) {
            repairs.push(format!("camera FOV {} clamped", self.camera.fov));
            self.camera.fov = self.camera.fov.clamp(1.0, 179.0);
        }
        if self.storage.directory.trim().is_empty() {
            repairs.push("empty storage directory reset to the default".into());
            self.storage.directory = StorageSettings::default().directory;
        }
        if self.storage.directory.contains("..") {
            repairs.push("storage directory may not leave the game folder".into());
            self.storage.directory = StorageSettings::default().directory;
        }
        self.rendering = self.rendering.clone().sanitized();
        self.performance = self.performance.clone().sanitized();
        repairs
    }

    /// The whole document as JSON.
    pub fn to_json(&self) -> Json {
        Json::Object(vec![
            ("format".into(), Json::String(CONFIG_MARKER.into())),
            ("version".into(), Json::int(self.version as i64)),
            ("profile".into(), Json::String(self.profile.clone())),
            ("recording".into(), recording_json(&self.recording)),
            (
                "playback".into(),
                Json::Object(vec![
                    ("default_speed".into(), Json::Number(self.playback.default_speed)),
                    ("auto_play".into(), Json::Bool(self.playback.auto_play)),
                    ("loop".into(), Json::Bool(self.playback.loop_mode)),
                    ("hud_hidden".into(), Json::Bool(self.playback.hud_hidden)),
                    (
                        "resolution_override".into(),
                        match self.playback.resolution_override {
                            Some((w, h)) => Json::Array(vec![Json::int(w as i64), Json::int(h as i64)]),
                            None => Json::Null,
                        },
                    ),
                    ("render_distance".into(), Json::int(self.playback.render_distance as i64)),
                    ("interpolate".into(), Json::Bool(self.playback.interpolate)),
                    ("remember_camera".into(), Json::Bool(self.playback.remember_camera)),
                ]),
            ),
            ("rendering".into(), self.rendering.to_json()),
            (
                "camera".into(),
                Json::Object(vec![
                    ("mode".into(), Json::String(self.camera.mode.id().into())),
                    ("fov".into(), Json::Number(self.camera.fov as f64)),
                    (
                        "shake".into(),
                        Json::Object(vec![
                            ("enabled".into(), Json::Bool(self.camera.shake.enabled)),
                            ("intensity".into(), Json::Number(self.camera.shake.intensity as f64)),
                            // Stored per mille so the file stays readable.
                            ("frequency".into(), Json::Number((self.camera.shake.frequency as f64 * 1000.0).round() / 1000.0)),
                        ]),
                    ),
                    ("smoothing".into(), Json::Bool(self.camera.smoothing)),
                    ("snap_ticks".into(), Json::int(self.camera.snap_ticks as i64)),
                    ("show_path".into(), Json::Bool(self.camera.show_path)),
                    ("default_interpolation".into(), Json::String(self.camera.default_interpolation.clone())),
                    ("third_person_distance".into(), Json::Number(self.camera.third_person_distance as f64)),
                ]),
            ),
            (
                "hud".into(),
                Json::Object(vec![
                    ("health".into(), Json::Bool(self.hud.health)),
                    ("hunger".into(), Json::Bool(self.hud.hunger)),
                    ("crosshair".into(), Json::Bool(self.hud.crosshair)),
                    ("debug".into(), Json::Bool(self.hud.debug)),
                    ("player_names".into(), Json::Bool(self.hud.player_names)),
                    ("entity_names".into(), Json::Bool(self.hud.entity_names)),
                    ("timer".into(), Json::Bool(self.hud.timer)),
                    ("transport".into(), Json::Bool(self.hud.transport)),
                    ("recording_indicator".into(), Json::Bool(self.hud.recording_indicator)),
                    ("indicator_anchor".into(), Json::String(self.hud.indicator_anchor.id().into())),
                    ("indicator_color".into(), Json::int(self.hud.indicator_color as i64)),
                    ("master_visible".into(), Json::Bool(self.hud.master_visible)),
                ]),
            ),
            ("keybinds".into(), self.keybinds.to_json()),
            (
                "storage".into(),
                Json::Object(vec![
                    ("directory".into(), Json::String(self.storage.directory.clone())),
                    ("auto_save_ticks".into(), Json::int(self.storage.auto_save_ticks as i64)),
                    ("max_replays".into(), Json::int(self.storage.max_replays as i64)),
                    ("keep_days".into(), Json::int(self.storage.keep_days as i64)),
                    ("thumbnails".into(), Json::Bool(self.storage.thumbnails)),
                    ("keep_recovered".into(), Json::Bool(self.storage.keep_recovered)),
                ]),
            ),
            ("performance".into(), self.performance.to_json()),
            (
                "ui".into(),
                Json::Object(vec![
                    ("theme".into(), Json::String(self.ui.theme.id().into())),
                    ("language".into(), Json::String(self.ui.language.clone())),
                    ("onboarding_done".into(), Json::Bool(self.ui.onboarding_done)),
                    ("toast_anchor".into(), Json::String(self.ui.toast_anchor.id().into())),
                    ("toasts".into(), Json::Bool(self.ui.toasts)),
                    ("grid_browser".into(), Json::Bool(self.ui.grid_browser)),
                ]),
            ),
        ])
    }

    pub fn to_pretty(&self) -> String {
        self.to_json().to_pretty()
    }

    /// Read a document, migrating it if it is older. Returns the repairs made.
    pub fn from_json(json: &Json) -> Result<(Self, Vec<String>), ConfigError> {
        let Some(version) = json.get("version").and_then(Json::as_u64) else {
            return Err(ConfigError::Missing("version".into()));
        };
        if version as u32 > CONFIG_VERSION {
            return Err(ConfigError::Newer(version as u32));
        }
        let migrated = migrate(json, version as u32);
        let mut config = Config::default();
        config.version = CONFIG_VERSION;
        config.profile = migrated.get("profile").and_then(Json::as_str).unwrap_or("default").to_owned();
        config.recording = recording_from_json(migrated.get("recording"));
        config.playback = PlaybackSettings {
            default_speed: migrated.path("playback/default_speed").and_then(Json::as_f64).unwrap_or(1.0),
            auto_play: migrated.path("playback/auto_play").and_then(Json::as_bool).unwrap_or(true),
            loop_mode: migrated.path("playback/loop").and_then(Json::as_bool).unwrap_or(false),
            hud_hidden: migrated.path("playback/hud_hidden").and_then(Json::as_bool).unwrap_or(false),
            resolution_override: migrated.path("playback/resolution_override").and_then(Json::as_array).and_then(|size| {
                match size {
                    [w, h] => Some((w.as_f64()? as u32, h.as_f64()? as u32)),
                    _ => None,
                }
            }),
            render_distance: migrated.path("playback/render_distance").and_then(Json::as_u64).unwrap_or(12) as u8,
            interpolate: migrated.path("playback/interpolate").and_then(Json::as_bool).unwrap_or(true),
            remember_camera: migrated.path("playback/remember_camera").and_then(Json::as_bool).unwrap_or(true),
        };
        config.rendering = RenderSettings::from_json(migrated.get("rendering").unwrap_or(&Json::Null));
        config.camera = CameraSettings {
            mode: match migrated.path("camera/mode").and_then(Json::as_str) {
                Some("first_person") => CameraMode::FirstPerson { player: String::new() },
                Some("third_person") => CameraMode::ThirdPerson { player: String::new(), distance: 4.0, side: 0.0, height: 0.5 },
                Some("spectator") => CameraMode::Spectator { player: String::new() },
                Some("path") => CameraMode::Path,
                _ => CameraMode::Free,
            },
            fov: migrated.path("camera/fov").and_then(Json::as_f64).unwrap_or(70.0) as f32,
            shake: ShakeSettings {
                enabled: migrated.path("camera/shake/enabled").and_then(Json::as_bool).unwrap_or(true),
                intensity: migrated.path("camera/shake/intensity").and_then(Json::as_f64).unwrap_or(1.0) as f32,
                frequency: migrated.path("camera/shake/frequency").and_then(Json::as_f64).unwrap_or(0.9) as f32,
            },
            smoothing: migrated.path("camera/smoothing").and_then(Json::as_bool).unwrap_or(true),
            snap_ticks: migrated.path("camera/snap_ticks").and_then(Json::as_u64).unwrap_or(10),
            show_path: migrated.path("camera/show_path").and_then(Json::as_bool).unwrap_or(true),
            default_interpolation: migrated
                .path("camera/default_interpolation")
                .and_then(Json::as_str)
                .unwrap_or("linear")
                .to_owned(),
            third_person_distance: migrated.path("camera/third_person_distance").and_then(Json::as_f64).unwrap_or(4.0) as f32,
        };
        config.hud = HudSettings {
            health: migrated.path("hud/health").and_then(Json::as_bool).unwrap_or(true),
            hunger: migrated.path("hud/hunger").and_then(Json::as_bool).unwrap_or(true),
            crosshair: migrated.path("hud/crosshair").and_then(Json::as_bool).unwrap_or(true),
            debug: migrated.path("hud/debug").and_then(Json::as_bool).unwrap_or(false),
            player_names: migrated.path("hud/player_names").and_then(Json::as_bool).unwrap_or(true),
            entity_names: migrated.path("hud/entity_names").and_then(Json::as_bool).unwrap_or(false),
            timer: migrated.path("hud/timer").and_then(Json::as_bool).unwrap_or(false),
            transport: migrated.path("hud/transport").and_then(Json::as_bool).unwrap_or(true),
            recording_indicator: migrated.path("hud/recording_indicator").and_then(Json::as_bool).unwrap_or(true),
            indicator_anchor: migrated
                .path("hud/indicator_anchor")
                .and_then(Json::as_str)
                .and_then(Anchor::from_id)
                .unwrap_or(Anchor::TopRight),
            indicator_color: migrated.path("hud/indicator_color").and_then(Json::as_u64).unwrap_or(0xFFFF_3B30) as u32,
            master_visible: migrated.path("hud/master_visible").and_then(Json::as_bool).unwrap_or(true),
        };
        config.keybinds = migrated.get("keybinds").map(Keybinds::from_json).unwrap_or_default();
        config.storage = StorageSettings {
            directory: migrated.path("storage/directory").and_then(Json::as_str).unwrap_or("voltlapse/replays").to_owned(),
            auto_save_ticks: migrated.path("storage/auto_save_ticks").and_then(Json::as_u64).unwrap_or(1_200),
            max_replays: migrated.path("storage/max_replays").and_then(Json::as_u64).unwrap_or(0) as u32,
            keep_days: migrated.path("storage/keep_days").and_then(Json::as_u64).unwrap_or(0) as u32,
            thumbnails: migrated.path("storage/thumbnails").and_then(Json::as_bool).unwrap_or(true),
            keep_recovered: migrated.path("storage/keep_recovered").and_then(Json::as_bool).unwrap_or(true),
        };
        config.performance = PerformanceSettings::from_json(migrated.get("performance").unwrap_or(&Json::Null));
        config.ui = UiSettings {
            theme: migrated.path("ui/theme").and_then(Json::as_str).and_then(Theme::from_id).unwrap_or(Theme::Dark),
            language: migrated.path("ui/language").and_then(Json::as_str).unwrap_or("").to_owned(),
            onboarding_done: migrated.path("ui/onboarding_done").and_then(Json::as_bool).unwrap_or(false),
            toast_anchor: migrated
                .path("ui/toast_anchor")
                .and_then(Json::as_str)
                .and_then(Anchor::from_id)
                .unwrap_or(Anchor::TopRight),
            toasts: migrated.path("ui/toasts").and_then(Json::as_bool).unwrap_or(true),
            grid_browser: migrated.path("ui/grid_browser").and_then(Json::as_bool).unwrap_or(true),
        };
        if version as u32 != CONFIG_VERSION {
            // Save repairs the caller can log, not user errors.
            let mut repairs = Vec::new();
            repairs.push(format!("migrated config from version {version} to {CONFIG_VERSION}"));
            let mut validation = config.validate();
            repairs.append(&mut validation);
            return Ok((config, repairs));
        }
        let repairs = config.validate();
        Ok((config, repairs))
    }

    pub fn parse(text: &str) -> Result<(Self, Vec<String>), ConfigError> {
        let json = json::parse(text).map_err(|error| ConfigError::Syntax(error.to_string()))?;
        Config::from_json(&json)
    }

    /// Check that a document really is one of ours, for import.
    pub fn looks_like_config(json: &Json) -> bool {
        json.get("format").and_then(Json::as_str) == Some(CONFIG_MARKER) || json.get("recording").is_some()
    }
}

fn recording_json(options: &RecordingOptions) -> Json {
    Json::Object(vec![
        ("tick_rate".into(), Json::int(options.tick_rate as i64)),
        ("block_ticks".into(), Json::int(options.block_ticks as i64)),
        ("max_block_bytes".into(), Json::int(options.max_block_bytes as i64)),
        ("compress".into(), Json::Bool(options.compress)),
        ("auto_save_ticks".into(), Json::int(options.auto_save_ticks as i64)),
        ("memory_budget_bytes".into(), Json::int(options.memory_budget_bytes as i64)),
        ("capture_raw_packets".into(), Json::Bool(options.capture_raw_packets)),
    ])
}

fn recording_from_json(json: Option<&Json>) -> RecordingOptions {
    let defaults = RecordingOptions::default();
    let Some(json) = json else {
        return defaults;
    };
    RecordingOptions {
        tick_rate: json.get("tick_rate").and_then(Json::as_u64).unwrap_or(defaults.tick_rate as u64) as u16,
        block_ticks: json.get("block_ticks").and_then(Json::as_u64).unwrap_or(defaults.block_ticks as u64) as u16,
        max_block_bytes: json.get("max_block_bytes").and_then(Json::as_u64).unwrap_or(defaults.max_block_bytes as u64) as usize,
        compress: json.get("compress").and_then(Json::as_bool).unwrap_or(defaults.compress),
        auto_save_ticks: json.get("auto_save_ticks").and_then(Json::as_u64).unwrap_or(defaults.auto_save_ticks),
        memory_budget_bytes: json
            .get("memory_budget_bytes")
            .and_then(Json::as_u64)
            .unwrap_or(defaults.memory_budget_bytes as u64) as usize,
        capture_raw_packets: json.get("capture_raw_packets").and_then(Json::as_bool).unwrap_or(defaults.capture_raw_packets),
    }
    .sanitized()
}

/// Rewrite an older document into the current shape.
///
/// * **1 → 2:** `recordTickRate` moved under `recording/tick_rate`; the flat
///   `hud*` booleans became `hud/*`; `camera_fov` became `camera/fov`.
/// * **2 → 3:** `playback/speed` became `playback/default_speed`; `theme` moved
///   under `ui/theme`; `renderResolution` became `rendering/width` and
///   `rendering/height`.
pub fn migrate(json: &Json, from: u32) -> Json {
    let mut document = json.clone();
    if from <= 1 {
        let mut recording = Json::Object(Vec::new());
        if let Some(value) = document.get("recordTickRate").cloned() {
            recording.set("tick_rate", value);
        }
        if let Some(value) = document.get("recordCompression").cloned() {
            recording.set("compress", value);
        }
        document.set("recording", recording);
        let mut hud = Json::Object(Vec::new());
        for (old, new) in [
            ("showHealth", "health"),
            ("showHunger", "hunger"),
            ("showCrosshair", "crosshair"),
            ("showDebug", "debug"),
        ] {
            if let Some(value) = document.get(old).cloned() {
                hud.set(new, value);
            }
        }
        document.set("hud", hud);
        let mut camera = Json::Object(Vec::new());
        if let Some(value) = document.get("camera_fov").cloned() {
            camera.set("fov", value);
        }
        document.set("camera", camera);
        document.set("version", Json::int(2));
    }
    if from <= 2 {
        let mut playback = document.get("playback").cloned().unwrap_or_else(|| Json::Object(Vec::new()));
        if let Some(value) = playback.get("speed").cloned() {
            playback.set("default_speed", value);
        }
        document.set("playback", playback);
        let mut ui = document.get("ui").cloned().unwrap_or_else(|| Json::Object(Vec::new()));
        if let Some(value) = document.get("theme").cloned() {
            ui.set("theme", value);
        }
        document.set("ui", ui);
        let mut rendering = document.get("rendering").cloned().unwrap_or_else(|| Json::Object(Vec::new()));
        if let Some(Json::Array(size)) = document.get("renderResolution").cloned() {
            if let [w, h] = size.as_slice() {
                rendering.set("width", w.clone());
                rendering.set("height", h.clone());
            }
        }
        document.set("rendering", rendering);
        document.set("version", Json::int(3));
    }
    document.set("format", Json::String(CONFIG_MARKER.into()));
    document.set("version", Json::int(CONFIG_VERSION as i64));
    document
}

/// Why a config could not be read.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfigError {
    /// The JSON did not parse.
    Syntax(String),
    /// A required field is absent.
    Missing(String),
    /// Written by a newer build than this one.
    Newer(u32),
    /// Not a Volt Lapse config at all.
    NotAConfig,
    /// A setting was given a value it cannot hold.
    BadValue { section: String, key: String, value: String },
    /// The setting named does not exist.
    UnknownKey { section: String, key: String },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Syntax(why) => write!(f, "config is not valid JSON: {why}"),
            ConfigError::Missing(field) => write!(f, "config has no '{field}' field"),
            ConfigError::Newer(version) => write!(
                f,
                "config version {version} is newer than this build's {CONFIG_VERSION}; update Volt Lapse"
            ),
            ConfigError::NotAConfig => f.write_str("file is not a Volt Lapse config"),
            ConfigError::BadValue { section, key, value } => {
                write!(f, "'{value}' is not a valid value for {section}.{key}")
            }
            ConfigError::UnknownKey { section, key } => {
                write!(f, "{section} has no setting called '{key}'")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip_through_json() {
        let config = Config::default();
        let text = config.to_pretty();
        let (back, repairs) = Config::parse(&text).expect("parses");
        assert!(repairs.is_empty(), "{repairs:?}");
        assert_eq!(back, config);
        assert!(text.contains("\"format\": \"" /* marker is present */));
        assert_eq!(back.version, CONFIG_VERSION);
    }

    #[test]
    fn every_section_can_be_reset_independently() {
        let mut config = Config::default();
        config.recording.tick_rate = 5;
        config.hud.debug = true;
        config.camera.fov = 100.0;
        config.reset(Section::Recording);
        assert_eq!(config.recording.tick_rate, 20);
        assert!(config.hud.debug);
        config.reset(Section::Hud);
        assert!(!config.hud.debug);
        assert_eq!(config.camera.fov, 100.0);
        config.profile = "cinematic".into();
        config.reset(Section::All);
        assert_eq!(config.camera.fov, 70.0);
        assert_eq!(config.profile, "cinematic", "the profile name survives a full reset");
    }

    #[test]
    fn sections_parse_from_ids() {
        for section in [Section::Recording, Section::Playback, Section::Rendering, Section::Camera, Section::Hud, Section::Keybinds, Section::Storage, Section::Performance, Section::Ui, Section::All] {
            assert_eq!(Section::from_id(section.id()), Some(section));
        }
        assert_eq!(Section::from_id("render"), Some(Section::Rendering));
        assert_eq!(Section::from_id("nope"), None);
    }

    #[test]
    fn validation_clamps_and_reports() {
        let mut config = Config::default();
        config.recording.tick_rate = 0;
        config.playback.default_speed = 500.0;
        config.playback.render_distance = 64;
        config.camera.fov = 400.0;
        config.storage.directory = "../escape".into();
        let repairs = config.validate();
        assert!(repairs.len() >= 4, "{repairs:?}");
        assert_eq!(config.recording.tick_rate, 20);
        assert_eq!(config.playback.default_speed, 1.0);
        assert_eq!(config.playback.render_distance, 32);
        assert_eq!(config.camera.fov, 179.0);
        assert_eq!(config.storage.directory, "voltlapse/replays");
    }

    #[test]
    fn version_1_documents_migrate() {
        let old = crate::json::parse(
            r#"{"version":1,"recordTickRate":30,"recordCompression":false,"showHealth":false,
                "showCrosshair":false,"camera_fov":85,"playback":{"speed":4.0},
                "theme":"light","renderResolution":[1280,720],"keybinds":{"screenshot":"key.keyboard.f3"}}"#,
        )
        .expect("parses");
        let (config, repairs) = Config::from_json(&old).expect("migrates");
        assert_eq!(config.version, CONFIG_VERSION);
        assert_eq!(config.recording.tick_rate, 30);
        assert!(!config.recording.compress);
        assert!(!config.hud.health);
        assert!(!config.hud.crosshair);
        assert_eq!(config.camera.fov, 85.0);
        assert_eq!(config.playback.default_speed, 4.0);
        assert_eq!(config.ui.theme, Theme::Light);
        assert_eq!(config.rendering.width, 1280);
        assert_eq!(config.rendering.height, 720);
        assert_eq!(config.keybinds.binding(crate::keybinds::Action::Screenshot), Some("key.keyboard.f3"));
        assert!(repairs.iter().any(|r| r.contains("migrated config from version 1")));
    }

    #[test]
    fn migration_is_idempotent_and_keeps_unknown_keys_out() {
        let old = crate::json::parse(r#"{"version":2,"playback":{"speed":0.5},"theme":"dark"}"#).unwrap();
        let once = migrate(&old, 2);
        let twice = migrate(&once, CONFIG_VERSION);
        assert_eq!(once.to_compact(), twice.to_compact());
        assert_eq!(once.path("playback/default_speed").and_then(Json::as_f64), Some(0.5));
    }

    #[test]
    fn newer_and_broken_documents_are_refused() {
        let newer = crate::json::parse(r#"{"version":99}"#).unwrap();
        assert_eq!(Config::from_json(&newer), Err(ConfigError::Newer(99)));
        let no_version = crate::json::parse(r#"{"recording":{}}"#).unwrap();
        assert!(matches!(Config::from_json(&no_version), Err(ConfigError::Missing(_))));
        assert!(matches!(Config::parse("{"), Err(ConfigError::Syntax(_))));
    }

    #[test]
    fn a_missing_document_only_fills_defaults() {
        let sparse = crate::json::parse(r#"{"version":3,"profile":"handmade"}"#).unwrap();
        let (config, _) = Config::from_json(&sparse).expect("parses");
        assert_eq!(config.profile, "handmade");
        assert_eq!(config.recording, RecordingOptions::default());
        assert_eq!(config.keybinds, Keybinds::default());
        assert_eq!(config.ui.theme, Theme::Dark);
    }

    #[test]
    fn ui_theme_ids_round_trip() {
        for theme in [Theme::Dark, Theme::Light, Theme::Auto] {
            assert_eq!(Theme::from_id(theme.id()), Some(theme));
        }
        assert!(!Theme::Light.is_dark());
        assert_eq!(Theme::from_id("system"), Some(Theme::Auto));
    }

    #[test]
    fn config_detection_helps_the_import_screen() {
        assert!(Config::looks_like_config(&Config::default().to_json()));
        assert!(!Config::looks_like_config(&crate::json::parse(r#"{"something":"else"}"#).unwrap()));
    }
    #[test]
    fn settings_can_be_read_and_written_by_key() {
        let mut config = Config::default();
        assert_eq!(config.get_key(Section::Recording, "tick_rate").as_deref(), Some("20"));
        assert_eq!(config.get_key(Section::Playback, "auto_play").as_deref(), Some("on"));
        config.set_key(Section::Recording, "tick_rate", "60").unwrap();
        assert_eq!(config.recording.tick_rate, 60);
        config.set_key(Section::Recording, "tick_rate", "900").unwrap();
        assert_eq!(config.recording.tick_rate, 20, "validate clamps an impossible rate");
        config.set_key(Section::Playback, "auto_play", "off").unwrap();
        assert!(!config.playback.auto_play);
        config.set_key(Section::Playback, "auto_play", "yes").unwrap();
        assert!(config.playback.auto_play);
        config.set_key(Section::Rendering, "format", "webm").unwrap();
        assert_eq!(config.rendering.format.id(), "webm");
        config.set_key(Section::Ui, "theme", "light").unwrap();
        assert!(config.ui.theme.id() == "light");
        config.set_key(Section::Camera, "fov", "55.5").unwrap();
        assert!((config.camera.fov - 55.5).abs() < 1e-3);
    }

    #[test]
    fn an_unknown_key_and_a_bad_value_are_named() {
        let mut config = Config::default();
        let error = config.set_key(Section::Hud, "nonsense", "on").unwrap_err();
        assert!(error.to_string().contains("nonsense"), "{error}");
        assert!(config.get_key(Section::Hud, "nonsense").is_none());
        let error = config.set_key(Section::Recording, "tick_rate", "fast").unwrap_err();
        assert!(error.to_string().contains("fast"), "{error}");
        let error = config.set_key(Section::Rendering, "format", "avi2").unwrap_err();
        assert!(error.to_string().contains("avi2"), "{error}");
        assert_eq!(config.rendering.format.id(), "mp4", "a rejected value changes nothing");
    }

    #[test]
    fn every_section_reports_its_own_keys() {
        let config = Config::default();
        for section in [Section::Recording, Section::Playback, Section::Rendering, Section::Camera, Section::Hud, Section::Storage, Section::Performance, Section::Ui] {
            assert!(config.get_key(section, "tick_rate").is_none() || section == Section::Recording);
        }
        assert!(config.get_key(Section::Storage, "directory").is_some());
        assert!(config.get_key(Section::Performance, "low_memory").is_some());
        assert!(config.get_key(Section::All, "anything").is_none());
    }

    #[test]
    fn the_shipped_default_config_is_the_default() {
        // The file the repository ships is the same document the game writes on
        // first launch, so a hand edit by a player cannot drift from the code.
        let text = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../default-config.json"));
        let (config, repairs) = Config::parse(text).expect("the shipped default config parses");
        assert!(repairs.is_empty(), "{repairs:?}");
        assert_eq!(config, Config::default());
    }

}
