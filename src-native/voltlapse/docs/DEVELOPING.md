# Working on Volt Lapse

## Layout

```text
src-native/voltlapse/
  horizon-native.json        native manifest: id, version, pins, hooks API
  compatibility.json         the version matrix in machine-readable form
  default-config.json        the defaults Config::default() produces
  content/horizon-mod.toml   content manifest for the .hmod half
  content/                   icon, textures, language files
  lang/                      en_us, de_de, ne_np (mirrored into content/)
  mixins/voltlapse/
    horizon-mixin.toml       every hook, binding and module
    runtime.rs               the support module staged into horizon-client
    hook_*.rs (22)           two-line adapters: `game.native_on(...)` + call
    engine/src/*.rs (27)     the actual logic, a normal Rust crate
    LICENSE, NOTICE
  tools/build.py             packager, validators, reproducibility
  tests/test_build.py        packager tests
  docs/                      this documentation
```

The engine is an ordinary Rust crate with no game dependency: that is why it
can be tested by `cargo test` (and read by anyone), and why the hooks are
trivial adapters. Anything non-trivial belongs in `engine/`, not in a hook.

### Module map

| Module | Responsibility |
| --- | --- |
| `lib.rs` | `Engine`: owns config, paths, library, recorder, player, renderer |
| `config.rs` | typed settings, tolerant load, migration, repair |
| `recording.rs` | tick capture, block layout, `.tmc` writer/reader |
| `playback.rs` | replay playback state, speed, seeking, looping |
| `keyframe.rs`, `interp.rs`, `camera.rs` | camera paths, easing, shake, matrices |
| `render.rs` | offline render jobs, frame capture, filter chain |
| `hud.rs`, `png.rs` | overlay drawing, screenshot and thumbnail encoding |
| `storage.rs`, `recovery.rs` | folders, quotas, retention, crash journal |
| `keybinds.rs`, `command.rs` | actions and their command grammar |
| `lang.rs` | translation catalog and formatting |
| `package.rs`, `zipstore.rs`, `lz.rs`, `crc.rs`, `wire.rs`, `json.rs` | the file formats |
| `perf.rs`, `sync.rs`, `bookmarks.rs`, `event.rs`, `api.rs`, `platform.rs` | budget, sharing, markers, hook events, public API, PC/Android |

## Building the packages

```sh
python3 src-native/voltlapse/tools/build.py                  # -> releases/
python3 src-native/voltlapse/tools/build.py --check          # verify only
python3 src-native/voltlapse/tools/build.py --json           # reports
python3 src-native/voltlapse/tools/build.py --out /tmp/out   # elsewhere
```

The packager is stdlib-only and deterministic. It refuses a bundle whose

* `horizon-native.json` has an unknown field, a bad id or a non-commit pin;
* `horizon-mixin.toml` disagrees with the manifest on id, version or
  `horizon_revision`;
* an injection names a helper that does not exist, a module names a missing
  source, or a crate has no files;
* a file inside the mixin folder is not `.rs`, `.toml`, `data/**`,
  `README`/`LICENSE`/`NOTICE`/`.gitignore`;
* either package would exceed the size or file-count limits.

It then re-opens both zips and checks entries, duplicates, permissions,
compression and the manifests again.

## Tests

```sh
python3 -m unittest discover -s src-native/voltlapse/tests -v
```

Ten tests cover the build, reproducibility, entry rules, the language files,
and four negative cases (missing helper, disallowed file, id mismatch,
tampered archive).

The engine's own tests are `cargo test`-able (they need no game). They are not
run in this repository's CI because it has no Rust toolchain; treat a change to
`engine/src` as unverified until compiled.

## Adding a hook

1. Add an `[[injection]]` to `horizon-mixin.toml` with `hook`,
   `hook_version`, `kind`, `helper` and `bindings`.
2. Write `hook_<name>.rs`:

   ```rust
   // Once a tick: sample the world into the recording and keep its clock.
   fn voltlapse_tick(game: &mut crate::game::Game<'_>) {
       crate::game::voltlapse::tick(game);
   }
   ```

   The helper is a free function in the game's `game` module namespace; the
   real work is `runtime.rs`'s `pub fn tick(game: &mut Game<'_>)`, reached as
   `crate::game::voltlapse::tick` because the module is staged there.

3. Do the work in `runtime.rs` or, better, `engine/src`.
4. If the hook's API number is higher than `hooks` in `horizon-native.json`,
   raise it, and add the row to `docs/COMPATIBILITY.md`.
5. Run `tools/build.py`; it fails if the helper or binding is wrong.

## Adding a setting or a string

Settings: add the field to the struct in `config.rs`, the default in
`Default::default`, the JSON key in `to_json`/`from_json`, and (if it is
user-facing) the field list in the settings panel. Then regenerate
`default-config.json` — the test `the_shipped_default_config_is_the_default`
fails until it matches.

Strings: add the key to `DEFAULTS` in `lang.rs` (keep the array length in
sync), then to `lang/en_us.json`. `de_de`/`ne_np` may lag: the catalog falls
back per key, and the test only requires that the shipped files have no *extra*
keys, not that they are complete.

## Licence

The engine and hooks are LGPL-3.0-or-later (see `LICENSE`); the shipped
packages carry `NOTICE` with the Replay Mod attribution. By contributing you
agree to licence your changes the same way.
