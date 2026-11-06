// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! The tick stream: `.tmc` "blocks" and the writer/reader pair around them.
//!
//! # Layout of `recording.tmc`
//!
//! ```text
//! header      "VTLR" format_version:u16 opcode_version:u8 tick_rate:u16
//!             blocks_per_index:u16 mc_version:str recorder:str
//! block*      "BLK1" first_tick:varint tick_count:varint
//!             raw_len:varint packed_len:varint packed_bytes
//! ```
//!
//! Every block holds a whole number of ticks, and each tick is
//! `event_count:varint` followed by that many events. A block's payload is
//! compressed with `lz::pack`, which stores itself if compression would grow it.
//!
//! Blocks are the seek unit: [`Recording::index`] records the byte offset,
//! first tick and tick count of each, so jumping to tick 12 000 reads one block
//! and decodes ~10 s of events, not the whole file. That is what makes the
//! timeline scrubber feel instant on a two-hour recording.

use crate::event::Event;
use crate::lz;
use crate::wire::{Reader, WireError, Writer};

/// File magic at the start of `recording.tmc`.
pub const MAGIC: [u8; 4] = *b"VTLR";
/// Block magic.
pub const BLOCK_MAGIC: [u8; 4] = *b"BLK1";
/// Container format version, bumped only for an incompatible layout change.
pub const FORMAT_VERSION: u16 = 1;
/// Event opcode table version (`event.rs`); a reader refuses a newer one.
pub const OPCODE_VERSION: u8 = 1;

/// How a recording is captured.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordingOptions {
    /// Capture ticks per second. 20 matches the game; lower values drop ticks
    /// on purpose (a "keyframe only" recording for a long build).
    pub tick_rate: u16,
    /// Ticks per block. Smaller blocks seek faster but compress worse.
    pub block_ticks: u16,
    /// Flush a block early when its payload grows past this.
    pub max_block_bytes: usize,
    /// Compress block payloads. Off makes a file inspectable with `xxd`, which
    /// is occasionally what a bug report needs.
    pub compress: bool,
    /// Write a recovery journal every N ticks (0 disables it).
    pub auto_save_ticks: u64,
    /// Keep at most this many uncompressed bytes buffered (Android low-memory
    /// mode lowers it; see `perf::Budget`).
    pub memory_budget_bytes: usize,
    /// Record raw packet bodies for packets the engine does not model.
    pub capture_raw_packets: bool,
}

impl Default for RecordingOptions {
    fn default() -> Self {
        Self {
            tick_rate: 20,
            block_ticks: 200,
            max_block_bytes: 256 * 1024,
            compress: true,
            auto_save_ticks: 1_200,
            memory_budget_bytes: 8 * 1024 * 1024,
            capture_raw_packets: true,
        }
    }
}

impl RecordingOptions {
    /// Clamp to what the format and the game can honour.
    pub fn sanitized(mut self) -> Self {
        self.tick_rate = self.tick_rate.clamp(1, 120);
        self.block_ticks = self.block_ticks.clamp(20, 2_400);
        self.max_block_bytes = self.max_block_bytes.clamp(4 * 1024, 8 * 1024 * 1024);
        self.memory_budget_bytes = self.memory_budget_bytes.clamp(256 * 1024, 256 * 1024 * 1024);
        self
    }
}

/// Why a recording could not be read.
#[derive(Clone, Debug, PartialEq)]
pub enum RecordingError {
    /// The file does not start with `VTLR`.
    NotARecording,
    /// Written by a newer engine with an incompatible layout.
    NewerFormat(u16),
    /// The opcode table is newer than this build knows.
    NewerOpcodes(u8),
    /// A block header or payload is broken.
    Corrupt(String),
    /// The stream ended in the middle of a block.
    Truncated,
}

impl std::fmt::Display for RecordingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordingError::NotARecording => f.write_str("not a Volt Lapse recording"),
            RecordingError::NewerFormat(v) => write!(f, "recording format {v} is newer than this build"),
            RecordingError::NewerOpcodes(v) => write!(f, "recording opcodes {v} are newer than this build"),
            RecordingError::Corrupt(why) => write!(f, "corrupt recording: {why}"),
            RecordingError::Truncated => f.write_str("recording ends mid-block"),
        }
    }
}

