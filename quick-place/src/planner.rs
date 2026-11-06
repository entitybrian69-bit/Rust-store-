//! Pure placement planning.
//!
//! Nothing here touches the Horizon API: no imports of `horizon_mod_sdk`, no host
//! calls. It turns "a player just placed a block at `origin`" into an ordered list
//! of candidate positions. The caller checks each candidate against the world and
//! decides what to do, so every rule below is testable with `cargo test` on the
//! host, even if the SDK or the WebAssembly layer never builds.

/// A block position in the dimension the placement happened in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// `self + offset`, or `None` when that would leave the `i32` range.
    pub fn offset(self, dx: i32, dy: i32, dz: i32) -> Option<Self> {
        Some(Self {
            x: self.x.checked_add(dx)?,
            y: self.y.checked_add(dy)?,
            z: self.z.checked_add(dz)?,
        })
    }
}

/// What shape to sweep around the block that was placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The eight horizontal neighbours in a 3x3 ring (floors, walls, platforms).
    Pad,
    /// A straight run away from the player's side of the placed block.
    Line,
    /// A column upward from the placed block.
    Pillar,
    /// All 26 neighbours (solid fills).
    Cube,
}

impl Mode {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "pad" => Some(Self::Pad),
            "line" => Some(Self::Line),
            "pillar" => Some(Self::Pillar),
            "cube" => Some(Self::Cube),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Pad => "pad",
            Self::Line => "line",
            Self::Pillar => "pillar",
            Self::Cube => "cube",
        }
    }
}

/// How `Mode::Line` picks its direction when the player's yaw is not available.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// The horizontal axis the player stands furthest along, seen from the block.
    Auto,
    /// Always the x axis.
    X,
    /// Always the z axis.
    Z,
}

/// One sweep request. `reach` is how far `Line` and `Pillar` run; `limit` is the
/// hard bound on how many candidates come back (the host's own cap is applied by
/// the caller, which also drops candidates that are not free).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub origin: Pos,
    pub player: Pos,
    pub mode: Mode,
    pub axis: Axis,
    pub reach: i32,
    pub limit: u32,
}

/// The hard bound on planned candidates, whatever the request asks for.
pub const MAX_CANDIDATES: u32 = 64;
/// The furthest `Line` and `Pillar` may run.
pub const MAX_REACH: i32 = 16;

/// `player - origin` on the x and z axes, which is all a line needs.
fn delta(origin: Pos, player: Pos) -> (i64, i64) {
    (
        i64::from(player.x) - i64::from(origin.x),
        i64::from(player.z) - i64::from(origin.z),
    )
}

/// The unit step a `Line` takes, away from the player.
///
/// The legacy server API exposes no yaw, so the direction comes from where the
/// player stands: the axis they are furthest along wins, and the line runs the
/// other way, out of their side of the block. A player standing exactly on the
/// block (or on the diagonal) falls back to the configured axis, then `+z`.
pub fn line_step(origin: Pos, player: Pos, axis: Axis) -> (i32, i32) {
    let (dx, dz) = delta(origin, player);
    let use_x = match axis {
        Axis::X => true,
        Axis::Z => false,
        Axis::Auto => dx.abs() > dz.abs() || (dx.abs() == dz.abs() && dz == 0 && dx != 0),
    };
    let step = if use_x {
        if dx < 0 {
            1
        } else {
            -1
        }
    } else if dz < 0 {
        1
    } else {
        -1
    };
    if use_x {
        (step, 0)
    } else {
        (0, step)
    }
}

