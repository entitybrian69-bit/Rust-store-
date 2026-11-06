# Installing Volt Lapse on PC

Volt Lapse ships two files that carry the same code:

| File | Use it when |
| --- | --- |
| `voltlapse.hnmod` | **The one to install.** Native code only; every launcher in use reads it. |
| `voltlapse.hmod` | The combined form: the same native code plus `horizon-mod.toml`, the icon and the language files. `docs/api/mod-files.md` defines it ("Both, in one `.hmod`"), but launchers are only switching to native `.hmod`s now — if yours doesn't list it, use the `.hnmod`. |

Windows, macOS and Linux are the same procedure.

## 1. Find the game folder

The game folder is the one holding `options.txt`. Typical locations:

* Windows: `%APPDATA%\.minecraft`
* macOS: `~/Library/Application Support/minecraft`
* Linux: `~/.minecraft`

## 2. Drop the file in

Horizon keeps the two kinds of mod apart: **content** mods go in
`horizon-mods/`, **native** mods in `horizon-native-mods/`. Volt Lapse is code,
so it goes in the native folder:

```text
<game-folder>/
├── options.txt
├── horizon-mods.txt           <- mod.<id>:true switches
├── horizon-mods/              <- content mods (.hmod)
├── horizon-native-mods/
│   └── voltlapse.hnmod        <- Volt Lapse
└── config/
    └── voltlapse/             <- created on first launch
```

A `.hmod` that carries native code (this mod's combined package) belongs in
`horizon-native-mods/` too — the launcher builds it by the native half it
finds inside.

Do **not** unzip the package. If your browser renamed it, make sure the
extension is exactly `.hnmod` (Windows hides known extensions by default).

## 3. Turn it on

Horizon mods are **off by default**. In the launcher (or in game, **Options →
Mods**) switch Volt Lapse on; the choice is written to `horizon-mods.txt` as
`mod.voltlapse:true`, and the row offers **Open folder**, which points at
`<game-folder>/horizon-native-mods/`.

## 4. First launch

Native mods are compiled into the game the first time they are seen, so the
first start after installing takes noticeably longer than later ones, and the
launcher shows a trust notice once. After the build finishes:

1. Open **Options → Controls → Volt Lapse** and check the bindings.
2. Enter a world and press **F6**. The indicator appears in the top-left.
3. Press **F6** again to stop; the replay is written to
   `<game-folder>/voltlapse/replays/`.

## 5. Where things are written

```text
<game-folder>/
├── voltlapse/
│   └── replays/*.vrc          recordings (storage.directory)
└── config/voltlapse/
    ├── config.json            settings, repaired on load if damaged
    ├── lang/<locale>.json     optional translation overrides
    ├── exports/               `/voltlapse export` copies, rendered frames
    ├── journal/               crash recovery, one .journal per session
    ├── cache/                 thumbnails
    ├── overlays/              watermark images
    └── profiles/              named setting profiles
```

`storage.directory` moves the replay folder; a relative value is joined to the
game folder, an absolute one is used as-is.

## 6. Uninstalling

Delete `horizon-native-mods/voltlapse.hnmod` (and `voltlapse.hmod`, if present),
switch it off, and restart. Recordings, config and exports are yours and are left alone; delete
`voltlapse/` and `config/voltlapse/` yourself if
you want them gone.

## Updating

Replace the package and restart. Config files are versioned: a newer build
migrates an older `config.json` and reports anything it had to change in chat.
Replay files are forward-compatible within format version 1 — a newer build
reads every replay an older one wrote, and refuses a file from a newer format
with a clear message instead of guessing.

If something goes wrong, see [`TROUBLESHOOTING.md`](TROUBLESHOOTING.md).
