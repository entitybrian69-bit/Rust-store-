//! Camera-path interpolation for `/vl path` and for future client playback.
//!
//! Pure maths: no SDK calls. The legacy server API exposes no camera, so this
//! module is what a client-side player would consume once one exists
//! (`TODO(api)` in the README); the server uses it to report the path a replay
//! describes.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraKeyframe {
    pub tick: u64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interpolation {
    Linear,
    CatmullRom,
    Bezier,
}

impl Interpolation {
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "linear" => Some(Self::Linear),
            "catmull_rom" | "catmullrom" => Some(Self::CatmullRom),
            "bezier" => Some(Self::Bezier),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::CatmullRom => "catmull_rom",
            Self::Bezier => "bezier",
        }
    }
}

fn lerp(a: f64, b: f64, u: f64) -> f64 {
    a + (b - a) * u
}

fn hermite(p1: f64, m0: f64, p2: f64, m1: f64, u: f64) -> f64 {
    let u2 = u * u;
    let u3 = u2 * u;
    (2.0 * u3 - 3.0 * u2 + 1.0) * p1
        + (u3 - 2.0 * u2 + u) * m0
        + (-2.0 * u3 + 3.0 * u2) * p2
        + (u3 - u2) * m1
}

fn axis(
    keys: &[CameraKeyframe],
    index: usize,
    get: impl Fn(&CameraKeyframe) -> f64,
) -> f64 {
    get(&keys[index])
}

/// The camera position at `tick`, clamped to the path's ends.
///
/// `tension` (0..=1) scales the Catmull-Rom tangents (0.5 is the classic
/// Catmull-Rom); `smoothing` (0..=1) pulls the Bézier control points towards the
/// neighbouring keyframes, so 0 keeps every segment straight. Both are ignored
/// by `linear`, which is exactly what the name says. The neighbours of the first
/// and last segments are extrapolated, so evenly spaced collinear keyframes stay
/// exactly on the line.
pub fn sample(
    keys: &[CameraKeyframe],
    tick: u64,
    mode: Interpolation,
    tension: f64,
    smoothing: f64,
) -> Option<(f64, f64, f64)> {
    let first = keys.first()?;
    let last = keys.last()?;
    if keys.len() == 1 || tick <= first.tick {
        return Some((first.x, first.y, first.z));
    }
    if tick >= last.tick {
        return Some((last.x, last.y, last.z));
    }
    // The segment [i, i + 1] that holds `tick`.
    let mut index = 0;
    while index + 1 < keys.len() && keys[index + 1].tick <= tick {
        index += 1;
    }
    let k0 = keys[index];
    let k1 = keys[index + 1];
    let span = (k1.tick - k0.tick) as f64;
    let u = if span <= 0.0 {
        1.0
    } else {
        ((tick - k0.tick) as f64 / span).clamp(0.0, 1.0)
    };
    let tension = tension.clamp(0.0, 1.0);
    let smoothing = smoothing.clamp(0.0, 1.0);

    let mut out = (0.0, 0.0, 0.0);
    for (slot, get) in [
        (0usize, (|k: &CameraKeyframe| k.x) as fn(&CameraKeyframe) -> f64),
        (1usize, |k: &CameraKeyframe| k.y),
        (2usize, |k: &CameraKeyframe| k.z),
    ] {
        let p1 = axis(keys, index, get);
        let p2 = axis(keys, index + 1, get);
        // A phantom neighbour on either side, so the first and last segments are
        // shaped like the ones inside the path.
        let p0 = if index == 0 {
            2.0 * p1 - p2
        } else {
            axis(keys, index - 1, get)
        };
        let p3 = if index + 2 < keys.len() {
            axis(keys, index + 2, get)
        } else {
            2.0 * p2 - p1
        };
        let value = match mode {
            Interpolation::Linear => lerp(p1, p2, u),
            Interpolation::CatmullRom => {
                let m0 = (p2 - p0) * tension;
                let m1 = (p3 - p1) * tension;
                hermite(p1, m0, p2, m1, u)
            }
            Interpolation::Bezier => {
                // Control points pulled towards the neighbouring keyframes by
                // `smoothing`: 0 keeps the segment straight, 1 lets the
                // neighbours bend it fully.
                let c1 = p1 + (p0 - p1) * smoothing / 3.0;
                let c2 = p2 + (p3 - p2) * smoothing / 3.0;
                let v = 1.0 - u;
                v * v * v * p1 + 3.0 * v * v * u * c1 + 3.0 * v * u * u * c2 + u * u * u * p2
            }
        };
        match slot {
            0 => out.0 = value,
            1 => out.1 = value,
            _ => out.2 = value,
        }
    }
    Some(out)
}