impl std::error::Error for RecordingError {}

impl From<WireError> for RecordingError {
    fn from(error: WireError) -> Self {
        match error {
            WireError::Truncated => RecordingError::Truncated,
            other => RecordingError::Corrupt(other.to_string()),
        }
    }
}

fn write_header(out: &mut Writer, options: &RecordingOptions, mc_version: &str, recorder: &str) {
    out.bytes(&MAGIC);
    out.u8((FORMAT_VERSION >> 8) as u8);
    out.u8(FORMAT_VERSION as u8);
    out.u8(OPCODE_VERSION);
    out.u8((options.tick_rate >> 8) as u8);
    out.u8(options.tick_rate as u8);
    out.u8((options.block_ticks >> 8) as u8);
    out.u8(options.block_ticks as u8);
    out.str(mc_version);
    out.str(recorder);
}

/// A finished block in memory.
#[derive(Clone, Debug)]
struct Block {
    first_tick: u64,
    ticks: u64,
    raw_len: usize,
    packed: Vec<u8>,
}

/// Running counters, shown by the recording HUD and `/voltlapse record stats`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub ticks: u64,
    pub events: u64,
    pub bytes_uncompressed: u64,
    pub bytes_written: u64,
    pub blocks: u64,
}

impl Stats {
    /// Bytes per tick, the number the HUD shows as "size rate".
    pub fn bytes_per_tick(&self) -> f64 {
        if self.ticks == 0 {
            0.0
        } else {
            self.bytes_written as f64 / self.ticks as f64
        }
    }

    /// Written / uncompressed, as a percentage (100 % means no gain).
    pub fn compression_percent(&self) -> f64 {
        if self.bytes_uncompressed == 0 {
            100.0
        } else {
            self.bytes_written as f64 / self.bytes_uncompressed as f64 * 100.0
        }
    }
}

/// Records events tick by tick into blocks, asynchronously from the game.
///
/// The recorder never touches the file system: it produces bytes and the
/// storage layer (`storage.rs`) writes them, which keeps a slow disk off the
/// client's tick thread and lets Android write on its own thread.
#[derive(Debug)]
pub struct Recorder {
    options: RecordingOptions,
    mc_version: String,
    recorder_name: String,
    blocks: Vec<Block>,
    payload: Writer,
    block_first_tick: u64,
    block_ticks: u64,
    tick_events: Vec<Event>,
    tick: u64,
    paused: bool,
    stats: Stats,
    last_auto_save: u64,
}

impl Recorder {
    pub fn new(options: RecordingOptions, mc_version: impl Into<String>, recorder_name: impl Into<String>) -> Self {
        let options = options.sanitized();
        Self {
            options,
            mc_version: mc_version.into(),
            recorder_name: recorder_name.into(),
            blocks: Vec::new(),
            payload: Writer::new(),
            block_first_tick: 0,
            block_ticks: 0,
            tick_events: Vec::new(),
            tick: 0,
            paused: false,
            stats: Stats::default(),
            last_auto_save: 0,
        }
    }

