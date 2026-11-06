package com.blockplacement;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Deque;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/**
 * Block Placement Reborn Logic
 * Core implementation for accurate placement, raycasting, grid snapping,
 * multi-block rotation/mirroring, collision checking, ghost previews, and undo/redo state management.
 */
public class BlockPlacementReborn {

    private boolean gridSnapEnabled = true;
    private double maxReachDistance = 5.0;
    private double gridResolution = 1.0; // 1.0 = full block grid, 0.5 = half-block slab/stair sub-grid
    private int rotationQuarterTurns = 0; // 0, 1 (90 deg), 2 (180 deg), 3 (270 deg)
    private MirrorAxis activeMirror = MirrorAxis.NONE;

    private static final int MAX_HISTORY_SIZE = 64;
    private final Deque<PlacementAction> undoStack = new ArrayDeque<>();
    private final Deque<PlacementAction> redoStack = new ArrayDeque<>();
    private final Set<BlockPos> occupiedBlocks = new HashSet<>();

    public void onEnable() {
        System.out.println("Block Placement Reborn initialized successfully!");
    }

    public void onDisable() {
        undoStack.clear();
        redoStack.clear();
        System.out.println("Block Placement Reborn disabled.");
    }

    /**
     * Primary entry point matching the embedded .hmod specification.
     */
    public Vector3d calculateTargetPlacement(Vector3d playerPos, Vector3d lookVec, Vector3d hitBlockPos, Direction face) {
        if (playerPos.distanceSquaredTo(hitBlockPos) > maxReachDistance * maxReachDistance) {
            return null; // Out of player reach
        }
        if (!gridSnapEnabled) {
            return hitBlockPos.offset(face);
        }
        return snapToGrid(hitBlockPos, face);
    }

    private Vector3d snapToGrid(Vector3d hitPos, Direction face) {
        Vector3d offsetPos = hitPos.offset(face);
        double snappedX = Math.floor(offsetPos.x / gridResolution) * gridResolution;
        double snappedY = Math.floor(offsetPos.y / gridResolution) * gridResolution;
        double snappedZ = Math.floor(offsetPos.z / gridResolution) * gridResolution;
        return new Vector3d(snappedX, snappedY, snappedZ);
    }

    /**
     * Performs 3D DDA voxel raycasting from player eye position along look vector.
     */
    public RaycastHit raycastVoxel(Vector3d eyePos, Vector3d lookVec) {
        Vector3d dir = lookVec.normalize();
        int x = (int) Math.floor(eyePos.x);
        int y = (int) Math.floor(eyePos.y);
        int z = (int) Math.floor(eyePos.z);

        int stepX = dir.x >= 0 ? 1 : -1;
        int stepY = dir.y >= 0 ? 1 : -1;
        int stepZ = dir.z >= 0 ? 1 : -1;

        double tMaxX = intBound(eyePos.x, dir.x);
        double tMaxY = intBound(eyePos.y, dir.y);
        double tMaxZ = intBound(eyePos.z, dir.z);

        double tDeltaX = dir.x == 0 ? Double.MAX_VALUE : Math.abs(1.0 / dir.x);
        double tDeltaY = dir.y == 0 ? Double.MAX_VALUE : Math.abs(1.0 / dir.y);
        double tDeltaZ = dir.z == 0 ? Double.MAX_VALUE : Math.abs(1.0 / dir.z);

        Direction hitFace = Direction.UP;
        double distance = 0.0;

        while (distance <= maxReachDistance) {
            BlockPos current = new BlockPos(x, y, z);
            if (occupiedBlocks.contains(current)) {
                return new RaycastHit(new Vector3d(x, y, z), hitFace, distance);
            }
            if (tMaxX < tMaxY) {
                if (tMaxX < tMaxZ) {
                    x += stepX;
                    distance = tMaxX;
                    tMaxX += tDeltaX;
                    hitFace = stepX > 0 ? Direction.WEST : Direction.EAST;
                } else {
                    z += stepZ;
                    distance = tMaxZ;
                    tMaxZ += tDeltaZ;
                    hitFace = stepZ > 0 ? Direction.NORTH : Direction.SOUTH;
                }
            } else {
                if (tMaxY < tMaxZ) {
                    y += stepY;
                    distance = tMaxY;
                    tMaxY += tDeltaY;
                    hitFace = stepY > 0 ? Direction.DOWN : Direction.UP;
                } else {
                    z += stepZ;
                    distance = tMaxZ;
                    tMaxZ += tDeltaZ;
                    hitFace = stepZ > 0 ? Direction.NORTH : Direction.SOUTH;
                }
            }
        }
        return null;
    }

    private double intBound(double s, double ds) {
        if (ds == 0) {
            return Double.MAX_VALUE;
        }
        double sIsInteger = Math.rint(s) == s ? 0.0 : 0.0;
        if (ds > 0) {
            return (Math.floor(s + 1.0) - s + sIsInteger) / ds;
        } else {
            return (s - Math.floor(s) + sIsInteger) / -ds;
        }
    }

    /**
     * Computes ghost-block preview positions for a multi-block structure with rotation and mirroring.
     */
    public GhostPreview computeGhostPreview(Vector3d anchor, List<BlockPos> localOffsets) {
        BlockPos base = BlockPos.fromVector(anchor);
        List<BlockPos> transformed = new ArrayList<>(localOffsets.size());
        boolean allValid = true;

        for (BlockPos offset : localOffsets) {
            BlockPos mirrored = offset.mirror(activeMirror);
            BlockPos rotated = mirrored.rotateY(rotationQuarterTurns);
            BlockPos worldPos = base.add(rotated);
            transformed.add(worldPos);
            if (occupiedBlocks.contains(worldPos)) {
                allValid = false;
            }
        }
        return new GhostPreview(transformed, allValid);
    }

