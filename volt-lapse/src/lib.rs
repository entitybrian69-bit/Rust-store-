//! Volt Lapse — a replay recorder and replayer for Horizon, built as a legacy
//! **WebAssembly server mod** (manifest `api = "0.3"`, `side = "server"`).
//!
//! A replay mod records what the game shows and replays it later. The legacy API
//! this mod is built against has no packets, no camera, no renderer and no
//! filesystem, so Volt Lapse records what a *server* can see and replays what a
//! *server* can change:
//!
//! * every tick (or every `record_interval_ticks`), a frame of every online
//!   player: name, position, dimension, game mode, health;
//! * chat (`record_chat`), block placement and breaking (`record_block_changes`);
//! * keyframes — the camera path — with a bounded block snapshot around the
//!   player (`chunk_radius`);
//! * on playback: the block changes go back into the world through
//!   `world::set-blocks`, the chat lines are broadcast again, and the camera path
//!   is sampled and reported (`/vl path`).
//!
//! Everything else a Replay Mod does — packets, entities, particles, sounds,
//! weather, inventories, time control, a timeline HUD, keybinds, a GUI, a `.mcpr`
//! file — has no interface at this revision. Each one is listed in the README's
//! "API gaps" table with the document that says so, and every one of those
//! settings is still validated, persisted and reported (a one-line warning at
//! start-up names the ones turned on) rather than silently ignored. Where the
//! fallback bites in the code there is a `TODO(api)` comment.

mod path;
mod recording;
mod settings;

pub use path::{preview, sample, CameraKeyframe, Interpolation};
pub use recording::{BlockChange, Cell, ChatLine, Frame, Keyframe, PlayerSample, Recording};
pub use settings::{Config, Kind, Spec, Value, SPECS};

use horizon_mod_sdk::server::{
    players, world, Args, BlockChange as SdkBlockChange, Player, Registration, ServerMod,
};
use horizon_mod_sdk::{info, storage, warn};

/// The live configuration, in the mod's own `storage`.
const CONFIG_KEY: &str = "voltlapse.config";
/// The recording index (`"<namespace> <name>"` per line).
const INDEX_KEY: &str = "voltlapse.index";
/// The shipped `config.toml`, compiled in: the defaults a fresh install starts
/// from, and the file unit tests check `SPECS` against.
pub const SHIPPED_CONFIG: &str = include_str!("../config.toml");

const TICKS_PER_SECOND: u64 = 20;
const TICKS_PER_MINUTE: u64 = 60 * TICKS_PER_SECOND;
/// Blocks sampled around the player for one keyframe snapshot.
const MAX_CELLS_PER_KEYFRAME: usize = 64;
/// Storage keys are `[a-z0-9_.-]{1,64}` and there may be 64 of them in total, so
/// names and namespaces stay short.
const MAX_NAME: usize = 28;
const MAX_NAMESPACE: usize = 12;
/// The host allows 1 MiB of `storage` writes per hook call; stay under it.
const WRITE_BUDGET: usize = 900 * 1024;
/// The host allows 4 MiB of `storage` in total for the whole mod, minus the
/// configuration and the index. A recording is capped below that.
const MAX_RECORDING_BYTES: usize = 3 * 1024 * 1024;
/// How much of a draft a periodic flush may write in one go.
const MAX_DRAFT_BYTES: usize = 200 * 1024;

const HELP: &str = "/vl record [name] | /vl stop | /vl play [name] [speed|p<n>] | /vl list | \
/vl config [key] [value] | /vl config reset | /vl keyframe | /vl path [name] | /vl help";

/// Settings whose effect needs an interface this API revision does not have.
/// They are validated, persisted and edited like every other setting; this table
/// is what the start-up warning and the README say about them.
const NO_API: &[(&str, &str)] = &[
    ("record_player_inventory", "the server player record has no inventory"),
    ("record_entity_metadata", "no entity list"),
    ("record_particles", "no particle stream"),
    ("record_sounds", "no sound stream"),
    ("record_weather", "no weather state"),
    ("record_time_of_day", "no clock"),
    ("record_mob_ai", "no mob state"),
    ("record_item_drops", "no entity list"),
    ("record_projectiles", "no entity list"),
    ("entity_tracking_radius", "no entities to track"),
    ("free_camera", "the camera belongs to the client"),
    ("spectator_lock", "the camera belongs to the client"),
    ("show_timeline", "no client HUD"),
    ("playback_hud", "no client HUD"),
    ("hide_hud_in_replay", "no client HUD"),
    ("render_distance", "the client owns render distance"),
    ("time_control", "no clock to control"),
    ("weather_control", "no weather to control"),
    ("fov_override", "the camera belongs to the client"),
    ("camera_roll_enabled", "the camera belongs to the client"),
    ("camera_collision", "the camera belongs to the client"),
    ("theme", "no client UI"),
    ("language", "no client UI"),
    ("ui_scale", "no client UI"),
    ("timeline_position", "no client HUD"),
    ("keybind_open_replay_menu", "no keybind interface"),
    ("keybind_toggle_record", "no keybind interface"),
    ("keybind_add_keyframe", "no keybind interface"),
    ("keybind_play_pause", "no keybind interface"),
    ("keybind_speed_up", "no keybind interface"),
    ("keybind_slow_down", "no keybind interface"),
    ("auto_delete_after_days", "no wall clock to age recordings by"),
];

struct Session {
    name: String,
    started: u64,
    last_frame: u64,
    last_keyframe: u64,
    recording: Recording,
}

enum Action {
    Blocks {
        dimension: String,
        changes: Vec<SdkBlockChange>,
    },
    Chat(String),
    Note(String),
}

struct Scheduled {
    tick: u64,
    action: Action,
}

struct Playback {
    name: String,
    /// The recording's first tick; playback positions are offsets from it.
    origin: u64,
    /// The recording's last tick.
    end: u64,
    started: u64,
    cursor: usize,
    schedule: Vec<Scheduled>,
    speed: f64,
}

struct QueueItem {
    key: String,
    body: String,
    attempts: u8,
}

