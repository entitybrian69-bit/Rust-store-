// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! The cinematic camera: modes, the free-fly camera, shake and the matrices the
//! renderer needs.
//!
//! In a replay there is no player to drive a camera from, so Volt Lapse owns it:
//! a first-person view of any recorded player, a third-person orbit, a
//! spectator follow or a free camera, and on top of any of those a camera path
//! from the timeline editor. The result is a [`CameraPose`] that
//! `hooks/camera.rs` hands to Horizon's `camera_pose` hook and that the offline
//! renderer uses as-is.

use crate::interp::{clamp_pitch, wrap_degrees};
use crate::keyframe::{CameraPath, CameraPose};
use crate::wire::Vec3;

/// Where the camera looks from.
#[derive(Clone, Debug, PartialEq)]
pub enum CameraMode {
    /// From a recorded player's eyes, following their head.
    FirstPerson { player: String },
    /// Orbits a recorded player, behind and slightly above.
    ThirdPerson { player: String, distance: f32, side: f32, height: f32 },
    /// Follows a recorded player's body but turns freely (Vanilla's spectator).
    Spectator { player: String },
    /// Flies on its own, driven by [`FreeCam`].
    Free,
    /// Follows a camera path from the timeline editor.
    Path,
}

impl CameraMode {
    pub fn id(&self) -> &'static str {
        match self {
            CameraMode::FirstPerson { .. } => "first_person",
            CameraMode::ThirdPerson { .. } => "third_person",
            CameraMode::Spectator { .. } => "spectator",
            CameraMode::Free => "free",
            CameraMode::Path => "path",
        }
    }

    /// The player this mode follows, if any.
    pub fn player(&self) -> Option<&str> {
        match self {
            CameraMode::FirstPerson { player } | CameraMode::Spectator { player } => Some(player),
            CameraMode::ThirdPerson { player, .. } => Some(player),
            _ => None,
        }
    }
}

/// A recorded player or entity the camera can look from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraTarget {
    pub pos: Vec3,
    pub eye_height: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for CameraTarget {
    fn default() -> Self {
        Self { pos: Vec3::new(0.0, 64.0, 0.0), eye_height: 1.62, yaw: 0.0, pitch: 0.0 }
    }
}

/// Camera shake settings; keyframes multiply these.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShakeSettings {
    pub enabled: bool,
    /// Multiplies every keyframe's shake value.
    pub intensity: f32,
    /// Oscillations per second of the slowest octave.
    pub frequency: f32,
}

impl Default for ShakeSettings {
    fn default() -> Self {
        Self { enabled: true, intensity: 1.0, frequency: 0.9 }
    }
}

/// The free camera: position, rotation and smoothed movement.
#[derive(Clone, Debug, PartialEq)]
pub struct FreeCam {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// Blocks per second at normal speed.
    pub speed: f32,
    /// 0 = instant, 1 = very lazy; the smoothing half-life in seconds.
    pub damping: f32,
    /// Current velocity, blocks per second in world space.
    velocity: Vec3,
}

impl Default for FreeCam {
    fn default() -> Self {
        Self { pos: Vec3::new(0.0, 70.0, 0.0), yaw: 0.0, pitch: 0.0, speed: 12.0, damping: 0.15, velocity: Vec3::ZERO }
    }
}

impl FreeCam {
    /// Drop the camera onto a target, keeping the speed settings.
    pub fn place_at(&mut self, target: CameraTarget) {
        self.pos = target.pos.add(Vec3::new(0.0, target.eye_height as f64, 0.0));
        self.yaw = target.yaw;
        self.pitch = target.pitch;
        self.velocity = Vec3::ZERO;
    }

