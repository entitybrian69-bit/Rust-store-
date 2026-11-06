// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! The recordable events.
//!
//! One enum covers everything a tick can carry: players and entities, blocks,
//! chat, world time and weather, difficulty, inventories and equipment,
//! particles, sounds and light. The variant order is the on-disk opcode table,
//! so **never renumber a variant**: add new ones at the end. `OPCODE_VERSION` in
//! `recording.rs` guards a reader against a file from a newer build.

use crate::wire::{Result, Reader, Vec3, Wire, Writer};

/// A block position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

impl Wire for BlockPos {
    fn write(&self, out: &mut Writer) {
        out.zz(self.x as i64);
        out.zz(self.y as i64);
        out.zz(self.z as i64);
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        Ok(BlockPos::new(
            i32::try_from(input.zz()?).map_err(|_| crate::wire::WireError::OutOfRange)?,
            i32::try_from(input.zz()?).map_err(|_| crate::wire::WireError::OutOfRange)?,
            i32::try_from(input.zz()?).map_err(|_| crate::wire::WireError::OutOfRange)?,
        ))
    }
}

/// An item stack, as an inventory or equipment slot holds it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemStack {
    /// `minecraft:diamond_sword`
    pub id: String,
    pub count: u8,
    /// Component patch or NBT, as text; empty for a plain stack.
    pub components: String,
}

impl Wire for ItemStack {
    fn write(&self, out: &mut Writer) {
        out.str(&self.id);
        out.u8(self.count);
        out.str(&self.components);
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        Ok(ItemStack { id: input.string()?, count: input.u8()?, components: input.string()? })
    }
}

/// Which set of slots a change belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Slot {
    Hotbar(u8),
    Main(u8),
    Armor(u8),
    Offhand,
    Container(u8),
    Cursor,
}

impl Wire for Slot {
    fn write(&self, out: &mut Writer) {
        match self {
            Slot::Hotbar(i) => {
                out.u8(0);
                out.u8(*i);
            }
            Slot::Main(i) => {
                out.u8(1);
                out.u8(*i);
            }
            Slot::Armor(i) => {
                out.u8(2);
                out.u8(*i);
            }
            Slot::Offhand => {
                out.u8(3);
                out.u8(0);
            }
            Slot::Container(i) => {
                out.u8(4);
                out.u8(*i);
            }
            Slot::Cursor => {
                out.u8(5);
                out.u8(0);
            }
        }
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        let tag = input.u8()?;
        let index = input.u8()?;
        Ok(match tag {
            0 => Slot::Hotbar(index),
            1 => Slot::Main(index),
            2 => Slot::Armor(index),
            3 => Slot::Offhand,
            4 => Slot::Container(index),
            _ => Slot::Cursor,
        })
    }
}

/// A chat or system message.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatLine {
    /// Rendered text, with formatting sections already resolved.
    pub text: String,
    /// True for action-bar text and other overlay messages.
    pub action_bar: bool,
    /// Sender name, empty for the server itself.
    pub sender: String,
}

/// Where a sound is played: relative ones follow the listener.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundEvent {
    pub id: String,
    pub pos: Vec3,
    pub volume: f32,
    pub pitch: f32,
    /// `master`, `music`, `weather`, `block`, `hostile`, `neutral`, `player`,
    /// `ambient`, `voice`, `record`.
    pub category: String,
    pub relative: bool,
}

/// A particle request, kept so a playback can re-emit it.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleEvent {
    pub id: String,
    pub pos: Vec3,
    pub velocity: Vec3,
    pub count: u32,
    pub spread: f32,
    pub long_distance: bool,
}

/// A chunk section's packed light data.
#[derive(Clone, Debug, PartialEq)]
pub struct LightUpdate {
    pub section: BlockPos,
    pub block_light: Vec<u8>,
    pub sky_light: Vec<u8>,
}