    pub fn options(&self) -> &RecordingOptions {
        &self.options
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Pause: events are dropped from here on, and the clock still advances so
    /// playback stays in real time (a pause is a hole, not a jump).
    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// Queue an event for the tick being recorded. Ignored while paused.
    pub fn record(&mut self, event: Event) {
        if self.paused {
            return;
        }
        self.tick_events.push(event);
    }

    /// Close the current tick, flushing a block when it is full.
    ///
    /// Returns `true` when a block was completed, which is the moment the
    /// caller may hand bytes to the storage thread.
    pub fn end_tick(&mut self) -> bool {
        if !self.paused && !self.tick_events.is_empty() {
            let mut events = std::mem::take(&mut self.tick_events);
            self.payload.varint(events.len() as u64);
            for event in &events {
                event.write(&mut self.payload);
                self.stats.events += 1;
            }
            events.clear();
        } else if !self.paused {
            // An empty tick is still a tick, so the timeline keeps its length.
            self.payload.varint(0);
        }
        self.tick_events.clear();
        if self.paused {
            // Nothing is written while paused; the tick counter still moves.
            self.tick += 1;
            self.block_first_tick = self.tick;
            return false;
        }
        self.tick += 1;
        self.block_ticks += 1;
        self.stats.ticks += 1;

        if self.block_ticks >= self.options.block_ticks as u64
            || self.payload.len() >= self.options.max_block_bytes
        {
            return self.flush_block();
        }
        false
    }

    /// Whether the auto-save interval has elapsed (and resets it).
    pub fn auto_save_due(&mut self) -> bool {
        if self.options.auto_save_ticks == 0 {
            return false;
        }
        if self.tick.saturating_sub(self.last_auto_save) >= self.options.auto_save_ticks {
            self.last_auto_save = self.tick;
            return true;
        }
        false
    }

    /// Close the current block.
    pub fn flush_block(&mut self) -> bool {
        if self.block_ticks == 0 {
            return false;
        }
        let payload = std::mem::take(&mut self.payload).into_bytes();
        let packed = if self.options.compress { lz::pack(&payload) } else { payload.clone() };
        self.stats.bytes_uncompressed += payload.len() as u64;
        self.stats.blocks += 1;
        self.blocks.push(Block {
            first_tick: self.block_first_tick,
            ticks: self.block_ticks,
            raw_len: payload.len(),
            packed,
        });
        self.block_first_tick = self.tick;
        self.block_ticks = 0;
        true
    }

    fn header_bytes(&self) -> Vec<u8> {
        let mut out = Writer::new();
        write_header(&mut out, &self.options, &self.mc_version, &self.recorder_name);
        out.into_bytes()
    }

    /// Serialise everything recorded so far. Used for crash recovery: the
    /// result is a valid recording with a shorter duration, never a broken one.
    pub fn snapshot(&mut self) -> Vec<u8> {
        self.flush_block();
        let mut out = self.header_bytes();
        for block in &self.blocks {
            write_block(&mut out, block);
        }
        out
    }

    /// Finish the recording and return the bytes of `recording.tmc`.
    pub fn finish(mut self) -> (Vec<u8>, Stats) {
        self.flush_block();
        let mut out = self.header_bytes();
        for block in &self.blocks {
            write_block(&mut out, block);
        }
        let mut stats = self.stats;
        stats.bytes_written = out.len() as u64;
        (out, stats)
    }
}

fn write_block(out: &mut Vec<u8>, block: &Block) {
    let mut header = Writer::new();
    header.bytes(&BLOCK_MAGIC);
    header.varint(block.first_tick);
    header.varint(block.ticks);
    header.varint(block.raw_len as u64);
    header.varint(block.packed.len() as u64);
    out.extend_from_slice(header.as_slice());
    out.extend_from_slice(&block.packed);
}

/// The header a recording carries.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordingHeader {
    pub format_version: u16,
    pub opcode_version: u8,
    pub tick_rate: u16,
    pub block_ticks: u16,
    pub mc_version: String,
    pub recorder: String,
}

impl RecordingHeader {
    /// Seconds of world time one tick covers.
    pub fn seconds_per_tick(&self) -> f64 {
        1.0 / self.tick_rate.max(1) as f64
    }
}

/// Where one block lives, for seeking.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockIndex {
    pub first_tick: u64,
    pub ticks: u64,
    pub offset: usize,
    pub length: usize,
}

/// A parsed `recording.tmc`, kept in memory and read lazily block by block.
#[derive(Clone, Debug)]
pub struct Recording {
    bytes: Vec<u8>,
    header: RecordingHeader,
    index: Vec<BlockIndex>,
}

