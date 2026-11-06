# Quick Place — legacy-runtime WASM prototype

**Preview only: no extra blocks are placed.** This prototype plans a line or plane of candidate positions, checks which are air, and tells the player the count. It deliberately does not write to the world because the pinned API cannot guarantee an air-only write (see Safety below). It is **not compatible with current PC Horizon**. It has not been tested in a PC game or on Android.

## Pinned runtime and SDK

* Legacy Horizon runtime, `cargo-horizon-mod` and `horizon-mod-sdk`: official [`horizon-rust/horizon` commit `62a01db8f006b422e6db24e34dcee05ad41760ba`](https://github.com/horizon-rust/horizon/tree/62a01db8f006b422e6db24e34dcee05ad41760ba), committed **2026-10-29 04:37:12 UTC**. This is the parent of the commit removing the game's WASM code-mod runtime. The SDK is pinned to the same revision in `Cargo.toml`; build scripts install the tool at that revision with its own lockfile.
* Rust toolchain `1.99.1`, target `wasm32-unknown-unknown`, pinned in `rust-toolchain.toml` and build scripts. `cdylib` emits the WASM core module required by the legacy builder; `rlib` allows host-side Rust tests.
* Manifest ID `quick_place`, API `0.3`, server side, capabilities `players` (the preview message) and `world` (air reads). `game-read` is client-only and rejected for server mods. See the pinned [manifest parser](https://github.com/horizon-rust/horizon/blob/62a01db8f006b422e6db24e34dcee05ad41760ba/crates/horizon-mods/src/manifest.rs) and [WIT](https://github.com/horizon-rust/horizon/blob/62a01db8f006b422e6db24e34dcee05ad41760ba/crates/horizon-mod-sdk/wit/horizon.wit).

The guest is built for WASM with no intentional OS-specific or native runtime dependencies or desktop input assumptions; the same `.hmod` is intended for both PC and Android **only where a compatible legacy WASM server-mod host exists**. The host's runtime is platform-specific. This is portability by construction, not proof of compatibility. No Android test or compatible Android runtime at this exact revision has been established; the [unofficial Android port](https://github.com/Vasysik/horizon-android/releases) has publicly available releases, but compatibility with this pinned server API has not been verified. Current official Horizon [removed the WASM runtime](https://github.com/horizon-rust/horizon/commit/e7614d2197ebe9ffdaa75e1bbb432dc0ce855e70).

## Usage and configuration

In a compatible legacy host, every player block-place event triggers a chat **preview** of how many planned targets are air. Only exactly `minecraft:air` counts; unloaded targets (`None`), other air variants and all non-air states are skipped conservatively. The hook makes no block changes. Repeated events produce repeated previews. The original player placement is not undone.

Edit [`config.toml`](config.toml) **before building**; it is embedded in the WASM and is not read from disk at runtime. Invalid or unknown settings prevent initialization. After edits rebuild and adjust the default-dependent tests:

| Setting | Default | Meaning |
| --- | --- | --- |
| `enabled` | `true` | Enable previews. |
| `max_blocks_per_sweep` | `3` | Number of extra candidates examined, excluding the original block; 0–64. Blocked candidates are skipped rather than replaced by farther ones. |
| `placement_mode` | `"line"` | `line`, or `plane` (horizontal, three-wide rows, center then left then right, proceeding forward). |
| `facing` | `"south"` | Fixed `north`, `east`, `south`, or `west`; no server-side player yaw is exposed. |

Sneaking cannot be required: the pinned server `Player` has no sneaking field. There is no `require_sneaking` setting. This design does not rely on a keybind, mouse, framerate, filesystem, network or threads in the guest. The planner in `src/planner.rs` has no SDK calls. It includes a yaw conversion helper for tests/future APIs, but the hook does not claim to know player yaw.

## Safety and API gaps

The legacy [`world.get-block`](https://github.com/horizon-rust/horizon/blob/62a01db8f006b422e6db24e34dcee05ad41760ba/crates/horizon-mod-sdk/wit/horizon.wit) reads a snapshot, while `world.set-blocks` queues unconditional `/setblock`-style mutations. The [host applies queued writes later](https://github.com/horizon-rust/horizon/blob/62a01db8f006b422e6db24e34dcee05ad41760ba/crates/horizon-client/src/game/server_mods.rs): an air block can become non-air after a read, including from another queued mod write, before our write is applied. Rechecking before enqueue is not atomic, and writes for another dimension can wait longer. Thus reading air and calling `set-blocks` cannot uphold **never overwrite non-air**. No write is attempted, even when all targets appear clear.

To make real placement safe, a compatible host must expose an atomic air-only/compare-and-set placement operation, including clear outcomes for unloaded chunks and concurrent writes. The API also lacks a normal inventory-consuming player-placement operation, server-side yaw and sneaking, and a general replaceability predicate. It cannot promise normal placement permissions or inventory costs. These are explicit `TODO(api)` items in the hook. Do not use this prototype as a building tool or on important worlds.

## Build and test (CI artifact, no registry publication)

No local `.hmod` has been produced in this environment: Rust/Cargo and rustup's distribution host are unavailable. Run the **Legacy Quick Place prototype CI** workflow on a GitHub branch or manually. It runs `./build.sh`, then uploads `target/horizon-mod/quick_place.hmod` and this README as the `quick-place-legacy-preview-only` **workflow artifact only if every step succeeds**. No registry submission, GitHub Release, or pre-release is created. CI must actually pass before an artifact can be claimed.

On a machine with rustup, Git and network access, run `./build.sh` or `./build.ps1`. Each script installs the pinned Rust target and exact historical builder, runs `cargo test`, builds the `.hmod`, executes the historical sandbox script `tests/placement.toml` against that artifact, and runs `cargo horizon-mod check`. A failed step removes the output. The first install compiles the large historical Wasmtime-based tool and can take considerable time. The SDK/tool and Rust are pinned; this repository does not yet have a generated Cargo.lock, so dependency resolution is not fully reproducible.

`tests/planning.rs` consumes the pure-planner cases in `tests/fixtures/planning.toml`, including line, plane, blocked positions, direction boundaries and coordinate overflow. `tests/placement.toml` is the legacy sandbox integration test: it checks the message, air filtering, and preservation of non-air blocks. The sandbox does not model all runtime scheduling or real-world conditions; neither this test nor CI proves PC or Android gameplay works.

## Installing a CI artifact for a legacy-host experiment

Only after a successful CI run, download the artifact. On a host built from the pinned revision **with its `mods` feature enabled** (the historical guide specifies `cargo build --release -p horizon-client --features mods`), copy `quick_place.hmod` to `horizon-mods/` next to `options.txt`, enable it in **Options → Mods**, and open a world. A dedicated server needs the file on the world host; installing on a multiplayer guest alone does nothing. This is a preview-only experiment, not an installation path for current Horizon. No runtime validation has been performed here.
