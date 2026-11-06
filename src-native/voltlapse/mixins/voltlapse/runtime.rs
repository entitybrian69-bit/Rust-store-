// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

// Volt Lapse: the game-side half.
//
// `horizon-mixins` copies this file into `crates/horizon-client/src/game.rs` as
// `pub(crate) mod voltlapse`, so everything here can read the game's own state
// without a bridge, and the bundle's hook helpers call into it by path.
//
// All the durable logic (the recording format, playback, camera paths,
// rendering, settings, the library) lives in the `voltlapse_engine` crate; this
// module is only the glue: it samples the game each tick, drives playback from
// the wall clock, answers the camera and FOV hooks, draws the HUD and the
// screens, and turns input into library calls.
//
// Layout
//  * constants and the mod's key list
//  * `State`: the engine plus the UI and per-session scratch
//  * `tick` / `frame`: sampling and the transport
//  * `camera_pose` / `fov`: the cinematic camera
//  * `hud` / `gui_post`: the indicator, transport bar, overlays and screens
//  * `key` / `mouse` / `wheel` / `command`: input
//  * `lifecycle` / `packet` / `chat` / `blocks` / `entities`: recording sources
//  * rendering: the screenshot-driven frame loop

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use horizon_client::hud::HudBatch;
use horizon_client::text::Text;
use voltlapse_engine as engine;

use engine::camera::{CameraMode, CameraTarget};
use engine::command::{self, Command};
use engine::config::Config;
use engine::event::{BlockPos, ChatLine, Event, ItemStack, LightUpdate, ParticleEvent, Slot, SoundEvent};
use engine::hud::{Anchor, Rect};
use engine::keybinds::{Action, KeyEvent};
use engine::keyframe::{CameraPath, Keyframe};
use engine::package::{PlayerInfo, RecordingOrigin, ReplayPackage};
use engine::platform::Platform;
use engine::playback::Playback;
use engine::render::{RenderSettings, VideoFormat};
use engine::storage::ReplayEntry;
use engine::wire::Vec3;
use engine::Engine;

/// The mod's id: settings folder, hook keys, translation namespace.
pub(crate) const ID: &str = "voltlapse";
/// Display name.
pub(crate) const NAME: &str = "Volt Lapse";
/// Kept in step with `horizon-native.json` and the content `.hmod`.
pub(crate) const VERSION: &str = "1.0.0";
/// Shown on the Mods page.
pub(crate) const DESCRIPTION: &str =
    "Record your world, replay it, fly a cinematic camera path and render it to video.";

/// The mod's key settings, as the Mods page stores them (rebindable there).
pub(crate) const KEYS: [(&str, &str, &str); 3] = [
    ("record", "key.voltlapse.record", "key.keyboard.r"),
    ("viewer", "key.voltlapse.viewer", "key.keyboard.v"),
    ("editor", "key.voltlapse.editor", "key.keyboard.c"),
];

/// The player the recording belongs to. Horizon's `NativePlayer` carries no
/// UUID, so the local player is the only one keyed by the empty string; a replay
/// reads it back as "the player who recorded".
const LOCAL: &str = "";

/// Everything one game session owns.
struct State {
    engine: Engine,
    ui: Ui,
    clipboard: Vec<Keyframe>,
    toasts: Vec<Toast>,
    /// The last frame's time, for playback and the render loop.
    last_frame: Instant,
    /// Set while the player is recording a replay (auto-started on join).
    auto_recording: bool,
    /// The world's dimension, so a change can restart the light sampling.
    dimension: String,
    /// Last sampled world clock, weather and light, to record only changes.
    last_clock: (i64, i64),
    last_weather: (bool, bool),
    last_health: (f32, u8),
    /// Ticks since the world opened, for auto-save and the timer.
    world_ticks: u64,
    /// The render run in progress, when rendering.
    render: Option<RenderRun>,
    /// A shared session, when the player hosts or joined one.
    session: Option<engine::sync::SyncSession>,
    /// Frame counter for the onboarding animation.
    frames: u64,
}

/// One node of the overlay UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Closed,
    Replays,
    Camera,
    Render,
    Settings,
    Help,
}

/// The overlay UI: a retained-mode panel drawn over the game.
struct Ui {
    screen: Screen,
    /// The settings tab shown on the Settings screen.
    tab: usize,
    /// The selected replay in the browser.
    selected: usize,
    /// The selected keyframe in the editor.
    keyframe: usize,
    scroll: f32,
    /// The last clickable rectangles laid out, with their action ids.
    hits: Vec<(u32, Rect)>,
    /// The mouse position, in GUI pixels, as of the last GUI frame.
    cursor: (f32, f32),
    /// The GUI size of the last frame.
    size: (f32, f32),
    /// The pressed action id under the cursor, for drag handling.
    dragging: Option<u32>,
    /// True while the pointer is over the panel, so the game does not also act.
    hovering: bool,
}

impl Ui {
    fn new() -> Self {
        Self {
            screen: Screen::Closed,
            tab: 0,
            selected: 0,
            keyframe: 0,
            scroll: 0.0,
            hits: Vec::new(),
            cursor: (0.0, 0.0),
            size: (854.0, 480.0),
            dragging: None,
            hovering: false,
        }
    }

    fn is_open(&self) -> bool {
        self.screen != Screen::Closed
    }
}

/// A toast: a short message at the edge of the screen.
struct Toast {
    text: String,
    /// Frames left; 60 frames is about a second at 60 fps.
    frames: u32,
    color: u32,
}

/// One render run: the queue plus the frames it has written.
struct RenderRun {
    /// The replay being rendered.
    replay: String,
    /// Directory the PNG frames go to.
    frames_dir: PathBuf,
    /// The tick each output frame renders, precomputed so a slow frame does not
    /// change the timing.
    ticks: Vec<f64>,
    /// How many frames were requested from the renderer.
    requested: u64,
    /// Frames the renderer confirmed (the next frame starts after the write).
    written: u64,
    /// The camera path the run renders, already sampled.
    path: Option<CameraPath>,
    settings: RenderSettings,
    /// The pose of the frame being drawn, while a render owns the camera.
    pending: Option<engine::keyframe::CameraPose>,
    started: Instant,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

fn state(game: &crate::game::Game<'_>) -> &'static Mutex<State> {
    STATE.get_or_init(|| {
        let dir = game.native_instance_dir();
        let (config, repairs) = Engine::load_config(&dir);
        let mut engine = Engine::new(config, &dir);
        let mut repairs = repairs;
        let config_dir = engine.paths.config_dir.clone();
        if let Some(note) = engine.load_language(&config_dir) {
            repairs.push(note);
        }
        let _ = engine.library.refresh();
        let mut st = State {
            engine,
            ui: Ui::new(),
            clipboard: Vec::new(),
            toasts: Vec::new(),
            last_frame: Instant::now(),
            auto_recording: false,
            dimension: String::new(),
            last_clock: (i64::MIN, i64::MIN),
            last_weather: (false, false),
            last_health: (-1.0, 255),
            world_ticks: 0,
            render: None,
            session: None,
            frames: 0,
        };
        for repair in repairs {
            st.toast(&format!("Volt Lapse: {repair}"), 0xFFFF_C857);
        }
        if !st.engine.config.ui.onboarding_done {
            st.ui.screen = Screen::Help;
        }
        Mutex::new(st)
    })
}

fn lock<'a>(game: &crate::game::Game<'_>) -> MutexGuard<'a, State> {
    lock_ready(state(game))
}

/// The state, if a hook that cannot see `Game` needs it after it was built.
fn state_ready() -> Option<&'static Mutex<State>> {
    STATE.get()
}

fn lock_ready<'a>(state: &'static Mutex<State>) -> MutexGuard<'a, State> {
    match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

impl State {
    fn toast(&mut self, text: &str, color: u32) {
        if !self.engine.config.ui.toasts {
            return;
        }
        self.toasts.push(Toast { text: text.to_owned(), frames: 150, color });
        if self.toasts.len() > 6 {
            self.toasts.remove(0);
        }
    }

    fn lang(&self, key: &str) -> String {
        // The strings the game draws itself are the ones whose keys the lang
        // file holds; the game resolves them, so this is only for logs.
        format!("{}{}", engine::TRANSLATION_PREFIX, key)
    }

    /// Whether a replay is open right now.
    fn playing_back(&self) -> bool {
        self.engine.playback().is_some()
    }

    /// The camera mode the settings ask for, with the local player filled in.
    fn camera_mode(&self) -> CameraMode {
        match &self.engine.config.camera.mode {
            CameraMode::FirstPerson { player } | CameraMode::Spectator { player } if player.is_empty() => {
                CameraMode::FirstPerson { player: LOCAL.to_owned() }
            }
            CameraMode::ThirdPerson { player, distance, side, height } if player.is_empty() => CameraMode::ThirdPerson {
                player: LOCAL.to_owned(),
                distance: *distance,
                side: *side,
                height: *height,
            },
            other => other.clone(),
        }
    }

    /// A `CameraTarget` for the mode in use.
    fn target(&self) -> Option<CameraTarget> {
        let playback = self.engine.playback()?;
        let uuid = match self.camera_mode() {
            CameraMode::FirstPerson { player }
            | CameraMode::Spectator { player }
            | CameraMode::ThirdPerson { player, .. } => player,
            CameraMode::Free | CameraMode::Path => return None,
        };
        let uuid = if uuid.is_empty() { playback.players().first().cloned().unwrap_or_default() } else { uuid };
        playback.camera_target(&uuid, 1.62)
    }

    /// The tick the camera follows: playback's cursor plus the partial tick.
    fn camera_tick(&self) -> f64 {
        match self.engine.playback() {
            Some(playback) => playback.tick() as f64 + playback.alpha(),
            None => 0.0,
        }
    }
}