impl Recording {
    /// Parse and index a recording. The bytes are kept; blocks are decoded on
    /// demand.
    pub fn parse(bytes: Vec<u8>) -> Result<Self, RecordingError> {
        let mut input = Reader::new(&bytes);
        let magic = input.take(4).map_err(|_| RecordingError::NotARecording)?;
        if magic != MAGIC {
            return Err(RecordingError::NotARecording);
        }
        let format_version = u16::from_be_bytes([input.u8()?, input.u8()?]);
        if format_version > FORMAT_VERSION {
            return Err(RecordingError::NewerFormat(format_version));
        }
        let opcode_version = input.u8()?;
        if opcode_version > OPCODE_VERSION {
            return Err(RecordingError::NewerOpcodes(opcode_version));
        }
        let tick_rate = u16::from_be_bytes([input.u8()?, input.u8()?]);
        let block_ticks = u16::from_be_bytes([input.u8()?, input.u8()?]);
        let mc_version = input.string()?;
        let recorder = input.string()?;
        let header = RecordingHeader {
            format_version,
            opcode_version,
            tick_rate,
            block_ticks,
            mc_version,
            recorder,
        };

        let mut index = Vec::new();
        let mut position = input.position();
        while position < bytes.len() {
            let mut cursor = Reader::new(&bytes[position..]);
            if cursor.take(4).map_err(|_| RecordingError::Truncated)? != BLOCK_MAGIC {
                return Err(RecordingError::Corrupt(format!("bad block magic at byte {position}")));
            }
            let first_tick = cursor.varint()?;
            let ticks = cursor.varint()?;
            let raw_len = cursor.varint()?;
            let packed_len = cursor.varint()? as usize;
            let header_len = cursor.position();
            let start = position + header_len;
            if start + packed_len > bytes.len() {
                return Err(RecordingError::Truncated);
            }
            index.push(BlockIndex {
                first_tick,
                ticks,
                offset: start,
                length: packed_len,
            });
            let _ = raw_len;
            position = start + packed_len;
        }
        Ok(Self { bytes, header, index })
    }

    pub fn header(&self) -> &RecordingHeader {
        &self.header
    }

    pub fn index(&self) -> &[BlockIndex] {
        &self.index
    }

    /// Total ticks recorded (including empty ones, so the duration is right).
    pub fn tick_count(&self) -> u64 {
        self.index.last().map(|b| b.first_tick + b.ticks).unwrap_or(0)
    }

    /// Duration in seconds.
    pub fn duration_seconds(&self) -> f64 {
        self.tick_count() as f64 / self.header.tick_rate.max(1) as f64
    }

    /// The tick a timestamp falls on.
    pub fn tick_at_seconds(&self, seconds: f64) -> u64 {
        let tick = (seconds * self.header.tick_rate as f64).round();
        tick.clamp(0.0, self.tick_count() as f64) as u64
    }