/// One recorded event.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A recorded player's position, rotation and movement flags.
    PlayerState {
        uuid: String,
        pos: Vec3,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
        sprinting: bool,
        sneaking: bool,
        flying: bool,
    },
    /// An entity appearing in the world.
    EntitySpawn {
        id: i32,
        uuid: String,
        kind: String,
        pos: Vec3,
        yaw: f32,
        pitch: f32,
        /// Spawn data (behaviour, variant, name), as JSON text.
        data: String,
    },
    /// An entity moving or turning.
    EntityMove { id: i32, pos: Vec3, yaw: f32, pitch: f32 },
    /// An entity leaving the world.
    EntityRemove { id: i32 },
    /// An entity's metadata changing (health, name, poses, variant).
    EntityMetadata { id: i32, data: String },
    /// A block being placed, broken or updated.
    BlockChange { pos: BlockPos, state: String, dimension: String },
    /// A chat line or system message.
    Chat(ChatLine),
    /// World time moving (or being set by `/time set`).
    WorldTime { game_time: i64, day_time: i64 },
    /// Weather starting, stopping or changing strength.
    Weather { raining: bool, thundering: bool, rain: f32, thunder: f32 },
    /// Difficulty, and whether the world locks it.
    Difficulty { level: u8, locked: bool },
    /// An inventory slot changing.
    Inventory { uuid: String, slot: Slot, stack: Option<ItemStack> },
    /// An equipment slot changing.
    Equipment { entity: i32, slot: Slot, stack: Option<ItemStack> },
    /// Particles spawned by the server (`/particle`, block breaking, ambience).
    Particle(ParticleEvent),
    /// A sound the server or another player played.
    Sound(SoundEvent),
    /// Light data for a chunk section.
    Light(LightUpdate),
    /// Player health, food and experience, for the HUD overlays.
    PlayerStatus {
        uuid: String,
        health: f32,
        food: u8,
        saturation: f32,
        level: u32,
        experience: f32,
    },
    /// A named UI state change (scoreboard, boss bar, title), kept as JSON.
    UiState { channel: String, data: String },
    /// A raw packet body, for full-fidelity capture of anything not modelled
    /// above; `name` is the packet's identifier.
    Packet { direction: u8, name: String, body: Vec<u8> },
    /// A free-standing marker the recorder wrote (chapter, bookmark, note).
    Marker { label: String, data: String },
}

/// Opcodes, in the order of [`Event`]. Never reuse a number.
pub mod opcode {
    pub const PLAYER_STATE: u8 = 0;
    pub const ENTITY_SPAWN: u8 = 1;
    pub const ENTITY_MOVE: u8 = 2;
    pub const ENTITY_REMOVE: u8 = 3;
    pub const ENTITY_METADATA: u8 = 4;
    pub const BLOCK_CHANGE: u8 = 5;
    pub const CHAT: u8 = 6;
    pub const WORLD_TIME: u8 = 7;
    pub const WEATHER: u8 = 8;
    pub const DIFFICULTY: u8 = 9;
    pub const INVENTORY: u8 = 10;
    pub const EQUIPMENT: u8 = 11;
    pub const PARTICLE: u8 = 12;
    pub const SOUND: u8 = 13;
    pub const LIGHT: u8 = 14;
    pub const PLAYER_STATUS: u8 = 15;
    pub const UI_STATE: u8 = 16;
    pub const PACKET: u8 = 17;
    pub const MARKER: u8 = 18;
    /// One past the last opcode.
    pub const COUNT: u8 = 19;
}

impl Event {
    /// The opcode written before the payload.
    pub fn opcode(&self) -> u8 {
        match self {
            Event::PlayerState { .. } => opcode::PLAYER_STATE,
            Event::EntitySpawn { .. } => opcode::ENTITY_SPAWN,
            Event::EntityMove { .. } => opcode::ENTITY_MOVE,
            Event::EntityRemove { .. } => opcode::ENTITY_REMOVE,
            Event::EntityMetadata { .. } => opcode::ENTITY_METADATA,
            Event::BlockChange { .. } => opcode::BLOCK_CHANGE,
            Event::Chat(_) => opcode::CHAT,
            Event::WorldTime { .. } => opcode::WORLD_TIME,
            Event::Weather { .. } => opcode::WEATHER,
            Event::Difficulty { .. } => opcode::DIFFICULTY,
            Event::Inventory { .. } => opcode::INVENTORY,
            Event::Equipment { .. } => opcode::EQUIPMENT,
            Event::Particle(_) => opcode::PARTICLE,
            Event::Sound(_) => opcode::SOUND,
            Event::Light(_) => opcode::LIGHT,
            Event::PlayerStatus { .. } => opcode::PLAYER_STATUS,
            Event::UiState { .. } => opcode::UI_STATE,
            Event::Packet { .. } => opcode::PACKET,
            Event::Marker { .. } => opcode::MARKER,
        }
    }