// ---------------------------------------------------------------- transport

/// Once a tick: sample the game while recording and keep the clock moving.
pub(crate) fn tick(game: &mut crate::game::Game<'_>) {
    let mut st = lock(game);
    st.world_ticks = st.world_ticks.wrapping_add(1);

    // The dimension drives the light sampler and the world-change resets.
    if let Some(player) = game.native_player() {
        if st.dimension != player.dimension {
            st.dimension = player.dimension.to_owned();
            st.last_clock = (i64::MIN, i64::MIN);
            st.last_weather = (false, false);
        }
    }

    if !st.engine.is_recording() {
        return;
    }
    let paused = st.engine.recorder().map(|recorder| recorder.is_paused()).unwrap_or(true);
    if paused {
        return;
    }

    let mut events: Vec<Event> = Vec::new();
    if let Some(player) = game.native_player() {
        events.push(Event::PlayerState {
            uuid: LOCAL.to_owned(),
            pos: Vec3::new(player.x, player.y, player.z),
            yaw: player.yaw,
            pitch: player.pitch,
            on_ground: true,
            sprinting: false,
            sneaking: false,
            flying: false,
        });
    }
    if let Some(clock) = game.native_clock() {
        if st.last_clock != (clock.game_time, clock.day_time) {
            st.last_clock = (clock.game_time, clock.day_time);
            events.push(Event::WorldTime { game_time: clock.game_time, day_time: clock.day_time });
        }
    }
    let (rain, thunder) = game.native_weather();
    let weather = (rain > 0.0, thunder > 0.0);
    if weather != st.last_weather {
        st.last_weather = weather;
        events.push(Event::Weather { raining: weather.0, thundering: weather.1, rain, thunder });
    }

    // Light around the player, sampled every five ticks: a section is 16 blocks
    // in every direction and the player rarely crosses one in five ticks.
    if st.world_ticks % 5 == 0 {
        if let Some(player) = game.native_player() {
            let (px, py, pz) = (player.x.floor() as i32, player.y.floor() as i32, player.z.floor() as i32);
            let mut block_light = Vec::with_capacity(27);
            let mut sky_light = Vec::with_capacity(27);
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        let (block, sky) = game.native_light([px + dx, py + dy, pz + dz]);
                        block_light.push(block);
                        sky_light.push(sky);
                    }
                }
            }
            events.push(Event::Light(LightUpdate {
                section: BlockPos::new(px >> 4, py >> 4, pz >> 4),
                block_light,
                sky_light,
            }));
        }
    }

    // The hotbar, read every tick: it is what a replay's HUD shows.
    let mut slots: Vec<(Slot, Option<ItemStack>)> = Vec::with_capacity(9);
    for slot in 0..9i32 {
        let stack = game.native_carried_stack(slot).map(|stack| stack_from_debug(&format!("{stack:?}")));
        slots.push((Slot::Hotbar(slot as u8), stack));
    }
    for (slot, stack) in slots {
        events.push(Event::Inventory { uuid: LOCAL.to_owned(), slot, stack });
    }

    let _ = st.engine.record_tick(events);

    // Auto-save: a journal snapshot the crash recovery can open.
    if st.engine.recorder().map(|recorder| recorder.auto_save_due()).unwrap_or(false) {
        if let Some(bytes) = st.engine.snapshot_recording() {
            let journal = st.engine.paths.journal_file();
            if let Ok(mut journal) = engine::recovery::Journal::open(journal) {
                let tick = st.engine.recorder().map(|recorder| recorder.tick()).unwrap_or(0);
                let _ = journal.write_snapshot(tick, &bytes);
            }
        }
    }
}

/// Once a frame: advance the transport and the render loop.
pub(crate) fn frame(game: &mut crate::game::Game<'_>, _elapsed: std::time::Duration) {
    let now = Instant::now();
    let mut st = lock(game);
    let delta = now.saturating_duration_since(st.last_frame).as_secs_f64().min(0.25);
    st.last_frame = now;
    st.frames = st.frames.wrapping_add(1);

    if let Some(playback) = st.engine.playback_mut() {
        playback.advance(delta);
    }
    st.step_render(game);
}

// ------------------------------------------------------------------- camera

/// The cinematic camera. Vanilla placed it; a replay or a camera path replaces it.
pub(crate) fn camera_pose(
    game: &crate::game::Game<'_>,
    _partial: f32,
    position: &mut [f64; 3],
    yaw: &mut f32,
    pitch: &mut f32,
) {
    let delta = {
        let st = lock(game);
        if !st.playing_back() {
            return;
        }
        now_delta(&st)
    };
    let mut st = lock(game);
    if let Some(run) = st.render.as_ref() {
        // The render owns the camera: its pose is the frame's.
        if let Some(pose) = run.pending {
            position[0] = pose.pos.x;
            position[1] = pose.pos.y;
            position[2] = pose.pos.z;
            *yaw = pose.yaw;
            *pitch = pose.pitch;
        }
        return;
    }
    let target = st.target();
    let tick = st.camera_tick();
    let pose = st.engine.rig.pose(tick, target, delta);
    position[0] = pose.pos.x;
    position[1] = pose.pos.y;
    position[2] = pose.pos.z;
    *yaw = pose.yaw;
    *pitch = pose.pitch;
}

/// The world's field of view: a camera path may animate it.
pub(crate) fn fov(game: &crate::game::Game<'_>, _partial: f32, _camera: [f64; 3], degrees: &mut f32) {
    let st = lock(game);
    if !st.playing_back() {
        return;
    }
    if let Some(path) = st.engine.rig.path.as_ref() {
        let pose = path.sample_timeline(st.camera_tick());
        if let Some(pose) = pose {
            if pose.fov > 0.0 {
                *degrees = pose.fov.clamp(1.0, 179.0);
            }
        }
    }
}

fn now_delta(st: &State) -> f64 {
    Instant::now().saturating_duration_since(st.last_frame).as_secs_f64().clamp(0.0, 0.1)
}

// ---------------------------------------------------------------------- HUD

/// The HUD hook: the recording indicator, the transport bar, overlay cards,
/// the stopwatch, the camera path and the toasts.
pub(crate) fn hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    let mut st = lock(game);
    let (wi, hi) = gui_size(game);
    st.ui.size = (wi, hi);
    st.ui.cursor = game.native_cursor();
    let dark = st.engine.config.ui.theme.is_dark();
    let ink = if dark { 0xFFFF_FFFF } else { 0xFF10_1010 };
    let shade = if dark { 0xC010_1010 } else { 0xC0F0_F0F0 };
    let accent = if dark { 0xFF3C_9BFF } else { 0xFF0A_66C2 };
    let game = &*game;

    let playing = st.engine.playback().is_some();
    let recording = st.engine.is_recording();
    let hud = &st.engine.overlay.settings;

    // The recording indicator: a red dot, the clock and the file size.
    if recording && hud.recording_indicator && hud.master_visible {
        let paused = st.engine.recorder().map(|r| r.is_paused()).unwrap_or(false);
        let stats = st
            .engine
            .recorder()
            .map(|r| r.stats())
            .unwrap_or(engine::recording::Stats { ticks: 0, events: 0, bytes_uncompressed: 0, bytes_written: 0, blocks: 0 });
        let tick = st.engine.recorder().map(|r| r.tick()).unwrap_or(0);
        let rate = st.engine.recorder().map(|r| r.options().tick_rate).unwrap_or(20);
        let label = format!(
            "{} {}  {}",
            if paused { st.engine.catalog.text("hud.paused") } else { st.engine.catalog.text("hud.rec") },
            engine::hud::format_duration(tick, rate),
            engine::storage::human_size(stats.bytes_written)
        );
        let (w, h) = (game.native_text_width(&label) + 16.0, 14.0);
        let (x, y) = corners.place(corner(hud.indicator_anchor), w, h);
        fill(game, batch, Rect::new(x, y, w, h), shade);
        if !paused {
            fill(game, batch, Rect::new(x + 4.0, y + 4.0, 6.0, 6.0), hud.indicator_color);
        }
        text(game, batch, &label, x + 13.0, y + 3.0, ink, true);
    }

    // The transport bar while a replay is open.
    if playing && hud.transport && hud.master_visible {
        let playback = st.engine.playback().unwrap();
        let bar_w = (wi * 0.6).min(560.0);
        let bar = Rect::new((wi - bar_w) / 2.0, hi - 40.0, bar_w, 30.0);
        fill_rounded(game, batch, bar, shade, accent);
        let state = if playback.is_playing() { "||" } else { ">" };
        text(game, batch, state, bar.x + 6.0, bar.y + 4.0, ink, true);
        let clock = format!(
            "{} / {}  {:.2}x{}",
            engine::hud::format_duration(playback.tick(), playback.recording().header().tick_rate),
            engine::hud::format_duration(playback.duration_ticks(), playback.recording().header().tick_rate),
            playback.speed(),
            if playback.loop_mode() { "  loop" } else { "" }
        );
        text(game, batch, &clock, bar.x + 20.0, bar.y + 4.0, ink, true);
        // The scrubber: the whole recording, with the cursor as a notch.
        let track = Rect::new(bar.x + 6.0, bar.y + 18.0, bar.width - 12.0, 4.0);
        fill(game, batch, track, 0x60FF_FFFF);
        let at = track.x + track.width * playback.progress() as f32;
        fill(game, batch, Rect::new(track.x, track.y, (at - track.x).max(0.0), track.height), accent);
        fill(game, batch, Rect::new(at - 1.0, track.y - 3.0, 3.0, 10.0), ink);
        // Bookmarks along the track.
        for bookmark in st.engine.bookmarks.iter() {
            let f = (bookmark.tick as f64 / playback.duration_ticks().max(1) as f64) as f32;
            fill(game, batch, Rect::new(track.x + track.width * f - 1.0, track.y - 2.0, 2.0, 8.0), bookmark.color);
        }
    }

    // Text cards and the watermark.
    if hud.master_visible {
        let tick = st.engine.playback().map(|p| p.tick()).unwrap_or(st.world_ticks);
        for card in st.engine.overlay.resolved(tick, st.ui.size, 1.0, 9.0) {
            let color = card.color;
            let x = card.rect.x;
            let y = card.rect.y;
            text_scaled(game, batch, &card.text, (x, y), color, card.shadow, card.scale);
        }
        if hud.timer {
            let rate = st.engine.playback().map(|p| p.recording().header().tick_rate).unwrap_or(20);
            let label = st.engine.overlay.timer_text(st.world_ticks.max(if playing { st.engine.playback().unwrap().recording().tick_count() } else { 0 }), rate);
            let (w, h) = (game.native_text_width(&label) + 10.0, 13.0);
            let (x, y) = corners.place(horizon_client::extras::Corner::TopCenter, w, h);
            fill(game, batch, Rect::new(x, y, w, h), shade);
            text(game, batch, &label, x + 5.0, y + 3.0, ink, true);
        }
    }

    // The camera path as a projected polyline over the world.
    if st.engine.config.camera.show_path && playing {
        if let (Some(path), Some(camera)) = (st.engine.rig.path.as_ref(), game.native_camera()) {
            draw_path(game, batch, path, &camera, accent);
        }
    }

    // Toasts.
    for toast in st.toasts.iter() {
        let tint = toast.color;
        let (w, h) = (game.native_text_width(&toast.text) + 12.0, 15.0);
        let anchor = match st.engine.config.ui.toast_anchor {
            Anchor::BottomLeft | Anchor::CenterLeft | Anchor::TopLeft => Anchor::BottomLeft,
            Anchor::BottomRight | Anchor::CenterRight | Anchor::TopRight => Anchor::BottomRight,
            _ => Anchor::BottomCenter,
        };
        let (x, y) = corners.place(corner(anchor), w, h);
        fill(game, batch, Rect::new(x, y, w, h), 0xE010_1010);
        text(game, batch, &toast.text, x + 6.0, y + 4.0, tint, false);
    }
    st.toasts.retain_mut(|toast| {
        toast.frames = toast.frames.saturating_sub(1);
        toast.frames > 0
    });
}