/// The ordered candidates for one placed block.
///
/// Order is part of the contract: the caller stops at its own cap, so the first
/// candidates are the ones a player expects. `Pad` runs north, north-east, east,
/// south-east, south, south-west, west, north-west (−z is north). `Cube` runs by
/// y, then z, then x, skipping the origin. Positions that would overflow `i32`
/// are left out. Duplicates never occur, and the origin is never a candidate.
pub fn plan(request: &Request) -> Vec<Pos> {
    let limit = request.limit.min(MAX_CANDIDATES) as usize;
    if limit == 0 {
        return Vec::new();
    }
    let reach = request.reach.clamp(0, MAX_REACH);
    let origin = request.origin;
    let mut out: Vec<Pos> = Vec::with_capacity(limit);

    match request.mode {
        Mode::Pad => {
            const RING: [(i32, i32); 8] = [
                (0, -1),
                (1, -1),
                (1, 0),
                (1, 1),
                (0, 1),
                (-1, 1),
                (-1, 0),
                (-1, -1),
            ];
            for (dx, dz) in RING {
                if out.len() == limit {
                    break;
                }
                if let Some(pos) = origin.offset(dx, 0, dz) {
                    out.push(pos);
                }
            }
        }
        Mode::Line => {
            let (sx, sz) = line_step(origin, request.player, request.axis);
            for step in 1..=reach {
                if out.len() == limit {
                    break;
                }
                if let Some(pos) = origin.offset(sx * step, 0, sz * step) {
                    out.push(pos);
                }
            }
        }
        Mode::Pillar => {
            for step in 1..=reach {
                if out.len() == limit {
                    break;
                }
                if let Some(pos) = origin.offset(0, step, 0) {
                    out.push(pos);
                }
            }
        }
        Mode::Cube => {
            'shells: for dy in -1..=1 {
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 && dz == 0 {
                            continue;
                        }
                        if out.len() == limit {
                            break 'shells;
                        }
                        if let Some(pos) = origin.offset(dx, dy, dz) {
                            out.push(pos);
                        }
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(mode: Mode, limit: u32) -> Request {
        Request {
            origin: Pos::new(0, 64, 0),
            player: Pos::new(0, 64, -3),
            mode,
            axis: Axis::Auto,
            reach: 3,
            limit,
        }
    }

    #[test]
    fn pad_is_the_eight_neighbours_in_a_fixed_order() {
        let got = plan(&request(Mode::Pad, 8));
        assert_eq!(
            got,
            vec![
                Pos::new(0, 64, -1),
                Pos::new(1, 64, -1),
                Pos::new(1, 64, 0),
                Pos::new(1, 64, 1),
                Pos::new(0, 64, 1),
                Pos::new(-1, 64, 1),
                Pos::new(-1, 64, 0),
                Pos::new(-1, 64, -1),
            ]
        );
    }

    #[test]
    fn pad_respects_the_limit_and_never_returns_the_origin() {
        let got = plan(&request(Mode::Pad, 3));
        assert_eq!(got.len(), 3);
        assert!(!got.contains(&Pos::new(0, 64, 0)));
        assert!(plan(&request(Mode::Pad, 0)).is_empty());
    }

    #[test]
    fn line_runs_away_from_the_player() {
        // The player stands to the north (−z): the line goes south.
        let mut req = request(Mode::Line, 3);
        assert_eq!(
            plan(&req),
            vec![Pos::new(0, 64, 1), Pos::new(0, 64, 2), Pos::new(0, 64, 3)]
        );
        // The player stands to the east (+x): the line goes west.
        req.player = Pos::new(5, 64, 0);
        assert_eq!(
            plan(&req),
            vec![Pos::new(-1, 64, 0), Pos::new(-2, 64, 0), Pos::new(-3, 64, 0)]
        );
        // The furthest axis wins, even on a diagonal.
        req.player = Pos::new(4, 64, -1);
        assert_eq!(plan(&req)[0], Pos::new(-1, 64, 0));
        // A configured axis overrides the geometry.
        req.axis = Axis::Z;
        assert_eq!(plan(&req)[0], Pos::new(0, 64, 1));
    }

    #[test]
    fn line_and_pillar_stop_at_reach() {
        let mut req = request(Mode::Line, 64);
        req.reach = 2;
        assert_eq!(plan(&req).len(), 2);
        req.mode = Mode::Pillar;
        assert_eq!(plan(&req), vec![Pos::new(0, 65, 0), Pos::new(0, 66, 0)]);
        req.reach = 0;
        assert!(plan(&req).is_empty());
    }

    #[test]
    fn cube_is_the_twenty_six_neighbours_without_the_origin() {
        let got = plan(&request(Mode::Cube, 64));
        assert_eq!(got.len(), 26);
        assert!(!got.contains(&Pos::new(0, 64, 0)));
        assert_eq!(got[0], Pos::new(-1, 63, -1));
        assert_eq!(got[25], Pos::new(1, 65, 1));
    }

    #[test]
    fn candidates_are_capped_and_never_overflow() {
        let mut req = request(Mode::Cube, u32::MAX);
        assert_eq!(plan(&req).len(), MAX_CANDIDATES as usize);
        req.origin = Pos::new(i32::MAX, i32::MAX, i32::MAX);
        req.mode = Mode::Pad;
        assert!(plan(&req).is_empty());
        req.mode = Mode::Pillar;
        assert!(plan(&req).is_empty());
    }

    #[test]
    fn modes_round_trip_through_their_names() {
        for mode in [Mode::Pad, Mode::Line, Mode::Pillar, Mode::Cube] {
            assert_eq!(Mode::parse(mode.name()), Some(mode));
        }
        assert_eq!(Mode::parse("diagonal"), None);
    }
}