    /// A short name for logs, the browser and `/voltlapse` output.
    pub fn kind(&self) -> &'static str {
        match self {
            Event::PlayerState { .. } => "player-state",
            Event::EntitySpawn { .. } => "entity-spawn",
            Event::EntityMove { .. } => "entity-move",
            Event::EntityRemove { .. } => "entity-remove",
            Event::EntityMetadata { .. } => "entity-metadata",
            Event::BlockChange { .. } => "block-change",
            Event::Chat(_) => "chat",
            Event::WorldTime { .. } => "world-time",
            Event::Weather { .. } => "weather",
            Event::Difficulty { .. } => "difficulty",
            Event::Inventory { .. } => "inventory",
            Event::Equipment { .. } => "equipment",
            Event::Particle(_) => "particle",
            Event::Sound(_) => "sound",
            Event::Light(_) => "light",
            Event::PlayerStatus { .. } => "player-status",
            Event::UiState { .. } => "ui-state",
            Event::Packet { .. } => "packet",
            Event::Marker { .. } => "marker",
        }
    }

    /// Write the opcode and payload.
    pub fn write(&self, out: &mut Writer) {
        out.u8(self.opcode());
        match self {
            Event::PlayerState { uuid, pos, yaw, pitch, on_ground, sprinting, sneaking, flying } => {
                out.str(uuid);
                pos.write(out);
                out.f32(*yaw);
                out.f32(*pitch);
                out.u8(
                    u8::from(*on_ground)
                        | u8::from(*sprinting) << 1
                        | u8::from(*sneaking) << 2
                        | u8::from(*flying) << 3,
                );
            }
            Event::EntitySpawn { id, uuid, kind, pos, yaw, pitch, data } => {
                out.zz(*id as i64);
                out.str(uuid);
                out.str(kind);
                pos.write(out);
                out.f32(*yaw);
                out.f32(*pitch);
                out.str(data);
            }
            Event::EntityMove { id, pos, yaw, pitch } => {
                out.zz(*id as i64);
                pos.write(out);
                out.f32(*yaw);
                out.f32(*pitch);
            }
            Event::EntityRemove { id } => out.zz(*id as i64),
            Event::EntityMetadata { id, data } => {
                out.zz(*id as i64);
                out.str(data);
            }
            Event::BlockChange { pos, state, dimension } => {
                pos.write(out);
                out.str(state);
                out.str(dimension);
            }
            Event::Chat(line) => {
                out.str(&line.text);
                out.u8(u8::from(line.action_bar));
                out.str(&line.sender);
            }
            Event::WorldTime { game_time, day_time } => {
                out.zz(*game_time);
                out.zz(*day_time);
            }
            Event::Weather { raining, thundering, rain, thunder } => {
                out.u8(u8::from(*raining) | u8::from(*thundering) << 1);
                out.f32(*rain);
                out.f32(*thunder);
            }
            Event::Difficulty { level, locked } => {
                out.u8(*level);
                out.bool(*locked);
            }
            Event::Inventory { uuid, slot, stack } => {
                out.str(uuid);
                slot.write(out);
                stack.write(out);
            }
            Event::Equipment { entity, slot, stack } => {
                out.zz(*entity as i64);
                slot.write(out);
                stack.write(out);
            }
            Event::Particle(event) => {
                out.str(&event.id);
                event.pos.write(out);
                event.velocity.write(out);
                out.varint_u32(event.count);
                out.f32(event.spread);
                out.bool(event.long_distance);
            }
            Event::Sound(event) => {
                out.str(&event.id);
                event.pos.write(out);
                out.f32(event.volume);
                out.f32(event.pitch);
                out.str(&event.category);
                out.bool(event.relative);
            }
            Event::Light(update) => {
                update.section.write(out);
                out.bytes(&update.block_light);
                out.bytes(&update.sky_light);
            }
            Event::PlayerStatus { uuid, health, food, saturation, level, experience } => {
                out.str(uuid);
                out.f32(*health);
                out.u8(*food);
                out.f32(*saturation);
                out.varint_u32(*level);
                out.f32(*experience);
            }
            Event::UiState { channel, data } => {
                out.str(channel);
                out.str(data);
            }
            Event::Packet { direction, name, body } => {
                out.u8(*direction);
                out.str(name);
                out.bytes(body);
            }
            Event::Marker { label, data } => {
                out.str(label);
                out.str(data);
            }
        }
    }

    /// Read one event, opcode first.
    pub fn read(input: &mut Reader<'_>) -> Result<Self> {
        let opcode = input.u8()?;
        Ok(match opcode {
            opcode::PLAYER_STATE => {
                let uuid = input.string()?;
                let pos = Vec3::read(input)?;
                let yaw = input.f32()?;
                let pitch = input.f32()?;
                let flags = input.u8()?;
                Event::PlayerState {
                    uuid,
                    pos,
                    yaw,
                    pitch,
                    on_ground: flags & 1 != 0,
                    sprinting: flags & 2 != 0,
                    sneaking: flags & 4 != 0,
                    flying: flags & 8 != 0,
                }
            }
            opcode::ENTITY_SPAWN => Event::EntitySpawn {
                id: input.zz()? as i32,
                uuid: input.string()?,
                kind: input.string()?,
                pos: Vec3::read(input)?,
                yaw: input.f32()?,
                pitch: input.f32()?,
                data: input.string()?,
            },
            opcode::ENTITY_MOVE => Event::EntityMove {
                id: input.zz()? as i32,
                pos: Vec3::read(input)?,
                yaw: input.f32()?,
                pitch: input.f32()?,
            },
            opcode::ENTITY_REMOVE => Event::EntityRemove { id: input.zz()? as i32 },
            opcode::ENTITY_METADATA => Event::EntityMetadata { id: input.zz()? as i32, data: input.string()? },
            opcode::BLOCK_CHANGE => Event::BlockChange {
                pos: BlockPos::read(input)?,
                state: input.string()?,
                dimension: input.string()?,
            },
            opcode::CHAT => Event::Chat(ChatLine {
                text: input.string()?,
                action_bar: input.bool()?,
                sender: input.string()?,
            }),
            opcode::WORLD_TIME => Event::WorldTime { game_time: input.zz()?, day_time: input.zz()? },
            opcode::WEATHER => {
                let flags = input.u8()?;
                Event::Weather {
                    raining: flags & 1 != 0,
                    thundering: flags & 2 != 0,
                    rain: input.f32()?,
                    thunder: input.f32()?,
                }
            }
            opcode::DIFFICULTY => Event::Difficulty { level: input.u8()?, locked: input.bool()? },
            opcode::INVENTORY => Event::Inventory {
                uuid: input.string()?,
                slot: Slot::read(input)?,
                stack: Option::<ItemStack>::read(input)?,
            },
            opcode::EQUIPMENT => Event::Equipment {
                entity: input.zz()? as i32,
                slot: Slot::read(input)?,
                stack: Option::<ItemStack>::read(input)?,
            },
            opcode::PARTICLE => Event::Particle(ParticleEvent {
                id: input.string()?,
                pos: Vec3::read(input)?,
                velocity: Vec3::read(input)?,
                count: input.varint_u32()?,
                spread: input.f32()?,
                long_distance: input.bool()?,
            }),
            opcode::SOUND => Event::Sound(SoundEvent {
                id: input.string()?,
                pos: Vec3::read(input)?,
                volume: input.f32()?,
                pitch: input.f32()?,
                category: input.string()?,
                relative: input.bool()?,
            }),
            opcode::LIGHT => Event::Light(LightUpdate {
                section: BlockPos::read(input)?,
                block_light: input.bytes_vec()?,
                sky_light: input.bytes_vec()?,
            }),
            opcode::PLAYER_STATUS => Event::PlayerStatus {
                uuid: input.string()?,
                health: input.f32()?,
                food: input.u8()?,
                saturation: input.f32()?,
                level: input.varint_u32()?,
                experience: input.f32()?,
            },
            opcode::UI_STATE => Event::UiState { channel: input.string()?, data: input.string()? },
            opcode::PACKET => Event::Packet {
                direction: input.u8()?,
                name: input.string()?,
                body: input.bytes_vec()?,
            },
            opcode::MARKER => Event::Marker { label: input.string()?, data: input.string()? },
            other => return Err(crate::wire::WireError::OutOfRange),
        })
    }
}