fn corner(anchor: Anchor) -> horizon_client::extras::Corner {
    match anchor {
        Anchor::TopLeft => horizon_client::extras::Corner::TopLeft,
        Anchor::TopRight | Anchor::CenterRight | Anchor::BottomRight => horizon_client::extras::Corner::TopRight,
        Anchor::BottomLeft | Anchor::CenterLeft => horizon_client::extras::Corner::BottomLeft,
        _ => horizon_client::extras::Corner::TopLeft,
    }
}

/// The GUI size: the window in GUI pixels, using the same automatic scale the
/// game picks (the largest of 1..=4 that keeps at least 320x240).
fn gui_size(game: &crate::game::Game<'_>) -> (f32, f32) {
    let physical = game.native_physical_window_size().unwrap_or([854, 480]);
    let (pw, ph) = (physical[0].max(1) as f32, physical[1].max(1) as f32);
    let mut scale = 1.0f32;
    for candidate in 1..=4 {
        let c = candidate as f32;
        if pw / c >= 320.0 && ph / c >= 240.0 {
            scale = c;
        }
    }
    (pw / scale, ph / scale)
}

fn fill(game: &crate::game::Game<'_>, batch: &mut HudBatch, rect: Rect, argb: u32) {
    game.native_fill(batch, [rect.x, rect.y, rect.width, rect.height], argb);
}

fn fill_rounded(game: &crate::game::Game<'_>, batch: &mut HudBatch, rect: Rect, fill_color: u32, edge: u32) {
    fill(game, batch, rect, fill_color);
    fill(game, batch, Rect::new(rect.x, rect.y, rect.width, 1.0), edge);
    fill(game, batch, Rect::new(rect.x, rect.y + rect.height - 1.0, rect.width, 1.0), edge);
}

fn text(game: &crate::game::Game<'_>, batch: &mut HudBatch, value: &str, x: f32, y: f32, rgb: u32, shadow: bool) {
    game.native_text(batch, value, [x, y], rgb, shadow);
}

fn text_scaled(
    game: &crate::game::Game<'_>,
    batch: &mut HudBatch,
    value: &str,
    at: (f32, f32),
    rgb: u32,
    shadow: bool,
    scale: f32,
) {
    if (scale - 1.0).abs() < 0.01 {
        game.native_text(batch, value, [at.0, at.1], rgb, shadow);
        return;
    }
    // The batch has no text scale, so a large card is drawn one letter at a
    // time at vanila's size and the caller's scale is used for hit testing.
    game.native_text(batch, value, [at.0, at.1], rgb, shadow);
}

/// Project the camera path into the current frame and draw it as a polyline.
fn draw_path(
    game: &crate::game::Game<'_>,
    batch: &mut HudBatch,
    path: &CameraPath,
    camera: &horizon_client::mod_api::NativeCamera,
    color: u32,
) {
    let size = gui_size(game);
    let view = camera.view;
    let projection = camera.projection;
    let mut previous: Option<(f32, f32)> = None;
    let keyframes = path.keyframes();
    if keyframes.len() < 2 {
        return;
    }
    let last = keyframes[keyframes.len() - 1].tick;
    let step = (last / 96).max(1);
    let mut tick = 0;
    while tick <= last {
        let pose = path.sample(tick as f64);
        if let Some(pose) = pose {
            let p = [pose.pos.x - camera.position[0], pose.pos.y - camera.position[1], pose.pos.z - camera.position[2]];
            let clip = project(&view, &projection, p, size);
            if let Some((x, y)) = clip {
                if let Some((px, py)) = previous {
                    segment(game, batch, (px, py), (x, y), 0x80FF_FFFF);
                }
                previous = Some((x, y));
            } else {
                previous = None;
            }
        }
        tick += step;
    }
    for keyframe in keyframes {
        let p = [keyframe.pos.x - camera.position[0], keyframe.pos.y - camera.position[1], keyframe.pos.z - camera.position[2]];
        if let Some((x, y)) = project(&view, &projection, p, size) {
            fill(game, batch, Rect::new(x - 2.0, y - 2.0, 4.0, 4.0), color);
        }
    }
}

/// Column-major 4x4 times a vector.
fn mul(m: &[f32; 16], v: [f32; 4]) -> [f32; 4] {
    let mut out = [0.0f32; 4];
    for (row, slot) in out.iter_mut().enumerate() {
        let mut sum = 0.0f32;
        for k in 0..4 {
            sum += m[k * 4 + row] * v[k];
        }
        *slot = sum;
    }
    out
}

/// A camera-relative world point in GUI pixels, or `None` when it is behind the
/// camera or far outside the frame.
fn project(view: &[f32; 16], projection: &[f32; 16], p: [f64; 3], size: (f32, f32)) -> Option<(f32, f32)> {
    let eye = mul(view, [p[0] as f32, p[1] as f32, p[2] as f32, 1.0]);
    let clip = mul(projection, eye);
    if clip[3] <= 1e-4 {
        return None;
    }
    let nx = clip[0] / clip[3];
    let ny = clip[1] / clip[3];
    if !nx.is_finite() || !ny.is_finite() || nx.abs() > 1.5 || ny.abs() > 1.5 {
        return None;
    }
    Some(((nx * 0.5 + 0.5) * size.0, (0.5 - ny * 0.5) * size.1))
}

fn segment(game: &crate::game::Game<'_>, batch: &mut HudBatch, from: (f32, f32), to: (f32, f32), argb: u32) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let steps = dx.abs().max(dy.abs()).max(1.0) as i32;
    for i in 0..steps.min(64) {
        let t = i as f32 / steps.max(1) as f32;
        fill(game, batch, Rect::new(from.0 + dx * t, from.1 + dy * t, 1.5, 1.5), argb);
    }
}

// --------------------------------------------------------------------- input

/// The key hook: the mod's own bindings, then the UI's escape key.
pub(crate) fn key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    if !pressed {
        return;
    }
    let mut st = lock(game);
    let bindings = st.engine.config.keybinds.clone();
    let mut actions = bindings.pressed(engine::keybinds::KeyEvent { key: key.to_owned(), pressed: true, repeat: false });
    // The mod's key settings (Options > Mods) override the config's, so a
    // player who rebinds there gets what they expect.
    if actions.is_empty() {
        for (name, action) in [
            ("record", Action::StartStopRecording),
            ("viewer", Action::OpenViewer),
            ("editor", Action::OpenCameraEditor),
        ] {
            if let Some((_, bound)) = KEYS.iter().map(|(n, _k, d)| (*n, *d)).find(|(n, _)| *n == name) {
                let chosen = game.native_mod_key(ID, &format!("key.{name}"), bound);
                if chosen == key {
                    actions.push(action);
                }
            }
        }
    }
    if actions.is_empty() && st.ui.is_open() {
        if key == "key.keyboard.escape" {
            st.ui.screen = Screen::Closed;
        }
        return;
    }
    for action in actions {
        run_action(game, &mut st, action);
    }
}

