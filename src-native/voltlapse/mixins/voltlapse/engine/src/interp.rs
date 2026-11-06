// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! Interpolation and easing for camera paths.
//!
//! A keyframe names how the segment *leaving* it is interpolated, which is how
//! Replay Mod's advanced keyframes behave: change one keyframe's mode and the
//! curve between it and the next one changes. Positions use linear,
//! Catmull-Rom or Hermite; scalars (FOV, roll, shake) use the same family with
//! eased time when the keyframe asks for it.
//!
//! Angles are interpolated along the shorter arc, so a camera panning from 350°
//! to 10° moves 20° forward instead of 340° back.

use crate::wire::Vec3;

/// How the segment after a keyframe is interpolated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Interpolation {
    /// Straight line, constant speed.
    #[default]
    Linear,
    /// Catmull-Rom: a smooth curve through every keyframe, using the two
    /// neighbours of each segment for its tangents. Overshoots slightly, which
    /// is usually what a fly-through wants.
    CatmullRom,
    /// Hermite with tangents from neighbouring keyframes, scaled by `tension`:
    /// 0 gives flat tangents (ease at every keyframe), 1 gives Catmull-Rom.
    Hermite,
    /// Smooth acceleration from rest.
    EaseIn,
    /// Smooth deceleration to rest.
    EaseOut,
    /// Accelerate then decelerate (smoothstep).
    EaseInOut,
    /// Hold the keyframe's value until the next one, then jump. Used for cuts.
    Constant,
}

impl Interpolation {
    /// The mode's id in camera project files.
    pub fn id(self) -> &'static str {
        match self {
            Interpolation::Linear => "linear",
            Interpolation::CatmullRom => "catmull_rom",
            Interpolation::Hermite => "hermite",
            Interpolation::EaseIn => "ease_in",
            Interpolation::EaseOut => "ease_out",
            Interpolation::EaseInOut => "ease_in_out",
            Interpolation::Constant => "constant",
        }
    }

    /// Parse an id, accepting the spellings a hand-edited file tends to use.
    pub fn from_id(id: &str) -> Option<Self> {
        match id.trim().to_ascii_lowercase().replace(['-', ' '], "_").as_str() {
            "linear" | "lerp" => Some(Interpolation::Linear),
            "catmull_rom" | "catmullrom" | "cubic_spline" | "spline" | "cubic" => Some(Interpolation::CatmullRom),
            "hermite" => Some(Interpolation::Hermite),
            "ease_in" | "easein" => Some(Interpolation::EaseIn),
            "ease_out" | "easeout" => Some(Interpolation::EaseOut),
            "ease_in_out" | "easeinout" | "smooth" => Some(Interpolation::EaseInOut),
            "constant" | "step" | "hold" => Some(Interpolation::Constant),
            _ => None,
        }
    }

    /// Every mode, for the editor's dropdown.
    pub const ALL: [Interpolation; 7] = [
        Interpolation::Linear,
        Interpolation::CatmullRom,
        Interpolation::Hermite,
        Interpolation::EaseIn,
        Interpolation::EaseOut,
        Interpolation::EaseInOut,
        Interpolation::Constant,
    ];

    /// Apply the mode's time curve to a 0..=1 segment position.
    pub fn ease(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Interpolation::Constant => 0.0,
            Interpolation::EaseIn => t * t * t,
            Interpolation::EaseOut => {
                let u = 1.0 - t;
                1.0 - u * u * u
            }
            Interpolation::EaseInOut => t * t * (3.0 - 2.0 * t),
            // Curve shapes are handled by the position/scalar evaluators.
            Interpolation::Linear | Interpolation::CatmullRom | Interpolation::Hermite => t,
        }
    }
}

/// Linear interpolation of two scalars.
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Linear interpolation of two angles in degrees, along the shorter arc.
pub fn lerp_angle(a: f64, b: f64, t: f64) -> f64 {
    let mut delta = (b - a) % 360.0;
    if delta > 180.0 {
        delta -= 360.0;
    } else if delta < -180.0 {
        delta += 360.0;
    }
    a + delta * t
}

/// Wrap an angle into -180..=180.
pub fn wrap_degrees(angle: f64) -> f64 {
    let mut value = angle % 360.0;
    if value > 180.0 {
        value -= 360.0;
    } else if value < -180.0 {
        value += 360.0;
    }
    value
}

/// Clamp pitch to what a camera can look at without flipping over.
pub fn clamp_pitch(pitch: f64) -> f64 {
    pitch.clamp(-90.0, 90.0)
}

/// Catmull-Rom between `p1` and `p2` with neighbours `p0` and `p3`.
pub fn catmull_rom(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f64) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    let calc = |a: f64, b: f64, c: f64, d: f64| {
        0.5 * ((2.0 * b) + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3)
    };
    Vec3::new(
        calc(p0.x, p1.x, p2.x, p3.x),
        calc(p0.y, p1.y, p2.y, p3.y),
        calc(p0.z, p1.z, p2.z, p3.z),
    )
}

