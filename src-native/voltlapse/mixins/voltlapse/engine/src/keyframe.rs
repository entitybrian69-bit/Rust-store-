// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! Camera paths: keyframes, their editing operations, and time remapping.
//!
//! A keyframe is a full camera state at a tick: position, yaw, pitch, roll, FOV
//! and a shake amount. Between keyframes the camera follows the outgoing
//! keyframe's [`Interpolation`]. The path also owns **time remapping**: speed
//! segments that make a stretch of the recording play back slower or faster for
//! the camera, which is how a speed ramp is built without moving a keyframe.

use crate::interp::{angle, catmull_rom, clamp_pitch, hermite, hermite_tangents, scalar, wrap_degrees, Interpolation};
use crate::json::Json;
use crate::wire::Vec3;

/// A camera state at one tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe {
    pub tick: u64,
    pub pos: Vec3,
    /// Degrees, -180..=180.
    pub yaw: f32,
    /// Degrees, -90..=90.
    pub pitch: f32,
    /// Degrees of camera roll (barrel rolls, Dutch angle).
    pub roll: f32,
    /// Vertical field of view in degrees; 70 is vanilla's default.
    pub fov: f32,
    pub interp: Interpolation,
    /// Hermite tangent scale; 1 is Catmull-Rom, 0 eases at every keyframe.
    pub tension: f32,
    /// Shake intensity 0..=1 for the segment leaving this keyframe.
    pub shake: f32,
    /// Shown in the timeline and the browser.
    pub label: String,
}

impl Default for Keyframe {
    fn default() -> Self {
        Self {
            tick: 0,
            pos: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            fov: 70.0,
            interp: Interpolation::Linear,
            tension: 1.0,
            shake: 0.0,
            label: String::new(),
        }
    }
}

impl Keyframe {
    pub fn new(tick: u64, pos: Vec3, yaw: f32, pitch: f32) -> Self {
        Self { tick, pos, yaw, pitch, ..Keyframe::default() }
    }

    /// JSON form, for camera project files.
    pub fn to_json(&self) -> Json {
        Json::Object(vec![
            ("tick".into(), Json::int(self.tick as i64)),
            ("x".into(), Json::Number(self.pos.x)),
            ("y".into(), Json::Number(self.pos.y)),
            ("z".into(), Json::Number(self.pos.z)),
            ("yaw".into(), Json::Number(self.yaw as f64)),
            ("pitch".into(), Json::Number(self.pitch as f64)),
            ("roll".into(), Json::Number(self.roll as f64)),
            ("fov".into(), Json::Number(self.fov as f64)),
            ("interpolation".into(), Json::String(self.interp.id().into())),
            ("tension".into(), Json::Number(self.tension as f64)),
            ("shake".into(), Json::Number(self.shake as f64)),
            ("label".into(), Json::String(self.label.clone())),
        ])
    }

    /// Read a keyframe, repairing anything out of range rather than refusing the
    /// file: a hand-edited project should open.
    pub fn from_json(json: &Json) -> Option<Self> {
        let mut keyframe = Keyframe::default();
        keyframe.tick = json.get("tick").and_then(Json::as_u64).unwrap_or(0);
        keyframe.pos = Vec3::new(
            json.get("x").and_then(Json::as_f64).unwrap_or(0.0),
            json.get("y").and_then(Json::as_f64).unwrap_or(0.0),
            json.get("z").and_then(Json::as_f64).unwrap_or(0.0),
        );
        if !keyframe.pos.is_finite() {
            return None;
        }
        keyframe.yaw = wrap_degrees(json.get("yaw").and_then(Json::as_f64).unwrap_or(0.0)) as f32;
        keyframe.pitch = clamp_pitch(json.get("pitch").and_then(Json::as_f64).unwrap_or(0.0)) as f32;
        keyframe.roll = wrap_degrees(json.get("roll").and_then(Json::as_f64).unwrap_or(0.0)) as f32;
        keyframe.fov = json
            .get("fov")
            .and_then(Json::as_f64)
            .filter(|fov| fov.is_finite())
            .unwrap_or(70.0)
            .clamp(1.0, 179.0) as f32;
        keyframe.interp = json
            .get("interpolation")
            .and_then(Json::as_str)
            .and_then(Interpolation::from_id)
            .unwrap_or(Interpolation::Linear);
        keyframe.tension = json.get("tension").and_then(Json::as_f64).unwrap_or(1.0).clamp(0.0, 1.0) as f32;
        keyframe.shake = json.get("shake").and_then(Json::as_f64).unwrap_or(0.0).clamp(0.0, 1.0) as f32;
        keyframe.label = json.get("label").and_then(Json::as_str).unwrap_or("").to_owned();
        Some(keyframe)
    }
}

