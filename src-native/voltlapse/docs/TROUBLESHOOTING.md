# Troubleshooting

## The mod does not load

* **Check the extension.** `voltlapse.hnmod` (or `.hmod`), not `.zip`, not
  `.hnmod.txt`. Windows hides extensions by default.
* **Check the folder.** Native packages belong in `horizon-mods/` beside
  `options.txt`, not in `mods/`.
* **First launch is slow.** Native mods are compiled in. If the launcher shows
  a build, wait for it; a failure names the file and line it stopped at.
* **Version mismatch.** Volt Lapse needs a Horizon whose native hook API is 410
  or later. An older build reports the hook it is missing.

## Nothing is recorded

* Look for the indicator. If it is not there, recording never started — check
  the keybinding, and try `/voltlapse record start` to tell a key problem from
  a storage problem.
* `/voltlapse info` says what the engine thinks it is doing.
* Ensure the replay folder is writable. On Android, this is usually a
  storage-permission problem (see `INSTALL_ANDROID.md`).
* If a recording was interrupted by a crash, the recovery journal is kept in
  `config/voltlapse/journal/<session>.journal`; the next launch offers to
  recover it, and `/voltlapse record stats` reports what is left.

## A replay will not open

* "not a Volt Lapse recording": the file is a `.mcpr` or a truncated copy.
  Replay Mod's format is not compatible — see `NOTICE`.
* "recording format N is newer than this build": recorded by a newer Volt
  Lapse. Update the mod.
* "recording ends mid-block": the file was copied while it was being written.
  Record again, or copy after stopping recording.

## The game stutters while recording

* Open **Volt Lapse → Performance** and lower the frame budget (µs); the engine
  stops recording particles first, then distant entities.
* Increase the block flush size (`recording.block_ticks`) — bigger blocks mean
  fewer writes, at the cost of losing more ticks if the game dies.
* On a low-end machine, record at 10 ticks per second
  (`recording.tick_rate`); playback interpolates, so the result still looks
  smooth.

## Rendering is slow, or produces black frames

* Rendering is offline and deterministic: a slow machine takes longer but
  produces the same frames. Let it run; `/voltlapse render status` reports
  progress.
* Black frames usually mean the camera ended up inside a block or outside the
  loaded world — check the path in the camera editor (`F9`) and that the replay
  has chunks at that time.
* Reduce resolution and motion blur first; both scale cost linearly.

## The HUD will not go away

`F1` toggles the overlay without touching the recording. If the indicator is
still there while recording, that is by design: it is the only sign the
recorder is running. Hide it in the render settings (`hud.indicator`).

## Settings keep resetting

A config that cannot be parsed is replaced by the defaults and reported in
chat. Check that `config/voltlapse/config.json` is valid JSON and that its
`version` is `3`; delete it to start clean. Values out of range are clamped,
not rejected, so a hand-edited file usually still loads.

## Reporting a bug

Include: the launcher's build log, `config/voltlapse/config.json`, the output of
`/voltlapse info`, and the size and first bytes of the replay if one is
involved (`xxd -l16 file.vrc` shows the zip magic).
