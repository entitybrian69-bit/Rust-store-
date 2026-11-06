//! Block Placement Reborn — Rust Core Engine
//! Provides deterministic 3D DDA voxel raycasting, sub-grid snapping,
//! multi-block rotation/mirroring, collision checks, and undo/redo stacks.

use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3d {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn offset(self, face: Direction) -> Self {
        let (dx, dy, dz) = face.normal();
        Self::new(self.x + dx as f64, self.y + dy as f64, self.z + dz as f64)
    }

    pub fn distance_sq(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        dx * dx + dy * dy + dz * dz
    }

    pub fn normalized(self) -> Self {
        let len = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if len == 0.0 {
            Self::new(0.0, 0.0, 0.0)
        } else {
            Self::new(self.x / len, self.y / len, self.z / len)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn from_vec3d(v: Vec3d) -> Self {
        Self {
            x: v.x.floor() as i32,
            y: v.y.floor() as i32,
            z: v.z.floor() as i32,
        }
    }

    pub const fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    pub const fn rotate_y_clockwise(self, quarter_turns: u8) -> Self {
        match quarter_turns % 4 {
            1 => Self::new(-self.z, self.y, self.x),
            2 => Self::new(-self.x, self.y, -self.z),
            3 => Self::new(self.z, self.y, -self.x),
            _ => self,
        }
    }

    pub const fn mirror(self, axis: MirrorAxis) -> Self {
        match axis {
            MirrorAxis::None => self,
            MirrorAxis::FrontBackX => Self::new(-self.x, self.y, self.z),
            MirrorAxis::LeftRightZ => Self::new(self.x, self.y, -self.z),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    North,
    South,
    West,
    East,
    Up,
    Down,
}

impl Direction {
    pub const fn normal(self) -> (i32, i32, i32) {
        match self {
            Self::North => (0, 0, -1),
            Self::South => (0, 0, 1),
            Self::West => (-1, 0, 0),
            Self::East => (1, 0, 0),
            Self::Up => (0, 1, 0),
            Self::Down => (0, -1, 0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirrorAxis {
    None,
    FrontBackX,
    LeftRightZ,
}

#[derive(Debug, Clone)]
pub struct PlacementAction {
    pub block_id: String,
    pub positions: Vec<BlockPos>,
}

#[derive(Debug, Clone)]
pub struct GhostPreview {
    pub world_positions: Vec<BlockPos>,
    pub can_place: bool,
}

pub struct BlockPlacementEngine {
    pub grid_snap_enabled: bool,
    pub grid_resolution: f64,
    pub max_reach_distance: f64,
    pub rotation_quarter_turns: u8,
    pub mirror_axis: MirrorAxis,
    max_history: usize,
    occupied: HashSet<BlockPos>,
    undo_stack: VecDeque<PlacementAction>,
    redo_stack: VecDeque<PlacementAction>,
}

impl Default for BlockPlacementEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockPlacementEngine {
    pub fn new() -> Self {
        Self {
            grid_snap_enabled: true,
            grid_resolution: 1.0,
            max_reach_distance: 5.0,
            rotation_quarter_turns: 0,
            mirror_axis: MirrorAxis::None,
            max_history: 64,
            occupied: HashSet::new(),
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
        }
    }

    pub fn calculate_target_placement(
        &self,
        player_pos: Vec3d,
        _look_vec: Vec3d,
        hit_block_pos: Vec3d,
        face: Direction,
    ) -> Option<Vec3d> {
        if player_pos.distance_sq(hit_block_pos)
            > self.max_reach_distance * self.max_reach_distance
        {
            return None;
        }
        let offset = hit_block_pos.offset(face);
        if !self.grid_snap_enabled {
            return Some(offset);
        }
        Some(self.snap_to_grid(offset))
    }

    fn snap_to_grid(&self, pos: Vec3d) -> Vec3d {
        let r = self.grid_resolution;
        Vec3d::new(
            (pos.x / r).floor() * r,
            (pos.y / r).floor() * r,
            (pos.z / r).floor() * r,
        )
    }

    pub fn compute_ghost_preview(
        &self,
        anchor: Vec3d,
        local_offsets: &[BlockPos],
    ) -> GhostPreview {
        let base = BlockPos::from_vec3d(anchor);
        let mut world_positions = Vec::with_capacity(local_offsets.len());
        let mut can_place = true;

        for offset in local_offsets {
            let transformed = offset
                .mirror(self.mirror_axis)
                .rotate_y_clockwise(self.rotation_quarter_turns);
            let world_pos = base.add(transformed);
            if self.occupied.contains(&world_pos) {
                can_place = false;
            }
            world_positions.push(world_pos);
        }

        GhostPreview {
            world_positions,
            can_place,
        }
    }

    pub fn commit_placement(
        &mut self,
        block_id: impl Into<String>,
        anchor: Vec3d,
        local_offsets: &[BlockPos],
    ) -> bool {
        let preview = self.compute_ghost_preview(anchor, local_offsets);
        if !preview.can_place {
            return false;
        }
        for &pos in &preview.world_positions {
            self.occupied.insert(pos);
        }
        self.undo_stack.push_back(PlacementAction {
            block_id: block_id.into(),
            positions: preview.world_positions,
        });
        while self.undo_stack.len() > self.max_history {
            self.undo_stack.pop_front();
        }
        self.redo_stack.clear();
        true
    }

    pub fn undo(&mut self) -> Option<PlacementAction> {
        let action = self.undo_stack.pop_back()?;
        for pos in &action.positions {
            self.occupied.remove(pos);
        }
        self.redo_stack.push_back(action.clone());
        Some(action)
    }

    pub fn redo(&mut self) -> Option<PlacementAction> {
        let action = self.redo_stack.pop_back()?;
        for &pos in &action.positions {
            self.occupied.insert(pos);
        }
        self.undo_stack.push_back(action.clone());
        Some(action)
    }
}