    /// Integrate movement for one frame.
    ///
    /// `forward`, `strafe` and `up` are -1..=1 inputs; `fast` multiplies speed
    /// by 4 (the sprint modifier), `slow` by 0.25 (precision).
    pub fn move_by(&mut self, forward: f32, strafe: f32, up: f32, fast: bool, slow: bool, delta_seconds: f64) {
        let b = basis(self.yaw, self.pitch, 0.0);
        let look = b.forward;
        let right = b.right;
        let world_up = Vec3::new(0.0, 1.0, 0.0);

        let mut wish = look.scale(forward as f64).add(right.scale(strafe as f64)).add(world_up.scale(up as f64));
        if wish.length() > 1.0 {
            wish = wish.normalized();
        }
        let mut speed = self.speed as f64;
        if fast {
            speed *= 4.0;
        }
        if slow {
            speed *= 0.25;
        }
        let target_velocity = wish.scale(speed);
        // Exponential smoothing framed in seconds, so the feel does not change
        // with the frame rate.
        let blend = if self.damping <= 0.0 {
            1.0
        } else {
            1.0 - (-delta_seconds / self.damping.max(1e-3) as f64).exp()
        };
        self.velocity = self.velocity.lerp(target_velocity, blend);
        self.pos = self.pos.add(self.velocity.scale(delta_seconds));
        if !self.pos.is_finite() {
            self.pos = Vec3::new(0.0, 70.0, 0.0);
            self.velocity = Vec3::ZERO;
        }
    }

    /// Turn the camera (degrees), clamping pitch like the game does.
    pub fn turn(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw = wrap_degrees(self.yaw as f64 + delta_yaw as f64) as f32;
        self.pitch = clamp_pitch(self.pitch as f64 + delta_pitch as f64) as f32;
    }

    pub fn pose(&self) -> CameraPose {
        CameraPose { pos: self.pos, yaw: self.yaw, pitch: self.pitch, roll: 0.0, fov: 70.0, shake: 0.0 }
    }
}

/// The camera rig: mode, path, shake and smoothing, producing poses.
#[derive(Clone, Debug)]
pub struct CameraRig {
    pub mode: CameraMode,
    pub path: Option<CameraPath>,
    pub free: FreeCam,
    pub shake: ShakeSettings,
    /// FOV the camera returns when no keyframe animates it.
    pub base_fov: f32,
    /// Vertical FOV scale from zoom (`1.0` is none).
    pub zoom: f32,
    /// Interpolate between the previous and the new pose to hide tick jitter.
    smoothing: bool,
    previous: Option<CameraPose>,
}

impl Default for CameraRig {
    fn default() -> Self {
        Self {
            mode: CameraMode::Free,
            path: None,
            free: FreeCam::default(),
            shake: ShakeSettings::default(),
            base_fov: 70.0,
            zoom: 1.0,
            smoothing: true,
            previous: None,
        }
    }
}

impl CameraRig {
    /// The pose for this frame.
    ///
    /// `tick` is the playback's (fractional) tick, `target` the recorded player
    /// the mode follows, `delta_seconds` the frame time for free-fly movement.
    pub fn pose(&mut self, tick: f64, target: Option<CameraTarget>, delta_seconds: f64) -> CameraPose {
        let raw = match &self.mode {
            CameraMode::Path => match &self.path {
                Some(path) => path.sample_timeline(tick).unwrap_or_else(|| self.fallback_pose(target)),
                None => self.fallback_pose(target),
            },
            CameraMode::FirstPerson { .. } => {
                let target = target.unwrap_or_default();
                CameraPose {
                    pos: target.pos.add(Vec3::new(0.0, target.eye_height as f64, 0.0)),
                    yaw: target.yaw,
                    pitch: target.pitch,
                    roll: 0.0,
                    fov: self.base_fov,
                    shake: 0.0,
                }
            }
            CameraMode::ThirdPerson { distance, side, height } => {
                let target = target.unwrap_or_default();
                let eye = target.pos.add(Vec3::new(0.0, target.eye_height as f64 + *height as f64, 0.0));
                let basis = basis(target.yaw, target.pitch, 0.0);
                let forward = basis.forward;
                let right = basis.right;
                CameraPose {
                    // Behind the player, offset to one side.
                    pos: eye.sub(forward.scale(*distance as f64)).add(right.scale(*side as f64)),
                    yaw: target.yaw,
                    pitch: target.pitch,
                    roll: 0.0,
                    fov: self.base_fov,
                    shake: 0.0,
                }
            }
            CameraMode::Spectator { .. } => {
                let target = target.unwrap_or_default();
                let mut pose = self.free.pose();
                pose.pos = target.pos.add(Vec3::new(0.0, target.eye_height as f64, 0.0));
                pose.fov = self.base_fov;
                pose
            }
            CameraMode::Free => {
                let mut pose = self.free.pose();
                pose.fov = self.base_fov;
                pose
            }
        };

        let mut pose = raw;
        // A path's shake track also drives a handheld free camera or a
        // third-person orbit; the path's own mode already carries FOV and roll.
        if self.mode != CameraMode::Path {
            if let Some(sample) = self.path.as_ref().and_then(|path| path.sample_timeline(tick)) {
                pose.shake = sample.shake;
            }
        }
        pose.fov = (pose.fov * self.zoom).clamp(1.0, 179.0);

        if self.smoothing {
            if let Some(previous) = self.previous {
                let blend = if delta_seconds <= 0.0 { 1.0 } else { (delta_seconds * 20.0).clamp(0.0, 1.0) };
                pose.pos = previous.pos.lerp(pose.pos, blend);
                pose.yaw = crate::interp::lerp_angle(previous.yaw as f64, pose.yaw as f64, blend) as f32;
                pose.pitch = crate::interp::lerp(previous.pitch as f64, pose.pitch as f64, blend) as f32;
                pose.roll = crate::interp::lerp(previous.roll as f64, pose.roll as f64, blend) as f32;
                pose.fov = crate::interp::lerp(previous.fov as f64, pose.fov as f64, blend) as f32;
            }
            self.previous = Some(pose);
        } else {
            self.previous = None;
        }

        if self.shake.enabled && pose.shake > 0.0 {
            pose = apply_shake(pose, tick, self.shake);
        }
        pose.pitch = clamp_pitch(pose.pitch as f64) as f32;
        pose
    }