/// `(tick, x, y, z)` samples from `from` to `to`, one per `step` ticks, always
/// including both ends. Bounded so a huge range cannot build a huge list.
pub fn preview(
    keys: &[CameraKeyframe],
    from: u64,
    to: u64,
    step: u64,
    mode: Interpolation,
    tension: f64,
    smoothing: f64,
) -> Vec<(u64, f64, f64, f64)> {
    let mut out = Vec::new();
    if keys.is_empty() || to < from {
        return out;
    }
    let step = step.max(1);
    let mut tick = from;
    // One slot is kept for the closing sample, so the result never exceeds 4096.
    while tick <= to && out.len() < 4095 {
        if let Some((x, y, z)) = sample(keys, tick, mode, tension, smoothing) {
            out.push((tick, x, y, z));
        }
        tick += step;
    }
    if out.last().map(|last| last.0) != Some(to) && out.len() < 4096 {
        if let Some((x, y, z)) = sample(keys, to, mode, tension, smoothing) {
            out.push((to, x, y, z));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(tick: u64, x: f64, y: f64, z: f64) -> CameraKeyframe {
        CameraKeyframe { tick, x, y, z }
    }

    fn line() -> Vec<CameraKeyframe> {
        vec![
            key(0, 0.0, 64.0, 0.0),
            key(10, 10.0, 64.0, 0.0),
            key(20, 20.0, 64.0, 0.0),
        ]
    }

    #[test]
    fn ends_are_exact_and_outside_is_clamped() {
        let keys = line();
        for mode in [
            Interpolation::Linear,
            Interpolation::CatmullRom,
            Interpolation::Bezier,
        ] {
            assert_eq!(sample(&keys, 0, mode, 0.5, 0.5), Some((0.0, 64.0, 0.0)));
            let middle = sample(&keys, 5, mode, 0.5, 0.5).expect("a midpoint");
            assert!(
                (middle.0 - 5.0).abs() < 1e-9,
                "{} at tick 5: {middle:?}",
                mode.name()
            );
            assert!((middle.1 - 64.0).abs() < 1e-9);
            assert_eq!(sample(&keys, 20, mode, 0.5, 0.5), Some((20.0, 64.0, 0.0)));
            assert_eq!(sample(&keys, 0, mode, 0.5, 0.5), Some((0.0, 64.0, 0.0)));
            assert_eq!(sample(&keys, 999, mode, 0.5, 0.5), Some((20.0, 64.0, 0.0)));
        }
        assert_eq!(sample(&keys, 0, Interpolation::Linear, 0.5, 0.5), Some((0.0, 64.0, 0.0)));
    }

    #[test]
    fn collinear_evenly_spaced_points_stay_on_the_line() {
        let keys = line();
        for (tick, expected) in [(2, 2.0), (5, 5.0), (7, 7.0), (15, 15.0)] {
            let got = sample(&keys, tick, Interpolation::CatmullRom, 0.5, 0.5).unwrap();
            assert!((got.0 - expected).abs() < 1e-9, "tick {tick}: {got:?}");
            assert!((got.1 - 64.0).abs() < 1e-9);
        }
    }

    #[test]
    fn one_keyframe_and_no_keyframes() {
        let single = vec![key(5, 1.0, 2.0, 3.0)];
        assert_eq!(sample(&single, 0, Interpolation::Bezier, 0.5, 0.5), Some((1.0, 2.0, 3.0)));
        assert_eq!(sample(&[], 0, Interpolation::Bezier, 0.5, 0.5), None);
    }

    #[test]
    fn bezier_smoothing_is_bounded_by_the_neighbours() {
        let keys = vec![
            key(0, 0.0, 0.0, 0.0),
            key(10, 0.0, 10.0, 0.0),
            key(20, 10.0, 10.0, 0.0),
        ];
        let straight = sample(&keys, 5, Interpolation::Bezier, 0.5, 0.0).unwrap();
        assert!((straight.0 - 0.0).abs() < 1e-9, "smoothing 0 keeps x: {straight:?}");
        let bent = sample(&keys, 5, Interpolation::Bezier, 0.5, 1.0).unwrap();
        assert!(bent.0 > 0.5, "smoothing 1 bends towards the next segment: {bent:?}");
    }

    #[test]
    fn preview_hits_both_ends_and_is_bounded() {
        let keys = line();
        let path = preview(&keys, 0, 20, 4, Interpolation::Linear, 0.5, 0.5);
        assert_eq!(path.first().map(|p| p.0), Some(0));
        assert_eq!(path.last().map(|p| p.0), Some(20));
        assert_eq!(path.len(), 6);
        let long = preview(&keys, 0, 100_000, 1, Interpolation::Linear, 0.5, 0.5);
        assert_eq!(long.len(), 4096, "the list stays bounded");
        assert!(preview(&keys, 10, 5, 1, Interpolation::Linear, 0.5, 0.5).is_empty());
    }

    #[test]
    fn names_round_trip() {
        for mode in [
            Interpolation::Linear,
            Interpolation::CatmullRom,
            Interpolation::Bezier,
        ] {
            assert_eq!(Interpolation::parse(mode.name()), Some(mode));
        }
        assert_eq!(Interpolation::parse("Catmull-Rom"), Some(Interpolation::CatmullRom));
        assert_eq!(Interpolation::parse("spline"), None);
    }
}
