// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! Performance: budgets, caching, level of detail and the render thread pool.
//!
//! Recording must not cost ticks and rendering must not starve the game, so the
//! engine keeps its resource decisions in one place: how much memory a recording
//! may buffer, which events may be skipped when a phone is struggling, how many
//! threads a render uses, and how far away geometry is simplified. Every policy
//! here is a pure function of its inputs, so the HUD's "why is it dropping
//! events" question has an answer that can be tested.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::event::Event;
use crate::json::Json;

/// Performance settings, from the config's performance section.
#[derive(Clone, Debug, PartialEq)]
pub struct PerformanceSettings {
    /// Worker threads for compression and thumbnail decoding (`0` = auto).
    pub capture_threads: usize,
    /// Worker threads for a render (`0` = auto).
    pub render_threads: usize,
    /// Memory the recorder may buffer, in MiB.
    pub buffer_mb: usize,
    /// The Android low-memory policy: fewer captures, smaller blocks, no
    /// thumbnails while recording.
    pub low_memory: bool,
    /// Replay chunk cache, in MiB.
    pub chunk_cache_mb: usize,
    /// Simplify far geometry while playing in the world.
    pub lod: bool,
    /// Distance in chunks where simplification starts.
    pub lod_start_chunks: u8,
    /// Drop particle and sound events when a tick overruns its budget.
    pub drop_transients_under_pressure: bool,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            capture_threads: 0,
            render_threads: 0,
            buffer_mb: 32,
            low_memory: false,
            chunk_cache_mb: 128,
            lod: true,
            lod_start_chunks: 8,
            drop_transients_under_pressure: true,
        }
    }
}

impl PerformanceSettings {
    /// Clamp to something a machine can actually do.
    pub fn sanitized(mut self) -> Self {
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        self.capture_threads = self.capture_threads.clamp(0, cores * 2);
        self.render_threads = self.render_threads.clamp(0, cores * 2);
        self.buffer_mb = self.buffer_mb.clamp(4, 1024);
        self.chunk_cache_mb = self.chunk_cache_mb.clamp(16, 8192);
        self.lod_start_chunks = self.lod_start_chunks.clamp(2, 32);
        if self.low_memory {
            // A phone that chose low-memory mode gets the conservative policy
            // whatever the rest of the file says.
            self.buffer_mb = self.buffer_mb.min(16);
            self.chunk_cache_mb = self.chunk_cache_mb.min(64);
            self.render_threads = self.render_threads.min(2);
            self.capture_threads = self.capture_threads.min(2);
            self.lod = true;
        }
        self
    }

    pub fn to_json(&self) -> Json {
        Json::Object(vec![
            ("capture_threads".into(), Json::int(self.capture_threads as i64)),
            ("render_threads".into(), Json::int(self.render_threads as i64)),
            ("buffer_mb".into(), Json::int(self.buffer_mb as i64)),
            ("low_memory".into(), Json::Bool(self.low_memory)),
            ("chunk_cache_mb".into(), Json::int(self.chunk_cache_mb as i64)),
            ("lod".into(), Json::Bool(self.lod)),
            ("lod_start_chunks".into(), Json::int(self.lod_start_chunks as i64)),
            ("drop_transients_under_pressure".into(), Json::Bool(self.drop_transients_under_pressure)),
        ])
    }

    pub fn from_json(json: &Json) -> Self {
        let defaults = PerformanceSettings::default();
        PerformanceSettings {
            capture_threads: json.get("capture_threads").and_then(Json::as_u64).unwrap_or(defaults.capture_threads as u64) as usize,
            render_threads: json.get("render_threads").and_then(Json::as_u64).unwrap_or(defaults.render_threads as u64) as usize,
            buffer_mb: json.get("buffer_mb").and_then(Json::as_u64).unwrap_or(defaults.buffer_mb as u64) as usize,
            low_memory: json.get("low_memory").and_then(Json::as_bool).unwrap_or(defaults.low_memory),
            chunk_cache_mb: json.get("chunk_cache_mb").and_then(Json::as_u64).unwrap_or(defaults.chunk_cache_mb as u64) as usize,
            lod: json.get("lod").and_then(Json::as_bool).unwrap_or(defaults.lod),
            lod_start_chunks: json.get("lod_start_chunks").and_then(Json::as_u64).unwrap_or(defaults.lod_start_chunks as u64) as u8,
            drop_transients_under_pressure: json
                .get("drop_transients_under_pressure")
                .and_then(Json::as_bool)
                .unwrap_or(defaults.drop_transients_under_pressure),
        }
        .sanitized()
    }
}

