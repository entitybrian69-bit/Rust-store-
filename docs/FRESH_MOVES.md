# Fresh Moves — player animation for Horizon (`.hnmod`)

A native Horizon mod that gives every player model — your own in third person and
everyone else's in multiplayer — a full set of fluid, weighty animation on top of
the vanilla one, plus first-person hand, item and camera motion and an
action-game combat layer.

- **Package:** `releases/fresh_moves-1.0.0.hnmod`
- **Bundle id / version:** `fresh_moves` `1.0.0` (native manifest schema 1)
- **Pinned engine:** Horizon `26.3`, `horizon` revision
  `a30ed9e44d35877d01f1ce5ceccb4346e9994c2d`, `mixins` revision
  `5a64af938ec86b00d53d16a408aabc0038fb3e57`, hook API `397`
- **Source:** [`src-mods/fresh_moves/`](../src-mods/fresh_moves)
- **Build + validate:** `python3 scripts/build_hnmod.py`

This is a **native** mod (`.hnmod`), not a content mod (`.hmod`). Horizon's
content pipeline has one capability — `registries` — which registers blocks,
items, recipes, tags, loot, creative tabs, mob effects, entity types and menus.
Nothing in it can pose a model, so player animation has to be code.

---

## 1. What it does

Every deformation below is **added to** the vanilla animation rather than
replacing it: walking legs, sprinting arms, crouching, eating, bow drawing and
everything else the engine already animates keep running, and Fresh Moves layers
its own motion on top. With the master switch off, the vanilla animation is
untouched — not a different asset, the original one.

### Movement

| State | What is added |
| :--- | :--- |
| **Idle** | Breathing (chest and shoulders), a slow weight shift from one foot to the other, and a small head sway. Nothing loops on an obvious beat. |
| **Walk** | Lean into the direction of travel, vertical bob at twice the stride, torso twist and counter-twist, wider arm counter-swing than vanilla, plus a bob in the first-person hand. |
| **Sprint** | Hard forward cant, longer stride, arms tucked in and driven back, head down, camera pulled back slightly, heavier first-person cant. |
| **Sneak** | Shoulders hunched over vanilla's crouch with the head kept up, knees softened, arms gathered. |
| **Jump** | Anticipation gather while the jump key is held on the ground, then arms up and body stretched on the way up. |
| **Fall** | Arms out, legs trailing, body rotating to face the fall; the faster the fall, the more the pose opens. |
| **Land** | Whole-body compression weighted by the fall speed (a hop is a blink, a three-block drop is a landing), knees absorbing, arms dropping, camera dipping. |
| **Swim** | Body pitched into the stroke, legs trailing, arms pulling; gentler surfacing bob at the water line. |
| **Climb** | Body to the wall, reach-and-pull offset per limb, head up. |
| **Ride** | Reins hand, hip roll with the mount, legs braced. |

### Actions

| State | What is added |
| :--- | :--- |
| **Mine** | The body follows the swing instead of standing behind it, shoulder rotating into the pick. |
| **Place** | A shorter follow-through than mine, weight forward. |
| **Tool swing** | Wrist and follow-through only — vanilla's own arc stays. |
| **Eat / drink** | Arm weight while the item is held, a settling wobble, a shield's extra brace; the game's own item pose still plays. |
| **Bow / crossbow / trident / spear** | The arm's held weight plus a shake that grows with the draw — a drawn bow is never still. |
| **Shield** | Braced arm, shoulder forward, hand raised. |
| **Use (brush, spyglass, wind charge)** | A steady hold with a small tremor instead of a frozen arm. |

### Combat

| State | What is added |
| :--- | :--- |
| **Sword combo** | Three hits, each a different arc: a downward diagonal from the right shoulder, a backhand that carries the turn the other way, then an overhead with both arms and the whole body behind it. A hit within 13 ticks continues the chain; a slower one starts it over. |
| **Heavy attack** | The third hit of the chain reads heavier: deeper body drop, both arms, legs bracing. |
| **Dodge / roll** | Sprint + sneak + a sideways push: a fast lean and tuck with the legs crossing under it. Visual only — your momentum is still the game's. |
| **Sprint attack** | A wide, low cut that only reads at a run, with the torso twisting through it. |
| **Hurt** | Chest pulled back, head away, arms flinching, hand jolting — strongest on the tick it lands, gone within half a second. |
| **Death** | Limbs let go over vanilla's own fall, so the ragdoll reads as weight rather than a cut animation. |

