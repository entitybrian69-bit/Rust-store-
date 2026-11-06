// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! Playback: the world state a replay reconstructs, and the transport that
//! moves through it.
//!
//! A replay is a list of events, not a list of frames, so playing one means
//! applying events to a world state in order. Seeking backwards rebuilds from
//! the start (a two-hour recording rebuilds in milliseconds because nothing but
//! the event list is touched); seeking forward applies only what lies between.
//! Positions are interpolated by a partial tick, so a 60 fps render of a 20 tps
//! recording moves smoothly instead of stepping.

use std::collections::BTreeMap;

use crate::event::{BlockPos, ChatLine, Event, ItemStack, LightUpdate, Slot};
use crate::camera::CameraTarget;
use crate::recording::{Recording, RecordingError};
use crate::wire::Vec3;

/// The speeds the transport offers, as multipliers of real time.
pub const SPEEDS: [f64; 8] = [0.1, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];

/// What is known about one entity at the current tick.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityState {
    pub id: i32,
    pub uuid: String,
    pub kind: String,
    pub pos: Vec3,
    /// Position one tick ago, for interpolation.
    pub previous: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub data: String,
}

/// A recorded player's live state.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerState {
    pub uuid: String,
    pub name: String,
    pub pos: Vec3,
    pub previous: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub sprinting: bool,
    pub sneaking: bool,
    pub flying: bool,
    pub health: f32,
    pub food: u8,
    pub level: u32,
    pub inventory: BTreeMap<Slot, ItemStack>,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            uuid: String::new(),
            name: String::new(),
            pos: Vec3::ZERO,
            previous: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
            sprinting: false,
            sneaking: false,
            flying: false,
            health: 20.0,
            food: 20,
            level: 0,
            inventory: BTreeMap::new(),
        }
    }
}

impl PlayerState {
    /// Where the camera sits at `alpha` between the previous and current tick.
    pub fn interpolated(&self, alpha: f64) -> Vec3 {
        self.previous.lerp(self.pos, alpha.clamp(0.0, 1.0))
    }
}

/// Weather, time and difficulty, as the replay knows them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldConditions {
    pub game_time: i64,
    pub day_time: i64,
    pub raining: bool,
    pub thundering: bool,
    pub rain: f32,
    pub thunder: f32,
    pub difficulty: u8,
    pub difficulty_locked: bool,
}

impl Default for WorldConditions {
    fn default() -> Self {
        Self {
            game_time: 0,
            day_time: 1_000,
            raining: false,
            thundering: false,
            rain: 0.0,
            thunder: 0.0,
            difficulty: 2,
            difficulty_locked: false,
        }
    }
}

/// The reconstructed world at the transport's cursor.
#[derive(Clone, Debug, Default)]
pub struct WorldState {
    pub entities: BTreeMap<i32, EntityState>,
    pub players: BTreeMap<String, PlayerState>,
    pub blocks: BTreeMap<(i32, i32, i32), String>,
    pub light: BTreeMap<(i32, i32, i32), LightUpdate>,
    pub conditions: WorldConditions,
    pub chat: Vec<ChatLine>,
    pub dimension: String,
  }