/// The recorder's live resource policy.
#[derive(Clone, Debug)]
pub struct Budget {
    pub settings: PerformanceSettings,
    /// Nanoseconds a tick's capture may take before transients are skipped.
    pub tick_budget_nanos: u64,
    /// The slowest tick seen since the last reset.
    pub worst_tick_nanos: u64,
    /// Ticks that overran their budget.
    pub overruns: u64,
    /// Events dropped because of pressure.
    pub dropped: u64,
}

impl Budget {
    pub fn new(settings: PerformanceSettings) -> Self {
        let settings = settings.sanitized();
        // A tick is 50 ms; spending more than 2 ms capturing is already a lot.
        let tick_budget_nanos = if settings.low_memory { 1_000_000 } else { 2_000_000 };
        Self { settings, tick_budget_nanos, worst_tick_nanos: 0, overruns: 0, dropped: 0 }
    }

    /// Buffer bytes the recorder may use.
    pub fn buffer_bytes(&self) -> usize {
        self.settings.buffer_mb * 1024 * 1024
    }

    /// Ticks per block that suit this machine: smaller on a phone so a block
    /// flush fits the buffer.
    pub fn block_ticks(&self) -> u16 {
        if self.settings.low_memory {
            100
        } else {
            200
        }
    }

    /// Whether an event may be captured while the tick is already over budget.
    /// Player, entity and block events are always kept; particles, sounds and
    /// raw packets are the ones a struggling device can live without.
    pub fn may_capture(&self, event: &Event, under_pressure: bool) -> bool {
        if !under_pressure {
            return true;
        }
        if !self.settings.drop_transients_under_pressure {
            return true;
        }
        !matches!(
            event,
            Event::Particle(_) | Event::Sound(_) | Event::Packet { .. } | Event::Light(_)
        )
    }

    /// Feed a tick's measured capture time back into the policy.
    pub fn record_tick_time(&mut self, nanos: u64) {
        self.worst_tick_nanos = self.worst_tick_nanos.max(nanos);
        if nanos > self.tick_budget_nanos {
            self.overruns += 1;
        }
    }

    /// True while the last tick overran.
    pub fn under_pressure(&self) -> bool {
        self.worst_tick_nanos > self.tick_budget_nanos
    }

    /// Forget the worst-case reading (called after a quiet second).
    pub fn reset_pressure(&mut self) {
        self.worst_tick_nanos = 0;
    }

    /// Threads to use for a render: the configured count, or the machine's
    /// core count when it is left on auto.
    pub fn render_threads(&self) -> usize {
        if self.settings.render_threads == 0 {
            std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).max(1)
        } else {
            self.settings.render_threads
        }
    }
}

/// How much detail a chunk at a distance gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LodLevel {
    /// Everything: full meshes, entity models, block entities.
    Full,
    /// Simplified meshes, no small entities.
    Reduced,
    /// Heightmap colours only.
    Far,
}

impl LodLevel {
    pub fn id(self) -> u8 {
        match self {
            LodLevel::Full => 0,
            LodLevel::Reduced => 1,
            LodLevel::Far => 2,
        }
    }

    /// The level for a distance in chunks.
    pub fn for_distance(distance_chunks: f32, start: u8, enabled: bool) -> LodLevel {
        if !enabled {
            return LodLevel::Full;
        }
        let start = start.max(2) as f32;
        if distance_chunks < start {
            LodLevel::Full
        } else if distance_chunks < start * 2.0 {
            LodLevel::Reduced
        } else {
            LodLevel::Far
        }
    }
}

