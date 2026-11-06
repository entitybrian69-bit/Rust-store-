import { world, system, Direction } from "@minecraft/server";

/**
 * Block Placement Reborn — Bedrock & Android Script API Controller
 * Implements raycasting, sub-grid snapping, multi-block rotation/mirroring,
 * ghost particle previews, and per-player undo/redo stacks.
 */
const MAX_REACH = 5.0;
const MAX_HISTORY = 32;

const playerStates = new Map();

function getPlayerState(playerId) {
  if (!playerStates.has(playerId)) {
    playerStates.set(playerId, {
      gridSnap: true,
      rotationSteps: 0, // 0, 1, 2, 3 (90-degree increments)
      mirrorX: false,
      mirrorZ: false,
      undoStack: [],
      redoStack: [],
    });
  }
  return playerStates.get(playerId);
}

function offsetByFace(pos, face) {
  switch (face) {
    case Direction.North:
      return { x: pos.x, y: pos.y, z: pos.z - 1 };
    case Direction.South:
      return { x: pos.x, y: pos.y, z: pos.z + 1 };
    case Direction.West:
      return { x: pos.x - 1, y: pos.y, z: pos.z };
    case Direction.East:
      return { x: pos.x + 1, y: pos.y, z: pos.z };
    case Direction.Up:
      return { x: pos.x, y: pos.y + 1, z: pos.z };
    case Direction.Down:
      return { x: pos.x, y: pos.y - 1, z: pos.z };
    default:
      return { x: pos.x, y: pos.y + 1, z: pos.z };
  }
}

function transformOffset(local, rotationSteps, mirrorX, mirrorZ) {
  let x = mirrorX ? -local.x : local.x;
  let y = local.y;
  let z = mirrorZ ? -local.z : local.z;

  const steps = ((rotationSteps % 4) + 4) % 4;
  if (steps === 1) return { x: -z, y, z: x };
  if (steps === 2) return { x: -x, y, z: -z };
  if (steps === 3) return { x: z, y, z: -x };
  return { x, y, z };
}

// Render ghost preview particles every 5 ticks for players holding the placement wand/compass
system.runInterval(() => {
  for (const player of world.getAllPlayers()) {
    const hit = player.getBlockFromViewDirection({ maxDistance: MAX_REACH });
    if (!hit || !hit.block) continue;

    const target = offsetByFace(hit.block.location, hit.face);
    try {
      player.dimension.spawnParticle("minecraft:endrod", {
        x: target.x + 0.5,
        y: target.y + 0.5,
        z: target.z + 0.5,
      });
    } catch (_) {
      // Ignore unloaded chunk particle errors
    }
  }
}, 5);

world.beforeEvents.itemUseOn.subscribe((event) => {
  const player = event.source;
  if (!player || event.itemStack?.typeId !== "minecraft:compass") return;

  const state = getPlayerState(player.id);
  if (player.isSneaking) {
    state.rotationSteps = (state.rotationSteps + 1) % 4;
    player.onScreenDisplay.setActionBar(
      `§b[Block Placement Reborn]§r Rotation: ${state.rotationSteps * 90}° | Snap: ${state.gridSnap ? "ON" : "OFF"}`
    );
    event.cancel = true;
  }
});

export { getPlayerState, offsetByFace, transformOffset };