### Polish

- **Eased blending.** Every channel is a weight that fades in and out over the
  configured blend time (0–0.6 s), so no state change snaps. A walk that stops
  coasts.
- **Head tracking.** The torso and shoulders turn a little toward where the head
  looks, and the head keeps a sliver of lag so the neck reads as weight.
- **First-person camera sway.** A fraction of a degree of pitch, yaw and roll
  following the stride, the sprint, the landing, the swing and a hit, plus a
  small vertical offset — added to the game's own shake, never instead of it.
- **Frame-rate independent.** Tick-rate events are smoothed at frame rate
  (`native_frame`), so the hand and the camera move at your frame rate, not at
  20 Hz.

---

## 2. Configuration

The mod has two layers, and they compose:

1. **The config file**, `config/fresh_moves-client.toml`, written by the game the
   first time it is missing and re-read when it changes on disk (Horizon polls
   mtimes once a second). Every key is documented in the file itself.
2. **The in-game page**, *Options → Mods → Fresh Moves*, which keeps its own
   copies of the switches in the game's mod settings (one small file per key).
   The page wins over the file for the switches it shows; *Reset to the config
   file* drops the page's copies so the file speaks alone again.

```toml
master = true          # false takes every pose off the player again
preset = "default"     # subtle | default | action
speed = 1.00           # 0.5 to 2.0: how fast the cycles run
intensity = 1.00       # 0.0 to 1.5: how far every offset goes
blend = 0.18           # 0.0 to 0.60 seconds: fade time per channel
first_person = true    # hand, item and arm motion
third_person = true    # body poses on the vanilla player model
head_tracking = true   # torso follows the head
fp_sway = true         # the extra camera sway

[animations]           # one switch per channel
idle = true  walk = true  sprint = true  sneak = true  jump = true
fall = true  land = true  swim = true  climb = true  ride = true
mine = true  place = true  eat = true  drink = true  bow = true
shield = true  item_use = true  tool_swing = true  combo = true
heavy = true  dodge = true  sprint_attack = true  hurt = true  death = true
```

**Presets** change intensity and blend only — a per-animation switch you turned
off stays off across a preset change.

| Preset | Intensity | Blend |
| :--- | :---: | :---: |
| Subtle | 0.55 | 0.30 s |
| Default | 1.00 | 0.18 s |
| Action | 1.35 | 0.10 s |

Malformed files never stop the mod: a key it cannot read leaves that field at its
default, unknown keys are ignored, and comments and case are free (`Preset =
"Action"` is the same as `preset = "action"`).

---

## 3. How it is built (and why it cannot break another mod)

The bundle is two support modules and twelve hook helpers. Nothing is patched
into another mod's code, and no hook point used here is cancellable.

| File | Role |
| :--- | :--- |
| `mixins/fresh_moves/runtime.rs` | The animation runtime: math, config, per-player animator, body/hand/camera pose generation, the registry, and the unit tests. **Names no game type**, so it is testable on its own. |
| `mixins/fresh_moves/native.rs` | The adapter: reads the game's player models, fills a `Senses` snapshot, and installs a pose on each model. All game API use is here. |
| `tick.rs`, `frame.rs`, `input.rs`, `hand.rs`, `shake.rs`, `cam.rs`, `cfg_register.rs`, `cfg_load.rs`, `cfg_reload.rs`, `mods.rs`, `settings.rs`, `click.rs` | One private function per hook, each a binding and nothing else. |

Hooks used, all by **name and contract version** from the engine's own
`native-hooks.json` rather than by source path, so a hook point can move in the
source without breaking the bundle:

| Hook | Version | Why |
| :--- | :---: | :--- |
| `tick` | 1 | Read the players in view and animate them. |
| `frame` | 1 | Advance the frame-rate smoothing for hand and camera. |
| `movement_input` | 1 | Read the local player's input (read only — nothing written back). |
| `render_hand_pose` | 1 | Add the first-person hand/item pose. |
| `camera_shake` | 1 | Add the sway to the game's shake. |
| `camera_distance` | 1 | Pull the third-person camera back at a run, in on a hit. |
| `register_configs` | 1 | Register `config/fresh_moves-client.toml`. |
| `config_loading` / `config_reloading` | 1 | Parse it, at startup and on change. |
| `mods` | 1 | Its row under Options → Mods. |
| `settings` / `settings_click` | 1 | The settings page and its buttons. |

