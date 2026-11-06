# PROGRESS — volt_lapse.hmod (resumable state)

Last updated: 2026-11-06. Branch `arena/5f02db8a-rust-store`.

Goal: deliver a real, toolchain-built `volt_lapse.hmod` ("Volt Lapse", a
Replay-Mod-style recorder for Horizon), with a download URL, a base64 copy for the
phone and this file, in one continuous run.

## State

| Piece | State |
| --- | --- |
| Recon (`hz/horizon.wit@62a01db8`) | done: only `log`, `storage`, `players`, `%world` are importable by a server mod |
| Manifest, toolchain pin, `build.sh` | done |
| `src/settings.rs` — 69 settings, one schema | done (6 unit tests) |
| `config.toml` — the 69 shipped defaults | done, generated from `SPECS`, checked by a unit test |
| `src/recording.rs` — `VL1` format + run compression | done (5 unit tests) |
| `src/path.rs` — linear / Catmull-Rom / Bézier | done (6 unit tests) |
| `src/lib.rs` — commands, recording engine, storage, playback | done (6 unit tests) |
| `tests/*.toml` — sandbox scripts | done: settings (24 steps), recording (13), playback (17) |
| `.github/workflows/volt-lapse.yml` | done: pinned build, tests, check, release upload, artifact committed back |
| CI green | **in progress** — see the run table below |
| Artifact delivered | **not yet** |

## How to resume

1. `git fetch origin && git log --oneline -3 origin/arena/5f02db8a-rust-store`
2. `gh run list --branch arena/5f02db8a-rust-store --workflow volt-lapse.yml --limit 3`
3. Compiler or test output: `git show origin/arena/5f02db8a-rust-store:dist/build.log | tail -160`
   (the workflow commits it on every run, passing or failing; Actions logs are not
   always fetchable from this sandbox).
4. Step-level conclusions: `gh api repos/entitybrian69-bit/Rust-store-/actions/runs/<id>/jobs`
   (avoid `gh run view --log`: the log blob host is unreachable here).
5. Once the job is green:
   ```sh
   gh release download volt-lapse-v0.1.0 --pattern 'volt_lapse.hmod*' --dir /tmp/vl
   sha256sum /tmp/vl/volt_lapse.hmod
   file /tmp/vl/volt_lapse.hmod          # expect a WebAssembly (component) binary
   ```
   Then fill in the receipt table and hand over the URL + base64.

## Receipts

| What | Value |
| --- | --- |
| Toolchain pin | `62a01db8f006b422e6db24e34dcee05ad41760ba`, Rust `1.99.1`, `wasm32-unknown-unknown` |
| Release tag | `volt-lapse-v0.1.0` |
| Artifact | `dist/volt_lapse.hmod` (committed by CI once the build passes) |
| SHA-256 | filled in after the green run |
| CI runs | first: `42235177030` (commit `6e0ce3f`) |

## Design notes worth keeping

- **Storage limits are the hard constraint**: 4 MiB and 64 keys per mod, 1 MiB of
  writes per hook call (`crates/horizon-mods/src/runtime.rs` at the pin). Hence:
  900 KiB write budget per tick, a write queue drained across ticks, a 3 MiB
  recording cap, short keys (`<ns>.rec.<name>.<n>`, `<name>` ≤ 28 chars,
  `<ns>` ≤ 12), and `make_room` deleting the oldest recording when the host
  refuses a write.
- **`safe_mode`** is a real clamp, not a label: `chunk_radius` ≤ 4, chunk ≤ 256 KiB,
  recording ≤ 900 KiB, speed ≤ 4x.
- **31 of the 69 settings need interfaces API 0.3 does not have.** They are
  validated, persisted, listed in a start-up warning, and documented with a
  `TODO(api)` and a citation per row; the table lives in `NO_API` in
  `src/lib.rs` and in the README's "API gaps" section. Nothing is a stub that
  silently does nothing: the rest of the settings each change what the code does.
- **`capture_fps`** really does cap the sampling rate: the effective stride is
  `max(record_interval_ticks, ceil(20 / capture_fps))`.
- **`compression_level`** really compresses: runs of identical consecutive frames
  become `R` records, and the level sets the shortest run collapsed.
- **`replay_folder` / `temp_folder`** are key namespaces, so changing them moves
  where new recordings and drafts go.
- **Sandbox scripts assert exact strings.** Every expected message in
  `tests/*.toml` was derived from the code, not from a guess; a mismatch names the
  step, the command and both strings.
