# Volt Lapse — a replay recorder and replayer for Horizon

**File:** `volt_lapse.hmod` · **Manifest id:** `volt_lapse` (the id you asked for,
`volt-lapse`, is the same name with the separator the packer allows: a manifest
`id` may only use `[a-z0-9_]`, so `volt_lapse` is what `/vl` is registered under) ·
**Display name:** Volt Lapse · **Commands:** `/voltlapse` and `/vl` · **API:**
`0.3`, `side = "server"` · **69 settings.**

A Replay-Mod-style recorder and replayer, built as a legacy WebAssembly **server**
mod for Horizon. It records what a server can see, stores it in the mod's own
`storage`, and replays what a server can change. It is honest about the rest: the
API this build targets has no packet capture, no camera, no renderer, no input
hooks and no filesystem, so a 1:1 clone of Replay Mod — `.mcpr` files, entity and
particle streams, a timeline HUD, keybinds — is **not possible on this API**. The
mod does not pretend: every setting that needs one of those interfaces is still
parsed, validated, persisted and editable, is listed in the start-up warning, and
is documented below as *No API* with a `TODO(api)` in the code.

## What it records, and what it cannot

| Recorded | How |
| --- | --- |
| Player frames | every `record_interval_ticks` ticks: name, position, dimension, game mode, health for every online player (`players::online`) |
| Chat | `record_chat`: the lines the server sees |
| Block changes | `record_block_changes`: breaks and places with their block state |
| Keyframes | `keyframe_interval_ticks`, `/vl keyframe`, a teleport (a jump > 8 blocks) or a dimension change, each with a block snapshot around the player (`chunk_radius`, ≤ 64 blocks) |
| Camera paths | the keyframe list, sampled with linear / Catmull-Rom / Bézier interpolation |