/// The text after `marker` up to the closing quote of a Rust string literal.
fn quoted_after(text: &str, marker: &str) -> Option<String> {
    let from = text.find(marker)? + marker.len();
    let rest = &text[from..];
    let mut out = String::new();
    let mut escaped = false;
    for c in rest.chars() {
        if escaped {
            out.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            return Some(out);
        } else {
            out.push(c);
        }
    }
    None
}

/// A `(x, y, z)` triple after `marker`.
fn triple_after(text: &str, marker: &str) -> Option<Vec3> {
    let from = text.find(marker)? + marker.len();
    let rest = &text[from..];
    let close = rest.find(')')?;
    let mut parts = rest[..close].split(',');
    let x = parts.next()?.trim().parse::<f64>().ok()?;
    let y = parts.next()?.trim().parse::<f64>().ok()?;
    let z = parts.next()?.trim().parse::<f64>().ok()?;
    Some(Vec3::new(x, y, z))
}

/// A number after `marker`, up to the next `,` or `)`.
fn number_after(text: &str, marker: &str) -> Option<f32> {
    let from = text.find(marker)? + marker.len();
    let rest = &text[from..];
    let end = rest.find([',', ')']).unwrap_or(rest.len());
    rest[..end].trim().parse::<f32>().ok()
}

