# PROGRESS — quick_place.hmod (resumable state)

Last updated: 2026-11-06 (session working branch `arena/5f02db8a-rust-store`).
Goal: deliver a real, toolchain-built `quick_place.hmod` (Rust/WASM), with download URL and
base64 for the phone.

## DELIVERED (2026-11-06)

- Artifact: `dist/quick_place.hmod` — **27,914 bytes**, SHA-256
  `7e7c528a31e19f284d83bb70223689bf8d2d1367a21e5c35489b817e24347e06`.
- Built by CI run `42219681280` (commit `11a56f4`) with Horizon
  `62a01db8f006b422e6db24e34dcee05ad41760ba` + Rust 1.99.1 + wasm32-unknown-unknown.
- Verified: 12 host unit tests ✓, `cargo horizon-mod build` ✓, sandbox scripts
  `tests/placement.toml` (4 steps) + `tests/command.toml` (6 steps) ✓,
  `cargo horizon-mod check` ✓ (27,914 B, "trial run ... ok").
- Release: `quick-place-v0.2.0` with `quick_place.hmod`, `.hmod.b64`, `.hmod.sha256`.
- Full build output: `dist/build.log` (committed by CI on every run, pass or fail).

## Where things stand

- Mod source written: `quick-place/` (manifest, `config.toml`, `src/{lib,planner,config}.rs`,
  `tests/{placement,command}.toml`, `build.sh`, `rust-toolchain.toml`).
- Workflow written: `.github/workflows/quick-place.yml` (build → test → check → commit
  `dist/quick_place.hmod{,.b64,.sha256}` back to the branch → release asset).
- Next action: push and drive the CI run to green; then fetch `dist/quick_place.hmod` via git
  (github.com is reachable from the sandbox), verify hash/size, publish URL + base64.

## Path 1 — build in the sandbox: IMPOSSIBLE (receipts in `qp-logs/`)

| Attempt | Result |
| --- | --- |
| `curl https://sh.rustup.rs` | `curl: (35) OpenSSL SSL_connect: SSL_ERROR_SYSCALL` — host blocked |
| `sudo apt-get install rustc cargo` | `E: Unable to locate package rustc`; `deb.debian.org` unreachable (`Connection failed [IP: 151.101.2.132 80]`) |
| `https://static.rust-lang.org/dist/rust-1.86.0-…tar.gz` | `SSL_ERROR_SYSCALL` — host blocked |
| crates.io / static.crates.io | `000` (unreachable) |
| docker / podman / nix | `command not found` |
| Pre-installed rustc/cargo | not present (`command -v` empty) |

Sandbox allowlist: `github.com`, `codeload.github.com`, `api.github.com`,
`registry.npmjs.org`, `pypi.org`, `files.pythonhosted.org`. No Rust toolchain can be
obtained or run locally. Workflow-artifact downloads (`productionresultssa8.blob.core.
windows.net`) and run logs (`results-receiver.actions.githubusercontent.com`) are blocked,
so CI must hand bytes back **through the repo or a release**, not through artifacts.

## Path 2 — GitHub Actions (active)

- Repo `entitybrian69-bit/Rust-store-`, branch `arena/5f02db8a-rust-store`; `gh` authenticated;
  the existing workflows run (run 42214898658 ✓).
- Prior session (branch `arena/47dabc30-rust-store`, run 42206448892 ✓, 4m28s) proved the
  recipe works: `rustup` + `cargo install --git … --rev 62a01db … cargo-horizon-mod` +
  `cargo horizon-mod build/test/check`. That prototype was **preview-only** (no world
  writes) and its artifact was never retrieved.
- This session's mod performs the writes (`world.set-blocks`, air-only) and the sandbox
  scripts assert the resulting blocks.

## Path 3 — exotic

Only needed if Path 2 fails after repeated fixes: Codespaces/devcontainer via API, or a
hosted build service. Not attempted yet.

## Facts pinned down (don't re-research)

- WASM removal: PRs #3350 (game), #3351 (crates), #3353 (launcher), #4560 (docs), merged
  2026-10-29/30. Last main commit with the runtime: **62a01db8f006b422e6db24e34dcee05ad41760ba**
  (2026-10-29T04:37:12Z). Removal commit: `e7614d2197ebe9ffdaa75e1bbb432dc0ce855e70`.
- `crates/cargo-horizon-mod` builds content mods *now*; at the pin it compiles
  `wasm32-unknown-unknown`, componentizes, and packs `mod.wasm` into the `.hmod`.
- API 0.3 server mod: `ServerMod::{init, command, block_placed}`;
  `world::{get_block -> Option<String>, set_blocks(&str, Vec<BlockChange>) -> Result<(),String>}`;
  `players::tell(&id, &text) -> bool`; manifest `side = "server"`, `api = "0.3"`,
  capabilities `world`, `players` (both server-allowed, neither experimental).
- Sandbox script keys: `join`, `blocks`, `place`, `command`, `expect.{told,blocks,feedback,error,commands}`.
