# File formats

Everything Volt Lapse writes is a plain file with a documented layout. Nothing
here needs the mod to read: a `.vrc` is a zip, its metadata is JSON, and the
recording stream is a chunked binary with a four-byte magic per piece.

Replay Mod's `.mcpr` is the ancestor of `.vrc`; the metadata keeps the fields
shared tooling expects (`fileFormat`, `fileFormatVersion`, `mcversion`,
`markers`, …) and adds the Volt Lapse camera path, HUD overlay, bookmarks and
thumbnail. `.vrc` is *not* `.mcpr`: a `.mcpr` holds a packet stream this engine
does not decode, and an `.vrc` holds the engine's own tick stream. Neither
opens in the other, by design — see `NOTICE`.

## 1. `.vrc` — a replay

A stored (uncompressed) zip with three entries:

```text
metaData.json     UTF-8 JSON, pretty printed
recording.tmc     the tick stream (see §2)
thumbnail.png     optional, 320×180 PNG
```

`fileFormat` is `"MCPR"` and `fileFormatVersion` is `14`, values Replay Mod
tooling already understands; `version` (the metadata document's own version) is
`1`. Unknown keys are ignored on read and dropped on write, so a newer build's
metadata still opens.

The writing stamp is fixed (1980-01-01 00:00, mode `0644`, stored), which makes
a replay reproducible: recording the same input twice, or re-exporting the same
replay, yields byte-identical files. That matters for `/voltlapse share`, which
compares hashes to decide whether to send a file at all.

### metaData.json

Written by `ReplayMetadata::to_json`, read by `from_json`, which repairs rather
than refuses (a missing key takes its default; a newer `version` than the build
knows is an error).

| Key | Meaning |
| --- | --- |
| `version` | metadata document version, `1` |
| `fileFormat`, `fileFormatVersion` | `"MCPR"`, `14` |
| `name`, `description` | the replay's title and notes shown in the viewer |
| `date` | Unix seconds the recording started |
| `duration` | length in **ticks** (divide by `tickRate` for seconds) |
| `tickRate` | ticks per second, `20` by default |
| `mcversion` | game version string from the recording header |
| `protocol` | protocol number of the session, `0` if unknown |
| `generator` | `Volt Lapse <version>` |
| `selfId` | the recorder's UUID |
| `players` | `[{uuid, name}]` |
| `serverName`, `singleplayer` | where it was recorded |
| `tags` | free-form strings for the library filter |
| `cameraPaths` | array of camera paths (§3) |
| `activePath` | index into `cameraPaths` |
| `overlays` | the HUD overlay configuration used for renders |
| `bookmarks` | `[{tick, name, color}]` markers on the timeline |
| `render` | last render job for this replay, or `null` |
| `notes` | free text |

## 2. `recording.tmc` — the tick stream

Little-endian, varint-encoded counters, string = `varint` length + UTF-8 bytes.

```text
header:
  "VTLR"                4 bytes magic
  u16 format_version    currently 1
  u8  opcode_version    currently 1
  u16 tick_rate         ticks per second (20 by default)
  u16 block_ticks       ticks per block (200 by default)
  str mc_version
  str recorder

blocks (repeated to end of file):
  "BLK1"                4 bytes magic
  varint first_tick
  varint ticks
  varint raw_len        uncompressed payload size
  varint packed_len     bytes that follow
  packed_len bytes      the payload, LZ-packed or raw
```

A reader builds an index by walking the block headers and never needs to
decompress to seek: `first_tick` locates the block, the payload is decoded on
demand. That is why a four-hour recording streams in constant memory.

The payload is a sequence of per-tick opcode records. `opcode_version` guards
the opcode table itself, separate from the container version, so a build can
add an opcode without invalidating old readers and — more importantly — refuses
a stream whose opcodes it does not know instead of misdecoding it.

### Packing

Each block is passed through `lz::pack`: an LZ77 encoder with a 32 KiB window,
4-byte minimum match, 4096-byte maximum match, hash chains of 48 and a
tag-byte literal/match split. Packing is only kept when it actually shrinks the
data, and the choice is recorded by a `VLC1` magic at the front. A block
without that magic is raw. `VLC1` is a Volt Lapse codec, not DEFLATE; a tool
that wants to read blocks without implementing it can use any decompressor
implementation of the same 60-line algorithm (`engine/src/lz.rs`).

## 3. Camera paths

Camera paths are stored *inside* the replay metadata under `cameraPaths`, one
JSON object per path (`CameraPath::to_json` / `from_json`). Copy one into a file
of your own if you want to move it between replays — it is self-contained:

```json
{
  "name": "Sunset flyby",
  "keyframes": [
    { "tick": 0,   "x": 0.5,  "y": 64.0, "z": 0.5,
      "yaw": 0.0,  "pitch": 0.0,   "roll": 0.0, "fov": 70.0,
      "interpolation": "linear", "tension": 0.5, "shake": 0.0, "label": "start" },
    { "tick": 200, "x": 40.0, "y": 80.0, "z": 12.0,
      "yaw": 45.0, "pitch": -12.0, "roll": 3.5, "fov": 55.0,
      "interpolation": "catmull_rom", "tension": 0.5, "shake": 0.2, "label": "" }
  ],
  "speed": [ { "from": 0, "to": 200, "speed": 2.0 } ]
}
```

* `interpolation` is the mode of the segment *starting* at that keyframe:
  `linear`, `catmull_rom`, `hermite`, `ease_in`, `ease_out`, `ease_in_out`,
  `constant`. Parsing accepts `-`/space for `_` and aliases such as `step` and
  `cubic_spline`.
* `tension` (0..1) shapes `hermite`; `shake` (0..1) blends in a handheld jitter
  for that segment.
* Angles are degrees; `pitch` is clamped to ±90, `fov` to 1..179, and yaw/roll
  wrap, so a hand-edited file cannot produce a broken camera.
* `speed` remaps time: between ticks `from` and `to` the world plays at
  `speed`×. Segments that overlap or have `to <= from` are ignored.
* Keyframes are sorted by `tick` on load; duplicates keep the later one.

## 4. `config.json`

See `default-config.json` at the bundle root for the exact shape. `version` is
`3`; sections are `recording`, `playback`, `rendering`, `camera`, `hud`,
`keybinds`, `storage`, `performance`, `ui`. Loading is tolerant: unknown keys
are dropped, out-of-range values are clamped to the nearest legal value, colour
strings accept `#RRGGBB`/`#AARRGGBB`/decimal and become `u32` again on write,
and a damaged file is replaced by the defaults with a message in chat. Writes
go to `config.json.tmp` and are renamed, so a crash mid-save cannot truncate
the config.

## 5. `lang/<locale>.json`

A flat object of `key: "text"` pairs. Missing keys fall back to English per
key, so a partial translation is always usable. `{0}`, `{1}` are positional
placeholders filled by `Catalog::format`. `locale_for` maps `de`/`de_de` →
`de_de`, `ne`/`ne_np` → `ne_np`, `es` → `es_es` (English fallback until a file
exists), everything else → `en_us`.

## 6. Packages

`voltlapse.hnmod` and `voltlapse.hmod` are deterministic stored zips. The
native manifest is `horizon-native.json`:

| Field | Meaning |
| --- | --- |
| `schema` | `1` |
| `id` | `voltlapse` — must equal the `horizon-mixin.toml` id |
| `version` | `1.0.0` — must equal the mixin manifest version |
| `game` | `26.3`, the game version the bundle was built for |
| `horizon` | the Horizon commit it was tested on |
| `mixins` | the horizon-mixins commit it was built with |
| `hooks` | the `NATIVE_HOOKS_API` the bundle's highest hook requires (`410`) |

`hooks` is not a revision pin: it says the bundle needs hook points of at least
that API level, so it also builds on any later Horizon that still has those
points. `horizon` then only records where it was tested.