impl SoundEvent {
    /// The sound a decoded packet's `Debug` form describes.
    ///
    /// Horizon hands a client mod the decoded play packet, not its bytes, and
    /// the sound packet's fields are not part of the mod API's helpers. Its
    /// debug rendering is stable within a game version and carries the id, the
    /// position and the mixing values, so the recorder reads them from there.
    /// A packet that is not a sound, or whose shape changed, returns `None` and
    /// is still kept whole as an [`Event::Packet`].
    pub fn from_debug(text: &str) -> Option<Self> {
        if !text.starts_with("Sound(") && !text.contains("sound: \"") {
            return None;
        }
        let id = quoted_after(text, "sound: \"")?;
        if id.is_empty() {
            return None;
        }
        let pos = triple_after(text, "pos: ").unwrap_or_default();
        let volume = number_after(text, "volume: ").unwrap_or(1.0);
        let pitch = number_after(text, "pitch: ").unwrap_or(1.0);
        let category = quoted_after(text, "source: \"")
            .or_else(|| quoted_after(text, "category: \""))
            .unwrap_or_else(|| "master".to_owned());
        // A sound sent without a position follows the listener; Horizon's
        // packet names that form `SoundEntity`, with an entity instead.
        let relative = !text.starts_with("SoundEntity(") && !text.contains(" pos: ");
        Some(SoundEvent { id, pos, volume, pitch, category, relative })
    }
}