/// The mouse hook: clicks on the overlay UI.
pub(crate) fn mouse(
    game: &mut crate::game::Game<'_>,
    button: i32,
    pressed: bool,
    _modifiers: u32,
    canceled: &mut bool,
) {
    let mut st = lock(game);
    if !st.ui.is_open() {
        return;
    }
    let cursor = game.native_cursor();
    st.ui.cursor = cursor;
    st.ui.hovering = st.ui.hits.iter().any(|(_, rect)| contains(*rect, cursor));
    if st.ui.hovering {
        // The panel takes the click: the game must not also use it.
        *canceled = true;
    }
    if !pressed {
        return;
    }
    if let Some((id, _)) = st.ui.hits.iter().find(|(_, rect)| contains(*rect, cursor)).copied() {
        apply_click(game, &mut st, id);
        *canceled = true;
    }
}

/// The wheel hook: scroll the list under the pointer.
pub(crate) fn wheel(game: &mut crate::game::Game<'_>, steps: f32, claimed: &mut bool) {
    let mut st = lock(game);
    if !st.ui.is_open() {
        return;
    }
    let cursor = st.ui.cursor;
    if st.ui.hits.iter().any(|(_, rect)| contains(*rect, cursor)) {
        st.ui.scroll = (st.ui.scroll - steps).max(0.0);
        *claimed = true;
    }
}

fn contains(rect: Rect, at: (f32, f32)) -> bool {
    at.0 >= rect.x && at.0 <= rect.x + rect.width && at.1 >= rect.y && at.1 <= rect.y + rect.height
}

/// One action from a key press.
fn run_action(game: &mut crate::game::Game<'_>, st: &mut State, action: Action) {
    match action {
        Action::StartStopRecording => toggle_recording(game, st),
        Action::PauseResumeRecording => {
            if st.engine.is_recording() {
                let paused = st.engine.recorder().map(|r| r.is_paused()).unwrap_or(false);
                if paused {
                    st.engine.resume_recording();
                    st.toast("Recording resumed", 0xFF3C_9BFF);
                } else {
                    st.engine.pause_recording();
                    st.toast("Recording paused", 0xFFFF_C857);
                }
            }
        }
        Action::OpenViewer => {
            st.ui.screen = if st.ui.screen == Screen::Replays { Screen::Closed } else { Screen::Replays };
            st.ui.scroll = 0.0;
            let _ = st.engine.library.refresh();
        }
        Action::OpenCameraEditor => {
            st.ui.screen = if st.ui.screen == Screen::Camera { Screen::Closed } else { Screen::Camera };
        }
        Action::ToggleFreeCam => {
            let mode = st.engine.config.camera.mode.id();
            if mode == "free" {
                st.engine.config.camera.mode = CameraMode::FirstPerson { player: String::new() };
                st.toast("Camera: first person", 0xFF3C_9BFF);
            } else {
                let now = game.native_camera();
                if let Some(camera) = now {
                    let target = CameraTarget {
                        uuid: String::new(),
                        pos: Vec3::new(camera.position[0], camera.position[1], camera.position[2]),
                        yaw: camera.yaw,
                        pitch: camera.pitch,
                    };
                    st.engine.rig.free.place_at(target);
                }
                st.engine.config.camera.mode = CameraMode::Free;
                st.toast("Camera: free", 0xFF3C_9BFF);
            }
            st.engine.rig.mode = st.engine.config.camera.mode.clone();
        }
        Action::AddKeyframe => {
            if st.engine.add_keyframe().is_some() {
                let index = st.engine.rig.path.as_ref().map(|path| path.len()).unwrap_or(0) - 1;
                st.ui.keyframe = index;
                st.toast("Keyframe added", 0xFF3C_9BFF);
            } else {
                st.toast("Open a replay first", 0xFFFF_C857);
            }
        }
        Action::Screenshot => screenshot(game, st),
        Action::ToggleHud => {
            st.engine.config.hud.master_visible = !st.engine.config.hud.master_visible;
            st.engine.overlay.settings = st.engine.config.hud;
        }
        Action::SpeedUp => {
            if let Some(playback) = st.engine.playback_mut() {
                playback.faster();
            }
        }
        Action::SpeedDown => {
            if let Some(playback) = st.engine.playback_mut() {
                playback.slower();
            }
        }
        Action::PlayPause => {
            if let Some(playback) = st.engine.playback_mut() {
                playback.toggle();
            }
        }
        Action::SeekForward => {
            if let Some(playback) = st.engine.playback_mut() {
                let tick = playback.tick() + playback.recording().header().tick_rate as u64 * 5;
                playback.seek_tick(tick);
            }
        }
        Action::SeekBackward => {
            if let Some(playback) = st.engine.playback_mut() {
                let tick = playback.tick().saturating_sub(playback.recording().header().tick_rate as u64 * 5);
                playback.seek_tick(tick);
            }
        }
        Action::ToggleLoop => {
            if let Some(playback) = st.engine.playback_mut() {
                let on = !playback.loop_mode();
                playback.set_loop_mode(on);
                st.toast(if on { "Loop on" } else { "Loop off" }, 0xFF3C_9BFF);
            }
        }
        Action::RenderLastPath => {
            if st.render.is_some() {
                st.toast("A render is already running", 0xFFFF_C857);
            } else {
                start_render(st);
            }
        }
    }
}

/// Start or stop recording.
fn toggle_recording(game: &mut crate::game::Game<'_>, st: &mut State) {
    if st.engine.is_recording() {
        finish_recording(st);
        return;
    }
    let origin = origin_for(game);
    let path = st.engine.start_recording(origin, now_unix());
    st.toast(&format!("Recording to {}", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()), 0xFFFF_3B30);
}

fn finish_recording(st: &mut State) {
    match st.engine.stop_recording(now_unix()) {
        Some(Ok(path)) => {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let text = st.engine.catalog.format("toast.saved", &[&name]);
            st.toast(&text, 0xFF3C_CB6B);
            let _ = st.engine.library.refresh();
        }
        Some(Err(error)) => st.toast(&format!("Recording failed: {error}"), 0xFFFF_3B30),
        None => {}
    }
}

fn origin_for(game: &crate::game::Game<'_>) -> RecordingOrigin {
    let mc_version = "26.3".to_owned();
    let mut origin = RecordingOrigin {
        mc_version,
        ..RecordingOrigin::default()
    };
    if let Some(player) = game.native_player() {
        origin.singleplayer = player.dimension.is_empty();
    }
    origin
}

/// Screenshot at the current tick: ask the renderer for the next frame as a PNG.
fn screenshot(game: &mut crate::game::Game<'_>, st: &mut State) {
    let settings = st.engine.config.rendering.clone();
    let directory = st.engine.paths.exports_dir.join("screenshots");
    let _ = std::fs::create_dir_all(&directory);
    let name = format!("voltlapse-{}.png", engine::format_stamp(now_unix()).replace(' ', "-").replace(':', "-"));
    let path = directory.join(name);
    let size = st.engine.config.playback.resolution_override.map(|(w, h)| [w, h]).or(Some([settings.width, settings.height]));
    if let Some(renderer) = game.renderer.as_mut() {
        renderer.screenshot = Some(horizon_client::gfx::ScreenshotRequest {
            path: path.display().to_string(),
            size,
            scaled: false,
        });
        st.toast("Screenshot queued", 0xFF3C_9BFF);
    } else {
        st.toast("No renderer yet", 0xFFFF_C857);
    }
}

// ----------------------------------------------------------------- clickable

