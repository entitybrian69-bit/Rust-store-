# GAPS — status after the Volt worldgen-api patch

Engine patch: Volt Launcher branch `biomes-unseen-worldgen-api`, commit `da13467`
(local clone `/home/user/Volt-launcher`; exported as `docs/biomes_unseen/volt-worldgen-api.patch`).
Push to `entitybrian69-bit/Volt-launcher` was attempted and **denied (403)** — the sandbox
GitHub token is scoped to this repository only. Apply the patch / push that branch from an
account with write access to Volt-launcher to publish it.

| Gap (v1) | Status now | Evidence |
|---|---|---|
| Biome registration | **CLOSED (code + data)** — `worldgen/biome` added to `WORLDGEN_BUNDLES` in `crates/horizon-client/src/mod_content.rs`; 44 biome JSONs in the .hmod | Volt commit da13467; `scripts/test_biomes_unseen.py` (44 biomes, climate ranges, feature refs) |
| Climate placement / spawn weights | **CLOSED** — `horizon:climate` ranges quantized ((v*10000) as i64, vanilla quantizeCoord) and appended to overworld/nether preset tables in `presets.json`; weight approximated by range width/offset (closest-match search) | `ClimateEntry` in Volt patch + Rust unit test `climate_entry_strips_keys_and_quantizes` |
| Worldgen features (trees/flora/ores) | **CLOSED (data)** — 12 tree, 12 flora, 1 ore configured+placed features (engine feature types read from `crates/horizon-world/src/worldgen/{tree,ore_feature,simple_block,placement}.rs`); biomes reference them at steps 6 (ores) / 9 (vegetal) | test_biomes_unseen cross-refs |
| Structures | **ADAPTED** — engine already supported mod structures/structure_sets/template_pools; Biomes Unseen ships none (BOP has no signature structures) | Volt WORLDGEN_BUNDLES (pre-existing) |
| Wood stairs/slabs/fences/doors/etc. | **CLOSED (data)** — template system accepts any vanilla block; 10 variants × 12 woods (176 blocks total) with full vanilla state-variant blockstates | test checks stairs=40, door=64, slab=3 variant coverage |
| Signs / boats | **CLOSED (data)** — oak_sign/oak_wall_sign templates with `wood_type` + `wall_block` (engine `crates/horizon-mods/src/content.rs:1282`), oak_boat item per wood | generator + validator |
| Per-biome config toggles | **ADAPTED** — deleting a biome JSON / narrowing its `horizon:climate` range disables/shrinks it; no engine config UI exists | documented, data-level |
| Custom sounds / particles | **OPEN — engine has no asset hook** (`grep -rn "particles/" crates/horizon-assets/src crates/horizon-client/src` in Volt = no loader; sounds only via vanilla `properties.sounds` groups, which all blocks use). Implementing a renderer/audio asset pipeline was not attempted: it cannot be compile-tested here (below) | grep evidence 2026-11-08 |
| `.hnmod` schema | **CLOSED** — real schema from `crates/horizon-mods/src/index/native.rs` (`deny_unknown_fields`): {schema:1,id,version,game,horizon,mixins,hooks?} + `mixins/<id>/horizon-mixin.toml`; rebuilt accordingly and schema-tested | test_biomes_unseen hnmod checks |
| Engine compile / client run / APK | **UNVERIFIED — no Rust toolchain obtainable in this sandbox**: `apt-get` repos unreachable, `static.rust-lang.org` blocked (only github/pypi/npm hosts allowed), no cargo/rustc anywhere on disk. Therefore `cargo build/test`, `horizon-launcher native-build`, `--world/--seed/--creative` + horizon-ctl runs, and the Android APK build could NOT be executed. The Volt Rust code was written against the exact surrounding APIs and unit-tested by construction, but it has not been compiled. Run: `cd Volt-launcher && git checkout biomes-unseen-worldgen-api && cargo test -p horizon-client mod_content` on a machine with Rust 1.95 | env checks this session |