/// What the camera is at a moment: the pose the renderer consumes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPose {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub fov: f32,
    /// 0..=1, scalar shake to apply.
    pub shake: f32,
}

impl Default for CameraPose {
    fn default() -> Self {
        Self { pos: Vec3::new(0.0, 64.0, 0.0), yaw: 0.0, pitch: 0.0, roll: 0.0, fov: 70.0, shake: 0.0 }
    }
}

/// A stretch of the timeline played at a different speed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeedSegment {
    /// First timeline tick of the segment.
    pub from: u64,
    /// One past the last timeline tick.
    pub to: u64,
    /// 0.1 is a tenth speed, 4.0 is four times.
    pub speed: f64,
}

/// A named camera path, as the timeline editor holds it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CameraPath {
    pub name: String,
    /// Sorted by tick.
    keyframes: Vec<Keyframe>,
    /// Sorted, non-overlapping speed segments.
    speed: Vec<SpeedSegment>,
}

impl CameraPath {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), keyframes: Vec::new(), speed: Vec::new() }
    }

    pub fn keyframes(&self) -> &[Keyframe] {
        &self.keyframes
    }

    pub fn speed_segments(&self) -> &[SpeedSegment] {
        &self.speed
    }

    pub fn is_empty(&self) -> bool {
        self.keyframes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.keyframes.len()
    }

    /// Insert a keyframe, keeping the list ordered; returns its index.
    pub fn add(&mut self, keyframe: Keyframe) -> usize {
        let at = self.keyframes.partition_point(|k| k.tick <= keyframe.tick);
        self.keyframes.insert(at, keyframe);
        at
    }

    pub fn remove(&mut self, index: usize) -> Option<Keyframe> {
        if index < self.keyframes.len() {
            Some(self.keyframes.remove(index))
        } else {
            None
        }
    }

    pub fn get(&self, index: usize) -> Option<&Keyframe> {
        self.keyframes.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Keyframe> {
        self.keyframes.get_mut(index)
    }

    /// Move a keyframe to a new tick, reordering if needed.
    pub fn retime(&mut self, index: usize, tick: u64) -> Option<usize> {
        let mut keyframe = self.remove(index)?;
        keyframe.tick = tick;
        Some(self.add(keyframe))
    }

    /// Drag-and-drop reordering: the keyframe at `from` takes the place at
    /// `to` in the sequence. The ticks are kept as a set, so the camera visits
    /// the same moments in a new order.
    pub fn reorder(&mut self, from: usize, to: usize) -> bool {
        if from >= self.keyframes.len() || to >= self.keyframes.len() || from == to {
            return false;
        }
        let ticks: Vec<u64> = self.keyframes.iter().map(|k| k.tick).collect();
        let moved = self.keyframes.remove(from);
        self.keyframes.insert(to, moved);
        let mut sorted_ticks = ticks;
        sorted_ticks.sort_unstable();
        for (keyframe, tick) in self.keyframes.iter_mut().zip(sorted_ticks) {
            keyframe.tick = tick;
        }
        true
    }

    /// Duplicate one keyframe just after itself. Used by copy/paste too.
    pub fn duplicate(&mut self, index: usize) -> Option<usize> {
        let original = self.keyframes.get(index)?.clone();
        let mut copy = original.clone();
        copy.tick = original.tick.saturating_add(1);
        Some(self.add(copy))
    }

    /// Copy a range into a clipboard: keyframes keep their offset from `from`.
    pub fn copy_range(&self, from_tick: u64, to_tick: u64) -> Vec<Keyframe> {
        let (lo, hi) = if from_tick <= to_tick { (from_tick, to_tick) } else { (to_tick, from_tick) };
        self.keyframes
            .iter()
            .filter(|k| k.tick >= lo && k.tick <= hi)
            .cloned()
            .collect()
    }

    /// Paste a clipboard so its first keyframe lands on `at_tick`.
    pub fn paste(&mut self, clipboard: &[Keyframe], at_tick: u64) -> Vec<usize> {
        let Some(first) = clipboard.first() else {
            return Vec::new();
        };
        let offset = at_tick as i64 - first.tick as i64;
        let mut inserted = Vec::with_capacity(clipboard.len());
        for keyframe in clipboard {
            let mut copy = keyframe.clone();
            copy.tick = (keyframe.tick as i64 + offset).max(0) as u64;
            inserted.push(self.add(copy));
        }
        inserted
    }

    /// Snap every keyframe to a multiple of `step` ticks.
    pub fn snap_all(&mut self, step: u64) {
        if step == 0 {
            return;
        }
        for keyframe in &mut self.keyframes {
            keyframe.tick = (keyframe.tick + step / 2) / step * step;
        }
        self.keyframes.sort_by_key(|k| k.tick);
    }

    /// The keyframe nearest a tick, for the editor's selection.
    pub fn nearest(&self, tick: u64) -> Option<usize> {
        self.keyframes
            .iter()
            .enumerate()
            .min_by_key(|(_, k)| k.tick.abs_diff(tick))
            .map(|(i, _)| i)
    }

    /// The last keyframe at or before `tick` (the one whose interpolation applies).
    pub fn segment_for(&self, tick: f64) -> Option<usize> {
        self.keyframes
            .iter()
            .rposition(|k| k.tick as f64 <= tick)
    }

    /// Duration the path covers.
    pub fn duration_ticks(&self) -> u64 {
        match (self.keyframes.first(), self.keyframes.last()) {
            (Some(first), Some(last)) => last.tick.saturating_sub(first.tick),
            _ => 0,
        }
    }

    /// Set (or replace) the speed of a range. Overlapping existing segments are
    /// trimmed so the list stays sorted and non-overlapping.
    pub fn set_speed(&mut self, from: u64, to: u64, speed: f64) {
        let (from, to) = if from <= to { (from, to) } else { (to, from) };
        if from == to {
            return;
        }
        let speed = speed.clamp(0.05, 20.0);
        let mut kept: Vec<SpeedSegment> = Vec::new();
        for segment in self.speed.drain(..) {
            if segment.to <= from || segment.from >= to {
                kept.push(segment);
                continue;
            }
            if segment.from < from {
                kept.push(SpeedSegment { from: segment.from, to: from, speed: segment.speed });
            }
            if segment.to > to {
                kept.push(SpeedSegment { from: to, to: segment.to, speed: segment.speed });
            }
        }
        kept.push(SpeedSegment { from, to, speed });
        kept.sort_by_key(|s| s.from);
        self.speed = kept;
    }

    /// The speed in force at a timeline tick.
    pub fn speed_at(&self, tick: u64) -> f64 {
        self.speed
            .iter()
            .find(|s| tick >= s.from && tick < s.to)
            .map(|s| s.speed)
            .unwrap_or(1.0)
    }

    /// Map raw timeline ticks to path ticks through the speed segments.
    ///
    /// A segment at 0.5x spends two timeline ticks per path tick, so the camera
    /// moves half as fast — slow motion. The result is a continuous, monotonic
    /// function so scrubbing backwards lands on the same place.
    pub fn remap_time(&self, timeline_tick: f64) -> f64 {
        let timeline_tick = timeline_tick.max(0.0);
        let mut path_tick = 0.0;
        let mut at = 0.0f64;
        for segment in &self.speed {
            let from = segment.from as f64;
            let to = segment.to as f64;
            if timeline_tick <= from {
                return path_tick + (timeline_tick - at);
            }
            if from > at {
                let span = from - at;
                if timeline_tick <= from {
                    return path_tick + (timeline_tick - at);
                }
                path_tick += span;
                at = from;
            }
            let span = (to - from).min(timeline_tick - at);
            path_tick += span * segment.speed;
            at += span;
            if timeline_tick <= to {
                return path_tick;
            }
        }
        path_tick + (timeline_tick - at)
    }

    /// Sample the camera at a path tick (already remapped).
    pub fn sample(&self, path_tick: f64) -> Option<CameraPose> {
        let first = self.keyframes.first()?;
        if self.keyframes.len() == 1 || path_tick <= first.tick as f64 {
            return Some(pose_of(first));
        }
        let last = self.keyframes.last()?;
        if path_tick >= last.tick as f64 {
            return Some(pose_of(last));
        }
        let index = self.keyframes.partition_point(|k| (k.tick as f64) <= path_tick) - 1;
        let a = &self.keyframes[index];
        let b = &self.keyframes[index + 1];
        let span = (b.tick - a.tick) as f64;
        let t = if span <= 0.0 { 0.0 } else { (path_tick - a.tick as f64) / span };
        Some(self.interpolate(index, t))
    }

    /// Sample at a timeline tick, applying time remapping.
    pub fn sample_timeline(&self, timeline_tick: f64) -> Option<CameraPose> {
        self.sample(self.remap_time(timeline_tick))
    }

    fn interpolate(&self, index: usize, t: f64) -> CameraPose {
        let a = &self.keyframes[index];
        let b = &self.keyframes[index + 1];
        let mode = a.interp;
        let pos = match mode {
            Interpolation::CatmullRom | Interpolation::Hermite => {
                let p0 = self.keyframes[index.saturating_sub(1)].pos;
                let p3 = self.keyframes[(index + 2).min(self.keyframes.len() - 1)].pos;
                if mode == Interpolation::CatmullRom {
                    catmull_rom(p0, a.pos, b.pos, p3, t)
                } else {
                    let (m1, m2) = hermite_tangents(p0, a.pos, b.pos, p3, a.tension as f64);
                    hermite(a.pos, b.pos, m1, m2, t)
                }
            }
            other => a.pos.lerp(b.pos, other.ease(t)),
        };
        CameraPose {
            pos: if pos.is_finite() { pos } else { a.pos },
            yaw: wrap_degrees(angle(a.yaw as f64, b.yaw as f64, t, mode)) as f32,
            pitch: clamp_pitch(angle(a.pitch as f64, b.pitch as f64, t, mode)) as f32,
            roll: wrap_degrees(scalar(a.roll as f64, b.roll as f64, t, mode)) as f32,
            fov: scalar(a.fov as f64, b.fov as f64, t, mode).clamp(1.0, 179.0) as f32,
            shake: scalar(a.shake as f64, b.shake as f64, t, mode).clamp(0.0, 1.0) as f32,
        }
    }

    pub fn to_json(&self) -> Json {
        Json::Object(vec![
            ("name".into(), Json::String(self.name.clone())),
            (
                "keyframes".into(),
                Json::Array(self.keyframes.iter().map(Keyframe::to_json).collect()),
            ),
            (
                "speed".into(),
                Json::Array(
                    self.speed
                        .iter()
                        .map(|s| {
                            Json::Object(vec![
                                ("from".into(), Json::int(s.from as i64)),
                                ("to".into(), Json::int(s.to as i64)),
                                ("speed".into(), Json::Number(s.speed)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }

    pub fn from_json(json: &Json) -> Option<Self> {
        let mut path = CameraPath::new(json.get("name").and_then(Json::as_str).unwrap_or("path"));
        if let Some(items) = json.get("keyframes").and_then(Json::as_array) {
            for item in items {
                if let Some(keyframe) = Keyframe::from_json(item) {
                    path.add(keyframe);
                }
            }
        }
        if let Some(items) = json.get("speed").and_then(Json::as_array) {
            for item in items {
                let from = item.get("from").and_then(Json::as_u64).unwrap_or(0);
                let to = item.get("to").and_then(Json::as_u64).unwrap_or(0);
                let speed = item.get("speed").and_then(Json::as_f64).unwrap_or(1.0);
                if to > from {
                    path.set_speed(from, to, speed);
                }
            }
        }
        Some(path)
    }

    /// Problems the editor shows before a render starts.
    pub fn problems(&self) -> Vec<String> {
        let mut issues = Vec::new();
        if self.keyframes.is_empty() {
            issues.push("camera path has no keyframes".to_owned());
        }
        for pair in self.keyframes.windows(2) {
            if pair[0].tick == pair[1].tick {
                issues.push(format!("two keyframes share tick {}", pair[0].tick));
            }
        }
        if self.duration_ticks() == 0 && self.keyframes.len() > 1 {
            issues.push("camera path has no duration".to_owned());
        }
        issues
    }
}

fn pose_of(keyframe: &Keyframe) -> CameraPose {
    CameraPose {
        pos: keyframe.pos,
        yaw: keyframe.yaw,
        pitch: keyframe.pitch,
        roll: keyframe.roll,
        fov: keyframe.fov,
        shake: keyframe.shake,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn straight_path() -> CameraPath {
        let mut path = CameraPath::new("test");
        path.add(Keyframe { tick: 0, pos: Vec3::new(0.0, 64.0, 0.0), yaw: 0.0, pitch: 0.0, fov: 70.0, ..Keyframe::default() });
        path.add(Keyframe { tick: 100, pos: Vec3::new(100.0, 64.0, 0.0), yaw: 90.0, pitch: 0.0, fov: 90.0, ..Keyframe::default() });
        path
    }

    #[test]
    fn samples_a_linear_segment() {
        let path = straight_path();
        let mid = path.sample(50.0).unwrap();
        assert!((mid.pos.x - 50.0).abs() < 1e-6);
        assert!((mid.yaw - 45.0).abs() < 1e-6);
        assert!((mid.fov - 80.0).abs() < 1e-6);
        // Outside the path holds the end poses.
        assert_eq!(path.sample(-10.0).unwrap().pos, Vec3::new(0.0, 64.0, 0.0));
        assert_eq!(path.sample(1_000.0).unwrap().pos, Vec3::new(100.0, 64.0, 0.0));
    }

    #[test]
    fn an_empty_path_has_no_pose_and_a_single_one_is_constant() {
        assert!(CameraPath::new("empty").sample(0.0).is_none());
        let mut path = CameraPath::new("one");
        path.add(Keyframe::new(10, Vec3::new(1.0, 2.0, 3.0), 10.0, 5.0));
        assert_eq!(path.sample(0.0).unwrap().pos, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(path.sample(99.0).unwrap().yaw, 10.0);
    }

    #[test]
    fn catmull_rom_segments_are_curved_not_cornered() {
        let mut path = CameraPath::new("arc");
        for (tick, x) in [(0u64, 0.0), (100, 10.0), (200, 20.0)] {
            path.add(Keyframe {
                tick,
                pos: Vec3::new(x, if tick == 100 { 10.0 } else { 0.0 }, 0.0),
                interp: Interpolation::CatmullRom,
                ..Keyframe::default()
            });
        }
        let mid = path.sample(50.0).unwrap();
        // A straight line between (0,0) and (10,10) would give y = 5.
        assert!(mid.pos.y > 3.0 && mid.pos.y < 6.0, "y = {}", mid.pos.y);
        assert!(mid.pos.x > 0.0 && mid.pos.x < 10.0);
    }

    #[test]
    fn easing_modes_change_speed_not_endpoints() {
        let mut eased = straight_path();
        eased.get_mut(0).unwrap().interp = Interpolation::EaseInOut;
        let quarter = eased.sample(25.0).unwrap();
        assert!(quarter.pos.x < 25.0);
        assert!((eased.sample(100.0).unwrap().pos.x - 100.0).abs() < 1e-6);
    }

    #[test]
    fn constant_holds_until_the_next_keyframe() {
        let mut path = straight_path();
        path.get_mut(0).unwrap().interp = Interpolation::Constant;
        assert!((path.sample(99.0).unwrap().pos.x - 0.0).abs() < 1e-6);
        assert!((path.sample(100.0).unwrap().pos.x - 100.0).abs() < 1e-6);
    }

    #[test]
    fn angles_wrap_the_short_way_and_pitch_clamps() {
        let mut path = CameraPath::new("wrap");
        path.add(Keyframe { tick: 0, yaw: 350.0, pitch: -89.0, ..Keyframe::default() });
        path.add(Keyframe { tick: 10, yaw: 10.0, pitch: 89.0, ..Keyframe::default() });
        let mid = path.sample(5.0).unwrap();
        assert!((mid.yaw - 0.0).abs() < 1e-6 || (mid.yaw - 360.0).abs() < 1e-6);
        path.add(Keyframe { tick: 20, pitch: 200.0, ..Keyframe::default() });
        assert!(path.sample(20.0).unwrap().pitch <= 90.0);
    }

    #[test]
    fn editing_operations_keep_the_list_ordered() {
        let mut path = CameraPath::new("edit");
        path.add(Keyframe::new(100, Vec3::ZERO, 0.0, 0.0));
        path.add(Keyframe::new(0, Vec3::ZERO, 0.0, 0.0));
        path.add(Keyframe::new(50, Vec3::ZERO, 0.0, 0.0));
        assert_eq!(path.keyframes().iter().map(|k| k.tick).collect::<Vec<_>>(), vec![0, 50, 100]);
        let new_index = path.retime(0, 75).unwrap();
        assert_eq!(path.get(new_index).unwrap().tick, 75);
        assert_eq!(path.keyframes().iter().map(|k| k.tick).collect::<Vec<_>>(), vec![50, 75, 100]);
        assert!(path.reorder(0, 2));
        assert_eq!(path.keyframes().iter().map(|k| k.tick).collect::<Vec<_>>(), vec![50, 75, 100]);
        assert_eq!(path.remove(0).unwrap().tick, 50);
        assert!(path.remove(9).is_none());
    }

    #[test]
    fn copy_paste_and_snap() {
        let mut path = CameraPath::new("clip");
        path.add(Keyframe::new(10, Vec3::new(1.0, 0.0, 0.0), 0.0, 0.0));
        path.add(Keyframe::new(20, Vec3::new(2.0, 0.0, 0.0), 0.0, 0.0));
        let clip = path.copy_range(10, 20);
        assert_eq!(clip.len(), 2);
        let pasted = path.paste(&clip, 100);
        assert_eq!(pasted.len(), 2);
        assert_eq!(path.get(pasted[0]).unwrap().tick, 100);
        assert_eq!(path.get(pasted[1]).unwrap().tick, 110);
        path.snap_all(50);
        assert!(path.keyframes().iter().all(|k| k.tick % 50 == 0));
        assert_eq!(path.nearest(5), Some(0));
        assert_eq!(path.keyframes().iter().map(|k| k.tick).collect::<Vec<_>>(), vec![0, 0, 100, 100]);
    }

    #[test]
    fn duplicate_adds_a_neighbour() {
        let mut path = straight_path();
        let index = path.duplicate(0).unwrap();
        assert_eq!(path.len(), 3);
        assert_eq!(path.get(index).unwrap().tick, 1);
    }

    #[test]
    fn time_remapping_slows_and_speeds_segments() {
        let mut path = CameraPath::new("ramp");
        path.add(Keyframe::new(0, Vec3::ZERO, 0.0, 0.0));
        path.add(Keyframe::new(400, Vec3::new(400.0, 0.0, 0.0), 0.0, 0.0));
        // Half speed from timeline 100 to 200 consumes 50 path ticks.
        path.set_speed(100, 200, 0.5);
        assert!((path.remap_time(0.0) - 0.0).abs() < 1e-9);
        assert!((path.remap_time(100.0) - 100.0).abs() < 1e-9);
        assert!((path.remap_time(200.0) - 150.0).abs() < 1e-9, "slowed");
        assert!((path.remap_time(250.0) - 200.0).abs() < 1e-9, "back to normal");
        // Monotonic, so scrubbing never jumps backwards.
        let mut previous = -1.0;
        for tick in 0..500 {
            let mapped = path.remap_time(tick as f64);
            assert!(mapped >= previous);
            previous = mapped;
        }
        // A quadruple-speed ramp: 40 timeline ticks cover 160 path ticks.
        path.set_speed(300, 340, 4.0);
        assert!((path.remap_time(340.0) - path.remap_time(300.0) - 160.0).abs() < 1e-9);
        assert_eq!(path.speed_at(150), 0.5);
        assert_eq!(path.speed_at(10), 1.0);
    }

    #[test]
    fn speed_segments_never_overlap() {
        let mut path = CameraPath::new("overlap");
        path.set_speed(0, 100, 0.5);
        path.set_speed(50, 150, 2.0);
        let segments = path.speed_segments();
        assert_eq!(segments.len(), 3);
        for pair in segments.windows(2) {
            assert!(pair[0].to <= pair[1].from, "segments overlap: {segments:?}");
        }
    }

    #[test]
    fn json_round_trips_and_repairs_out_of_range_values() {
        let mut path = straight_path();
        path.set_speed(10, 20, 0.25);
        path.get_mut(0).unwrap().shake = 0.5;
        let json = path.to_json();
        let back = CameraPath::from_json(&json).unwrap();
        assert_eq!(back, path);

        let hand_edited = crate::json::parse(
            r#"{"name":"hand","keyframes":[{"tick":5,"x":1,"y":2,"z":3,"pitch":200,"fov":900,"interpolation":"ease_in"}]}"#,
        )
        .unwrap();
        let repaired = CameraPath::from_json(&hand_edited).unwrap();
        let keyframe = repaired.get(0).unwrap();
        assert_eq!(keyframe.pitch, 90.0);
        assert_eq!(keyframe.fov, 179.0);
        assert_eq!(keyframe.interp, Interpolation::EaseIn);
    }

    #[test]
    fn problems_are_reported() {
        assert_eq!(CameraPath::new("empty").problems().len(), 1);
        let mut path = CameraPath::new("dup");
        path.add(Keyframe::new(5, Vec3::ZERO, 0.0, 0.0));
        path.add(Keyframe::new(5, Vec3::ZERO, 0.0, 0.0));
        assert!(path.problems().iter().any(|p| p.contains("share tick 5")));
    }
}