#[derive(Default)]
struct Metrics {
    frames: u64,
    blocks_replayed: u64,
    recordings: u64,
}

pub struct VoltLapse {
    config: Config,
    session: Option<Session>,
    playback: Option<Playback>,
    /// The last server tick seen, for hooks that do not carry one.
    last_tick: u64,
    /// `auto_start_on_world_load` fires once.
    auto_started: bool,
    /// Storage writes waiting for a tick with enough budget.
    queue: Vec<QueueItem>,
    budget: usize,
    last_flush: u64,
    metrics: Metrics,
}

// ------------------------------------------------------------------ names ----

/// A storage-safe identifier: lowercase `[a-z0-9_-]`, no repeated or edge `_`.
fn clean(text: &str, max: usize) -> String {
    let mut out = String::new();
    for character in text.trim().chars() {
        let mapped = if character.is_ascii_alphanumeric() {
            character.to_ascii_lowercase()
        } else if character == '-' {
            '-'
        } else {
            '_'
        };
        if mapped == '_' && out.ends_with('_') {
            continue;
        }
        out.push(mapped);
    }
    out.trim_matches('_').chars().take(max).collect()
}

/// A recording's name: what `/vl list` shows and what its keys are called.
fn recording_name(text: &str) -> String {
    let name = clean(text, MAX_NAME);
    if name.is_empty() {
        "recording".to_string()
    } else {
        name
    }
}

/// `replay_folder` / `temp_folder` become a key namespace, so moving recordings
/// to another folder really does put new ones under another set of keys.
fn namespace(config: &Config, key: &str, fallback: &str) -> String {
    let name = clean(config.text_of(key), MAX_NAMESPACE);
    if name.is_empty() {
        fallback.to_string()
    } else {
        name
    }
}

fn rec_base(ns: &str, name: &str) -> String {
    format!("{ns}.rec.{name}")
}

fn read_key(key: &str) -> Option<String> {
    storage::get(key).and_then(|bytes| String::from_utf8(bytes).ok())
}

