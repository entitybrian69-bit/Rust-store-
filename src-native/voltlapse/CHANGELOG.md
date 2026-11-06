# Changelog

All notable changes to Volt Lapse are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
semantic versioning for the packages and a separate integer for the replay
format (see `docs/FORMATS.md`).

## [1.0.0] — 2026-11-06

First release. Native Horizon mod, game 26.3, native hook API 410.

### Added

* **Recording** — tick-accurate capture of entities, blocks, particles, weather,
  time, inventory, chat and camera state; pause/resume within one session;
  block-wise flush with LZ packing; crash journal and recovery on the next
  launch; live stats in the HUD and `/voltlapse record stats`.
* **Playback** — replay browser and player with speed control (0.1×–16×, eight steps), per-tick
  interpolation between samples, seeking, looping, bookmarks and a
  constant-memory streaming decoder.
* **Camera** — keyframe paths with seven interpolation modes (linear,
  Catmull-Rom, Hermite, ease in/out/in-out, constant), per-segment tension and
  shake, FOV and roll, free-cam, snapping, and a live preview in the editor.
* **Rendering** — deterministic offline frame rendering of a camera path with
  resolution from 640×360 up to 8K and frame rates the encoder supports, motion blur, depth of field and a
  post-shader frame capture, plus a render queue and status reporting.
* **HUD** — indicator, timecode and stats overlay; configurable corner, scale,
  colour and visibility; screenshot and thumbnail capture into the replay.
* **Storage** — profile-aware replay library, search and sort, quotas and
  retention, thumbnails, exports.
* **Performance** — frame budget, adaptive sampling and a per-mod cache, with
  the budget visible in the HUD.
* **UI** — six-tab in-game viewer (Recording, Playback, Rendering, Camera, HUD,
  Storage) plus a settings and keybinding screen, fully mouse and keyboard
  driven, laid out to survive any GUI scale.
* **Commands** — `/voltlapse` with 11 verbs, sub-verb completion and
  human-readable errors; every keybinding has a command equivalent.
* **Localisation** — 65 strings in English, German and Nepali, with per-key
  English fallback and user overrides from `config/voltlapse/lang/`.
* **Multiplayer** — recording on servers and Realms with no server-side
  install; `/voltlapse share` host/join/leave/status for handing a replay to
  another player.
* **Compatibility** — version matrix for shaders, servers and loaders in
  `compatibility.json` and `docs/COMPATIBILITY.md`.
* **Packaging** — deterministic `.hnmod` and `.hmod` output from a stdlib-only
  packager, with validators and ten tests.

### Notes

* Replay format version 1, opcode version 1. `.mcpr` files are not importable;
  see `NOTICE`.
* PC and Android share one implementation; Android limits rendering to what
  the device's memory allows.

[1.0.0]: https://github.com/entitybrian69-bit/Rust-store-/releases/tag/voltlapse-1.0.0