| Not recorded | Why |
| --- | --- |
| Packets, entities, particles, sounds, weather, items, inventories, mob AI | the legacy WIT has no such interface (see [API gaps](#api-gaps)) |
| The `.mcpr` file, replay folder, temp files | no filesystem; recordings live in `storage` |
| Camera, FOV, HUD, timeline, keybinds, GUI | those are client-side interfaces; this is a server mod |
| Yaw/pitch of players | the server `player` record has no rotation |

Playback pushes the recorded block changes back through `world::set-blocks`,
broadcasts the recorded chat, notes each keyframe and runs the timeline at the
requested speed, looping if asked.

## Install

### PC

1. Download `volt_lapse.hmod` (release asset, or `dist/volt_lapse.hmod` in this
   repository) and check its SHA-256 against the published `.sha256`.
2. Put it in Horizon's mods folder and enable it. It is a **server** mod
   (`side = "server"`), so it runs where the world is simulated.
3. In game, `/vl help`.

### Android

Android builds are **WASM only** in this line, so the same `volt_lapse.hmod`
applies — no native code, no NDK, nothing to compile on the phone. Copy it to the
device, decode the base64 asset if that is how you moved it, and drop the file in
the mods folder:

```sh
base64 -d volt_lapse.hmod.b64 > volt_lapse.hmod
```

**Not tested:** this mod was built and tested on Linux (host unit tests + the
sandbox) — not in a game session on PC, and not on Android. Everything it calls is
API 0.3 that the pinned SDK generates, and the sandbox runs the real component,
but "runs in a game on my phone" is not something this repository can claim.

## Commands

`/voltlapse` and `/vl` are the same command (the long name is the manifest's, the
short one is what you will type).

| Command | What it does |
| --- | --- |
| `/vl` or `/vl status` | recording / playback state |
| `/vl record [name]` | start recording (`file_naming` decides the default name) |
| `/vl stop` | stop and save, or stop a playback |
| `/vl play [name] [speed\|p<n>]` | replay the newest (or named) recording at a speed or `speed_presets` slot |
| `/vl list` | the saved recordings, newest first |
| `/vl config` | the settings that differ from the defaults |
| `/vl config <key>` | one setting's value and type |
| `/vl config <key> <value>` | set it (validated, persisted) |
| `/vl config reset` | back to the shipped defaults |
| `/vl keyframe` | add a camera keyframe at your position |
| `/vl path [name]` | sample the camera path of a recording |
| `/vl help` | the list above, in chat |

## The 69 settings

Every one of them lives in `config.toml`, is compiled into the WASM as the
shipped default, is persisted in the mod's `storage` (so it survives a restart),
can be read and written with `/vl config <key> <value>`, is type- and
range-checked with an error that names the key, and is reset by
`/vl config reset`. Keys are matched case-insensitively and `.`/`-`/space are the
same as `_`, so `/vl config Record.Interval.Ticks 5` works.

### Recording (24 settings)

| Setting | Type | Default | Effect |
| --- | --- | --- | --- |
| `enabled` | bool | `true` | Master switch: `/vl record` and both auto-starts refuse while it is false. |
| `auto_start_on_join` | bool | `false` | Starts one recording when a player joins a server with none running. |
| `auto_start_on_world_load` | bool | `false` | Starts one recording on the first tick after the mod loads (the API has no world-load event). |
| `record_interval_ticks` | int 1..=20 | `1` | Sample a frame every N ticks; `capture_fps` can stretch the stride, never shorten it. |
| `record_chat` | bool | `true` | Records the chat lines the server sees; playback broadcasts them again. |
| `record_player_inventory` | bool | `true` | **No API.** Stored, validated, named in the start-up warning; not applied. |
| `record_entity_metadata` | bool | `false` | **No API.** No entity list exists; not applied. |
| `record_block_changes` | bool | `true` | Records breaking and placing, replayed through `world::set-blocks`. |
| `record_particles` | bool | `false` | **No API.** No particle stream; not applied. |
| `record_sounds` | bool | `false` | **No API.** No sound stream; not applied. |
| `record_weather` | bool | `false` | **No API.** No weather state; not applied. |
| `record_time_of_day` | bool | `false` | **No API.** No clock in the server API; not applied. |
| `record_dimension_changes` | bool | `true` | A player changing dimension inserts a keyframe at that tick. |
| `record_mob_ai` | bool | `false` | **No API.** No mob state; not applied. |
| `record_item_drops` | bool | `false` | **No API.** No entity list; not applied. |
| `record_projectiles` | bool | `false` | **No API.** No entity list; not applied. |
| `chunk_radius` | int 0..=16 | `8` | Radius of the block snapshot taken at each automatic keyframe (`safe_mode` caps it at 4; a snapshot is at most 64 blocks). |
| `entity_tracking_radius` | int 0..=256 | `64` | **No API.** No entities; not applied. |
| `max_recording_size_mb` | int 1..=65536 | `2048` | Cap for one recording: the tail is trimmed until it fits. Also capped at 3 MiB, the platform quota minus the config and index keys. |
| `max_recording_duration_min` | int 1..=1440 | `120` | A recording stops itself after this many minutes of ticks and saves. |
| `buffer_flush_interval_sec` | int 1..=60 | `5` | With `experimental_features`, the unfinished buffer is spilled to `temp_folder` every N seconds (20 ticks/s). |
| `compression_level` | int 0..=9 | `6` | Collapses runs of identical consecutive frames into `R` records: 0 writes every frame, 9 collapses any repeat. |
| `keyframe_interval_ticks` | int 1..=72000 | `600` | An automatic keyframe, with its block snapshot, every N ticks. |
| `capture_fps` | int 1..=60 | `20` | Target sampling rate: the frame stride is at least `20 / capture_fps` ticks. |
### Playback (16 settings)

| Setting | Type | Default | Effect |
| --- | --- | --- | --- |
| `default_speed` | float 0.1..=8.0 | `1.0` | Speed `/vl play <name>` uses when no speed is given. |
| `speed_presets` | list of floats 0.1..=8.0, max 16 items | `[0.1, 0.25, 0.5, 1, 2, 4, 8]` | The list `/vl play <name> p<n>` indexes: `p1` is the first entry. |
| `smooth_camera` | bool | `true` | false samples straight lines for `/vl path` whatever `camera_interpolation` says. |
| `camera_interpolation` | enum linear|catmull_rom|bezier | `\"catmull_rom\"` | Path sampling: linear, Catmull-Rom or Bézier. |
| `free_camera` | bool | `false` | **No API.** The camera belongs to the client; not applied. |
| `spectator_lock` | bool | `false` | **No API.** The camera belongs to the client; not applied. |
| `show_timeline` | bool | `true` | **No API.** No client HUD; not applied. |
| `show_keyframes` | bool | `true` | Playback broadcasts a note at each keyframe. |
| `show_path_preview` | bool | `true` | false makes `/vl path` print the keyframe count instead of sampling the path. |
| `playback_hud` | bool | `true` | **No API.** No client HUD; not applied. |
| `hide_hud_in_replay` | bool | `false` | **No API.** No client HUD; not applied. |
| `render_distance` | int 2..=32 | `12` | **No API.** The client owns render distance; not applied. |
| `time_control` | bool | `false` | **No API.** No clock to control; not applied. |
| `weather_control` | bool | `false` | **No API.** No weather to control; not applied. |
| `loop_playback` | bool | `false` | Playback restarts from its first tick instead of finishing. |
| `skip_to_keyframe_on_load` | bool | `true` | Playback starts at the first keyframe, skipping the actions before it. |
### Camera and paths (7 settings)

| Setting | Type | Default | Effect |
| --- | --- | --- | --- |
| `keyframe_snap_ticks` | int 0..=20 | `1` | Minimum spacing of `/vl path` samples (the preview's step). |
| `path_smoothing` | float 0.0..=1.0 | `0.5` | Bézier control points pulled towards each segment's ends. |
| `path_tension` | float 0.0..=1.0 | `0.5` | Catmull-Rom tangent scale. |
| `fov_override` | float 30.0..=120.0 | `70.0` | **No API.** The camera's field of view belongs to the client; not applied. |
| `camera_roll_enabled` | bool | `false` | **No API.** The camera belongs to the client; not applied. |
| `camera_collision` | bool | `false` | **No API.** The camera belongs to the client; not applied. |
| `auto_keyframe_on_teleport` | bool | `true` | A jump of more than 8 blocks between samples inserts a keyframe. |
### Storage and files (7 settings)

| Setting | Type | Default | Effect |
| --- | --- | --- | --- |
| `replay_folder` | text, max 64 | `\"replays\"` | Namespace for recording keys: changing it sends new recordings to another set of keys. |
| `temp_folder` | text, max 64 | `\"replays/tmp\"` | Namespace for the draft spill. |
| `file_naming` | enum timestamp|world_name|manual | `\"timestamp\"` | timestamp (`rec-<tick>`), world_name (`world-<tick>`; the API has no world name) or manual (a name is required). |
| `auto_delete_after_days` | int 0..=3650 | `0` | **No API.** No wall clock to age recordings by; not applied. |
| `max_replays_kept` | int 0..=10000 | `0` | Keep only the newest N recordings; 0 keeps all. |
| `write_chunk_size_kb` | int 1..=8192 | `1024` | Bytes per storage key, clamped to 900 KiB — the platform's 1 MiB per-call write budget with headroom. |
| `atomic_write` | bool | `true` | Write the chunks first and the manifest last, so a half-written recording is never listed. |
### UI (4 settings)

| Setting | Type | Default | Effect |
| --- | --- | --- | --- |
| `theme` | enum dark|light|auto | `\"dark\"` | **No API.** No client UI; not applied. |
| `language` | text, max 16 | `\"en\"` | **No API.** No client UI; not applied. |
| `ui_scale` | float 0.5..=3.0 | `1.0` | **No API.** No client UI; not applied. |
| `timeline_position` | enum bottom|top|hidden | `\"bottom\"` | **No API.** No client HUD; not applied. |
### Keybinds (6 settings)

| Setting | Type | Default | Effect |
| --- | --- | --- | --- |
| `keybind_open_replay_menu` | text, max 24 | `\"F6\"` | **No API.** No keybind interface; `/vl` commands are the interface. |
| `keybind_toggle_record` | text, max 24 | `\"F7\"` | **No API.** No keybind interface; use `/vl record` and `/vl stop`. |
| `keybind_add_keyframe` | text, max 24 | `\"K\"` | **No API.** No keybind interface; use `/vl keyframe`. |
| `keybind_play_pause` | text, max 24 | `\"Space\"` | **No API.** No keybind interface; use `/vl play`. |
| `keybind_speed_up` | text, max 24 | `\"ArrowUp\"` | **No API.** No keybind interface; pass a speed to `/vl play`. |
| `keybind_slow_down` | text, max 24 | `\"ArrowDown\"` | **No API.** No keybind interface; pass a speed to `/vl play`. |
### Advanced (5 settings)

| Setting | Type | Default | Effect |
| --- | --- | --- | --- |
| `debug_logging` | bool | `false` | Writes debug lines (storage writes, drafts) to the server log. |
| `log_level` | enum error|warn|info|debug|trace | `\"info\"` | error/warn/info/debug/trace: below info the start-up line and the hints go quiet. |
| `metrics_opt_in` | bool | `false` | Adds local counters to `/vl status` (frames, replay writes, recordings). Nothing is ever sent anywhere. |
| `experimental_features` | bool | `false` | Enables the draft spill and shows the queued writes in `/vl status`. |
| `safe_mode` | bool | `true` | Conservative clamps: `chunk_radius` ≤ 4, chunk size ≤ 256 KiB, recording ≤ 900 KiB, speed ≤ 4x. |

## Recording format (`VL1`)

A recording is a line-based text stream (`src/recording.rs`, unit-tested for
round-trips). Escapes: `\\s` space, `\\n` newline, `\\t` tab, `\\\\` backslash.

| Record | Meaning |
| --- | --- |
| `VL1` | header, exactly once, first line |
| `M <name> <created tick>` | the recording's name and creation tick |
| `F <tick> <dimension> <players>` then `P` lines | a frame and its players |
| `P <name> <x> <y> <z> <game mode> <health>` | one player in the frame above |
| `R <first tick> <count>` | `count` frames identical to the one before, at consecutive ticks (`compression_level`) |
| `C <tick> <player> <text>` | chat |
| `B <tick> <p\|b> <x> <y> <z> <state>` | block placed or broken |
| `K <tick> <x> <y> <z>` | camera keyframe |
| `S <keyframe> <x> <y> <z> <state>` | one block of a keyframe's snapshot |

## Storage layout and limits

There is no filesystem: a recording is a set of `storage` keys, chunked and
written inside the host's budget.

| Key | Contents |
| --- | --- |
| `voltlapse.config` | the whole configuration, as TOML |
| `voltlapse.index` | `"<namespace> <name>"` per line, oldest first |
| `<replay_folder>.rec.<name>.<n>` | chunk `n` of the `VL1` stream |
| `<replay_folder>.rec.<name>.meta` | `chunks=… created=… bytes=… trimmed=…` |
| `<temp_folder>.rec.<name>.draft` | the unfinished buffer (`buffer_flush_interval_sec`) |

The host allows **4 MiB** and **64 keys** per mod, and **1 MiB of writes per hook
call** (`crates/horizon-mods/src/runtime.rs` at the pinned revision:
`STORAGE_QUOTA`, `STORAGE_MAX_KEYS`, `STORAGE_WRITE_BUDGET`; keys are
`[a-z0-9_.-]{1,64}`). So Volt Lapse writes 900 KiB at most per tick, queues the
rest and drains it on the following ticks, caps one recording at 3 MiB, and
deletes the oldest recording when the host says it is out of room
(`make_room`, three attempts per chunk).

## API gaps

The pinned WIT is
[`crates/horizon-mod-sdk/wit/horizon.wit@62a01db8`](https://github.com/horizon-rust/horizon/blob/62a01db8f006b422e6db24e34dcee05ad41760ba/crates/horizon-mod-sdk/wit/horizon.wit).
The server world imports exactly `log`, `storage`, `players` and `%world`.

| Volt Lapse wants | API 0.3 has | Consequence |
| --- | --- | --- |
| Packet capture / `.mcpr` | nothing; no network surface | recordings are sampled server state, not a packet capture (`TODO(api)`) |
| Filesystem (`replay_folder`, `temp_folder`, chunked files) | `storage`: key/value bytes, 4 MiB, 64 keys | folders become key namespaces |
| Camera, FOV, roll, collision, HUD, timeline, GUI, keybinds | client interfaces (`render`, `hud`, `ui`, `input`) are not available to a server mod | settings stored and documented, not applied |
| Entities, particles, sounds, weather, time | no interface | not recorded |
| Inventory, item drops, projectiles, mob AI | no interface | not recorded |
| Wall clock (timestamp naming, age-based deletion) | none | tick counter names files; `auto_delete_after_days` cannot run |
| Player rotation (yaw/pitch) | `player` has id, name, x, y, z, dimension, game-mode, health | paths interpolate positions only |

`docs/api/mod-files.md` in the same revision records that the old `mod.wasm`
entry point no longer runs — this mod is a componentized `.hmod`, not a
`mod.wasm` folder. PR #3353 ("Launcher: WebAssembly mods need a native port")
is the direction that removed the WebAssembly runtime; `62a01db8` is the last
main commit before its removal commit `e7614d2`.

## Build it yourself

The whole build is CI, with the real toolchain — no hand-made archives:

```sh
cd volt-lapse
bash ./build.sh          # host unit tests, build, sandbox tests, registry check
```

`build.sh` installs Rust `1.99.1` with `wasm32-unknown-unknown`, installs
`cargo-horizon-mod` from Horizon at `62a01db8f006b422e6db24e34dcee05ad41760ba`,
then runs `cargo test`, `cargo horizon-mod build`, `cargo horizon-mod test --hmod
target/horizon-mod/volt_lapse.hmod` and `cargo horizon-mod check`. Everything it
prints is in `build.log`, which CI commits to `dist/build.log` on every run,
passing or failing.

`.github/workflows/volt-lapse.yml` runs that on every push that touches
`volt-lapse/`, publishes `volt_lapse.hmod`, `.b64` and `.sha256` as release assets
and commits the built artifact back to the branch.

## Tests

| Test | Covers |
| --- | --- |
| `src/settings.rs` (5) | every default parses and round-trips, the shipped `config.toml` equals the defaults, types and ranges, normalisation, the file parser |
| `src/recording.rs` (5) | `VL1` round-trip with spaces and escapes, span/summary, malformed input, run compression |
| `src/path.rs` (6) | linear/Catmull-Rom/Bézier sampling, exact ends, clamping, bounded previews |
| `src/lib.rs` (6) | schedule batching and keyframe skipping, storage-safe names, `file_naming`, UTF-8 chunking, the 69-key invariants, the *No API* table |
| `tests/settings.toml` (23 steps) | read/write/validate/persist/reset through `/vl config` |
| `tests/recording.toml` (14 steps) | record → keyframe → save → list → path, persistence across a reload, the error paths |
| `tests/playback.toml` (17 steps) | replay writes blocks back, chat, speed, presets, looping |

## Licence

Same as the repository: see `LICENSE` at the root.