fn index_entries() -> Vec<(String, String)> {
    read_key(INDEX_KEY)
        .map(|text| {
            text.lines()
                .filter_map(|line| {
                    let mut parts = line.split_whitespace();
                    let ns = parts.next()?;
                    let name = parts.next()?;
                    Some((ns.to_string(), name.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn write_index(entries: &[(String, String)]) {
    let text = entries
        .iter()
        .map(|(ns, name)| format!("{ns} {name}"))
        .collect::<Vec<_>>()
        .join("\n");
    if let Err(error) = storage::set(INDEX_KEY, text.as_bytes()) {
        warn(&format!("Volt Lapse: could not write the index: {error}"));
    }
}

/// Splits on character boundaries so a chunk is always valid UTF-8.
fn split_chunks(text: &str, size: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if current.len() + character.len_utf8() > size {
            out.push(std::mem::take(&mut current));
        }
        current.push(character);
    }
    if !current.is_empty() {
        out.push(current);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn clip(text: &str, size: usize) -> String {
    if text.len() <= size {
        return text.to_string();
    }
    let mut end = size;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

fn recording_push_keyframe(recording: &mut Recording, tick: u64, x: f64, y: f64, z: f64) -> u32 {
    recording.keyframes.push(Keyframe { tick, x, y, z });
    (recording.keyframes.len() - 1) as u32
}

fn file_name_for(config: &Config, wanted: &str, tick: u64) -> String {
    let wanted = wanted.trim();
    if !wanted.is_empty() {
        return wanted.to_string();
    }
    match config.text_of("file_naming") {
        "manual" => String::new(),
        // `TODO(api)`: the legacy API exposes no world name, so "world name"
        // records under a fixed prefix and the tick.
        "world_name" => format!("world-{tick}"),
        // `TODO(api)`: the API exposes no wall clock; the tick counter stands in.
        _ => format!("rec-{tick}"),
    }
}

// ---------------------------------------------------------------- capture ----
//
// Free functions rather than methods: `tick` borrows `self.session` mutably and
// `self.config` immutably at the same time, which only works on disjoint fields.

fn capture_frame(config: &Config, session: &mut Session, tick: u64) {
    let mut frame = Frame {
        tick,
        dimension: String::new(),
        players: Vec::new(),
    };
    for player in players::online() {
        if frame.dimension.is_empty() {
            frame.dimension = player.dimension.clone();
        }
        frame.players.push(PlayerSample {
            name: player.name.clone(),
            x: player.x,
            y: player.y,
            z: player.z,
            game_mode: match player.game_mode {
                players::GameMode::Survival => "survival".into(),
                players::GameMode::Creative => "creative".into(),
                players::GameMode::Adventure => "adventure".into(),
                players::GameMode::Spectator => "spectator".into(),
            },
            health: player.health,
        });
    }
    let previous = session.recording.frames.last().cloned();
    if let Some(previous) = &previous {
        // A teleport has no event of its own: a jump between samples is the
        // observable trace of one.
        if config.bool_of("auto_keyframe_on_teleport") {
            let jumped = previous.players.iter().any(|old| {
                frame
                    .players
                    .iter()
                    .filter(|new| new.name == old.name)
                    .any(|new| {
                        let dx = new.x - old.x;
                        let dy = new.y - old.y;
                        let dz = new.z - old.z;
                        dx * dx + dy * dy + dz * dz > 64.0
                    })
            });
            if jumped {
                if let Some(new) = frame.players.first() {
                    recording_push_keyframe(&mut session.recording, tick, new.x, new.y, new.z);
                }
            }
        }
        // A dimension change is a replay chapter of its own.
        if config.bool_of("record_dimension_changes")
            && !previous.dimension.is_empty()
            && !frame.dimension.is_empty()
            && previous.dimension != frame.dimension
        {
            if let Some(new) = frame.players.first() {
                recording_push_keyframe(&mut session.recording, tick, new.x, new.y, new.z);
            }
        }
    }
    session.recording.frames.push(frame);
    session.last_frame = tick;
}

fn capture_keyframe(config: &Config, session: &mut Session, tick: u64) {
    let online = players::online();
    let player = match online.first() {
        Some(player) => player,
        None => return,
    };
    let index =
        recording_push_keyframe(&mut session.recording, tick, player.x, player.y, player.z);
    session.last_keyframe = tick;

    let mut radius = config.int_of("chunk_radius").clamp(0, 16) as i32;
    if config.bool_of("safe_mode") {
        radius = radius.min(4);
    }
    if radius == 0 {
        return;
    }
    let base_x = player.x.floor() as i32;
    let base_y = player.y.floor() as i32;
    let base_z = player.z.floor() as i32;
    let mut taken = 0usize;
    'scan: for dy in -1..=1 {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if taken >= MAX_CELLS_PER_KEYFRAME {
                    break 'scan;
                }
                let (x, y, z) = (base_x + dx, base_y + dy, base_z + dz);
                if let Some(state) = world::get_block(&player.dimension, x, y, z) {
                    session.recording.cells.push(Cell {
                        keyframe: index,
                        x,
                        y,
                        z,
                        state,
                    });
                    taken += 1;
                }
            }
        }
    }
}

/// Turns a recording into the actions playback runs, in tick order.
fn build_schedule(
    recording: &Recording,
    skip_to_keyframe: bool,
    show_keyframes: bool,
) -> Vec<Scheduled> {
    let mut out: Vec<Scheduled> = Vec::new();
    let start = if skip_to_keyframe {
        recording
            .keyframes
            .first()
            .map_or(recording.start_tick(), |key| key.tick)
    } else {
        recording.start_tick()
    };
    let dimension = recording
        .frames
        .first()
        .map(|frame| frame.dimension.clone())
        .unwrap_or_else(|| "minecraft:overworld".into());

    let mut current_tick: Option<u64> = None;
    let mut batch: Vec<SdkBlockChange> = Vec::new();
    for change in &recording.blocks {
        if change.tick < start {
            continue;
        }
        if current_tick != Some(change.tick) {
            if let Some(tick) = current_tick {
                out.push(Scheduled {
                    tick,
                    action: Action::Blocks {
                        dimension: dimension.clone(),
                        changes: std::mem::take(&mut batch),
                    },
                });
            }
            current_tick = Some(change.tick);
        }
        batch.push(SdkBlockChange {
            x: change.x,
            y: change.y,
            z: change.z,
            state: change.state.clone(),
        });
    }
    if let Some(tick) = current_tick {
        out.push(Scheduled {
            tick,
            action: Action::Blocks {
                dimension,
                changes: batch,
            },
        });
    }
    for line in &recording.chat {
        if line.tick >= start {
            out.push(Scheduled {
                tick: line.tick,
                action: Action::Chat(format!("<{}> {}", line.player, line.text)),
            });
        }
    }
    if show_keyframes {
        for key in &recording.keyframes {
            if key.tick >= start {
                out.push(Scheduled {
                    tick: key.tick,
                    action: Action::Note(format!(
                        "Volt Lapse: keyframe at {:.1} {:.1} {:.1}",
                        key.x, key.y, key.z
                    )),
                });
            }
        }
    }
    out.sort_by_key(|scheduled| scheduled.tick);
    out
}

// ---------------------------------------------------------------- the mod ----

impl VoltLapse {
    // ------------------------------------------------------------ config ----

    fn load_config() -> Config {
        match read_key(CONFIG_KEY) {
            Some(text) => match Config::parse(&text) {
                Ok(config) => config,
                Err(error) => {
                    warn(&format!(
                        "stored configuration is invalid ({error}); using the shipped defaults"
                    ));
                    Config::parse(SHIPPED_CONFIG).unwrap_or_default()
                }
            },
            None => Config::parse(SHIPPED_CONFIG).unwrap_or_default(),
        }
    }

    fn level_rank(name: &str) -> u8 {
        match name {
            "error" => 0,
            "warn" => 1,
            "info" => 2,
            "debug" => 3,
            _ => 4,
        }
    }

    /// `log_level` decides what is written to the server log.
    fn log_info(&self, message: &str) {
        if Self::level_rank(self.config.text_of("log_level")) >= 2 {
            info(message);
        }
    }

    fn log_debug(&self, message: &str) {
        if self.config.bool_of("debug_logging")
            && Self::level_rank(self.config.text_of("log_level")) >= 3
        {
            info(message);
        }
    }

    fn save_config(&self) {
        if let Err(error) = storage::set(CONFIG_KEY, self.config.render().as_bytes()) {
            warn(&format!("Volt Lapse: could not persist the configuration: {error}"));
        }
    }

    /// The settings whose effect needs an interface this API revision lacks.
    fn unsupported(&self) -> Vec<&'static str> {
        let defaults = Config::default();
        NO_API
            .iter()
            .filter(|(key, _)| self.config.get(key) != defaults.get(key))
            .map(|(key, _)| *key)
            .collect()
    }

    // ----------------------------------------------------------- storage ----

    fn size_cap(&self) -> usize {
        let wanted = (self.config.int_of("max_recording_size_mb").max(1) as usize) * 1024 * 1024;
        let mut cap = wanted.min(MAX_RECORDING_BYTES);
        if self.config.bool_of("safe_mode") {
            // safe_mode: keep a recording well inside one hook call's budget.
            cap = cap.min(WRITE_BUDGET);
        }
        cap
    }

    fn chunk_bytes(&self) -> usize {
        let wanted = (self.config.int_of("write_chunk_size_kb").max(1) as usize) * 1024;
        let mut size = wanted.clamp(1024, WRITE_BUDGET);
        if self.config.bool_of("safe_mode") {
            size = size.min(256 * 1024);
        }
        size
    }

    /// Writes queued items while this tick's 1 MiB budget lasts; the rest waits.
    fn drain_queue(&mut self) {
        loop {
            let Some(item) = self.queue.first() else { break };
            let key = item.key.clone();
            let body = item.body.clone();
            if body.len() > self.budget {
                break;
            }
            match storage::set(&key, body.as_bytes()) {
                Ok(()) => {
                    self.budget = self.budget.saturating_sub(body.len());
                    self.queue.remove(0);
                    self.log_debug(&format!("Volt Lapse: wrote {key} ({} bytes)", body.len()));
                }
                Err(error) => {
                    self.queue[0].attempts += 1;
                    let attempts = self.queue[0].attempts;
                    warn(&format!("Volt Lapse: {key} was refused ({error}), attempt {attempts}"));
                    if attempts >= 3 || !self.make_room(&key) {
                        self.queue.remove(0);
                    }
                    break;
                }
            }
        }
    }

    /// Deletes the oldest recording that is not part of `keep_key`.
    fn make_room(&mut self, keep_key: &str) -> bool {
        for (ns, name) in index_entries() {
            let base = rec_base(&ns, &name);
            if keep_key.starts_with(&base) {
                continue;
            }
            self.drop_recording(&ns, &name);
            warn(&format!(
                "Volt Lapse: deleted the oldest recording ({name}) to make room in storage"
            ));
            return true;
        }
        false
    }

    fn drop_recording(&self, ns: &str, name: &str) {
        let base = rec_base(ns, name);
        if let Some(meta) = read_key(&format!("{base}.meta")) {
            if let Some(chunks) = meta
                .split_whitespace()
                .find_map(|field| field.strip_prefix("chunks="))
                .and_then(|value| value.parse::<usize>().ok())
            {
                for index in 0..chunks {
                    storage::delete(&format!("{base}.{index}"));
                }
            }
        }
        storage::delete(&format!("{base}.meta"));
        storage::delete(&format!("{base}.draft"));
        let entries: Vec<(String, String)> = index_entries()
            .into_iter()
            .filter(|(entry_ns, entry_name)| !(entry_ns == ns && entry_name == name))
            .collect();
        write_index(&entries);
    }

    /// `max_replays_kept`, oldest first. 0 keeps everything.
    fn prune_kept(&self) {
        let keep = self.config.int_of("max_replays_kept").max(0) as usize;
        if keep == 0 {
            return;
        }
        let entries = index_entries();
        if entries.len() <= keep {
            return;
        }
        // Oldest are at the front: keep the newest `keep` of them.
        for (ns, name) in entries.iter().take(entries.len().saturating_sub(keep)) {
            self.drop_recording(ns, name);
        }
    }

    // --------------------------------------------------------- recordings ----

    fn queue_recording(&mut self, recording: &Recording) -> Result<String, String> {
        let level = self.config.int_of("compression_level");
        let mut recording = recording.clone();
        let cap = self.size_cap();
        let mut text = recording.render_with(level);
        let mut trimmed = false;
        while text.len() > cap
            && (recording.frames.len() > 1
                || recording.cells.len() > 1
                || recording.blocks.len() > 1)
        {
            trimmed = true;
            if recording.frames.len() > 1 {
                recording.frames.pop();
            } else if recording.cells.len() > 1 {
                recording.cells.pop();
            } else {
                recording.blocks.pop();
            }
            text = recording.render_with(level);
        }
        if text.len() > cap {
            return Err(format!(
                "the recording is over the {cap}-byte cap (max_recording_size_mb)"
            ));
        }

        let ns = namespace(&self.config, "replay_folder", "replays");
        let name = recording_name(&recording.name);
        let base = rec_base(&ns, &name);
        let chunks = split_chunks(&text, self.chunk_bytes());
        let meta = format!(
            "chunks={} created={} bytes={} trimmed={}",
            chunks.len(),
            recording.created_tick,
            text.len(),
            trimmed
        );
        let atomic = self.config.bool_of("atomic_write");
        let mut items: Vec<QueueItem> = Vec::new();
        if atomic {
            // Manifest last: a half-written recording is never listed.
            for (index, body) in chunks.iter().enumerate() {
                items.push(QueueItem {
                    key: format!("{base}.{index}"),
                    body: body.clone(),
                    attempts: 0,
                });
            }
            items.push(QueueItem {
                key: format!("{base}.meta"),
                body: meta,
                attempts: 0,
            });
        } else {
            items.push(QueueItem {
                key: format!("{base}.meta"),
                body: meta,
                attempts: 0,
            });
            for (index, body) in chunks.iter().enumerate() {
                items.push(QueueItem {
                    key: format!("{base}.{index}"),
                    body: body.clone(),
                    attempts: 0,
                });
            }
        }
        self.queue.extend(items);

        let mut entries = index_entries();
        if !entries
            .iter()
            .any(|(entry_ns, entry_name)| entry_ns == &ns && entry_name == &name)
        {
            entries.push((ns.clone(), name.clone()));
            write_index(&entries);
        }
        let temp = namespace(&self.config, "temp_folder", "replays_tmp");
        storage::delete(&format!("{temp}.rec.{name}.draft"));
        self.prune_kept();
        Ok(format!(
            "saving {name} ({} chunk{}{})",
            chunks.len(),
            if chunks.len() == 1 { "" } else { "s" },
            if trimmed { ", trimmed to the cap" } else { "" }
        ))
    }

    fn find_entry(&self, name: &str) -> Option<(String, String)> {
        let wanted = recording_name(name);
        index_entries()
            .into_iter()
            .rev()
            .find(|(_, entry_name)| entry_name == &wanted)
    }

    fn load_recording(&self, name: &str) -> Result<Recording, String> {
        let (ns, entry_name) = self
            .find_entry(name)
            .ok_or_else(|| format!("no recording called {name:?} (try /vl list)"))?;
        let base = rec_base(&ns, &entry_name);
        let meta = read_key(&format!("{base}.meta"))
            .ok_or_else(|| format!("recording {entry_name:?} is still being written"))?;
        let chunks: usize = meta
            .split_whitespace()
            .find_map(|field| field.strip_prefix("chunks="))
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| format!("recording {entry_name:?} has a damaged manifest"))?;
        let mut text = String::new();
        for index in 0..chunks {
            let chunk = read_key(&format!("{base}.{index}"))
                .ok_or_else(|| format!("recording {entry_name:?} is missing chunk {index}"))?;
            text.push_str(&chunk);
        }
        Recording::parse(&text)
    }

    // ----------------------------------------------------------- capture ----

    /// `capture_fps` caps the sampling rate: the effective stride is the larger
    /// of `record_interval_ticks` and the stride that rate implies.
    fn effective_interval(&self) -> u64 {
        let ticks = self.config.int_of("record_interval_ticks").max(1) as u64;
        let fps = self.config.int_of("capture_fps").clamp(1, 60) as u64;
        let from_fps = (TICKS_PER_SECOND + fps - 1) / fps;
        ticks.max(from_fps.max(1))
    }

    fn start_recording(&mut self, wanted: &str, tick: u64) -> Result<String, String> {
        if self.session.is_some() {
            return Err("already recording (use /vl stop)".into());
        }
        if !self.config.bool_of("enabled") {
            return Err(
                "recording is off (enabled = false); turn it on with /vl config enabled true".into(),
            );
        }
        let name = file_name_for(&self.config, wanted, tick);
        if name.is_empty() {
            return Err("file_naming = manual needs a name: /vl record <name>".into());
        }
        let name = recording_name(&name);
        self.session = Some(Session {
            recording: Recording::new(name.clone(), tick),
            name,
            started: tick,
            last_frame: tick,
            last_keyframe: tick,
        });
        self.last_flush = tick;
        Ok(format!(
            "recording as {name:?} (every {} ticks, keyframe every {} ticks, {} fps target)",
            self.effective_interval(),
            self.config.int_of("keyframe_interval_ticks"),
            self.config.int_of("capture_fps")
        ))
    }

    fn stop_recording(&mut self, tick: u64) -> Result<String, String> {
        let session = self
            .session
            .take()
            .ok_or("not recording (use /vl record)")?;
        let ticks = tick.saturating_sub(session.started);
        let message = self.queue_recording(&session.recording)?;
        self.metrics.recordings += 1;
        Ok(format!("stopped after {ticks} ticks: {message}"))
    }

    /// `buffer_flush_interval_sec`: before the recording is finished its buffer
    /// is spilled to `temp_folder` every interval, so a crash costs one interval.
    fn spill_draft(&mut self, tick: u64) {
        if !self.config.bool_of("experimental_features") {
            return;
        }
        let interval = (self.config.int_of("buffer_flush_interval_sec").max(1) as u64)
            * TICKS_PER_SECOND;
        if tick.saturating_sub(self.last_flush) < interval {
            return;
        }
        let (name, body) = match self.session.as_ref() {
            Some(session) => {
                let text = session.recording.render_with(self.config.int_of("compression_level"));
                (session.name.clone(), clip(&text, MAX_DRAFT_BYTES))
            }
            None => return,
        };
        self.last_flush = tick;
        let temp = namespace(&self.config, "temp_folder", "replays_tmp");
        let key = format!("{temp}.rec.{name}.draft");
        if body.len() > self.budget {
            return;
        }
        match storage::set(&key, body.as_bytes()) {
            Ok(()) => {
                self.budget = self.budget.saturating_sub(body.len());
                self.log_debug(&format!("Volt Lapse: flushed a draft of {name}"));
            }
            Err(error) => warn(&format!("Volt Lapse: could not flush a draft: {error}")),
        }
    }

    // ---------------------------------------------------------- playback ----

    fn start_playback(&mut self, args: &str, tick: u64) -> Result<String, String> {
        let mut parts = args.split_whitespace();
        let first = parts.next().unwrap_or("");
        let (name, speed_text) = if first.is_empty() || first.parse::<f64>().is_ok() {
            let newest = index_entries()
                .last()
                .map(|(_, name)| name.clone())
                .ok_or("no recordings yet: /vl record, then /vl stop")?;
            (newest, first)
        } else {
            (first.to_string(), parts.next().unwrap_or(""))
        };
        if self.playback.is_some() {
            return Err("already playing: wait for it to finish".into());
        }
        let recording = self.load_recording(&name)?;
        let speed = if speed_text.trim().is_empty() {
            self.config.float_of("default_speed")
        } else if let Some(index) = speed_text
            .strip_prefix('p')
            .and_then(|rest| rest.parse::<usize>().ok())
        {
            // `p1`..`p16`: a slot in speed_presets.
            let presets = self.config.list_of("speed_presets");
            *presets
                .get(index.saturating_sub(1))
                .ok_or_else(|| format!("speed preset p{index} is not in speed_presets"))?
        } else {
            let value: f64 = speed_text
                .trim()
                .parse()
                .map_err(|_| format!("speed {speed_text:?} is not a number, and not p1..p16"))?;
            if !value.is_finite() || !(0.1..=8.0).contains(&value) {
                return Err(format!("speed {value} is outside 0.1..=8"));
            }
            value
        };
        let mut speed = speed;
        if self.config.bool_of("safe_mode") {
            speed = speed.min(4.0);
        }
        let skip = self.config.bool_of("skip_to_keyframe_on_load");
        let schedule = build_schedule(&recording, skip, self.config.bool_of("show_keyframes"));
        let origin = recording.start_tick();
        let end = recording.end_tick();
        let actions = schedule.len();
        let keyframes = recording.keyframes.len();
        let span = end.saturating_sub(origin);
        self.playback = Some(Playback {
            name: recording.name.clone(),
            origin,
            end,
            started: tick,
            cursor: 0,
            schedule,
            speed,
        });
        Ok(format!(
            "playing {:?} at {speed}x: {span} ticks, {keyframes} keyframes, {actions} scheduled actions",
            recording.name
        ))
    }

    fn tick_playback(&mut self, tick: u64) {
        let looped = self.config.bool_of("loop_playback");
        enum Outcome {
            Keep,
            Loop,
            Done(String),
        }
        let Some(playback) = self.playback.as_mut() else {
            return;
        };
        let elapsed = tick.saturating_sub(playback.started);
        let position = playback.origin + (elapsed as f64 * playback.speed) as u64;
        let mut writes = 0u64;
        while playback.cursor < playback.schedule.len()
            && playback.schedule[playback.cursor].tick <= position
        {
            let action = &playback.schedule[playback.cursor].action;
            match action {
                Action::Blocks {
                    dimension,
                    changes,
                } => {
                    match world::set_blocks(dimension, changes) {
                        Ok(()) => writes += 1,
                        Err(error) => {
                            warn(&format!("Volt Lapse: replay could not set blocks: {error}"));
                        }
                    }
                }
                Action::Chat(line) => players::broadcast(line),
                Action::Note(note) => players::broadcast(note),
            }
            playback.cursor += 1;
        }
        let outcome = if position > playback.end && playback.cursor >= playback.schedule.len() {
            if looped {
                playback.started = tick;
                playback.cursor = 0;
                Outcome::Loop
            } else {
                Outcome::Done(playback.name.clone())
            }
        } else {
            Outcome::Keep
        };
        self.metrics.blocks_replayed += writes;
        match outcome {
            Outcome::Keep => {}
            Outcome::Loop => players::broadcast("Volt Lapse: replay looped"),
            Outcome::Done(name) => {
                self.playback = None;
                players::broadcast(&format!("Volt Lapse: {name} finished"));
            }
        }
    }

    // ------------------------------------------------------------ status ----

    fn status(&self, tick: u64) -> String {
        let recording = match &self.session {
            Some(session) => format!(
                "recording {:?} ({} frames, {} ticks)",
                session.name,
                session.recording.frames.len(),
                tick.saturating_sub(session.started)
            ),
            None => "not recording".to_string(),
        };
        let playback = match &self.playback {
            Some(playback) => format!(
                "playing {:?} at {}x ({} of {} actions)",
                playback.name,
                playback.speed,
                playback.cursor,
                playback.schedule.len()
            ),
            None => "not playing".to_string(),
        };
        let mut text = format!("Volt Lapse: {recording}; {playback}");
        if self.config.bool_of("metrics_opt_in") {
            // Local counters only: nothing is ever sent anywhere.
            text.push_str(&format!(
                "; local metrics: {} frames, {} replay writes, {} recordings",
                self.metrics.frames, self.metrics.blocks_replayed, self.metrics.recordings
            ));
        }
        if self.config.bool_of("experimental_features") {
            text.push_str(&format!("; {} queued writes", self.queue.len()));
        }
        text
    }

    fn config_command(&mut self, key: &str, value: &str) -> Result<Option<String>, String> {
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() {
            let changed = self.config.changed();
            if changed.is_empty() {
                return Ok(Some(format!(
                    "Volt Lapse: all {} settings are at their defaults (use /vl config <key>)",
                    SPECS.len()
                )));
            }
            return Ok(Some(format!(
                "Volt Lapse: {} of {} settings changed: {}",
                changed.len(),
                SPECS.len(),
                changed.join("; ")
            )));
        }
        if key.eq_ignore_ascii_case("reset") {
            self.config.reset();
            self.save_config();
            return Ok(Some(format!(
                "Volt Lapse: all {} settings reset to their defaults",
                SPECS.len()
            )));
        }
        let spec = settings::spec_for(key)
            .ok_or_else(|| format!("unknown setting {key:?} (see /vl config)"))?;
        if value.is_empty() {
            let current = self
                .config
                .get(spec.key)
                .map(Value::render)
                .unwrap_or_else(|| "<unset>".into());
            return Ok(Some(format!(
                "{} = {} ({})",
                spec.key,
                current,
                spec.kind.type_name()
            )));
        }
        self.config
            .set(spec.key, value)
            .map_err(|error| format!("{error} (expected {})", spec.kind.expect()))?;
        self.save_config();
        self.log_debug(&format!("Volt Lapse: {} is now {value}", spec.key));
        let rendered = self
            .config
            .get(spec.key)
            .map(Value::render)
            .unwrap_or_default();
        Ok(Some(format!("{} = {}", spec.key, rendered)))
    }

    fn path_command(&self, wanted: &str) -> Result<String, String> {
        let recording = if wanted.trim().is_empty() {
            match &self.session {
                Some(session) => session.recording.clone(),
                None => {
                    let newest = index_entries()
                        .last()
                        .map(|(_, name)| name.clone())
                        .ok_or("no keyframes yet: /vl record, or /vl keyframe")?;
                    self.load_recording(&newest)?
                }
            }
        } else {
            self.load_recording(wanted)?
        };
        if recording.keyframes.is_empty() {
            return Err("that recording has no keyframes (/vl keyframe adds one)".into());
        }
        if !self.config.bool_of("show_path_preview") {
            return Ok(format!(
                "Volt Lapse: {} keyframes (path preview is off: show_path_preview = false)",
                recording.keyframes.len()
            ));
        }
        // `smooth_camera = false` samples straight lines whatever the mode says.
        let mode = if self.config.bool_of("smooth_camera") {
            Interpolation::parse(self.config.text_of("camera_interpolation"))
                .unwrap_or(Interpolation::CatmullRom)
        } else {
            Interpolation::Linear
        };
        let keys: Vec<CameraKeyframe> = recording
            .keyframes
            .iter()
            .map(|key| CameraKeyframe {
                tick: key.tick,
                x: key.x,
                y: key.y,
                z: key.z,
            })
            .collect();
        let from = recording.start_tick();
        let to = recording.end_tick();
        let snap = self.config.int_of("keyframe_snap_ticks").max(1) as u64;
        let samples = preview(
            &keys,
            from,
            to,
            snap.max(to.saturating_sub(from) / 64 + 1),
            mode,
            self.config.float_of("path_tension"),
            self.config.float_of("path_smoothing"),
        );
        let first = samples.first().copied().unwrap_or((from, 0.0, 0.0, 0.0));
        let last = samples.last().copied().unwrap_or(first);
        Ok(format!(
            "Volt Lapse: {} keyframes, {} ticks, {} {} samples; from ({:.1}, {:.1}, {:.1}) to ({:.1}, {:.1}, {:.1})",
            keys.len(),
            to.saturating_sub(from),
            samples.len(),
            mode.name(),
            first.1,
            first.2,
            first.3,
            last.1,
            last.2,
            last.3
        ))
    }

    fn list_command(&self) -> String {
        let entries = index_entries();
        if entries.is_empty() {
            return "Volt Lapse: no recordings yet (/vl record, then /vl stop)".into();
        }
        let mut lines = Vec::new();
        for (_, name) in entries.iter().rev().take(6) {
            match self.load_recording(name) {
                Ok(recording) => lines.push(recording.summary()),
                Err(error) => lines.push(format!("{name}: {error}")),
            }
        }
        format!(
            "Volt Lapse: {} recording(s): {}",
            entries.len(),
            lines.join(" | ")
        )
    }

    fn record_block(&mut self, placed: bool, pos: horizon_mod_sdk::server::BlockPos, state: &str) {
        if !self.config.bool_of("record_block_changes") {
            return;
        }
        let tick = self.last_tick;
        if let Some(session) = self.session.as_mut() {
            session.recording.blocks.push(BlockChange {
                tick,
                placed,
                x: pos.x,
                y: pos.y,
                z: pos.z,
                state: state.to_string(),
            });
        }
    }
}

// ---------------------------------------------------------------- the mod ----

impl ServerMod for VoltLapse {
    fn init() -> Result<(Self, Registration), String> {
        let config = Self::load_config();
        let mut mod_state = Self {
            config,
            session: None,
            playback: None,
            last_tick: 0,
            auto_started: false,
            queue: Vec::new(),
            budget: WRITE_BUDGET,
            last_flush: 0,
            metrics: Metrics::default(),
        };
        mod_state.log_info(&format!(
            "Volt Lapse {}: {} settings, every {} ticks, keyframe every {} ticks",
            env!("CARGO_PKG_VERSION"),
            SPECS.len(),
            mod_state.effective_interval(),
            mod_state.config.int_of("keyframe_interval_ticks")
        ));
        let unsupported = mod_state.unsupported();
        if !unsupported.is_empty() {
            warn(&format!(
                "Volt Lapse: {} setting(s) need interfaces this API revision does not have, so they \
                 are stored but not applied: {} (see README \"API gaps\")",
                unsupported.len(),
                unsupported.join(", ")
            ));
        }
        Ok((
            mod_state,
            // Both names run the same command; `/vl` is the short one.
            Registration::new()
                .command("voltlapse")
                .command("vl")
                .on_tick()
                .on_chat()
                .on_blocks()
                .on_joins(),
        ))
    }

    fn tick(&mut self, tick: u64) {
        self.last_tick = tick;
        self.budget = WRITE_BUDGET;
        self.drain_queue();

        if !self.auto_started {
            self.auto_started = true;
            if self.config.bool_of("auto_start_on_world_load") && self.session.is_none() {
                // The legacy API has no world-load event; the first tick after the
                // mod loads is the closest observable equivalent.
                match self.start_recording("", tick) {
                    Ok(message) => players::broadcast(&format!("Volt Lapse: {message}")),
                    Err(error) => warn(&format!("Volt Lapse: auto_start_on_world_load: {error}")),
                }
            }
        }

        if self.session.is_some() {
            let interval = self.effective_interval();
            let keyframe_interval = self.config.int_of("keyframe_interval_ticks").max(1) as u64;
            let duration_cap =
                (self.config.int_of("max_recording_duration_min") as u64) * TICKS_PER_MINUTE;
            let mut stop = false;
            // Disjoint field borrows: `self.config` shared, `self.session` unique.
            let config = &self.config;
            if let Some(session) = self.session.as_mut() {
                if tick.saturating_sub(session.started) >= duration_cap {
                    stop = true;
                } else {
                    if tick.saturating_sub(session.last_frame) >= interval {
                        capture_frame(config, session, tick);
                        self.metrics.frames += 1;
                    }
                    if tick.saturating_sub(session.last_keyframe) >= keyframe_interval {
                        capture_keyframe(config, session, tick);
                    }
                }
            }
            if stop {
                match self.stop_recording(tick) {
                    Ok(message) => players::broadcast(&format!(
                        "Volt Lapse: recording hit max_recording_duration_min; {message}"
                    )),
                    Err(error) => warn(&format!("Volt Lapse: automatic stop failed: {error}")),
                }
            }
            self.spill_draft(tick);
        }

        self.tick_playback(tick);
    }

    fn player_joined(&mut self, player: &Player) {
        if self.session.is_none() && self.config.bool_of("auto_start_on_join") {
            let message = match self.start_recording("", self.last_tick) {
                Ok(message) => format!("Volt Lapse: {message}"),
                Err(error) => format!("Volt Lapse: {error}"),
            };
            players::tell(&player.id, &message);
        }
    }

    fn chat(&mut self, player: &Player, text: &str) {
        if !self.config.bool_of("record_chat") {
            return;
        }
        let tick = self.last_tick;
        if let Some(session) = self.session.as_mut() {
            session.recording.chat.push(ChatLine {
                tick,
                player: player.name.clone(),
                text: text.to_string(),
            });
        }
    }

    fn block_placed(&mut self, _player: &Player, pos: horizon_mod_sdk::server::BlockPos, state: &str) {
        self.record_block(true, pos, state);
    }

    fn block_broken(&mut self, _player: &Player, pos: horizon_mod_sdk::server::BlockPos, state: &str) {
        self.record_block(false, pos, state);
    }

    fn command(&mut self, _player: &Player, name: &str, args: &Args) -> Result<Option<String>, String> {
        if name != "voltlapse" && name != "vl" {
            return Err(format!("Volt Lapse owns no command called {name}"));
        }
        self.budget = WRITE_BUDGET;
        let text = args.text().trim();
        let mut parts = text.splitn(3, char::is_whitespace);
        let sub = parts.next().unwrap_or("").to_ascii_lowercase();
        let first = parts.next().unwrap_or("").trim().to_string();
        let rest = parts.next().unwrap_or("").trim().to_string();
        let tick = self.last_tick;

        match sub.as_str() {
            "" | "status" => Ok(Some(self.status(tick))),
            "help" => Ok(Some(HELP.into())),
            "record" | "start" => {
                let label = if first.is_empty() { rest.as_str() } else { first.as_str() };
                self.start_recording(label, tick).map(Some)
            }
            "stop" => self.stop_recording(tick).map(Some),
            "play" => {
                let combined = if rest.is_empty() {
                    first.clone()
                } else {
                    format!("{first} {rest}")
                };
                self.start_playback(&combined, tick).map(Some)
            }
            "list" => Ok(Some(self.list_command())),
            "config" | "settings" => {
                let (key, value) = if rest.is_empty() {
                    (first.clone(), String::new())
                } else {
                    (first.clone(), rest.clone())
                };
                self.config_command(&key, &value)
            }
            "keyframe" => {
                let online = players::online();
                if online.is_empty() {
                    return Err("no player online to take a keyframe from".into());
                }
                let player = &online[0];
                let session = self
                    .session
                    .as_mut()
                    .ok_or("keyframes go into a recording: /vl record first")?;
                let index = recording_push_keyframe(
                    &mut session.recording,
                    tick,
                    player.x,
                    player.y,
                    player.z,
                );
                Ok(Some(format!(
                    "Volt Lapse: keyframe {index} at {:.1} {:.1} {:.1} (tick {tick})",
                    player.x, player.y, player.z
                )))
            }
            "path" => self.path_command(&first).map(Some),
            other => Err(format!("unknown subcommand {other:?}; {HELP}")),
        }
    }
}

horizon_mod_sdk::export_server_mod!(VoltLapse);

#[cfg(test)]
mod tests {
    use super::*;

    fn recording_with_actions() -> Recording {
        let mut recording = Recording::new("demo", 100);
        recording.frames.push(Frame {
            tick: 100,
            dimension: "minecraft:overworld".into(),
            players: vec![PlayerSample {
                name: "Alex".into(),
                x: 0.0,
                y: 64.0,
                z: 0.0,
                game_mode: "survival".into(),
                health: 20.0,
            }],
        });
        recording.blocks.push(BlockChange {
            tick: 120,
            placed: true,
            x: 1,
            y: 64,
            z: 0,
            state: "minecraft:stone".into(),
        });
        recording.blocks.push(BlockChange {
            tick: 120,
            placed: true,
            x: 2,
            y: 64,
            z: 0,
            state: "minecraft:stone".into(),
        });
        recording.chat.push(ChatLine {
            tick: 130,
            player: "Alex".into(),
            text: "hi".into(),
        });
        recording.keyframes.push(Keyframe {
            tick: 110,
            x: 0.0,
            y: 64.0,
            z: 0.0,
        });
        recording
    }

    #[test]
    fn the_schedule_batches_blocks_per_tick_and_can_skip_to_the_keyframe() {
        let schedule = build_schedule(&recording_with_actions(), true, true);
        let ticks: Vec<u64> = schedule.iter().map(|s| s.tick).collect();
        assert_eq!(ticks, vec![110, 120, 130], "ordered, 120's blocks batched");
        match &schedule[1].action {
            Action::Blocks { changes, .. } => assert_eq!(changes.len(), 2),
            _ => panic!("expected blocks at 120"),
        }
        let without_skip = build_schedule(&recording_with_actions(), false, false);
        assert_eq!(without_skip.len(), 2, "no keyframe note");
    }

    #[test]
    fn names_and_namespaces_are_storage_safe() {
        // [a-z0-9_.-]{1,64}, the host's rule.
        assert_eq!(recording_name("My Replay #1"), "my_replay_1");
        assert_eq!(recording_name("   "), "recording");
        assert_eq!(recording_name("a/b"), "a_b");
        assert_eq!(recording_name(&"x".repeat(200)).len(), MAX_NAME);
        let config = Config::default();
        assert_eq!(namespace(&config, "replay_folder", "replays"), "replays");
        assert_eq!(namespace(&config, "temp_folder", "replays_tmp"), "replays_tmp");
        let mut config = Config::default();
        config.set("replay_folder", "My Folder").unwrap();
        assert_eq!(namespace(&config, "replay_folder", "replays"), "my_folder");
    }

    #[test]
    fn file_naming_honours_the_setting() {
        let mut config = Config::default();
        assert_eq!(file_name_for(&config, "", 42), "rec-42");
        assert_eq!(file_name_for(&config, "named", 42), "named");
        config.set("file_naming", "manual").unwrap();
        assert_eq!(file_name_for(&config, "", 42), "");
        config.set("file_naming", "world_name").unwrap();
        assert_eq!(file_name_for(&config, "", 7), "world-7");
    }

    #[test]
    fn chunks_split_on_character_boundaries() {
        let text = "érr".repeat(400);
        let chunks = split_chunks(&text, 100);
        assert_eq!(chunks.concat(), text);
        for chunk in &chunks {
            assert!(chunk.len() <= 100, "{} bytes", chunk.len());
            assert!(std::str::from_utf8(chunk.as_bytes()).is_ok());
        }
        assert_eq!(split_chunks("", 10), vec![String::new()]);
    }

    #[test]
    fn every_setting_is_documented_and_has_a_value() {
        assert_eq!(SPECS.len(), 69);
        for spec in SPECS {
            assert!(!spec.key.is_empty());
            assert!(!spec.default.is_empty());
        }
        // The file people read, the table the code uses: one list.
        assert_eq!(Config::parse(SHIPPED_CONFIG).unwrap(), Config::default());
        let shipped_keys = SHIPPED_CONFIG
            .lines()
            .filter(|line| line.contains(" = "))
            .count();
        assert_eq!(shipped_keys, SPECS.len());
    }

    #[test]
    fn every_no_api_entry_names_a_real_setting() {
        for (key, why) in NO_API {
            assert!(settings::spec_for(key).is_some(), "{key} is not a setting");
            assert!(!why.is_empty());
        }
    }
}
