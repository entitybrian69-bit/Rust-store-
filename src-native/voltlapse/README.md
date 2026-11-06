# Volt Lapse

Record, replay, re-film and re-time your Minecraft world without leaving the game.

Volt Lapse is a *native* Horizon mod: recording, playback, camera paths and the
renderer are Rust inside the game process, not a Java mod driving a screen
recorder. It writes compact `.vrc` replays, plays them back at any speed, lays
offline camera paths with keyframes, and renders those paths out to video at
whatever resolution and frame rate you ask for — frame by frame, so slow
machines produce the same result as fast ones.

| | |
| --- | --- |
| Package | `voltlapse.hnmod` (native) / `voltlapse.hmod` (native + content) |
| Version | 1.0.0 |
| Horizon game | 26.3 |
| Horizon API | 0.2 |
| Side | client |
| Licence | LGPL-3.0-or-later |

## What it does

* **Recording** — a frame-accurate capture of entities, blocks, particles,
  weather, time, inventory, chat and camera state. Pause without losing the
  timeline, resume on a new segment, and recover an interrupted session from
  the next launch.
* **Playback** — open any replay and scrub it with speed control, per-frame
  interpolation, looping and bookmarks. Replays are streamed from disk, so a
  four-hour recording costs the same memory as a four-second one.
* **Camera paths** — keyframes with easing, field-of-view, roll and shake,
  edited in game with a live preview and stored inside the replay, so a path
  travels with the recording it was made for.
* **Rendering** — offline frame rendering of a camera path from 640×360 to 8K, at 24,
  30, 60, 120, 144, 240 or 480 fps with motion blur, depth of field and a determinism guarantee.
* **HUD** — an indicator, timecode and stats overlay that never covers the
  parts of the screen you asked to keep clear.
* **Storage** — profile-aware libraries, size quotas, thumbnails and exports.
* **Performance** — a frame budget, adaptive sampling and a per-mod cache that
  keeps the recording overhead in single-digit percentages.
* **UI** — six-tab in-game viewer plus a keybinding screen, translated into
  English, German and Nepali.
* **Multiplayer** — record on servers, share replays with `/voltlapse share`.
* **Compatibility** — see `compatibility.json` and `docs/COMPATIBILITY.md`.
* **Modding** — the replay, path and package formats are documented in
  `docs/FORMATS.md`, and every hook is public API.

## Install

* Windows / macOS / Linux desktop: see [`docs/INSTALL_PC.md`](docs/INSTALL_PC.md).
* Android: see [`docs/INSTALL_ANDROID.md`](docs/INSTALL_ANDROID.md).

Both paths are the same three steps — drop `voltlapse.hnmod` into
`<game-folder>/horizon-native-mods/`, switch it on in **Options → Mods** (mods
are off by default), then start the game so the launcher can build it. The
`.hmod` variant is the combined package: the same code plus the icon, the
language files and the `voltlapse:` content namespace.

## First recording

1. Press **F6** (or run `/voltlapse record start`). The indicator turns red.
2. Play. Press **F7** to pause, **F6** again to stop and save.
3. Press **F8** to open the viewer, pick the replay, press **Space** to watch.
4. Press **F9** to open the camera editor, add keyframes with **K**, then
   **F11** (or `/voltlapse render start`) to render the path.

Everything is rebindable in Options → Controls → Volt Lapse. Defaults are
listed in [`docs/KEYBINDS.md`](docs/KEYBINDS.md).

## Commands

```
/voltlapse record  start|stop|pause|resume|save|stats
/voltlapse play    [replay] [at <seconds>] [speed <x>]
/voltlapse render  start|queue|cancel|status
/voltlapse config  <section> [key] [value]
/voltlapse keyframe add|delete|clear|list
/voltlapse bookmark add|remove|list|next|previous
/voltlapse list | info | export | help
/voltlapse share   host|join|leave|status
```

Tab completion walks the verb tree; `/voltlapse help` prints the same list in
game.

## Configuration

Settings live in `config/voltlapse/config.json` and are validated on load: a
key that does not exist, a value out of range or a stale schema version is
repaired and reported in chat instead of failing the launch. The shipped
defaults are [`default-config.json`](default-config.json), and the engine has a
test that keeps the two identical.

Translations live in `config/voltlapse/lang/<locale>.json` and fall back to
English per key. `en_us`, `de_de` and `ne_np` ship inside the package.

## Build

```sh
python3 tools/build.py                 # writes releases/voltlapse.{hnmod,hmod}
python3 tools/build.py --check         # verify existing packages, never write
python3 tools/build.py --json          # machine-readable report
```

The packager is dependency-free (stdlib only) and reproducible: entries are
sorted, stored uncompressed and stamped with a fixed timestamp, so two builds
of the same tree are byte-identical. Tests:

```sh
python3 -m unittest discover -s src-native/voltlapse/tests -v
```

## Layout

```
src-native/voltlapse/
  horizon-native.json      native manifest (hooks, bindings, entry points)
  compatibility.json       verified game/loader matrix
  default-config.json      the defaults the engine ships with
  content/                 the .hmod payload (manifest, icon, languages)
  lang/                    translations, mirrored into content/assets
  mixins/voltlapse/        the code: runtime glue, 22 hook adapters, engine/
    engine/src/            27 modules: recording, playback, camera, render, ...
  tools/build.py           packager and validators
  tests/                   packager tests
  docs/                    this documentation set
```

## Credits

Volt Lapse stands on the work of the Replay Mod team, whose format and feature
set shaped this one; the Horizon project, whose native mod API made it possible;
and the Minecraft community. Full attributions are in
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) and `NOTICE`.

Volt Lapse is free software under the GNU Lesser General Public License,
version 3 or later. See [`LICENSE`](LICENSE).