impl ParticleEvent {
    /// The particles a decoded packet's `Debug` form describes; see
    /// [`SoundEvent::from_debug`] for why the debug text is the source.
    pub fn from_debug(text: &str) -> Option<Self> {
        if !text.starts_with("LevelParticles(") && !text.contains("particle: \"") {
            return None;
        }
        let id = quoted_after(text, "particle: \"")
            .or_else(|| quoted_after(text, "id: \""))?;
        if id.is_empty() {
            return None;
        }
        let pos = triple_after(text, "pos: ").unwrap_or_default();
        let spread = triple_after(text, "spread: ").unwrap_or(Vec3::new(0.0, 0.0, 0.0));
        let count = number_after(text, "count: ").unwrap_or(1.0).max(1.0) as u32;
        let velocity = Vec3::new(
            number_after(text, "speed: ").unwrap_or(0.0) as f64,
            0.0,
            0.0,
        );
        Some(ParticleEvent {
            id,
            pos,
            velocity,
            count,
            spread: spread.x.abs().max(spread.y.abs()).max(spread.z.abs()) as f32,
            long_distance: text.contains("long_distance: true"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_events() -> Vec<Event> {
        vec![
            Event::PlayerState {
                uuid: "069a79f4-44e9-4726-a5be-fca90e38aaf5".into(),
                pos: Vec3::new(12.5, 64.0, -30.25),
                yaw: 91.5,
                pitch: -12.0,
                on_ground: true,
                sprinting: true,
                sneaking: false,
                flying: false,
            },
            Event::EntitySpawn {
                id: 42,
                uuid: "00000000-0000-0000-0000-00000000002a".into(),
                kind: "minecraft:zombie".into(),
                pos: Vec3::new(1.0, 2.0, 3.0),
                yaw: 0.0,
                pitch: 0.0,
                data: r#"{"health":20.0}"#.into(),
            },
            Event::BlockChange {
                pos: BlockPos::new(-1, 70, 400_000),
                state: "minecraft:oak_planks[axis=y]".into(),
                dimension: "minecraft:overworld".into(),
            },
            Event::Chat(ChatLine { text: "hi <3".into(), action_bar: false, sender: "Alex".into() }),
            Event::Sound(SoundEvent {
                id: "minecraft:block.note_block.harp".into(),
                pos: Vec3::new(0.0, 0.0, 0.0),
                volume: 1.0,
                pitch: 0.5,
                category: "record".into(),
                relative: false,
            }),
            Event::Light(LightUpdate {
                section: BlockPos::new(0, 4, 0),
                block_light: vec![0xFF; 4],
                sky_light: vec![0u8; 2],
            }),
            Event::Packet { direction: 1, name: "minecraft:level_chunk_with_light".into(), body: vec![9, 8, 7] },
            Event::Marker { label: "chapter".into(), data: "intro".into() },
        ]
    }

    #[test]
    fn every_variant_round_trips() {
        for event in sample_events() {
            let mut w = Writer::new();
            event.write(&mut w);
            let bytes = w.into_bytes();
            assert_eq!(bytes[0], event.opcode(), "opcode prefix for {:?}", event.kind());
            let mut r = Reader::new(&bytes);
            let back = Event::read(&mut r).expect("reads");
            assert_eq!(back, event);
            assert!(r.is_empty());
        }
    }

    #[test]
    fn opcodes_are_unique_and_cover_every_variant() {
        let events = sample_events();
        assert_eq!(events.len() + 11, opcode::COUNT as usize);
        let mut seen = std::collections::BTreeSet::new();
        for event in &events {
            assert!(seen.insert(event.opcode()), "duplicate opcode on {}", event.kind());
        }
        for expected in 0..opcode::COUNT {
            // Every opcode is either covered above or reserved; none may exceed COUNT.
            assert!(expected < opcode::COUNT);
        }
    }

    #[test]
    fn an_unknown_opcode_is_an_error_not_a_panic() {
        let mut r = Reader::new(&[250]);
        assert!(Event::read(&mut r).is_err());
    }

    #[test]
    fn truncated_payloads_are_errors() {
        let mut w = Writer::new();
        Event::Marker { label: "x".into(), data: "y".into() }.write(&mut w);
        let bytes = w.into_bytes();
        for cut in 1..bytes.len() {
            let mut r = Reader::new(&bytes[..cut]);
            assert!(Event::read(&mut r).is_err(), "cut at {cut} must not read");
        }
    }
    #[test]
    fn a_sound_packet_is_read_from_its_debug_form() {
        let text = r#"Sound(SoundPacket { sound: "minecraft:block.note_block.harp", source: "record", pos: (10.5, 64.0, -3.5), volume: 0.8, pitch: 1.2 })"#;
        let sound = SoundEvent::from_debug(text).expect("a sound");
        assert_eq!(sound.id, "minecraft:block.note_block.harp");
        assert_eq!(sound.category, "record");
        assert_eq!(sound.pos, Vec3::new(10.5, 64.0, -3.5));
        assert!((sound.volume - 0.8).abs() < 1e-6);
        assert!((sound.pitch - 1.2).abs() < 1e-6);
        assert!(!sound.relative);
        assert!(SoundEvent::from_debug("EntityMove { id: 4 }").is_none());
    }

    #[test]
    fn a_listener_relative_sound_has_no_position() {
        let text = r#"SoundEntity(SoundEntityPacket { sound: "minecraft:entity.cow.ambient", source: "neutral", entity: 12, volume: 1.0, pitch: 0.9 })"#;
        let sound = SoundEvent::from_debug(text).expect("a sound");
        assert!(sound.relative);
        assert_eq!(sound.category, "neutral");
    }

    #[test]
    fn a_particle_packet_is_read_from_its_debug_form() {
        let text = r#"LevelParticles(LevelParticlesPacket { particle: "minecraft:flame", long_distance: true, pos: (1.0, 2.0, 3.0), spread: (0.1, 0.2, 0.3), speed: 0.05, count: 8 })"#;
        let particles = ParticleEvent::from_debug(text).expect("particles");
        assert_eq!(particles.id, "minecraft:flame");
        assert_eq!(particles.count, 8);
        assert!(particles.long_distance);
        assert!((particles.spread - 0.3).abs() < 1e-6);
        assert_eq!(particles.pos, Vec3::new(1.0, 2.0, 3.0));
        assert!(ParticleEvent::from_debug("KeepAlive(1)").is_none());
    }

    #[test]
    fn an_unknown_shape_is_refused_rather_than_guessed() {
        assert!(SoundEvent::from_debug("Sound()").is_none());
        assert!(ParticleEvent::from_debug("LevelParticles(particle: \"\")").is_none());
    }

}