impl WorldState {
    fn apply(&mut self, event: &Event) {
        match event {
            Event::PlayerState { uuid, pos, yaw, pitch, on_ground, sprinting, sneaking, flying } => {
                let player = self.players.entry(uuid.clone()).or_default();
                player.uuid = uuid.clone();
                player.previous = player.pos;
                player.pos = *pos;
                player.yaw = *yaw;
                player.pitch = *pitch;
                player.on_ground = *on_ground;
                player.sprinting = *sprinting;
                player.sneaking = *sneaking;
                player.flying = *flying;
                if player.name.is_empty() {
                    player.name = uuid.clone();
                }
            }
            Event::EntitySpawn { id, uuid, kind, pos, yaw, pitch, data } => {
                self.entities.insert(
                    *id,
                    EntityState {
                        id: *id,
                        uuid: uuid.clone(),
                        kind: kind.clone(),
                        pos: *pos,
                        previous: *pos,
                        yaw: *yaw,
                        pitch: *pitch,
                        data: data.clone(),
                    },
                );
            }
            Event::EntityMove { id, pos, yaw, pitch } => {
                if let Some(entity) = self.entities.get_mut(id) {
                    entity.previous = entity.pos;
                    entity.pos = *pos;
                    entity.yaw = *yaw;
                    entity.pitch = *pitch;
                }
            }
            Event::EntityRemove { id } => {
                self.entities.remove(id);
            }
            Event::EntityMetadata { id, data } => {
                if let Some(entity) = self.entities.get_mut(id) {
                    entity.data = data.clone();
                }
            }
            Event::BlockChange { pos, state, dimension } => {
                if !self.dimension.is_empty() && *dimension != self.dimension {
                    // A dimension change replaces the block view entirely.
                    self.blocks.clear();
                    self.light.clear();
                }
                self.dimension = dimension.clone();
                self.blocks.insert((pos.x, pos.y, pos.z), state.clone());
            }
            Event::Chat(line) => {
                self.chat.push(line.clone());
                // The scrolling window keeps the last 100 lines, as the chat does.
                if self.chat.len() > 100 {
                    self.chat.remove(0);
                }
            }
            Event::WorldTime { game_time, day_time } => {
                self.conditions.game_time = *game_time;
                self.conditions.day_time = *day_time;
            }
            Event::Weather { raining, thundering, rain, thunder } => {
                self.conditions.raining = *raining;
                self.conditions.thundering = *thundering;
                self.conditions.rain = *rain;
                self.conditions.thunder = *thunder;
            }
            Event::Difficulty { level, locked } => {
                self.conditions.difficulty = *level;
                self.conditions.difficulty_locked = *locked;
            }
            Event::Inventory { uuid, slot, stack } => {
                let player = self.players.entry(uuid.clone()).or_default();
                player.uuid = uuid.clone();
                match stack {
                    Some(stack) => {
                        player.inventory.insert(*slot, stack.clone());
                    }
                    None => {
                        player.inventory.remove(slot);
                    }
                }
            }
            Event::Equipment { entity, slot, stack } => {
                if let Some(entity) = self.entities.get_mut(entity) {
                    let mut data = entity.data.clone();
                    if !data.is_empty() {
                        data.push(';');
                    }
                    match stack {
                        Some(stack) => data.push_str(&format!("equipment.{:?}={}", slot, stack.id)),
                        None => data.push_str(&format!("equipment.{:?}=", slot)),
                    }
                    entity.data = data;
                }
            }
            Event::PlayerStatus { uuid, health, food, level, .. } => {
                let player = self.players.entry(uuid.clone()).or_default();
                player.uuid = uuid.clone();
                player.health = *health;
                player.food = *food;
                player.level = *level;
            }
            Event::Light(update) => {
                let pos = update.section;
                self.light.insert((pos.x, pos.y, pos.z), update.clone());
            }
            Event::UiState { .. } | Event::Particle(_) | Event::Sound(_) | Event::Packet { .. } | Event::Marker { .. } => {
                // Transient: the HUD, the particle emitters and the sound engine
                // consume these as they are played, not as world state.
            }
        }
    }

    /// The lowest block state at a position, for the renderer's culling debug.
    pub fn block_at(&self, pos: BlockPos) -> Option<&str> {
        self.blocks.get(&(pos.x, pos.y, pos.z)).map(String::as_str)
    }

    /// The light level recorded for a position's section, block channel (0-15).
    pub fn block_light_at(&self, pos: BlockPos) -> Option<u8> {
        let section = (pos.x >> 4, (pos.y >> 4) << 4, pos.z >> 4);
        let packed = self.light.get(&section)?;
        packed.block_light.first().copied()
    }
}