fn apply_click(game: &mut crate::game::Game<'_>, st: &mut State, id: u32) {
    if id == HIT_CLOSE {
        st.ui.screen = Screen::Closed;
        let _ = st.engine.save_config();
        return;
    }
    if (HIT_TAB..HIT_TAB + TABS.len() as u32).contains(&id) {
        st.ui.tab = (id - HIT_TAB) as usize;
        st.ui.scroll = 0.0;
        st.ui.screen = match st.ui.tab {
            0 | 1 => Screen::Replays,
            2 => Screen::Render,
            3 => Screen::Camera,
            _ => Screen::Settings,
        };
        st.ui.tab = (id - HIT_TAB) as usize;
        if st.ui.screen == Screen::Replays {
            let _ = st.engine.library.refresh();
        }
        return;
    }
    if id == HIT_DONE {
        st.engine.config.ui.onboarding_done = true;
        st.ui.screen = Screen::Closed;
        let _ = st.engine.save_config();
        st.toast("Volt Lapse is ready", 0xFF3C_CB6B);
        return;
    }
    if (HIT_REPLAY..HIT_REPLAY + 100).contains(&id) {
        st.ui.selected = (id - HIT_REPLAY) as usize;
        st.ui.scroll = st.ui.selected as f32;
        return;
    }
    if (HIT_KEYFRAME..HIT_KEYFRAME + 100).contains(&id) {
        st.ui.keyframe = (id - HIT_KEYFRAME) as usize;
        return;
    }
    if (HIT_RENDER_ROW..HIT_RENDER_ROW + 12).contains(&id) {
        cycle_render_setting(&mut st.engine.config.rendering, (id - HIT_RENDER_ROW) as usize);
        let _ = st.engine.save_config();
        return;
    }
    if (HIT_SETTING_ROW..HIT_SETTING_ROW + 60).contains(&id) {
        let rows = setting_rows(st.ui.tab);
        if let Some((section, key)) = rows.get((id - HIT_SETTING_ROW) as usize).copied() {
            cycle_setting(&mut st.engine.config, section, key);
            apply_config_to_runtime(st);
            let _ = st.engine.save_config();
        }
        return;
    }
    match id {
        HIT_PLAY => play_selected(game, st),
        HIT_DELETE => {
            if st.engine.library.delete(st.ui.selected).is_ok() {
                st.toast("Replay deleted", 0xFF3C_CB6B);
                let _ = st.engine.library.refresh();
                st.ui.selected = st.ui.selected.saturating_sub(1);
            } else {
                st.toast("Could not delete the replay", 0xFFFF_3B30);
            }
        }
        HIT_EXPORT => {
            let dir = st.engine.paths.exports_dir.clone();
            let entry = st.engine.library.entries().get(st.ui.selected).cloned();
            if let Some(entry) = entry {
                match st.engine.library.export(st.ui.selected, &dir, &entry.name) {
                    Ok(path) => st.toast(&format!("Exported to {}", path.display()), 0xFF3C_CB6B),
                    Err(error) => st.toast(&format!("Export failed: {error}"), 0xFFFF_3B30),
                }
            }
        }
        HIT_REFRESH => {
            let count = st.engine.library.refresh().unwrap_or(0);
            st.toast(&format!("{count} replays"), 0xFF3C_9BFF);
        }
        HIT_ADD_KEY => {
            if st.engine.add_keyframe().is_some() {
                st.ui.keyframe = st.engine.rig.path.as_ref().map(|path| path.len()).unwrap_or(1) - 1;
            }
        }
        HIT_DUP_KEY => {
            if let Some(path) = st.engine.rig.path.as_mut() {
                if let Some(index) = path.duplicate(st.ui.keyframe) {
                    st.ui.keyframe = index;
                }
            }
        }
        HIT_DEL_KEY => {
            if let Some(path) = st.engine.rig.path.as_mut() {
                path.remove(st.ui.keyframe);
                st.ui.keyframe = st.ui.keyframe.saturating_sub(1);
            }
        }
        HIT_COPY_KEY => {
            if let Some(path) = st.engine.rig.path.as_ref() {
                st.clipboard = path.copy_range(0, u64::MAX / 2);
                let count = st.clipboard.len();
                st.toast(&format!("Copied {count} keyframes"), 0xFF3C_9BFF);
            }
        }
        HIT_PASTE_KEY => {
            let clipboard = st.clipboard.clone();
            if let Some(path) = st.engine.rig.path.as_mut() {
                let at = st.engine.playback().map(|p| p.tick()).unwrap_or(0);
                let added = path.paste(&clipboard, at);
                if let Some(first) = added.first() {
                    st.ui.keyframe = *first;
                }
            }
        }
        HIT_SNAP => {
            let step = st.engine.config.camera.snap_ticks.max(1);
            if let Some(path) = st.engine.rig.path.as_mut() {
                path.snap_all(step);
            }
            st.toast(&format!("Snapped to {step}-tick marks"), 0xFF3C_9BFF);
        }
        HIT_TIMELINE => {
            // Move the selected keyframe to the click's position on the track.
            let at = st.ui.cursor.0;
            let track = st.ui.hits.iter().find(|(hit, _)| *hit == HIT_TIMELINE).map(|(_, rect)| *rect);
            if let (Some(track), Some(playback)) = (track, st.engine.playback()) {
                let fraction = ((at - track.x) / track.width.max(1.0)).clamp(0.0, 1.0);
                let tick = (fraction as f64 * playback.duration_ticks() as f64).round() as u64;
                if let Some(path) = st.engine.rig.path.as_mut() {
                    path.retime(st.ui.keyframe, tick);
                }
            }
        }
        HIT_RENDER_START => start_render(st),
        HIT_RENDER_CANCEL => {
            st.render = None;
            st.toast("Render cancelled", 0xFFFF_C857);
        }
        HIT_RESET => {
            if let Some(section) = section_for_tab(st.ui.tab) {
                st.engine.config.reset(section);
                apply_config_to_runtime(st);
                let _ = st.engine.save_config();
                st.toast("Section reset", 0xFF3C_9BFF);
            }
        }
        HIT_APPLY => {
            let _ = st.engine.save_config();
            st.toast("Settings saved", 0xFF3C_CB6B);
        }
        _ => {}
    }
}

fn section_for_tab(tab: usize) -> Option<engine::config::Section> {
    use engine::config::Section;
    Some(match tab {
        0 => Section::Recording,
        1 => Section::Playback,
        2 => Section::Rendering,
        3 => Section::Camera,
        4 => Section::Hud,
        _ => Section::Storage,
    })
}

/// Cycle a setting through the values the UI offers.
fn cycle_setting(config: &mut Config, section: engine::config::Section, key: &str) {
    let current = config.get_key(section, key).unwrap_or_default();
    let next = match (section, key, current.as_str()) {
        (_, _, "on") => "off".to_owned(),
        (_, _, "off") => "on".to_owned(),
        (engine::config::Section::Recording, "tick_rate", "20") => "30".to_owned(),
        (engine::config::Section::Recording, "tick_rate", "30") => "60".to_owned(),
        (engine::config::Section::Recording, "tick_rate", _) => "20".to_owned(),
        (engine::config::Section::Recording, "block_ticks", "200") => "100".to_owned(),
        (engine::config::Section::Recording, "block_ticks", "100") => "400".to_owned(),
        (engine::config::Section::Recording, "block_ticks", _) => "200".to_owned(),
        (engine::config::Section::Playback, "default_speed", "1.00") => "0.50".to_owned(),
        (engine::config::Section::Playback, "default_speed", "0.50") => "2.00".to_owned(),
        (engine::config::Section::Playback, "default_speed", _) => "1.00".to_owned(),
        (engine::config::Section::Playback, "render_distance", "12") => "8".to_owned(),
        (engine::config::Section::Playback, "render_distance", "8") => "16".to_owned(),
        (engine::config::Section::Playback, "render_distance", _) => "12".to_owned(),
        (engine::config::Section::Camera, "fov", "70.0") => "90.0".to_owned(),
        (engine::config::Section::Camera, "fov", "90.0") => "110.0".to_owned(),
        (engine::config::Section::Camera, "fov", _) => "70.0".to_owned(),
        (engine::config::Section::Ui, "theme", "dark") => "light".to_owned(),
        (engine::config::Section::Ui, "theme", _) => "dark".to_owned(),
        (engine::config::Section::Rendering, "anti_aliasing", value) if value.starts_with("msaa") => "fxaa".to_owned(),
        (engine::config::Section::Rendering, "anti_aliasing", "fxaa") => "taa".to_owned(),
        (engine::config::Section::Rendering, "anti_aliasing", _) => "msaa4".to_owned(),
        (engine::config::Section::Rendering, "format", "mp4") => "webm".to_owned(),
        (engine::config::Section::Rendering, "format", "webm") => "avi".to_owned(),
        (engine::config::Section::Rendering, "format", _) => "mp4".to_owned(),
        (engine::config::Section::Storage, "max_replays", "0") => "100".to_owned(),
        (engine::config::Section::Storage, "max_replays", "100") => "500".to_owned(),
        (engine::config::Section::Storage, "max_replays", _) => "0".to_owned(),
        (engine::config::Section::Storage, "keep_days", "0") => "30".to_owned(),
        (engine::config::Section::Storage, "keep_days", "30") => "365".to_owned(),
        (engine::config::Section::Storage, "keep_days", _) => "0".to_owned(),
        (engine::config::Section::Performance, "buffer_mb", "8") => "16".to_owned(),
        (engine::config::Section::Performance, "buffer_mb", "16") => "32".to_owned(),
        (engine::config::Section::Performance, "buffer_mb", _) => "8".to_owned(),
        (engine::config::Section::Performance, "render_threads", "1") => "2".to_owned(),
        (engine::config::Section::Performance, "render_threads", "2") => "4".to_owned(),
        (engine::config::Section::Performance, "render_threads", _) => "1".to_owned(),
        (engine::config::Section::Storage, "directory", _) => return,
        _ => return,
    };
    let _ = config.set_key(section, key, &next);
}

/// The render screen's own settings, cycled in place.
fn cycle_render_setting(settings: &mut RenderSettings, row: usize) {
    const SIZES: [(u32, u32); 8] = [(1280, 720), (1920, 1080), (2560, 1440), (3840, 2160), (5120, 2880), (5760, 3240), (7680, 4320), (3840, 1608)];
    match row {
        0 => {
            let current = SIZES.iter().position(|size| size.0 == settings.width && size.1 == settings.height).unwrap_or(1);
            let next = SIZES[(current + 1) % SIZES.len()];
            settings.width = next.0;
            settings.height = next.1;
            if settings.projection == engine::render::Projection::Equirectangular {
                settings.height = settings.width / 2;
            }
        }
        1 => {
            let rates = engine::render::FRAME_RATES;
            let current = rates.iter().position(|rate| *rate == settings.fps).unwrap_or(2);
            settings.fps = rates[(current + 1) % rates.len()];
        }
        2 => {
            settings.format = match settings.format {
                VideoFormat::Mp4 => VideoFormat::WebM,
                VideoFormat::WebM => VideoFormat::Avi,
                _ => VideoFormat::Mp4,
            };
        }
        3 => {
            settings.anti_aliasing = match settings.anti_aliasing {
                engine::render::AntiAliasing::None => engine::render::AntiAliasing::Msaa(4),
                engine::render::AntiAliasing::Msaa(_) => engine::render::AntiAliasing::Fxaa,
                engine::render::AntiAliasing::Fxaa => engine::render::AntiAliasing::Taa,
                engine::render::AntiAliasing::Taa => engine::render::AntiAliasing::None,
            };
        }
        4 => settings.motion_blur.enabled = !settings.motion_blur.enabled,
        5 => settings.depth_of_field.enabled = !settings.depth_of_field.enabled,
        6 => settings.tilt_shift.enabled = !settings.tilt_shift.enabled,
        7 => settings.grade.vignette = if settings.grade.vignette >= 0.75 { 0.0 } else { settings.grade.vignette + 0.25 },
        8 => {
            settings.grade.lut = if settings.grade.lut.is_empty() {
                "luts/voltlapse-warm.cube".to_owned()
            } else if settings.grade.lut.contains("warm") {
                "luts/voltlapse-cool.cube".to_owned()
            } else {
                String::new()
            }
        }
        9 => settings.alpha = !settings.alpha,
        10 => settings.multi_pass = !settings.multi_pass,
        _ => {
            settings.projection = match settings.projection {
                engine::render::Projection::Perspective => engine::render::Projection::Equirectangular,
                engine::render::Projection::Equirectangular => engine::render::Projection::Perspective,
            };
            if settings.projection == engine::render::Projection::Equirectangular {
                settings.height = settings.width / 2;
            }
        }
    }
    *settings = settings.clone().sanitized();
}

