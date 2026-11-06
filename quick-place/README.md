# Quick Place — legacy WebAssembly `.hmod` (prototype)

`quick_place.hmod` is a **real, toolchain-built** Horizon mod: `cargo horizon-mod build`
(pinned to the last Horizon revision with the WebAssembly server runtime) compiles
`quick-place/src/` to `wasm32-unknown-unknown`, componentizes it, and packs the `.hmod`
with Horizon's own `horizon-mods` packer. Nothing in the archive is hand-assembled.

**Read this before using it:**

- **WASM-only.** It is a legacy WebAssembly server mod (`api = "0.3"`, `side = "server"`),
  not a native mixin bundle.
- **Not compatible with current PC Horizon.** Upstream removed the WebAssembly mod runtime
  in commit [`e7614d2197ebe9ffdaa75e1bbb432dc0ce855e70`](https://github.com/horizon-rust/horizon/commit/e7614d2197ebe9ffdaa75e1bbb432dc0ce855e70)
  (2026-10-29). Current builds refuse `mod.wasm` (`docs/api/mod-files.md`: "a legacy
  WebAssembly mod: it doesn't run any more"). It only runs on a host that still has the
  legacy runtime at the pinned revision.
- **Not tested in a game on PC or on Android.** It is verified by the pinned toolchain's
  own sandbox (`cargo horizon-mod test`), which drives the real host ABI without a window.
  That is not a gameplay test.
- **Cross-platform by construction, not by test.** The guest is plain `wasm32` with no
  OS-specific code, no filesystem, no threads, no input assumptions — so the `.hmod` is
  the same file everywhere a compatible legacy host exists. No Android runtime at this
  revision has been verified.
- **Prototype.** No registry publication. Do not use it on worlds you care about.

## Pinned toolchain

| What | Pin |
| --- | --- |
| Horizon (SDK, host ABI, `cargo-horizon-mod`) | [`62a01db8f006b422e6db24e34dcee05ad41760ba`](https://github.com/horizon-rust/horizon/tree/62a01db8f006b422e6db24e34dcee05ad41760ba), committed **2026-10-29 04:37:12 UTC** — the parent of the commit that removed the runtime |
| Rust | `1.99.1`, target `wasm32-unknown-unknown` (`rust-toolchain.toml`) |
| Packer | `cargo horizon-mod build` from that revision, installed with `--locked` |
| Mod API | `api = "0.3"`, `side = "server"`, capabilities `world` + `players` |

### Relaxations (and why each is required)

1. **Legacy runtime pin instead of current Horizon** — the current engine has no
   WebAssembly mod support at all; a WASM artifact can only be built and loaded against
   the pre-removal revision.
2. **`api = "0.3"`, `side = "server"`** — the block-placed hook and world access exist only
   on the experimental server-mod line.
3. **`capabilities = ["world", "players"]`** — `world` for `get-block`/`set-blocks`,
   `players` for the placer's id/dimension and one feedback line. `game-read` is
   client-only and is rejected for server mods, so player rotation is unavailable (below).
4. **`cdylib` + `rlib`** — `cdylib` is what the builder componentizes; `rlib` lets the pure
   planner and config parser run under `cargo test` on the host.
5. **Config compiled in** (`include_str!`) — the legacy API has no filesystem access, so
   `config.toml` is baked into the module at build time; edit and rebuild to change it.

## What the hook does

`block_placed(player, pos, state)` fires after a player places a block. The mod:

1. plans the candidate positions for `placement_mode` (`src/planner.rs`, pure Rust);
2. asks `world.get-block` for each candidate and keeps only `minecraft:air`;
3. writes the placed block **state** into at most `max_blocks_per_sweep` of them with
   `world.set-blocks`;
4. tells the player `Quick Place: placed N block(s) with <mode> mode`.

A non-air neighbour, `cave_air`/`void_air`, water, or a position in an unloaded chunk is
**never** overwritten: the air check happens immediately before the write is queued.

### `config.toml`

| Setting | Default | Meaning |
| --- | --- | --- |
| `enabled` | `true` | `false` loads the mod and its command but never places anything |
| `max_blocks_per_sweep` | `4` | Blocks added per placement, `0..=64` (0 disables writes) |
| `require_sneaking` | `false` | **Inert** — API 0.3 exposes no sneaking state; `true` logs a warning and is ignored (fail-closed documented, not faked) |
| `placement_mode` | `"pad"` | `pad` (3×3 ring), `line` (runs away from the player), `pillar` (column up), `cube` (26 neighbours) |

`line`/`pillar` reach 3 blocks. Unknown, duplicated or missing settings are a load error.

### Command

`/quickplace` (or `/quickplace status`) reports the settings and how many blocks the
session has placed; `/quickplace <pad|line|pillar|cube>` switches mode for the session.

## API gaps (`TODO(api)`)

- **No yaw on the server `Player`.** `line` derives its direction from the player's
  position relative to the placed block (`planner::line_step`); it is a heuristic, not the
  player's look direction.
- **No sneaking state.** `require_sneaking` cannot be honoured and is inert.
- **No replaceability predicate.** Only exactly `minecraft:air` is treated as free, which
  is conservative: it never overwrites, but it also skips `cave_air` and plants.
- **No atomic air-only write.** `set-blocks` queues unconditional `/setblock`-style
  changes that the host applies later; between the `get-block` check and the write another
  actor (including another queued mod write) could fill the position. The check is
  best-effort, which is why this stays a prototype.
- **No inventory/placement-permission model.** Sweeps do not consume items and do not
  re-check build permissions; they write through the server's own block update path.

## Build, test, verify

```bash
cd quick-place
./build.sh          # rustup target, pinned cargo-horizon-mod, cargo test,
                    # cargo horizon-mod build, cargo horizon-mod test, cargo horizon-mod check
```

`build.sh` fails and deletes the artifact unless **all** of these pass:

- `cargo test` — planner and config parser unit tests on the host (no SDK calls);
- `cargo horizon-mod build` — the real wasm build + pack;
- `cargo horizon-mod test --hmod target/horizon-mod/quick_place.hmod` — the sandbox runs
  `tests/placement.toml` and `tests/command.toml`, which place blocks, trigger the hook,
  and assert the resulting block states (air-only, limit, non-air preserved, command
  feedback);
- `cargo horizon-mod check` — the registry's checks on the archive.

CI (`.github/workflows/quick-place.yml`) does exactly this on every push to
`quick-place/**`, then commits the built `.hmod` and its base64 to `dist/` and attaches
them to the `quick-place-v0.2.0` release.