/// The transport: what is playing, from where, at what speed.
#[derive(Clone, Debug)]
pub struct Playback {
    recording: Recording,
    state: WorldState,
    /// Fractional tick cursor; the whole part is the applied tick.
    cursor: f64,
    playing: bool,
    speed: f64,
    loop_mode: bool,
    /// Tick the loop returns to (a bookmark or a selection start).
    loop_start: u64,
    /// Tick the loop stops at (`None` is the end of the recording).
    loop_end: Option<u64>,
    /// HUD visibility, toggled with F1 semantics.
    pub hud_hidden: bool,
    /// Render resolution override, independent of the window.
    pub resolution_override: Option<(u32, u32)>,
    /// Speed changes apply immediately, but a render ignores the transport.
    pub respect_speed_segments: bool,
    /// Last applied tick, so seeking backwards knows to rebuild.
    applied_tick: u64,
    /// How many events the current state has seen, for diagnostics.
    applied_events: u64,
}

impl Playback {
    /// Start a playback at tick 0, paused.
    pub fn new(recording: Recording) -> Self {
        let mut playback = Self {
            recording,
            state: WorldState::default(),
            cursor: 0.0,
            playing: false,
            speed: 1.0,
            loop_mode: false,
            loop_start: 0,
            loop_end: None,
            hud_hidden: false,
            resolution_override: None,
            respect_speed_segments: true,
            applied_tick: 0,
            applied_events: 0,
        };
        playback.rebuild(0);
        playback
    }

    pub fn recording(&self) -> &Recording {
        &self.recording
    }

    pub fn world(&self) -> &WorldState {
        &self.state
    }

    pub fn tick(&self) -> u64 {
        self.cursor.floor().max(0.0) as u64
    }

    /// The partial tick between the applied tick and the next one, for
    /// interpolating positions at frame rate.
    pub fn alpha(&self) -> f64 {
        (self.cursor - self.cursor.floor()).clamp(0.0, 1.0)
    }

    pub fn time_seconds(&self) -> f64 {
        self.cursor / self.recording.header().tick_rate.max(1) as f64
    }

    pub fn duration_seconds(&self) -> f64 {
        self.recording.duration_seconds()
    }

    pub fn duration_ticks(&self) -> u64 {
        self.recording.tick_count()
    }