/// A small LRU cache for decoded replay chunks.
#[derive(Debug)]
pub struct ChunkCache {
    capacity: usize,
    bytes: usize,
    /// Insertion-ordered: the front is the least recently used.
    order: Vec<u64>,
    entries: HashMap<u64, (usize, Vec<u8>)>,
    hits: u64,
    misses: u64,
}

impl ChunkCache {
    pub fn new(capacity_bytes: usize) -> Self {
        Self { capacity: capacity_bytes.max(1), bytes: 0, order: Vec::new(), entries: HashMap::new(), hits: 0, misses: 0 }
    }

    pub fn get(&mut self, key: u64) -> Option<&[u8]> {
        if self.entries.contains_key(&key) {
            self.hits += 1;
            self.order.retain(|k| *k != key);
            self.order.push(key);
            self.entries.get(&key).map(|(_, data)| data.as_slice())
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn insert(&mut self, key: u64, data: Vec<u8>) {
        let size = data.len();
        if size > self.capacity {
            return;
        }
        if let Some((old, _)) = self.entries.remove(&key) {
            self.bytes -= old;
            self.order.retain(|k| *k != key);
        }
        while self.bytes + size > self.capacity {
            let Some(victim) = self.order.first().copied() else { break };
            self.order.remove(0);
            if let Some((old, _)) = self.entries.remove(&victim) {
                self.bytes -= old;
            }
        }
        self.bytes += size;
        self.order.push(key);
        self.entries.insert(key, (size, data));
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Fraction of lookups served from memory.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.bytes = 0;
    }
}

/// Map over items on several threads, keeping the input order.
///
/// Used for frame post-processing and thumbnail decoding, where each item is
/// independent but the output order matters. Results are collected back into
/// their slots, so the order and the result are deterministic.
pub fn parallel_map<T, U, F>(items: Vec<T>, threads: usize, work: F) -> Vec<U>
where
    T: Send,
    U: Send,
    F: Fn(T) -> U + Sync,
{
    let count = items.len();
    if count == 0 {
        return Vec::new();
    }
    let threads = threads.clamp(1, count);
    if threads == 1 {
        return items.into_iter().map(work).collect();
    }
    let slots: Vec<Mutex<Option<U>>> = (0..count).map(|_| Mutex::new(None)).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let queue: Mutex<Vec<Option<T>>> = Mutex::new(items.into_iter().map(Some).collect());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            let _worker = scope.spawn(|| loop {
                let index = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if index >= count {
                    break;
                }
                let item = {
                    let mut queue = queue.lock().expect("queue lock");
                    queue.get_mut(index).and_then(Option::take)
                };
                if let Some(item) = item {
                    let result = work(item);
                    *slots[index].lock().expect("slot lock") = Some(result);
                }
            });
        }
    });
    slots
        .into_iter()
        .map(|slot| slot.into_inner().expect("slot lock").expect("every item mapped"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{BlockPos, ChatLine};
    use crate::wire::Vec3;

    #[test]
    fn settings_clamp_and_low_memory_shrinks_everything() {
        let settings = PerformanceSettings { render_threads: 999, buffer_mb: 10_000, chunk_cache_mb: 0, lod_start_chunks: 200, ..PerformanceSettings::default() }.sanitized();
        assert!(settings.render_threads <= 64);
        assert_eq!(settings.buffer_mb, 1024);
        assert_eq!(settings.chunk_cache_mb, 16);
        assert_eq!(settings.lod_start_chunks, 32);

        let low = PerformanceSettings { low_memory: true, buffer_mb: 512, chunk_cache_mb: 4096, ..PerformanceSettings::default() }.sanitized();
        assert_eq!(low.buffer_mb, 16);
        assert_eq!(low.chunk_cache_mb, 64);
        assert!(low.lod);
    }

    #[test]
    fn performance_settings_round_trip() {
        let settings = PerformanceSettings { buffer_mb: 64, low_memory: true, ..PerformanceSettings::default() }.sanitized();
        assert_eq!(PerformanceSettings::from_json(&settings.to_json()), settings);
    }

    #[test]
    fn a_budget_reports_pressure_and_drops_only_transients() {
        let mut budget = Budget::new(PerformanceSettings::default());
        assert!(!budget.under_pressure());
        assert_eq!(budget.block_ticks(), 200);
        budget.record_tick_time(budget.tick_budget_nanos + 1);
        assert!(budget.under_pressure());
        assert_eq!(budget.overruns, 1);

        let particle = Event::Particle(crate::event::ParticleEvent {
            id: "minecraft:flame".into(),
            pos: Vec3::ZERO,
            velocity: Vec3::ZERO,
            count: 1,
            spread: 0.0,
            long_distance: false,
        });
        let chat = Event::Chat(ChatLine { text: "hi".into(), action_bar: false, sender: String::new() });
        let block = Event::BlockChange { pos: BlockPos::new(0, 0, 0), state: "minecraft:stone".into(), dimension: "overworld".into() };
        assert!(!budget.may_capture(&particle, true));
        assert!(budget.may_capture(&chat, true));
        assert!(budget.may_capture(&block, true));
        assert!(budget.may_capture(&particle, false));
        budget.reset_pressure();
        assert!(!budget.under_pressure());
    }

    #[test]
    fn low_memory_uses_smaller_blocks() {
        let budget = Budget::new(PerformanceSettings { low_memory: true, ..PerformanceSettings::default() });
        assert_eq!(budget.block_ticks(), 100);
        assert_eq!(budget.buffer_bytes(), 16 * 1024 * 1024);
    }

    #[test]
    fn lod_levels_step_with_distance() {
        assert_eq!(LodLevel::for_distance(4.0, 8, true), LodLevel::Full);
        assert_eq!(LodLevel::for_distance(10.0, 8, true), LodLevel::Reduced);
        assert_eq!(LodLevel::for_distance(100.0, 8, true), LodLevel::Far);
        assert_eq!(LodLevel::for_distance(100.0, 8, false), LodLevel::Full);
        assert_eq!(LodLevel::Far.id(), 2);
    }

    #[test]
    fn the_cache_evicts_the_least_recently_used() {
        let mut cache = ChunkCache::new(10);
        cache.insert(1, vec![0; 4]);
        cache.insert(2, vec![0; 4]);
        assert!(cache.get(1).is_some(), "touch 1, so 2 is now the oldest");
        cache.insert(3, vec![0; 4]); // evicts 2
        assert!(cache.get(2).is_none());
        assert!(cache.get(1).is_some());
        assert_eq!(cache.len(), 2);
        assert!(cache.bytes() <= 10);
        assert!(cache.hit_rate() > 0.5);
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn the_cache_refuses_an_oversized_entry() {
        let mut cache = ChunkCache::new(4);
        cache.insert(1, vec![0; 8]);
        assert!(cache.is_empty());
    }

    #[test]
    fn parallel_map_keeps_order_and_handles_degenerate_inputs() {
        assert!(parallel_map(Vec::<u8>::new(), 4, |x| x).is_empty());
        let single = parallel_map(vec![3, 1, 2], 1, |x| x * 2);
        assert_eq!(single, vec![6, 2, 4]);
        let many = parallel_map((0..1_000).collect::<Vec<i32>>(), 8, |x| x * x);
        assert_eq!(many.len(), 1_000);
        assert_eq!(many[999], 999 * 999);
        assert_eq!(many, (0..1_000).map(|x| x * x).collect::<Vec<_>>());
    }

    #[test]
    fn parallel_map_can_use_more_threads_than_items() {
        let result = parallel_map(vec![1, 2], 32, |x| x + 1);
        assert_eq!(result, vec![2, 3]);
    }
}