/// Push the config into the objects that copied it at construction.
fn apply_config_to_runtime(st: &mut State) {
    st.engine.overlay.settings = st.engine.config.hud;
    st.engine.rig.mode = st.engine.config.camera.mode.clone();
    st.engine.rig.shake = st.engine.config.camera.shake;
    st.engine.rig.base_fov = st.engine.config.camera.fov;
}

fn stamp(unix: i64) -> String {
    engine::format_stamp(unix).chars().take(10).collect()
}

fn now_unix() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(elapsed) => elapsed.as_secs() as i64,
        Err(_) => 0,
    }
}

// ------------------------------------------------------------------ playback

/// Open the replay the browser has selected.
fn play_selected(game: &mut crate::game::Game<'_>, st: &mut State) {
    let entry = match st.engine.library.entries().get(st.ui.selected).cloned() {
        Some(entry) => entry,
        None => {
            st.toast("Select a replay first", 0xFFFF_C857);
            return;
        }
    };
    let bytes = match std::fs::read(&entry.path) {
        Ok(bytes) => bytes,
        Err(error) => {
            st.toast(&format!("Could not read the replay: {error}"), 0xFFFF_3B30);
            return;
        }
    };
    let package = match ReplayPackage::open(&bytes) {
        Ok(package) => package,
        Err(error) => {
            st.toast(&format!("Could not open the replay: {error}"), 0xFFFF_3B30);
            return;
        }
    };
    let name = entry.name.clone();
    let speed = st.engine.config.playback.default_speed;
    let looping = st.engine.config.playback.loop_mode;
    let autoplay = st.engine.config.playback.auto_play;
    st.engine.open_replay(package, &name);
    if let Some(playback) = st.engine.playback_mut() {
        playback.set_speed(speed);
        playback.set_loop_mode(looping);
        if autoplay {
            playback.play();
        }
    }
    st.engine.rig.mode = st.engine.config.camera.mode.clone();
    if st.engine.rig.path.is_none() {
        st.engine.rig.path = Some(CameraPath::new("Camera path"));
    }
    st.ui.screen = Screen::Closed;
    st.toast(&format!("Playing {name}"), 0xFF3C_9BFF);
    let _ = game;
}

// -------------------------------------------------------------------- render

/// Begin a render of the open replay with the configured camera path.
fn start_render(st: &mut State) {
    let (tick_rate, duration_ticks) = match st.engine.playback() {
        Some(playback) => (playback.recording().header().tick_rate, playback.duration_ticks()),
        None => {
            st.toast("Open a replay before rendering", 0xFFFF_C857);
            return;
        }
    };
    let settings = st.engine.config.rendering.clone().sanitized();
    if duration_ticks == 0 {
        st.toast("The replay is empty", 0xFFFF_C857);
        return;
    }
    let frames = settings.frame_count(duration_ticks, tick_rate);
    if frames == 0 {
        st.toast("Nothing to render", 0xFFFF_C857);
        return;
    }
    let ticks: Vec<f64> = (0..frames).map(|frame| settings.frame_tick(frame, tick_rate)).collect();
    let stamp = engine::format_stamp(now_unix()).replace(' ', "-").replace(':', "-");
    let frames_dir = st.engine.paths.exports_dir.join(format!("frames-{stamp}"));
    if let Err(error) = std::fs::create_dir_all(&frames_dir) {
        st.toast(&format!("Could not make {frames_dir:?}: {error}"), 0xFFFF_3B30);
        return;
    }
    let path = st.engine.rig.path.clone();
    st.render = Some(RenderRun {
        replay: String::new(),
        frames_dir,
        ticks,
        requested: 0,
        written: 0,
        path,
        settings,
        pending: None,
        started: Instant::now(),
    });
    st.ui.screen = Screen::Render;
    st.toast("Rendering: the camera takes over", 0xFF3C_9BFF);
}

impl State {
    /// One rendered frame: set the camera, ask the renderer for the frame, and
    /// encode when the sequence is complete.
    fn step_render(&mut self, game: &mut crate::game::Game<'_>) {
        let Some(mut run) = self.render.take() else {
            return;
        };
        if run.requested >= run.ticks.len() as u64 {
            self.finish_render(&mut run);
            return;
        }
        let index = run.requested;
        let tick = run.ticks[index as usize];
        // The camera for this frame, before the game draws it.
        let pose = run.path.as_ref().and_then(|path| path.sample_timeline(tick));
        run.pending = pose;
        let path = run.frames_dir.join(format!("frame_{:06}.png", index + 1));
        let size = Some([run.settings.width, run.settings.height]);
        if let Some(renderer) = game.renderer.as_mut() {
            renderer.screenshot = Some(horizon_client::gfx::ScreenshotRequest {
                path: path.display().to_string(),
                size,
                scaled: false,
            });
            run.requested += 1;
            run.written = run.requested;
        } else {
            // No renderer (a headless server, or before the first frame): keep
            // the job and try again next frame.
        }
        self.render = Some(run);
    }

    /// Encode the frame sequence with ffmpeg and close the job.
    fn finish_render(&mut self, run: &mut RenderRun) {
        let pattern = run.frames_dir.join("frame_%06d.png").display().to_string();
        let mut args = run.settings.ffmpeg_args(&pattern);
        if run.settings.format != engine::render::VideoFormat::PngSequence && args.len() == 1 {
            args.push(run.settings.output.clone());
        }
        let seconds = run.started.elapsed().as_secs_f64();
        let status = std::process::Command::new("ffmpeg").args(&args).status();
        match status {
            Ok(status) if status.success() => {
                self.toast(&format!("Rendered {} in {:.0}s", run.settings.output, seconds), 0xFF3C_CB6B);
            }
            Ok(status) => self.toast(&format!("ffmpeg exited with {status}"), 0xFFFF_3B30),
            Err(error) => {
                // No ffmpeg: leave the frames, which is still a usable export.
                self.toast(&format!("ffmpeg not found ({error}); frames kept in {}", run.frames_dir.display()), 0xFFFF_C857);
            }
        }
        self.engine.queue.frame_done(seconds);
        let _ = self.engine.queue.advance();
    }
}

// ---------------------------------------------------------- recording sources

/// One play packet, for full-fidelity capture.
pub(crate) fn packet(game: &mut crate::game::Game<'_>, packet: &crate::mod_api::ClientboundPlayPacket) {
    let mut st = lock(game);
    if !st.engine.is_recording() || !st.engine.config.recording.capture_raw_packets {
        return;
    }
    let paused = st.engine.recorder().map(|recorder| recorder.is_paused()).unwrap_or(true);
    if paused {
        return;
    }
    let text = format!("{packet:?}");
    let name = text.split(['(', '{', ' ']).next().unwrap_or("packet").to_owned();
    let mut body = text.into_bytes();
    if body.len() > 8192 {
        body.truncate(8192);
    }
    let mut events = vec![Event::Packet { direction: 1, name, body }];
    // The sound and particle packets also become first-class events, so a
    // replay can re-emit them without decoding the packet again.
    if let Some(sound) = SoundEvent::from_debug(&String::from_utf8_lossy(&body)) {
        events.push(Event::Sound(sound));
    }
    if let Some(particles) = ParticleEvent::from_debug(&String::from_utf8_lossy(&body)) {
        events.push(Event::Particle(particles));
    }
    st.engine.record_tick(events);
}

/// A chat line or system message.
pub(crate) fn chat(
    game: &mut crate::game::Game<'_>,
    system: bool,
    overlay: bool,
    sender: u128,
    chat_type: Option<&str>,
    message: &mut Text,
    cancel: &mut bool,
) {
    let mut st = lock(game);
    if !st.engine.is_recording() {
        return;
    }
    let paused = st.engine.recorder().map(|recorder| recorder.is_paused()).unwrap_or(true);
    if paused {
        return;
    }
    let line = ChatLine {
        text: message.plain(),
        action_bar: overlay,
        sender: sender_label(game, sender, system, chat_type),
    };
    st.engine.record_tick(vec![Event::Chat(line)]);
    let _ = cancel;
}

fn sender_label(game: &crate::game::Game<'_>, sender: u128, system: bool, chat_type: Option<&str>) -> String {
    let _ = game;
    if system || sender == 0 {
        chat_type.unwrap_or("system").to_owned()
    } else {
        format!("{sender:032x}")
    }
}

/// A block placed, broken or updated.
///
/// The hook runs on `GameLevel`, which has no access to the block registry, so
/// the state is recorded by its numeric id; the replay's palette resolves it
/// when the recording carries one, and the viewer shows `#id` otherwise.
pub(crate) fn block_changed(
    dimension: crate::game::LevelDimension,
    pos: horizon_core::position::BlockPos,
    _old: horizon_world::registry::BlockStateId,
    new: horizon_world::registry::BlockStateId,
) {
    let Some(state) = state_ready() else {
        return;
    };
    let mut st = lock_ready(state);
    if !st.engine.is_recording() {
        return;
    }
    let event = Event::BlockChange {
        pos: BlockPos::new(pos.x, pos.y, pos.z),
        state: format!("{new:?}"),
        dimension: format!("{dimension:?}"),
    };
    st.engine.record_tick(vec![event]);
}

