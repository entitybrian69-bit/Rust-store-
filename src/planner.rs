// Pure planning: no SDK calls or host access.
/// A position in block coordinates. This planner is independent of Horizon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pos { pub x: i32, pub y: i32, pub z: i32 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing { North, East, South, West }

/// Minecraft yaw: 0 = south, 90 = west, 180 = north, 270 = east.
/// Non-finite angles do not identify a direction.
pub fn facing_from_yaw(yaw: f32) -> Option<Facing> {
    if !yaw.is_finite() { return None; }
    let yaw = yaw.rem_euclid(360.0);
    Some(if !(45.0..315.0).contains(&yaw) { Facing::South }
    else if yaw < 135.0 { Facing::West }
    else if yaw < 225.0 { Facing::North }
    else { Facing::East })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Line, Plane }

/// Plan at most `limit` candidates *in addition to* the player's block.
/// Plane is a forward, horizontal strip: center, left, right on each row.
/// `replaceable` must come from a trusted host check; unloaded positions must
/// return false. Invalid offsets are skipped. The result does not modify the
/// world. The limit applies to candidate positions, not successful placements.
pub fn plan<F>(origin: Pos, facing: Facing, mode: Mode, limit: u32, mut replaceable: F) -> Vec<Pos>
where F: FnMut(Pos) -> bool {
    let (dx, dz) = match facing {
        Facing::North => (0_i32, -1_i32), Facing::East => (1, 0),
        Facing::South => (0, 1), Facing::West => (-1, 0),
    };
    let mut out = Vec::new();
    // 64 is a hard planning bound, independent of the host's quota.
    for i in 0..limit.min(64) {
        let (forward, side) = match mode {
            Mode::Line => (i as i32 + 1, 0),
            Mode::Plane => (i as i32 / 3 + 1, [0_i32, -1, 1][i as usize % 3]),
        };
        let ox = dx * forward + dz * side;
        let oz = dz * forward - dx * side;
        let Some(x) = origin.x.checked_add(ox) else { continue };
        let Some(z) = origin.z.checked_add(oz) else { continue };
        let p = Pos { x, y: origin.y, z };
        if replaceable(p) { out.push(p); }
    }
    out
}