    fn block_at(&self, tick: u64) -> Option<usize> {
        // Blocks are ordered and contiguous, so a binary search finds the one
        // holding `tick` in log n.
        match self.index.binary_search_by(|block| {
            if tick < block.first_tick {
                std::cmp::Ordering::Greater
            } else if tick >= block.first_tick + block.ticks {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        }) {
            Ok(at) => Some(at),
            Err(_) => None,
        }
    }

    fn decode_block(&self, at: usize) -> Result<Vec<Vec<Event>>, RecordingError> {
        let block = self.index[at];
        let packed = &self.bytes[block.offset..block.offset + block.length];
        let unpacked = lz::unpack(packed).map_err(|why| RecordingError::Corrupt(why.to_owned()))?;
        let mut input = Reader::new(&unpacked);
        let mut ticks = Vec::with_capacity(block.ticks as usize);
        for _ in 0..block.ticks {
            let count = input.varint()? as usize;
            let mut events = Vec::with_capacity(count.min(4096));
            for _ in 0..count {
                events.push(Event::read(&mut input)?);
            }
            ticks.push(events);
        }
        Ok(ticks)
    }

    /// Every event recorded on one tick.
    pub fn events_in_tick(&self, tick: u64) -> Result<Vec<Event>, RecordingError> {
        let Some(at) = self.block_at(tick) else {
            return Ok(Vec::new());
        };
        let block = self.index[at];
        let ticks = self.decode_block(at)?;
        Ok(ticks
            .get((tick - block.first_tick) as usize)
            .cloned()
            .unwrap_or_default())
    }

    /// Walk every event between two ticks in order. The callback gets the tick
    /// each event belongs to, which is what a playback's world state needs.
    pub fn for_each_event(
        &self,
        from_tick: u64,
        to_tick: u64,
        mut visit: impl FnMut(u64, &Event),
    ) -> Result<(), RecordingError> {
        let from = from_tick.min(self.tick_count());
        let to = to_tick.min(self.tick_count());
        for at in 0..self.index.len() {
            let block = self.index[at];
            if block.first_tick + block.ticks <= from || block.first_tick >= to {
                continue;
            }
            for (offset, events) in self.decode_block(at)?.iter().enumerate() {
                let tick = block.first_tick + offset as u64;
                if tick < from || tick >= to {
                    continue;
                }
                for event in events {
                    visit(tick, event);
                }
            }
        }
        Ok(())
    }

    /// Count events of each kind, for the browser's file details.
    pub fn event_summary(&self) -> Result<Vec<(&'static str, u64)>, RecordingError> {
        let mut counts: Vec<(&'static str, u64)> = Vec::new();
        self.for_each_event(0, self.tick_count(), |_, event| {
            let kind = event.kind();
            match counts.iter_mut().find(|(k, _)| *k == kind) {
                Some(entry) => entry.1 += 1,
                None => counts.push((kind, 1)),
            }
        })?;
        counts.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        Ok(counts)
    }

    /// Raw bytes, for the storage layer to write.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::Vec3;

    fn options() -> RecordingOptions {
        RecordingOptions { block_ticks: 4, auto_save_ticks: 8, ..RecordingOptions::default() }
    }

    fn sample(tick: u64) -> Event {
        Event::EntityMove {
            id: tick as i32,
            pos: Vec3::new(tick as f64, 64.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
        }
    }

    fn record(ticks: u64) -> Recorder {
        let mut recorder = Recorder::new(options(), "26.3", "Volt Lapse test");
        for tick in 0..ticks {
            recorder.record(sample(tick));
            if tick % 3 == 0 {
                recorder.record(Event::Marker { label: "beat".into(), data: tick.to_string() });
            }
            recorder.end_tick();
        }
        recorder
    }

    #[test]
    fn writes_a_header_and_blocks_and_reads_them_back() {
        let (bytes, stats) = record(10).finish();
        assert!(bytes.starts_with(&MAGIC));
        assert_eq!(stats.ticks, 10);
        assert_eq!(stats.blocks, 3); // 4 + 4 + 2 ticks
        let recording = Recording::parse(bytes).expect("parses");
        assert_eq!(recording.header().tick_rate, 20);
        assert_eq!(recording.header().mc_version, "26.3");
        assert_eq!(recording.tick_count(), 10);
        assert!((recording.duration_seconds() - 0.5).abs() < 1e-9);
        assert_eq!(recording.events_in_tick(0).unwrap().len(), 2);
        assert_eq!(recording.events_in_tick(1).unwrap().len(), 1);
        assert_eq!(recording.events_in_tick(99).unwrap().len(), 0);
        let mut moves = 0;
        let mut markers = 0;
        recording
            .for_each_event(0, 10, |tick, event| match event.kind() {
                "entity-move" => {
                    moves += 1;
                    assert!(matches!(event, Event::EntityMove { id, .. } if *id == tick as i32));
                }
                "marker" => {
                    markers += 1;
                    assert_eq!(tick % 3, 0);
                }
                other => panic!("unexpected {other}"),
            })
            .unwrap();
        assert_eq!((moves, markers), (10, 4));
    }

    #[test]
    fn seeking_reads_only_the_matching_block() {
        let (bytes, _) = record(100).finish();
        let recording = Recording::parse(bytes).unwrap();
        let events = recording.events_in_tick(97).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(recording.tick_at_seconds(1.0), 20);
        assert_eq!(recording.tick_at_seconds(-5.0), 0);
        assert_eq!(recording.tick_at_seconds(9_999.0), 100);
    }

    #[test]
    fn pausing_leaves_no_events_but_keeps_the_clock() {
        let mut recorder = Recorder::new(options(), "26.3", "test");
        recorder.record(sample(0));
        recorder.end_tick();
        recorder.pause();
        recorder.record(sample(1));
        recorder.end_tick();
        recorder.resume();
        recorder.record(sample(2));
        recorder.end_tick();
        let recording = Recording::parse(recorder.finish().0).unwrap();
        assert_eq!(recording.tick_count(), 3);
        assert_eq!(recording.events_in_tick(1).unwrap().len(), 0);
        assert_eq!(recording.events_in_tick(2).unwrap().len(), 1);
    }

    #[test]
    fn a_snapshot_is_a_valid_shorter_recording() {
        let mut recorder = record(10);
        let snapshot = recorder.snapshot();
        let recording = Recording::parse(snapshot).expect("snapshot parses");
        assert_eq!(recording.tick_count(), 10);
        // Recording continues after a snapshot without duplicating blocks.
        recorder.record(sample(10));
        recorder.end_tick();
        let (bytes, _) = recorder.finish();
        let full = Recording::parse(bytes).unwrap();
        assert_eq!(full.tick_count(), 11);
    }

    #[test]
    fn auto_save_fires_on_its_interval_only() {
        let mut recorder = Recorder::new(options(), "26.3", "test");
        assert!(!recorder.auto_save_due());
        for _ in 0..8 {
            recorder.end_tick();
        }
        assert!(recorder.auto_save_due());
        assert!(!recorder.auto_save_due());
    }

    #[test]
    fn bad_input_is_reported_precisely() {
        assert_eq!(Recording::parse(b"nope".to_vec()), Err(RecordingError::NotARecording));
        let mut bytes = record(4).finish().0;
        bytes[4] = 0x00;
        bytes[5] = 0x09; // format 9
        assert_eq!(Recording::parse(bytes), Err(RecordingError::NewerFormat(9)));

        let mut bytes = record(4).finish().0;
        bytes.truncate(bytes.len() - 3);
        assert_eq!(Recording::parse(bytes), Err(RecordingError::Truncated));

        let mut bytes = record(4).finish().0;
        let at = bytes.windows(4).position(|w| w == BLOCK_MAGIC).unwrap();
        bytes[at] = b'X';
        assert!(matches!(Recording::parse(bytes), Err(RecordingError::Corrupt(_))));
    }

    #[test]
    fn compression_keeps_repetitive_recordings_small() {
        let mut recorder = Recorder::new(
            RecordingOptions { block_ticks: 400, ..RecordingOptions::default() },
            "26.3",
            "test",
        );
        for tick in 0..400 {
            recorder.record(Event::PlayerState {
                uuid: "player".into(),
                pos: Vec3::new(0.5, 64.0, 0.5),
                yaw: 45.0,
                pitch: 0.0,
                on_ground: true,
                sprinting: false,
                sneaking: false,
                flying: false,
            });
            recorder.record(sample(tick));
            recorder.end_tick();
        }
        let (bytes, stats) = recorder.finish();
        assert!(stats.compression_percent() < 60.0, "ratio {}", stats.compression_percent());
        assert!(bytes.len() < 8_000, "{} bytes", bytes.len());
        let recording = Recording::parse(bytes).unwrap();
        assert_eq!(recording.tick_count(), 400);
        assert_eq!(recording.events_in_tick(399).unwrap().len(), 2);
    }

    #[test]
    fn options_are_clamped_to_sane_ranges() {
        let options = RecordingOptions { tick_rate: 0, block_ticks: 1, max_block_bytes: 1, memory_budget_bytes: 1, ..RecordingOptions::default() }
            .sanitized();
        assert_eq!(options.tick_rate, 1);
        assert_eq!(options.block_ticks, 20);
        assert_eq!(options.max_block_bytes, 4096);
        assert_eq!(options.memory_budget_bytes, 256 * 1024);
    }

    #[test]
    fn event_summary_counts_kinds() {
        let recording = Recording::parse(record(9).finish().0).unwrap();
        let summary = recording.event_summary().unwrap();
        assert_eq!(summary[0], ("entity-move", 9));
        assert_eq!(summary[1], ("marker", 3));
    }
}