    /**
     * Commits a multi-block placement if collision checks pass, recording the action in the Undo stack.
     */
    public boolean commitPlacement(String blockId, Vector3d anchor, List<BlockPos> localOffsets) {
        GhostPreview preview = computeGhostPreview(anchor, localOffsets);
        if (!preview.canPlace) {
            return false;
        }
        occupiedBlocks.addAll(preview.worldPositions);
        PlacementAction action = new PlacementAction(blockId, preview.worldPositions);
        undoStack.push(action);
        while (undoStack.size() > MAX_HISTORY_SIZE) {
            undoStack.removeLast();
        }
        redoStack.clear();
        return true;
    }

    public PlacementAction undo() {
        if (undoStack.isEmpty()) {
            return null;
        }
        PlacementAction action = undoStack.pop();
        for (BlockPos pos : action.positions) {
            occupiedBlocks.remove(pos);
        }
        redoStack.push(action);
        return action;
    }

    public PlacementAction redo() {
        if (redoStack.isEmpty()) {
            return null;
        }
        PlacementAction action = redoStack.pop();
        occupiedBlocks.addAll(action.positions);
        undoStack.push(action);
        return action;
    }

    public void rotateClockwise() {
        rotationQuarterTurns = (rotationQuarterTurns + 1) & 3;
    }

    public void setActiveMirror(MirrorAxis axis) {
        this.activeMirror = axis;
    }

    public void setGridSnapEnabled(boolean enabled) {
        this.gridSnapEnabled = enabled;
    }

    public void setMaxReachDistance(double reach) {
        this.maxReachDistance = reach;
    }
}

class Vector3d {
    public double x, y, z;

    public Vector3d(double x, double y, double z) {
        this.x = x;
        this.y = y;
        this.z = z;
    }

    public Vector3d offset(Direction face) {
        return new Vector3d(x + face.x, y + face.y, z + face.z);
    }

    public double distanceSquaredTo(Vector3d other) {
        double dx = x - other.x;
        double dy = y - other.y;
        double dz = z - other.z;
        return dx * dx + dy * dy + dz * dz;
    }

    public Vector3d normalize() {
        double len = Math.sqrt(x * x + y * y + z * z);
        if (len == 0.0) {
            return new Vector3d(0, 0, 0);
        }
        return new Vector3d(x / len, y / len, z / len);
    }
}

enum Direction {
    NORTH(0, 0, -1),
    SOUTH(0, 0, 1),
    WEST(-1, 0, 0),
    EAST(1, 0, 0),
    UP(0, 1, 0),
    DOWN(0, -1, 0);

    public final int x, y, z;

    Direction(int x, int y, int z) {
        this.x = x;
        this.y = y;
        this.z = z;
    }
}

enum MirrorAxis {
    NONE,
    FRONT_BACK_X,
    LEFT_RIGHT_Z
}

final class BlockPos {
    public final int x, y, z;

    public BlockPos(int x, int y, int z) {
        this.x = x;
        this.y = y;
        this.z = z;
    }

    public static BlockPos fromVector(Vector3d v) {
        return new BlockPos((int) Math.floor(v.x), (int) Math.floor(v.y), (int) Math.floor(v.z));
    }

    public BlockPos add(BlockPos other) {
        return new BlockPos(x + other.x, y + other.y, z + other.z);
    }

    public BlockPos rotateY(int quarterTurnsClockwise) {
        int turns = ((quarterTurnsClockwise % 4) + 4) % 4;
        switch (turns) {
            case 1:
                return new BlockPos(-z, y, x);
            case 2:
                return new BlockPos(-x, y, -z);
            case 3:
                return new BlockPos(z, y, -x);
            default:
                return this;
        }
    }

    public BlockPos mirror(MirrorAxis axis) {
        switch (axis) {
            case FRONT_BACK_X:
                return new BlockPos(-x, y, z);
            case LEFT_RIGHT_Z:
                return new BlockPos(x, y, -z);
            default:
                return this;
        }
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof BlockPos)) return false;
        BlockPos that = (BlockPos) o;
        return x == that.x && y == that.y && z == that.z;
    }

    @Override
    public int hashCode() {
        return (x * 73856093) ^ (y * 19349663) ^ (z * 83492791);
    }
}

final class RaycastHit {
    public final Vector3d blockPos;
    public final Direction face;
    public final double distance;

    public RaycastHit(Vector3d blockPos, Direction face, double distance) {
        this.blockPos = blockPos;
        this.face = face;
        this.distance = distance;
    }
}

final class GhostPreview {
    public final List<BlockPos> worldPositions;
    public final boolean canPlace;

    public GhostPreview(List<BlockPos> worldPositions, boolean canPlace) {
        this.worldPositions = Collections.unmodifiableList(worldPositions);
        this.canPlace = canPlace;
    }
}

final class PlacementAction {
    public final String blockId;
    public final List<BlockPos> positions;

    public PlacementAction(String blockId, List<BlockPos> positions) {
        this.blockId = blockId;
        this.positions = Collections.unmodifiableList(new ArrayList<>(positions));
    }
}