    fn fallback_pose(&self, target: Option<CameraTarget>) -> CameraPose {
        match target {
            Some(target) => CameraPose {
                pos: target.pos.add(Vec3::new(0.0, target.eye_height as f64, 0.0)),
                yaw: target.yaw,
                pitch: target.pitch,
                roll: 0.0,
                fov: self.base_fov,
                shake: 0.0,
            },
            None => self.free.pose(),
        }
    }

    /// Enable or disable frame-to-frame smoothing.
    pub fn set_smoothing(&mut self, enabled: bool) {
        self.smoothing = enabled;
        if !enabled {
            self.previous = None;
        }
    }
}

/// Deterministic value noise in -1..=1 from a tick and a channel.
fn noise(tick: f64, channel: u32) -> f64 {
    let floor = tick.floor();
    let fraction = tick - floor;
    let at = |step: f64| {
        let mut hash = (step as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        hash ^= (channel as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
        hash ^= hash >> 30;
        hash = hash.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        hash ^= hash >> 27;
        (hash >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    };
    // Smoothstep between integer ticks so shake never buzzes at 20 Hz.
    let smooth = fraction * fraction * (3.0 - 2.0 * fraction);
    let a = at(floor);
    let b = at(floor + 1.0);
    a + (b - a) * smooth
}

/// Apply handheld shake to a pose. Bounded, deterministic and centred on zero,
/// so a shake track never drifts the camera away from its path.
pub fn apply_shake(mut pose: CameraPose, tick: f64, settings: ShakeSettings) -> CameraPose {
    let amount = (pose.shake * settings.intensity).clamp(0.0, 1.0) as f64;
    if amount <= 0.0 {
        pose.shake = 0.0;
        return pose;
    }
    let frequency = settings.frequency.max(0.05) as f64;
    let time = tick / 20.0 * frequency * 20.0; // ticks to shake steps
    let yaw = noise(time, 1) * 1.4 * amount;
    let pitch = noise(time + 37.0, 2) * 1.1 * amount;
    let roll = noise(time + 71.0, 3) * 2.2 * amount;
    let drift = Vec3::new(noise(time + 11.0, 4), noise(time + 23.0, 5), noise(time + 53.0, 6)).scale(0.06 * amount);
    pose.yaw = wrap_degrees(pose.yaw as f64 + yaw) as f32;
    pose.pitch = clamp_pitch(pose.pitch as f64 + pitch) as f32;
    pose.roll = wrap_degrees(pose.roll as f64 + roll) as f32;
    pose.pos = pose.pos.add(drift);
    pose.shake = 0.0;
    pose
}

/// The camera's orthonormal basis, for drawing the path and for the renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Basis {
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
}

/// Forward/right/up for a yaw, pitch and roll (degrees), using the game's
/// convention: yaw 0 looks towards +Z (south), yaw 90 towards -X (west), and
/// the right-hand vector is derived from yaw alone so looking straight up or
/// down cannot degenerate it.
pub fn basis(yaw: f32, pitch: f32, roll: f32) -> Basis {
    let (sin_yaw, cos_yaw) = (yaw as f64).to_radians().sin_cos();
    let (sin_pitch, cos_pitch) = (pitch as f64).to_radians().sin_cos();
    let forward = Vec3::new(-sin_yaw * cos_pitch, -sin_pitch, cos_yaw * cos_pitch).normalized();
    let right = Vec3::new(-cos_yaw, 0.0, -sin_yaw);
    let mut up = right.cross(forward).normalized();
    if roll != 0.0 {
        let angle = (roll as f64).to_radians();
        let (sin_r, cos_r) = angle.sin_cos();
        let rolled_right = right.scale(cos_r).add(up.scale(sin_r));
        let rolled_up = up.scale(cos_r).sub(right.scale(sin_r));
        right = rolled_right.normalized();
        up = rolled_up.normalized();
    }
    Basis { forward, right, up }
}

/// A 4×4 matrix in row-major order, for the renderer's view/projection work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4(pub [[f64; 4]; 4]);

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    /// A right-handed perspective projection (OpenGL convention, depth in
    /// -1..=1), which is what Horizon's renderer expects.
    pub fn perspective(fov_y_degrees: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let fov = (fov_y_degrees as f64).clamp(1.0, 179.0);
        let aspect = (aspect as f64).max(1e-3);
        let near = (near as f64).max(1e-4);
        let far = (far as f64).max(near + 1e-3);
        let f = 1.0 / (fov.to_radians() / 2.0).tan();
        Mat4([
            [f / aspect, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, (far + near) / (near - far), (2.0 * far * near) / (near - far)],
            [0.0, 0.0, -1.0, 0.0],
        ])
    }

    /// The view matrix for a pose (translation then rotation).
    pub fn view(pose: CameraPose) -> Mat4 {
        let b = basis(pose.yaw, pose.pitch, pose.roll);
        Mat4([
            [b.right.x, b.right.y, b.right.z, -b.right.dot(pose.pos)],
            [b.up.x, b.up.y, b.up.z, -b.up.dot(pose.pos)],
            [-b.forward.x, -b.forward.y, -b.forward.z, b.forward.dot(pose.pos)],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    /// Row-major element access, for tests and for a 4×4 uniform upload.
    pub fn at(&self, row: usize, column: usize) -> f64 {
        self.0[row][column]
    }
}

/// The direction a pixel looks in for an equirectangular (360°) frame.
///
/// `u` and `v` are 0..=1 across and down the image; `roll` rotates the result,
/// so a VR render can be levelled after the fact.
pub fn equirect_direction(u: f64, v: f64, roll: f32) -> Vec3 {
    let lon = (u * 2.0 - 1.0) * std::f64::consts::PI;
    let lat = (0.5 - v) * std::f64::consts::PI;
    let (sin_lat, cos_lat) = lat.sin_cos();
    let direction = Vec3::new(lon.sin() * cos_lat, sin_lat, lon.cos() * cos_lat);
    if roll == 0.0 {
        direction.normalized()
    } else {
        // Rotate about the up axis by the roll, matching `basis`.
        let angle = (roll as f64).to_radians();
        let (sin_r, cos_r) = angle.sin_cos();
        Vec3::new(direction.x * cos_r - direction.z * sin_r, direction.y, direction.x * sin_r + direction.z * cos_r)
            .normalized()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyframe::Keyframe;

    fn target() -> CameraTarget {
        CameraTarget { pos: Vec3::new(10.0, 64.0, 20.0), eye_height: 1.62, yaw: 0.0, pitch: 0.0 }
    }

    #[test]
    fn first_person_sits_at_the_eye() {
        let mut rig = CameraRig { mode: CameraMode::FirstPerson { player: "Alex".into() }, ..CameraRig::default() };
        rig.set_smoothing(false);
        let pose = rig.pose(0.0, Some(target()), 0.05);
        assert!((pose.pos.y - 65.62).abs() < 1e-6);
        assert_eq!(pose.pos.x, 10.0);
        assert_eq!(pose.yaw, 0.0);
    }

    #[test]
    fn third_person_orbit_sits_behind_the_player() {
        let mut rig = CameraRig {
            mode: CameraMode::ThirdPerson { player: "Alex".into(), distance: 4.0, side: 0.0, height: 0.5 },
            ..CameraRig::default()
        };
        rig.set_smoothing(false);
        let pose = rig.pose(0.0, Some(target()), 0.05);
        // Yaw 0 looks towards +Z, so "behind" is -Z.
        assert!((pose.pos.z - (20.0 - 4.0)).abs() < 1e-6, "z = {}", pose.pos.z);
        assert!((pose.pos.y - (64.0 + 1.62 + 0.5)).abs() < 1e-6);
        // Yaw 90 looks towards -X, so "behind" the player is +X.
        let mut yawed = target();
        yawed.yaw = 90.0;
        let pose = rig.pose(0.0, Some(yawed), 0.05);
        assert!((pose.pos.x - (10.0 + 4.0)).abs() < 1e-6, "x = {}", pose.pos.x);
    }

    #[test]
    fn free_cam_moves_forward_with_damping() {
        let mut free = FreeCam { speed: 10.0, damping: 0.0, ..FreeCam::default() };
        free.move_by(1.0, 0.0, 0.0, false, false, 1.0);
        assert!((free.pos.z - 10.0).abs() < 1e-6, "z = {}", free.pos.z);
        // Strafe right at yaw 0: facing +Z (south), the player's right is -X.
        free.move_by(0.0, 1.0, 0.0, false, false, 1.0);
        assert!((free.pos.x + 10.0).abs() < 1e-6, "x = {}", free.pos.x);
        // Fast moves four times as far.
        let mut free = FreeCam { speed: 10.0, damping: 0.0, ..FreeCam::default() };
        free.move_by(1.0, 0.0, 0.0, true, false, 1.0);
        assert!((free.pos.z - 40.0).abs() < 1e-6);
        // Diagonal input is normalised, not 1.41x faster.
        let mut free = FreeCam { speed: 10.0, damping: 0.0, ..FreeCam::default() };
        free.move_by(1.0, 1.0, 0.0, false, false, 1.0);
        assert!((free.pos.sub(Vec3::new(0.0, 70.0, 0.0)).length() - 10.0).abs() < 1e-6);
    }

    #[test]
    fn turning_clamps_pitch() {
        let mut free = FreeCam::default();
        free.turn(400.0, 200.0);
        assert_eq!(free.pitch, 90.0);
        assert!(free.yaw >= -180.0 && free.yaw <= 180.0);
    }

    #[test]
    fn path_mode_follows_keyframes_and_time_remap() {
        let mut path = CameraPath::new("fly");
        path.add(Keyframe::new(0, Vec3::new(0.0, 64.0, 0.0), 0.0, 0.0));
        path.add(Keyframe::new(100, Vec3::new(100.0, 64.0, 0.0), 90.0, 0.0));
        let mut rig = CameraRig { mode: CameraMode::Path, path: Some(path), ..CameraRig::default() };
        rig.set_smoothing(false);
        let pose = rig.pose(50.0, None, 0.05);
        assert!((pose.pos.x - 50.0).abs() < 1e-6, "x = {}", pose.pos.x);
        assert!((pose.yaw - 45.0).abs() < 1e-6);
    }

    #[test]
    fn shake_is_deterministic_bounded_and_silent_at_zero() {
        let settings = ShakeSettings::default();
        let pose = CameraPose { shake: 0.0, ..CameraPose::default() };
        assert_eq!(apply_shake(pose, 12.0, settings), pose);

        let shaky = CameraPose { shake: 1.0, ..CameraPose::default() };
        let a = apply_shake(shaky, 12.0, settings);
        let b = apply_shake(shaky, 12.0, settings);
        assert_eq!(a, b, "same tick gives the same shake");
        assert_ne!(a.yaw, 0.0);
        assert!(a.yaw.abs() <= 1.5 && a.roll.abs() <= 2.5, "bounded: {a:?}");
        assert_eq!(a.shake, 0.0, "consumed");

        // A disabled shake setting leaves the pose alone.
        let settings = ShakeSettings { enabled: false, ..settings };
        assert_eq!(apply_shake(shaky, 12.0, settings), shaky);
    }

    #[test]
    fn shake_is_smooth_between_ticks() {
        let settings = ShakeSettings::default();
        let pose = CameraPose { shake: 1.0, ..CameraPose::default() };
        let a = apply_shake(pose, 10.0, settings).yaw;
        let b = apply_shake(pose, 10.01, settings).yaw;
        assert!((a - b).abs() < 0.05, "no buzz between ticks: {a} vs {b}");
    }

    #[test]
    fn basis_is_orthonormal_and_points_where_expected() {
        let b = basis(0.0, 0.0, 0.0);
        assert!((b.forward - Vec3::new(0.0, 0.0, 1.0)).length() < 1e-9);
        assert!((b.right - Vec3::new(-1.0, 0.0, 0.0)).length() < 1e-9, "right = {:?}", b.right);
        assert!((b.up - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-9);
        // Yaw 90 looks west, as the game's yaw does.
        let b = basis(90.0, 0.0, 0.0);
        assert!((b.forward - Vec3::new(-1.0, 0.0, 0.0)).length() < 1e-9, "forward = {:?}", b.forward);
        for (yaw, pitch, roll) in [(37.0, 12.0, 0.0), (-90.0, 45.0, 30.0), (180.0, -89.0, -15.0)] {
            let b = basis(yaw, pitch, roll);
            assert!(b.forward.dot(b.right).abs() < 1e-9);
            assert!(b.forward.dot(b.up).abs() < 1e-9);
            assert!(b.right.dot(b.up).abs() < 1e-9);
            assert!((b.forward.length() - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn perspective_and_view_matrices_are_sane() {
        let projection = Mat4::perspective(70.0, 16.0 / 9.0, 0.05, 1000.0);
        assert!(projection.at(0, 0) > 0.0);
        assert_eq!(projection.at(3, 2), -1.0);
        // A camera at the origin looking down +Z sees a point in front of it at
        // negative view-space z.
        let view = Mat4::view(CameraPose { pos: Vec3::ZERO, yaw: 0.0, pitch: 0.0, ..CameraPose::default() });
        let z = view.at(2, 0) * 3.0 + view.at(2, 1) * 0.0 + view.at(2, 2) * 5.0 + view.at(2, 3);
        assert!(z < 0.0, "point ahead is in front of the camera: {z}");
    }

    #[test]
    fn equirect_directions_cover_the_sphere() {
        let center = equirect_direction(0.5, 0.5, 0.0);
        assert!((center - Vec3::new(0.0, 0.0, 1.0)).length() < 1e-9);
        let top = equirect_direction(0.5, 0.0, 0.0);
        assert!(top.y > 0.99);
        let bottom = equirect_direction(0.5, 1.0, 0.0);
        assert!(bottom.y < -0.99);
        for u in [0.0, 0.1, 0.5, 0.9, 1.0] {
            for v in [0.0, 0.25, 0.5, 0.75, 1.0] {
                assert!((equirect_direction(u, v, 0.0).length() - 1.0).abs() < 1e-9);
                assert!((equirect_direction(u, v, 25.0).length() - 1.0).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn smoothing_softens_a_jump_between_frames() {
        let mut rig = CameraRig { mode: CameraMode::Free, ..CameraRig::default() };
        rig.free.pos = Vec3::new(0.0, 70.0, 0.0);
        let first = rig.pose(0.0, None, 0.05);
        rig.free.pos = Vec3::new(100.0, 70.0, 0.0);
        let second = rig.pose(0.05, None, 0.05);
        assert!(second.pos.x > first.pos.x && second.pos.x < 100.0, "x = {}", second.pos.x);
        rig.set_smoothing(false);
        let third = rig.pose(0.1, None, 0.05);
        assert_eq!(third.pos.x, 100.0);
    }
}