    /// 0..=1 progress, for the timeline scrubber.
    pub fn progress(&self) -> f64 {
        let total = self.duration_ticks();
        if total == 0 {
            0.0
        } else {
            (self.cursor / total as f64).clamp(0.0, 1.0)
        }
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn speed(&self) -> f64 {
        self.speed
    }

    pub fn loop_mode(&self) -> bool {
        self.loop_mode
    }

    pub fn set_loop_mode(&mut self, enabled: bool) {
        self.loop_mode = enabled;
    }

    /// Restrict the loop to a range (a bookmark pair, a camera path).
    pub fn set_loop_range(&mut self, start: u64, end: Option<u64>) {
        self.loop_start = start.min(self.duration_ticks());
        self.loop_end = end.map(|end| end.min(self.duration_ticks()));
    }

    pub fn play(&mut self) {
        if self.cursor >= self.duration_ticks() as f64 {
            self.rebuild(0);
            self.cursor = 0.0;
        }
        self.playing = true;
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn toggle(&mut self) {
        if self.playing {
            self.pause();
        } else {
            self.play();
        }
    }

    /// Set an arbitrary speed (clamped to the transport's range).
    pub fn set_speed(&mut self, speed: f64) {
        self.speed = speed.clamp(SPEEDS[0], SPEEDS[SPEEDS.len() - 1]);
    }

    /// Step to the next speed in [`SPEEDS`].
    pub fn faster(&mut self) {
        let next = SPEEDS.iter().copied().find(|s| *s > self.speed + 1e-9);
        self.speed = next.unwrap_or(self.speed);
    }

    /// Step to the previous speed in [`SPEEDS`].
    pub fn slower(&mut self) {
        let next = SPEEDS.iter().rev().copied().find(|s| *s < self.speed - 1e-9);
        self.speed = next.unwrap_or(self.speed);
    }

    /// The index of the current speed in [`SPEEDS`], for the HUD.
    pub fn speed_index(&self) -> Option<usize> {
        SPEEDS.iter().position(|s| (s - self.speed).abs() < 1e-9)
    }

    /// Advance by wall-clock seconds. Returns `true` when the recording ended.
    pub fn advance(&mut self, delta_seconds: f64) -> bool {
        if !self.playing || delta_seconds <= 0.0 {
            return false;
        }
        let tick_rate = self.recording.header().tick_rate.max(1) as f64;
        let advance = delta_seconds * tick_rate * self.speed;
        let end = self.effective_end();
        let mut cursor = self.cursor + advance;
        let mut ended = false;
        if cursor >= end as f64 {
            if self.loop_mode {
                let start = self.loop_start as f64;
                let span = (end as f64 - start).max(1.0);
                cursor = start + (cursor - end as f64) % span;
                self.rebuild(cursor.floor() as u64);
            } else {
                cursor = end as f64;
                ended = true;
                self.playing = false;
            }
        }
        self.cursor = cursor;
        let target = self.tick();
        if target != self.applied_tick {
            self.advance_to(target);
        }
        ended
    }

    fn effective_end(&self) -> u64 {
        self.loop_end.unwrap_or_else(|| self.duration_ticks()).min(self.duration_ticks())
    }

    /// Jump to a tick, rebuilding or advancing as needed.
    pub fn seek_tick(&mut self, tick: u64) {
        let tick = tick.min(self.duration_ticks());
        if tick < self.applied_tick {
            self.rebuild(tick);
        } else {
            self.advance_to(tick);
        }
        self.cursor = tick as f64;
    }

    /// Jump to a fraction of the recording, as dragging the scrubber does.
    pub fn seek_fraction(&mut self, fraction: f64) {
        let tick = (fraction.clamp(0.0, 1.0) * self.duration_ticks() as f64).round() as u64;
        self.seek_tick(tick);
    }

    /// Jump by a signed number of seconds (rewind / fast-forward buttons).
    pub fn seek_seconds(&mut self, seconds: f64) {
        let tick_rate = self.recording.header().tick_rate.max(1) as f64;
        let target = self.cursor + seconds * tick_rate;
        self.seek_tick(target.max(0.0) as u64);
    }

    /// Rebuild the world state from the start of the recording.
    pub fn rebuild(&mut self, tick: u64) {
        self.state = WorldState::default();
        self.applied_tick = 0;
        self.applied_events = 0;
        self.advance_to(tick);
    }

    fn advance_to(&mut self, tick: u64) {
        if tick <= self.applied_tick {
            self.applied_tick = self.applied_tick.max(tick);
            return;
        }
        let from = self.applied_tick;
        let mut applied = 0u64;
        let mut error: Option<RecordingError> = None;
        {
            let state = &mut self.state;
            let result = self.recording.for_each_event(from, tick, |_, event| {
                state.apply(event);
                applied += 1;
            });
            if let Err(why) = result {
                error = Some(why);
            }
        }
        if error.is_some() {
            // A corrupt tail stops playback at the last good tick rather than
            // panicking mid-frame.
            self.playing = false;
        }
        self.applied_events += applied;
        self.applied_tick = tick;
    }

    /// How many events the current state has applied (diagnostics).
    pub fn applied_events(&self) -> u64 {
        self.applied_events
    }

    /// The player UUIDs this replay can follow, in the order they appear.
    pub fn players(&self) -> Vec<String> {
        self.state.players.keys().cloned().collect()
    }

    /// A name for a player UUID, for the HUD and the browser.
    pub fn player_name(&self, uuid: &str) -> Option<&str> {
        self.state.players.get(uuid).map(|p| p.name.as_str())
    }

    /// The camera target for a player: their interpolated eye position and head
    /// rotation, ready to hand to a [`crate::camera::CameraRig`].
    pub fn camera_target(&self, uuid: &str, eye_height: f32) -> Option<CameraTarget> {
        let player = self.state.players.get(uuid)?;
        let pos = player.interpolated(self.alpha());
        Some(CameraTarget { pos, eye_height, yaw: player.yaw, pitch: player.pitch })
    }

    /// The local player's own state, which the first-person camera starts from.
    pub fn primary_player(&self) -> Option<&PlayerState> {
        self.state.players.values().next()
    }

    /// Move the fractional cursor without applying events. Only the playback
    /// itself and the offline renderer (which drives frames between ticks) use
    /// this.
    pub fn set_cursor(&mut self, cursor: f64) {
        self.cursor = cursor.max(0.0).min(self.duration_ticks() as f64 + 1.0);
    }

    /// Set a player's display name (the metadata editor can rename).
    pub fn set_player_name(&mut self, uuid: &str, name: &str) {
        if let Some(player) = self.state.players.get_mut(uuid) {
            player.name = name.to_owned();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording::{Recorder, RecordingOptions};

    fn recording_of(ticks: u64) -> Recording {
        let mut recorder = Recorder::new(
            RecordingOptions { block_ticks: 10, ..RecordingOptions::default() },
            "26.3",
            "test",
        );
        for tick in 0..ticks {
            recorder.record(Event::PlayerState {
                uuid: "alex".into(),
                pos: Vec3::new(tick as f64, 64.0, 0.0),
                yaw: tick as f32,
                pitch: 0.0,
                on_ground: true,
                sprinting: tick % 2 == 0,
                sneaking: false,
                flying: false,
            });
            if tick == 0 {
                recorder.record(Event::EntitySpawn {
                    id: 7,
                    uuid: "zombie".into(),
                    kind: "minecraft:zombie".into(),
                    pos: Vec3::new(0.0, 64.0, 5.0),
                    yaw: 0.0,
                    pitch: 0.0,
                    data: "{}".into(),
                });
                recorder.record(Event::Chat(ChatLine { text: "hello".into(), action_bar: false, sender: "Alex".into() }));
                recorder.record(Event::WorldTime { game_time: 100, day_time: 1000 });
            }
            if tick == 5 {
                recorder.record(Event::BlockChange {
                    pos: BlockPos::new(1, 64, 1),
                    state: "minecraft:stone".into(),
                    dimension: "minecraft:overworld".into(),
                });
                recorder.record(Event::Weather { raining: true, thundering: false, rain: 1.0, thunder: 0.0 });
                recorder.record(Event::EntityRemove { id: 7 });
                recorder.record(Event::PlayerStatus {
                    uuid: "alex".into(),
                    health: 15.0,
                    food: 18,
                    saturation: 2.0,
                    level: 5,
                    experience: 0.5,
                });
            }
            if tick == 7 {
                recorder.record(Event::EntityMove { id: 7, pos: Vec3::new(1.0, 64.0, 5.0), yaw: 90.0, pitch: 0.0 });
            }
            recorder.end_tick();
        }
        Recording::parse(recorder.finish().0).expect("parses")
    }

    #[test]
    fn a_fresh_playback_is_paused_at_zero() {
        let playback = Playback::new(recording_of(20));
        assert!(!playback.is_playing());
        assert_eq!(playback.tick(), 0);
        assert_eq!(playback.duration_ticks(), 20);
        assert!((playback.duration_seconds() - 1.0).abs() < 1e-9);
        assert_eq!(playback.world().players.len(), 1);
        assert_eq!(playback.world().chat.len(), 1);
        assert_eq!(playback.world().conditions.day_time, 1000);
    }

    #[test]
    fn seeking_forward_applies_events_and_backwards_rebuilds() {
        let mut playback = Playback::new(recording_of(20));
        playback.seek_tick(6);
        assert_eq!(playback.tick(), 6);
        assert_eq!(playback.world().block_at(BlockPos::new(1, 64, 1)), Some("minecraft:stone"));
        assert!(playback.world().conditions.raining);
        assert!(playback.world().entities.is_empty(), "the zombie was removed at tick 5");
        assert_eq!(playback.world().players["alex"].health, 15.0);

        playback.seek_tick(2);
        assert!(playback.world().block_at(BlockPos::new(1, 64, 1)).is_none());
        assert_eq!(playback.world().entities.len(), 1);
        assert_eq!(playback.world().players["alex"].health, 20.0);
    }

    #[test]
    fn advancing_uses_the_tick_rate_and_speed() {
        let mut playback = Playback::new(recording_of(100));
        playback.play();
        playback.set_speed(1.0);
        playback.advance(0.5); // 10 ticks at 20 tps
        assert_eq!(playback.tick(), 10);
        playback.set_speed(2.0);
        playback.advance(0.5); // another 20
        assert_eq!(playback.tick(), 30);
        playback.set_speed(0.1);
        playback.advance(1.0); // 2 ticks
        assert_eq!(playback.tick(), 32);
    }

    #[test]
    fn speeds_step_through_the_table_and_clamp() {
        let mut playback = Playback::new(recording_of(10));
        playback.set_speed(1.0);
        playback.faster();
        assert_eq!(playback.speed(), 2.0);
        for _ in 0..10 {
            playback.faster();
        }
        assert_eq!(playback.speed(), 16.0);
        for _ in 0..20 {
            playback.slower();
        }
        assert_eq!(playback.speed(), 0.1);
        playback.set_speed(1_000.0);
        assert_eq!(playback.speed(), 16.0);
        assert_eq!(playback.speed_index(), Some(7));
    }

    #[test]
    fn the_end_pauses_or_loops() {
        let mut playback = Playback::new(recording_of(20));
        playback.play();
        let ended = playback.advance(5.0);
        assert!(ended);
        assert_eq!(playback.tick(), 20);
        assert!(!playback.is_playing());

        playback.set_loop_mode(true);
        playback.play();
        let ended = playback.advance(2.0);
        assert!(!ended);
        assert!(playback.is_playing());
        assert!(playback.tick() < 20);
    }

    #[test]
    fn a_loop_range_keeps_playing_inside_it() {
        let mut playback = Playback::new(recording_of(40));
        playback.set_loop_mode(true);
        playback.set_loop_range(10, Some(20));
        playback.seek_tick(15);
        playback.play();
        for _ in 0..20 {
            playback.advance(0.1);
            assert!((10..20).contains(&playback.tick()), "tick {}", playback.tick());
        }
    }

    #[test]
    fn the_scrubber_maps_fractions_to_ticks() {
        let mut playback = Playback::new(recording_of(100));
        playback.seek_fraction(0.5);
        assert_eq!(playback.tick(), 50);
        assert!((playback.progress() - 0.5).abs() < 1e-9);
        playback.seek_fraction(2.0);
        assert_eq!(playback.tick(), 100);
        playback.seek_fraction(-1.0);
        assert_eq!(playback.tick(), 0);
    }

    #[test]
    fn rewinding_and_skipping_by_seconds() {
        let mut playback = Playback::new(recording_of(200));
        playback.seek_tick(100);
        playback.seek_seconds(1.0);
        assert_eq!(playback.tick(), 120);
        playback.seek_seconds(-2.0);
        assert_eq!(playback.tick(), 80);
        playback.seek_seconds(-100.0);
        assert_eq!(playback.tick(), 0);
    }

    #[test]
    fn camera_targets_interpolate_between_ticks() {
        let mut playback = Playback::new(recording_of(20));
        playback.seek_tick(10);
        playback.set_cursor(10.5);
        let target = playback.camera_target("alex", 1.62).unwrap();
        // Like the game, the render lerps from the previous tick to the current
        // one by the partial tick, so half a tick past tick 10 is 9.5.
        assert!((target.pos.x - 9.5).abs() < 1e-6, "x = {}", target.pos.x);
        assert_eq!(target.eye_height, 1.62);
    }

    #[test]
    fn player_listing_and_renaming() {
        let mut playback = Playback::new(recording_of(5));
        assert_eq!(playback.players(), vec!["alex".to_owned()]);
        playback.set_player_name("alex", "Alex");
        assert_eq!(playback.player_name("alex"), Some("Alex"));
        assert_eq!(playback.player_name("nobody"), None);
        assert!(playback.primary_player().is_some());
    }

    #[test]
    fn an_empty_recording_plays_nothing_and_does_not_panic() {
        let mut playback = Playback::new(recording_of(0));
        assert_eq!(playback.duration_ticks(), 0);
        playback.play();
        assert!(playback.advance(1.0));
        playback.seek_fraction(0.5);
        assert_eq!(playback.tick(), 0);
        assert!(playback.world().players.is_empty());
    }
}
