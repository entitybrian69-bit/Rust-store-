# Installing Volt Lapse on PC

Volt Lapse ships two files that carry the same code:

| File | Use it when |
| --- | --- |
| `voltlapse.hnmod` | You only want the mod (recording, replay, camera, rendering). |
| `voltlapse.hmod` | You also want the icon, the English/German/Nepali language files and the `voltlapse:` content namespace registered by the normal mod list. |

Windows, macOS and Linux are the same procedure.

## 1. Find the game folder

The game folder is the one holding `options.txt`. Typical locations:

* Windows: `%APPDATA%\.minecraft`
* macOS: `~/Library/Application Support/minecraft`
* Linux: `~/.minecraft`

## 2. Drop the file in

Native mods live beside the launcher's other bundles:

```text
<game-folder>/
├── options.txt
├── horizon-mods/
│   └── voltlapse.hnmod        <- native mod, no load-order switch
└── config/
    └── voltlapse/             <- created on first launch
```

If you installed the `.hmod` instead, put it in the same folder; the game
registers both halves under the single id `voltlapse` and refuses to run them
twice.

Do **not** unzip the package. If your browser renamed it, make sure the
extension is exactly `.hnmod` (Windows hides known extensions by default).

## 3. First launch

Native mods are compiled into the game the first time they are seen, so the
first start after installing takes noticeably longer than later ones, and the
launcher shows a trust notice once. After the build finishes:

1. Open **Options → Controls → Volt Lapse** and check the bindings.
2. Enter a world and press **F6**. The indicator appears in the top-left.
3. Press **F6** again to stop; the replay is written to
   `<game-folder>/replays/voltlapse/`.

## 4. Where things are written

```text
<game-folder>/
├── config/voltlapse/
│   ├── config.json            settings, repaired on load if damaged
│   └── lang/<locale>.json     optional translation overrides
├── replays/voltlapse/*.vrc    recordings, one file per replay
└── exports/voltlapse/         `/voltlapse export` copies and rendered frames
```

## 5. Uninstalling

Delete `horizon-mods/voltlapse.hnmod` (and `voltlapse.hmod`, if present) and
restart. Recordings, config and exports are yours and are left alone; delete
`replays/voltlapse/`, `exports/voltlapse/` and `config/voltlapse/` yourself if
you want them gone.

## Updating

Replace the package and restart. Config files are versioned: a newer build
migrates an older `config.json` and reports anything it had to change in chat.
Replay files are forward-compatible within format version 1 — a newer build
reads every replay an older one wrote, and refuses a file from a newer format
with a clear message instead of guessing.

If something goes wrong, see [`TROUBLESHOOTING.md`](TROUBLESHOOTING.md).
