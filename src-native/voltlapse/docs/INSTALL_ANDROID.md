# Installing Volt Lapse on Android

Horizon on Android uses the same package formats as the desktop build. Both
architectures are supported (arm64-v8a and armeabi-v7a); the package is plain
Rust source plus assets, so the launcher builds it for the device on first
launch.

## 1. Locate the game directory

The launcher's game directory is normally:

```text
/storage/emulated/0/Android/data/<launcher-package>/files/.minecraft
```

or, for launchers that keep data app-private:

```text
/data/data/<launcher-package>/files/.minecraft
```

Inside it you need the same folders the PC build uses: `options.txt`,
`horizon-mods/`, `config/`, `replays/`.

## 2. Copy the package

Using a file manager (or `adb push`):

```sh
adb push voltlapse.hmod /sdcard/Download/
# then move it, in the file manager, into
#   .minecraft/horizon-mods/voltlapse.hmod
```

`voltlapse.hmod` is the better choice on Android: it carries the language files
and the icon, and those are read through the content namespace regardless of
locale.

## 3. First launch

* The first build is slow on mobile — expect a minute or more, and keep the
  app in the foreground.
* Storage permission matters: without it the mod cannot create
  `replays/voltlapse/` and will say so in chat rather than failing silently.
  Grant "All files access" (Android 11+) or keep the game directory in
  app-private storage.

## 4. Playing on a phone

There is no keyboard, so use the on-screen controls the launcher provides and
the commands:

```text
/voltlapse record start
/voltlapse record stop
/voltlapse play
/voltlapse render start
```

Every action that has a keybinding also has a command, and every command is tab
completable. If your launcher maps a controller, bind the actions in
**Options → Controls → Volt Lapse** — the mod reads vanilla key names, so any
input the game can bind works (`key.mouse.middle`, `key.keyboard.f6`, …).

## 5. Performance and heat

Recording is the expensive part on a phone. The defaults are chosen for
mid-range hardware:

* 20 ticks per second, block flush every 200 ticks;
* recording events sampled, not every particle;
* rendering is *not* recommended on-device below 4 GB of RAM — export the
  replay to a PC and render there (`/voltlapse export`, then copy the `.vrc`).

If the game stutters while recording, open **Volt Lapse → Performance** and
lower the frame budget, or run `/voltlapse config performance frame_budget_us 1500`.

## 6. Sharing between phone and PC

Replays are ordinary files. Copy `.vrc` files between
`replays/voltlapse/` on both devices; nothing is platform specific, and a
replay carries its camera paths with it.