**Sharing a model with other mods.** A player model has one pose slot
(`Mob::emote`, the same `BodyPose` hook Horizon's cosmetics emotes use). Fresh
Moves only installs its pose when the slot is empty or already its own: a
cosmetics emote, or another animation mod's pose, is left exactly as it is and
Fresh Moves takes the model back when that pose ends. It also never cancels a
hook, never writes to the world, the inventory or the player's motion, and never
touches another mod's settings folder or config file.

Nothing here requires a dependency: the bundle uses only the standard library and
whatever the game crate already links.

---

## 4. Install

### PC (Horizon launcher, `horizon-rust/horizon`)

1. Put `fresh_moves-1.0.0.hnmod` in the launcher's `horizon-native-mods/`
   folder (beside the launcher's other native bundles).
2. Start the launcher and install/launch the game once. The launcher compiles
   every `.hnmod` in that folder into the client (Rust; first build takes a few
   minutes, later starts are incremental).
3. In game: **Options → Mods** shows *Fresh Moves 1.0.0*. Open it to configure.

### Android (Starling, formerly Horizon Android)

1. Import `fresh_moves-1.0.0.hnmod` in the app (the native-mods screen accepts
   the file directly) or copy it into `horizon-native-mods/`.
2. The app builds the bundle in its build service / a GitHub runner and swaps the
   library in on the next launch. Builds are SHA-256 checked against the library
   that replaces the old one.
3. The in-game **Options → Mods** page is the same as on PC.

If the Android build service is unavailable for the revision you run, the bundle
can be compiled into the client from source (it needs no Cargo changes: it is a
support module plus hooks, compiled by the launcher's own build). The launcher is
the supported path on both platforms.

> The `.hnmod` is a **source bundle**, by design: Horizon compiles native mods
> from source into the pinned engine, which is what makes the fast paths fast.
> It is not an Android `.so` or an APK.

---

## 5. Build and validate

```sh
# Check the source folder and rebuild the .hnmod deterministically, then validate it
python3 scripts/build_hnmod.py

# Validate the packaged archive only
python3 scripts/build_hnmod.py --check-only
```

The validator enforces the reader's own rules — the zip's shape, paths and
limits (512 entries, 64 MiB, 64 MiB per file), the manifest's fields
(`deny_unknown_fields`, `[a-z0-9_]{1,64}` id, 40-hex revisions), the
`mixins/<id>/` prefix, which files may live there, that `[mod]` id/version and
`[compatibility] horizon_revision` agree with the manifest, that every hook name
exists in the vendored `scripts/native-hooks-catalog.json` at the version used,
that every helper is exactly one private function, and that no payload uses inner
attributes, `mod name;`, `include!`, `unsafe`, or a `todo!`/`panic!`.

The unit tests in `runtime.rs` (`#[cfg(test)] mod tests`) cover the config
round-trip, a broken file, comments and case, presets, the easings, smoothing,
channel fade, the three-hit combo and its window, landings, flinches, and that no
pose exceeds what the engine can draw.

---

## 6. Assumptions and limits

- **Tested against a pinned source, not run here.** One Horizon revision is
  named in the manifest (`a30ed9e4…`) and every game field and signature the
  adapter uses was read from it. If a later revision renames a field, the
  launcher's build fails with the compiler's error rather than misbehaving
  quietly — that is the point of `hooks` + `horizon_revision`.
- **Vanilla item poses are kept.** Eat, drink, bow and shield use the engine's
  own item animation; Fresh Moves adds the arm's weight, the tremor and the
  first-person additions on top instead of replacing a pose it does not own.
- **Third person needs the vanilla player model.** Fresh Moves poses
  `Mob::emote` on player models. Other mods' custom player models that do not go
  through that hook are not posed.
- **Other players' switches are yours.** A multiplayer client poses other
  players with *your* settings; their config is not readable from your window.
- **One mod's pose slot.** If another animation mod also installs a pose on
  player models, the first one to take a model keeps it (see §3) — Fresh Moves
  never fights for it.
- **No items, blocks or world content** — this mod adds none.