/// Hermite between `p1` and `p2` with explicit tangents `m1` and `m2`.
pub fn hermite(p1: Vec3, p2: Vec3, m1: Vec3, m2: Vec3, t: f64) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    p1.scale(h00).add(m1.scale(h10)).add(p2.scale(h01)).add(m2.scale(h11))
}

/// Hermite tangents from the neighbours, as Catmull-Rom derives them, scaled by
/// `tension` (0 flattens them into an ease at each keyframe).
pub fn hermite_tangents(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, tension: f64) -> (Vec3, Vec3) {
    let tension = tension.clamp(0.0, 1.0);
    let m1 = p2.sub(p0).scale(0.5 * tension);
    let m2 = p3.sub(p1).scale(0.5 * tension);
    (m1, m2)
}

/// Interpolate a scalar along a segment, honouring the segment's easing.
pub fn scalar(a: f64, b: f64, t: f64, mode: Interpolation) -> f64 {
    lerp(a, b, mode.ease(t))
}

/// Interpolate an angle along a segment, honouring the segment's easing.
pub fn angle(a: f64, b: f64, t: f64, mode: Interpolation) -> f64 {
    lerp_angle(a, b, mode.ease(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn easing_curves_hit_their_endpoints_and_clamp() {
        for mode in Interpolation::ALL {
            close(mode.ease(0.0), 0.0);
            if mode == Interpolation::Constant {
                close(mode.ease(1.0), 0.0);
            } else {
                close(mode.ease(1.0), 1.0);
            }
            close(mode.ease(-1.0), 0.0);
            close(mode.ease(2.0), mode.ease(1.0));
        }
    }

    #[test]
    fn ease_in_starts_slow_and_ease_out_starts_fast() {
        assert!(Interpolation::EaseIn.ease(0.25) < 0.25);
        assert!(Interpolation::EaseOut.ease(0.25) > 0.25);
        assert!(Interpolation::EaseInOut.ease(0.5) > 0.49 && Interpolation::EaseInOut.ease(0.5) < 0.51);
    }

    #[test]
    fn angles_take_the_short_way_round() {
        close(lerp_angle(350.0, 10.0, 0.5), 360.0);
        close(lerp_angle(10.0, 350.0, 0.5), 0.0);
        close(lerp_angle(-170.0, 170.0, 0.5), 180.0);
        close(wrap_degrees(370.0), 10.0);
        close(wrap_degrees(-190.0), 170.0);
        close(clamp_pitch(120.0), 90.0);
    }

    #[test]
    fn catmull_rom_passes_through_its_control_points() {
        let p0 = Vec3::new(0.0, 0.0, 0.0);
        let p1 = Vec3::new(1.0, 2.0, 3.0);
        let p2 = Vec3::new(4.0, 5.0, 6.0);
        let p3 = Vec3::new(7.0, 8.0, 9.0);
        let start = catmull_rom(p0, p1, p2, p3, 0.0);
        let end = catmull_rom(p0, p1, p2, p3, 1.0);
        close(start.x, 1.0);
        close(start.y, 2.0);
        close(end.x, 4.0);
        close(end.z, 6.0);
        // A straight line stays straight.
        let mid = catmull_rom(
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            0.5,
        );
        close(mid.x, 0.5);
        close(mid.y, 0.0);
    }

    #[test]
    fn hermite_with_full_tension_matches_catmull_rom() {
        let (p0, p1, p2, p3) = (
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.5, 0.0),
            Vec3::new(2.0, 2.0, -1.0),
            Vec3::new(3.0, 1.0, 4.0),
        );
        let (m1, m2) = hermite_tangents(p0, p1, p2, p3, 1.0);
        for step in 0..=10 {
            let t = step as f64 / 10.0;
            let a = catmull_rom(p0, p1, p2, p3, t);
            let b = hermite(p1, p2, m1, m2, t);
            assert!(a.sub(b).length() < 1e-9, "step {step}: {a:?} vs {b:?}");
        }
    }

    #[test]
    fn zero_tension_holds_still_at_both_ends() {
        let (m1, m2) = hermite_tangents(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(20.0, 0.0, 0.0),
            0.0,
        );
        close(m1.length(), 0.0);
        close(m2.length(), 0.0);
        let quarter = hermite(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), m1, m2, 0.25);
        // Ease at the start: less than a quarter of the way after a quarter of the time.
        assert!(quarter.x < 2.5);
    }

    #[test]
    fn ids_round_trip_and_accept_aliases() {
        for mode in Interpolation::ALL {
            assert_eq!(Interpolation::from_id(mode.id()), Some(mode));
        }
        assert_eq!(Interpolation::from_id("Cubic-Spline"), Some(Interpolation::CatmullRom));
        assert_eq!(Interpolation::from_id("step"), Some(Interpolation::Constant));
        assert_eq!(Interpolation::from_id("nonsense"), None);
    }

    #[test]
    fn scalars_follow_the_segment_easing() {
        close(scalar(0.0, 10.0, 0.5, Interpolation::Linear), 5.0);
        close(scalar(0.0, 10.0, 0.0, Interpolation::EaseInOut), 0.0);
        close(angle(0.0, 90.0, 0.5, Interpolation::Linear), 45.0);
    }
}