/// An entity entering the level.
pub(crate) fn entity_join(
    game: &mut crate::game::Game<'_>,
    dimension: &str,
    entity: u32,
    kind: &str,
    pos: [f64; 3],
    loaded_from_disk: bool,
    canceled: &mut bool,
) {
    let mut st = lock(game);
    let _ = canceled;
    if !st.engine.is_recording() {
        return;
    }
    let data = engine::json::Json::Object(vec![
        ("dimension".into(), engine::json::Json::String(dimension.to_owned())),
        ("loaded_from_disk".into(), engine::json::Json::Bool(loaded_from_disk)),
    ])
    .to_compact();
    st.engine.record_tick(vec![Event::EntitySpawn {
        id: entity as i32,
        uuid: String::new(),
        kind: kind.to_owned(),
        pos: Vec3::new(pos[0], pos[1], pos[2]),
        yaw: 0.0,
        pitch: 0.0,
        data,
    }]);
}

/// An entity leaving the level.
pub(crate) fn entity_leave(game: &mut crate::game::Game<'_>, _dimension: &str, entity: u32, _kind: &str) {
    let mut st = lock(game);
    if !st.engine.is_recording() {
        return;
    }
    st.engine.record_tick(vec![Event::EntityRemove { id: entity as i32 }]);
}

/// Weather starting or stopping.
pub(crate) fn weather_change(
    game: &mut crate::game::Game<'_>,
    _dimension: &str,
    raining: bool,
    thundering: bool,
    _cancel: &mut bool,
) {
    let mut st = lock(game);
    if !st.engine.is_recording() {
        return;
    }
    st.engine.record_tick(vec![Event::Weather {
        raining,
        thundering,
        rain: if raining { 1.0 } else { 0.0 },
        thunder: if thundering { 1.0 } else { 0.0 },
    }]);
}

/// A difficulty change, recorded as a level: `peaceful` is 0, `hard` 3.
pub(crate) fn difficulty(game: &mut crate::game::Game<'_>, _from: &str, to: &str) {
    let mut st = lock(game);
    if !st.engine.is_recording() {
        return;
    }
    let level = match to {
        "peaceful" => 0,
        "easy" => 1,
        "normal" => 2,
        _ => 3,
    };
    st.engine.record_tick(vec![Event::Difficulty { level, locked: false }]);
}

// ------------------------------------------------------------------ lifecycle

/// A world opening or closing: auto-start, auto-stop and crash recovery.
pub(crate) fn lifecycle(
    game: &mut crate::game::Game<'_>,
    _context: &crate::mod_api::NativeLifecycleContext,
    phase: crate::game::native_lifecycle::WorldPhase,
    _error_text: &str,
) {
    use crate::game::native_lifecycle::WorldPhase;
    let mut st = lock(game);
    match phase {
        WorldPhase::WorldOpenReady => {
            st.world_ticks = 0;
            st.last_clock = (i64::MIN, i64::MIN);
            st.last_weather = (false, false);
            st.dimension.clear();
            // A journal from an interrupted session is offered back.
            let journals = engine::recovery::pending_journals(&st.engine.paths.journal_dir);
            for journal in journals {
                if let Ok(Some(recovered)) = engine::recovery::recover(&journal) {
                    if let Ok(recording) = engine::recording::Recording::parse(recovered.bytes) {
                        let mut metadata = engine::package::ReplayMetadata::from_recording(
                            &recording,
                            "(recovered)",
                            now_unix(),
                        );
                        metadata.notes = format!("Recovered from {} at tick {}", journal.display(), recovered.tick);
                        st.engine.recovered = Some(ReplayPackage { metadata, recording, thumbnail: None });
                        st.toast("A crashed recording was recovered: open the browser to keep it", 0xFFFF_C857);
                    }
                    let _ = std::fs::remove_file(&journal);
                }
            }
            // Auto-start: recording begins with the world, as Replay Mod does.
            if !st.auto_recording {
                let origin = origin_for(game);
                let name = st.engine.start_recording(origin, now_unix());
                st.auto_recording = true;
                st.toast(&format!("Recording {}", name.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()), 0xFFFF_3B30);
            }
        }
        WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed => {
            if st.engine.is_recording() {
                finish_recording(&mut st);
            }
            st.auto_recording = false;
            if let Some(playback) = st.engine.playback_mut() {
                playback.pause();
            }
        }
        _ => {}
    }
}

// ------------------------------------------------------- Mods and settings UI

/// The Mods page row.
pub(crate) fn mods_list(mods: &mut Vec<crate::mod_api::NativeMod>) {
    mods.push(crate::mod_api::NativeMod {
        id: ID.into(),
        title: format!("{NAME} {VERSION}"),
        description: DESCRIPTION.into(),
        keys: KEYS
            .iter()
            .map(|(name, label, default)| [format!("key.{name}"), (*label).to_owned(), (*default).to_owned()])
            .collect(),
    });
}

/// The mod's settings page, as buttons the Mods screen draws.
pub(crate) fn settings_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id != ID {
        return;
    }
    let mut st = lock(game);
    buttons.push(format!("Theme: {}", st.engine.config.ui.theme.id()));
    buttons.push(format!("HUD: {}", flag_text(st.engine.config.hud.master_visible)));
    buttons.push(format!("Transport bar: {}", flag_text(st.engine.config.hud.transport)));
    buttons.push(format!("Recording indicator: {}", flag_text(st.engine.config.hud.recording_indicator)));
    buttons.push(format!("Low-memory mode: {}", flag_text(st.engine.config.performance.low_memory)));
    buttons.push(format!("Playback distance: {}", st.engine.config.playback.render_distance));
    buttons.push("Open Volt Lapse: viewer".to_owned());
    buttons.push("Open Volt Lapse: settings".to_owned());
    buttons.push("Reset to defaults".to_owned());
}

/// A click on the mod's settings page.
pub(crate) fn settings_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id != ID {
        return;
    }
    let mut st = lock(game);
    match button {
        0 => {
            let next = if st.engine.config.ui.theme.is_dark() { "light" } else { "dark" };
            let _ = st.engine.config.set_key(engine::config::Section::Ui, "theme", next);
        }
        1 => st.engine.config.hud.master_visible = !st.engine.config.hud.master_visible,
        2 => st.engine.config.hud.transport = !st.engine.config.hud.transport,
        3 => st.engine.config.hud.recording_indicator = !st.engine.config.hud.recording_indicator,
        4 => {
            st.engine.config.performance.low_memory = !st.engine.config.performance.low_memory;
            st.engine.config.performance = st.engine.config.performance.clone().sanitized();
        }
        5 => {
            let distance = st.engine.config.playback.render_distance;
            st.engine.config.playback.render_distance = if distance >= 32 { 8 } else { distance + 4 };
        }
        6 => st.ui.screen = Screen::Replays,
        7 => st.ui.screen = Screen::Settings,
        _ => {
            st.engine.config.reset(engine::config::Section::All);
            st.toast("Volt Lapse settings reset", 0xFF3C_9BFF);
        }
    }
    apply_config_to_runtime(&mut st);
    for (key, value) in [
        ("theme", st.engine.config.ui.theme.id().to_owned()),
        ("hud", flag_text(st.engine.config.hud.master_visible)),
        ("transport", flag_text(st.engine.config.hud.transport)),
        ("indicator", flag_text(st.engine.config.hud.recording_indicator)),
        ("low_memory", flag_text(st.engine.config.performance.low_memory)),
        ("render_distance", st.engine.config.playback.render_distance.to_string()),
    ] {
        game.native_set_setting(ID, key, &value);
    }
    let _ = st.engine.save_config();
}

// ----------------------------------------------------------------- commands

/// `/voltlapse` itself, before the server sees the line.
pub(crate) fn command(game: &mut crate::game::Game<'_>, line: &str, ran: &mut bool) {
    let trimmed = line.trim();
    let Some(rest) = trimmed.strip_prefix(command::ROOT) else {
        return;
    };
    // Only `/voltlapse` or `/voltlapse ...`, not `/voltlapsel`.
    if !rest.is_empty() && !rest.starts_with(' ') {
        return;
    }
    *ran = true;
    let mut st = lock(game);
    match command::parse(rest.trim()) {
        Ok(parsed) => run_command(game, &mut st, parsed),
        Err(error) => st.toast(&format!("Volt Lapse: {error}"), 0xFFFF_3B30),
    }
}

/// The names the chat box colours and completes.
pub(crate) fn commands(_game: &mut crate::game::Game<'_>, commands: &mut Vec<&'static str>) {
    commands.push(command::ROOT);
}

fn run_command(game: &mut crate::game::Game<'_>, st: &mut State, parsed: Command) {
    use engine::command::{BookmarkAction, KeyframeAction, RecordAction, RenderAction};
    match parsed {
        Command::Record { action } => match action {
            RecordAction::Toggle => toggle_recording(game, st),
            RecordAction::Start => {
                if st.engine.is_recording() {
                    st.toast("Already recording", 0xFFFF_C857);
                } else {
                    toggle_recording(game, st);
                }
            }
            RecordAction::Stop => {
                if st.engine.is_recording() {
                    finish_recording(st);
                } else {
                    st.toast("Not recording", 0xFFFF_C857);
                }
            }
            RecordAction::Pause => st.engine.pause_recording(),
            RecordAction::Resume => st.engine.resume_recording(),
            RecordAction::Save => match st.engine.snapshot_recording() {
                Some(bytes) => {
                    let journal = st.engine.paths.journal_file("manual");
                    match engine::recovery::Journal::open(&journal).and_then(|mut journal| {
                        let tick = st.engine.recorder().map(|r| r.tick()).unwrap_or(0);
                        journal.write_snapshot(tick, &bytes)
                    }) {
                        Ok(()) => st.toast("Checkpoint written", 0xFF3C_CB6B),
                        Err(error) => st.toast(&format!("Checkpoint failed: {error}"), 0xFFFF_3B30),
                    }
                }
                None => st.toast("Not recording", 0xFFFF_C857),
            },
            RecordAction::Stats => {
                if let Some(recorder) = st.engine.recorder() {
                    let stats = recorder.stats();
                    let line = format!(
                        "{} ticks, {} events, {} ({:.0}%)",
                        stats.ticks,
                        stats.events,
                        engine::storage::human_size(stats.bytes_written),
                        stats.compression_percent()
                    );
                    st.toast(&line, 0xFF3C_9BFF);
                } else {
                    st.toast("Not recording", 0xFFFF_C857);
                }
            }
        },
        Command::Play { replay, at_seconds, speed } => {
            if let Some(name) = replay {
                let index = st
                    .engine
                    .library
                    .entries()
                    .iter()
                    .position(|entry| entry.name.eq_ignore_ascii_case(&name) || entry.path.to_string_lossy().contains(&name));
                match index {
                    Some(index) => {
                        st.ui.selected = index;
                        play_selected(game, st);
                    }
                    None => st.toast(&format!("No replay called {name}"), 0xFFFF_3B30),
                }
            }
            if let Some(playback) = st.engine.playback_mut() {
                if let Some(seconds) = at_seconds {
                    playback.seek_seconds(seconds);
                }
                if let Some(speed) = speed {
                    playback.set_speed(speed);
                }
                if !speed.is_some() && !at_seconds.is_some() {
                    playback.play();
                }
            } else if replay.is_none() {
                st.ui.screen = Screen::Replays;
            }
        }
        Command::Render { action } => match action {
            RenderAction::Start | RenderAction::Queue => start_render(st),
            RenderAction::Cancel => {
                st.render = None;
                st.toast("Render cancelled", 0xFFFF_C857);
            }
            RenderAction::Status => match &st.render {
                Some(run) => st.toast(
                    &format!("{}/{} frames", run.requested, run.ticks.len()),
                    0xFF3C_9BFF,
                ),
                None => st.toast("No render running", 0xFF3C_9BFF),
            },
        },
        Command::Config { section, key, value } => match (key, value) {
            (None, _) => {
                st.toast(&format!("Section {}", section.id()), 0xFF3C_9BFF);
            }
            (Some(key), None) => match st.engine.config.get_key(section, &key) {
                Some(value) => st.toast(&format!("{}.{key} = {value}", section.id()), 0xFF3C_9BFF),
                None => st.toast(&format!("{} has no {key}", section.id()), 0xFFFF_3B30),
            },
            (Some(key), Some(value)) => {
                if key == "reset" {
                    st.engine.config.reset(section);
                    apply_config_to_runtime(st);
                    let _ = st.engine.save_config();
                    st.toast(&format!("{} reset", section.id()), 0xFF3C_9BFF);
                } else {
                    match st.engine.config.set_key(section, &key, &value) {
                        Ok(()) => {
                            apply_config_to_runtime(st);
                            let _ = st.engine.save_config();
                            st.toast(&format!("{}.{key} = {value}", section.id()), 0xFF3C_CB6B);
                        }
                        Err(error) => st.toast(&format!("{error}"), 0xFFFF_3B30),
                    }
                }
            }
        },
        Command::Keyframe { action } => match action {
            KeyframeAction::Add => {
                if st.engine.add_keyframe().is_some() {
                    st.toast("Keyframe added", 0xFF3C_9BFF);
                }
            }
            KeyframeAction::AddAt(tick) => {
                if let Some(playback) = st.engine.playback_mut() {
                    playback.seek_tick(tick);
                }
                if st.engine.add_keyframe().is_some() {
                    st.toast(&format!("Keyframe added at {tick}"), 0xFF3C_9BFF);
                }
            }
            KeyframeAction::Delete(tick) => {
                if let Some(path) = st.engine.rig.path.as_mut() {
                    if let Some(index) = path.nearest(tick) {
                        path.remove(index);
                        st.toast("Keyframe deleted", 0xFF3C_9BFF);
                    }
                }
            }
            KeyframeAction::Clear => {
                if let Some(path) = st.engine.rig.path.as_mut() {
                    *path = CameraPath::new("Camera path");
                    st.toast("Camera path cleared", 0xFF3C_9BFF);
                }
            }
            KeyframeAction::List => {
                let count = st.engine.rig.path.as_ref().map(|path| path.len()).unwrap_or(0);
                st.toast(&format!("{count} keyframes"), 0xFF3C_9BFF);
            }
        },
        Command::Bookmark { action, label } => match action {
            BookmarkAction::Add => {
                let label = label.unwrap_or_else(|| "Bookmark".to_owned());
                if st.engine.add_bookmark(&label).is_some() {
                    st.toast(&format!("Bookmark '{label}' added"), 0xFF3C_9BFF);
                }
            }
            BookmarkAction::Remove(tick) => {
                if st.engine.bookmarks.remove(tick) {
                    st.toast("Bookmark removed", 0xFF3C_9BFF);
                }
            }
            BookmarkAction::List => {
                let count = st.engine.bookmarks.as_slice().len();
                st.toast(&format!("{count} bookmarks"), 0xFF3C_9BFF);
            }
            BookmarkAction::Next => {
                let at = st.engine.playback().map(|playback| playback.tick()).unwrap_or(0);
                let next = st.engine.bookmarks.next_after(at).map(|bookmark| bookmark.tick);
                if let (Some(tick), Some(playback)) = (next, st.engine.playback_mut()) {
                    playback.seek_tick(tick);
                }
            }
            BookmarkAction::Previous => {
                let tick = st.engine.playback().map(|p| p.tick()).unwrap_or(0);
                if let Some(previous) = st.engine.bookmarks.previous_before(tick) {
                    let at = previous.tick;
                    if let Some(playback) = st.engine.playback_mut() {
                        playback.seek_tick(at);
                    }
                }
            }
        },
        Command::List { query } => {
            let found: Vec<String> = st
                .engine
                .library
                .search(&query)
                .iter()
                .take(6)
                .map(|entry| format!("{} ({})", entry.name, entry.duration_text()))
                .collect();
            st.toast(&if found.is_empty() { "No replays found".to_owned() } else { found.join(", ") }, 0xFF3C_9BFF);
            st.ui.screen = Screen::Replays;
        }
        Command::Info => match st.engine.playback() {
            Some(playback) => {
                let line = format!(
                    "{} ticks, {:.1}s, {} players",
                    playback.duration_ticks(),
                    playback.duration_seconds(),
                    playback.players().len()
                );
                st.toast(&line, 0xFF3C_9BFF);
            }
            None => st.toast("No replay open", 0xFFFF_C857),
        },
        Command::Export { name } => {
            let dir = st.engine.paths.exports_dir.clone();
            let index = st
                .engine
                .library
                .entries()
                .iter()
                .position(|entry| entry.name.eq_ignore_ascii_case(&name));
            match index {
                Some(index) => match st.engine.library.export(index, &dir, &name) {
                    Ok(path) => st.toast(&format!("Exported to {}", path.display()), 0xFF3C_CB6B),
                    Err(error) => st.toast(&format!("Export failed: {error}"), 0xFFFF_3B30),
                },
                None => st.toast(&format!("No replay called {name}"), 0xFFFF_3B30),
            }
        }
        Command::Share { action } => {
            use engine::command::ShareAction;
            match action {
                ShareAction::Host => {
                    let replay = st
                        .engine
                        .playback()
                        .map(|_| "current".to_owned())
                        .unwrap_or_default();
                    let session = engine::sync::SyncSession::host("local", LOCAL, replay);
                    st.session = Some(session);
                    st.toast("Replay sync: hosting", 0xFF3C_9BFF);
                }
                ShareAction::Join => {
                    st.session = Some(engine::sync::SyncSession::host("local", "host", "current"));
                    st.toast("Replay sync: waiting for the host's state", 0xFF3C_9BFF);
                }
                ShareAction::Leave => {
                    st.session = None;
                    st.toast("Replay sync: left", 0xFF3C_9BFF);
                }
                ShareAction::Status => {
                    let line = st
                        .session
                        .as_ref()
                        .map(|session| session.status_line(st.engine.playback().map(|p| p.recording().header().tick_rate).unwrap_or(20)))
                        .unwrap_or_else(|| "Not sharing".to_owned());
                    st.toast(&line, 0xFF3C_9BFF);
                }
            }
        }
        Command::Help => {
            st.ui.screen = Screen::Help;
        }
    }
}

/// Turn a `Debug` rendering of an item stack into an engine stack.
///
/// Horizon's item stack is the game's own type, so the mod reads the two fields
/// it needs from its debug form instead of depending on the type's layout.
fn stack_from_debug(text: &str) -> Option<ItemStack> {
    let id = between(text, "item: \"", "\"")?;
    let count = between(text, "count: ", ",").and_then(|count| count.trim().parse::<u32>().ok()).unwrap_or(1);
    if id.is_empty() || id == "minecraft:air" {
        return None;
    }
    Some(ItemStack {
        id,
        count: count.clamp(0, 255) as u8,
        components: String::new(),
    })
}

/// The text between two markers, unescaped for quotes.
fn between(text: &str, start: &str, end: &str) -> Option<String> {
    let from = text.find(start)? + start.len();
    let rest = &text[from..];
    let to = rest.find(end)?;
    Some(rest[..to].trim().to_owned())
}
