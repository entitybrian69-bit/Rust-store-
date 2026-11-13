// Fresh Moves: the adapter to the game.
//
// Compiled into `crate::game` as the support module `fresh_moves_native`
// (`[[module]] name = "fresh_moves_native"`). It reads the game's own player
// models once a tick, hands the runtime a `Senses` snapshot, and lays the
// runtime's pose over each model through the engine's documented pose hook,
// `Mob::emote` (`crate::mobs::anim::BodyPose`, the same one Horizon's
// cosmetics emotes use), so the vanilla model keeps its skin, armour, held
// items and name tag and gains our motion on top.
//
// Only what the game itself already does is touched:
//   * a model with somebody else's pose (a cosmetics emote, another mod) is
//     left exactly as it is, and gets ours back when that pose ends;
//   * nothing is written to the world, the inventory or the player's motion:
//     this is a client-side animation mod.

use super::Game;
use super::fresh_moves as fm;
use crate::mobs::Mob;
use crate::mobs::anim::BodyPose;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The pose laid over a player model: it holds only the entity id and reads the
/// runtime's animator for that id each frame, so a tick updates the animation
/// without allocating a new pose.
#[derive(Debug)]
struct PoseOf {
    id: u32,
}

impl BodyPose for PoseOf {
    fn pose(&self, age: f32, _at_rest: &[[f32; 9]; 6], poses: &mut [[f32; 9]; 6]) -> bool {
        fm::body_pose(self.id, age, poses)
    }
}

/// The entity id of our own pose, if `pose` is ours rather than a cosmetics
/// emote or another mod's.
fn ours_id(pose: &Arc<dyn BodyPose>) -> Option<u32> {
    let any: &dyn std::any::Any = &**pose;
    any.downcast_ref::<PoseOf>().map(|pose| pose.id)
}

/// Puts our pose on `model` when it may have one, and takes ours off when it
/// may not. A model somebody else is already posing is left alone: a cosmetics
/// emote or another animation mod keeps the model for as long as it wants it.
fn dress(model: &mut Mob, id: u32) {
    let ours = model.emote.as_ref().and_then(ours_id);
    if model.emote.is_some() && ours.is_none() {
        return;
    }
    if !fm::poses_enabled() {
        if ours.is_some() {
            model.emote = None;
        }
        return;
    }
    if ours == Some(id) {
        // Already ours, and reading the same animator.
        return;
    }
    model.emote = Some(Arc::new(PoseOf { id }) as Arc<dyn BodyPose>);
}

/// Whether a model is a player, so the runtime animates it.
fn is_player(model: &Mob) -> bool {
    model.kind.id == "minecraft:player"
}

/// One model's state, as the runtime wants it.
fn senses_of(model: &Mob, local: bool) -> fm::Senses {
    let v = model.velocity;
    let speed = ((v.x * v.x + v.z * v.z).sqrt()) as f32;
    // The window's own model is not always given its sprint flag by the local
    // simulation; a speed past a walk's is the same thing, measured.
    let sprinting = model.extra.sprinting || (local && speed > 0.125);
    fm::Senses {
        age: model.age as f32,
        on_ground: model.on_ground,
        sprinting,
        crouching: model.extra.crouching,
        climbing: model.climbing,
        swimming: model.fluids.in_water,
        riding: model.extra.passenger,
        vy: v.y as f32,
        speed,
        walk_position: model.limb_position,
        walk_speed: model.limb_speed,
        hurt_time: model.hurt_time,
        death_time: model.death_time,
        swing: model.anim.attack_anim,
        use_progress: 0.0,
        use_kind: fm::UseKind::None,
        head_yaw: model.head_yaw,
        head_pitch: model.head_pitch,
        local,
        ..fm::Senses::default()
    }
}

/// What item the local player is using, and how far in: the kind only shapes
/// the first-person additions, so a name it does not know is harmless.
fn use_state(game: &Game<'_>) -> (f32, fm::UseKind) {
    let Some((slot, ticks)) = game.using else {
        return (0.0, fm::UseKind::None);
    };
    let duration = game.use_duration.max(1) as f32;
    let progress = fm::clamp_f(ticks as f32 / duration, 0.0, 1.0);
    let kind = game
        .inventory
        .get(slot)
        .and_then(Option::as_ref)
        .map(|stack| classify(&stack.item))
        .unwrap_or(fm::UseKind::Other);
    (progress, kind)
}

/// The kind of use an item id is, by the names vanilla gives them.
fn classify(item: &str) -> fm::UseKind {
    let name = item.strip_prefix("minecraft:").unwrap_or(item);
    match name {
        "bow" | "crossbow" | "trident" | "spear" => fm::UseKind::Bow,
        "shield" => fm::UseKind::Shield,
        "potion" | "splash_potion" | "lingering_potion" | "honey_bottle" | "milk_bucket"
        | "ominous_bottle" => fm::UseKind::Drink,
        "spyglass" | "brush" | "wind_charge" => fm::UseKind::Other,
        _ => fm::UseKind::Eat,
    }
}

/// Once a client tick: read every player the window can see, animate it, and
/// keep the pose on its model.
pub(super) fn tick(game: &mut Game<'_>) {
    // Only a window with a world in it has models to pose. A dedicated server,
    // a server thread or the title screen leaves this alone.
    if game.player_model.is_none() {
        // No world, no players: nothing in view to animate, and the state of
        // the last one is let go rather than kept for the menu screen.
        fm::forget_all();
        return;
    }
    load_settings_once(game);
    let (use_progress, use_kind) = use_state(game);

    if let Some(model) = game.player_model.as_mut() {
        let id = model.id;
        let mut senses = senses_of(model, true);
        senses.use_progress = use_progress;
        senses.use_kind = use_kind;
        fm::tick_player(id, senses);
        dress(model, id);
    }

    // Every other player in view: other players' own config is not ours to
    // read, so their poses are cut when this window switches third person off.
    for model in game.mobs.iter_mut() {
        if !is_player(model) {
            continue;
        }
        let id = model.id;
        let senses = senses_of(model, false);
        fm::tick_player(id, senses);
        dress(model, id);
    }
}

/// Once a frame: advance the frame-rate smoothing the first-person hand and the
/// camera use.
pub(super) fn frame(elapsed: std::time::Duration) {
    fm::frame(elapsed.as_secs_f32());
}

/// The movement input, for the local player's dodge and gait.
pub(super) fn movement_input(forward: &mut f32, strafe: &mut f32, jump: &mut bool, sneak: &mut bool) {
    // Read only: changing the player's input is not this mod's business.
    fm::set_input(*forward, *strafe, *jump, *sneak);
}

/// Adds the first-person hand and item pose.
pub(super) fn hand_pose(swing: f32, equip: f32, x: &mut f32, y: &mut f32, z: &mut f32, pitch: &mut f32, yaw: &mut f32, roll: &mut f32, scale: &mut f32) {
    if !fm::first_person_enabled() {
        return;
    }
    let mut pose = fm::HandPose {
        scale: 1.0,
        ..fm::HandPose::default()
    };
    fm::hand_pose(swing, equip, &mut pose);
    *x += pose.x;
    *y += pose.y;
    *z += pose.z;
    *pitch += pose.pitch;
    *yaw += pose.yaw;
    *roll += pose.roll;
    *scale *= pose.scale;
}

/// Adds the camera sway to the game's own shake.
pub(super) fn camera_shake(partial: f32, shake: &mut crate::game::native_hooks::NativeShake) {
    let mut ours = fm::Shake::default();
    fm::camera_shake(partial, &mut ours);
    shake.pitch += ours.pitch;
    shake.yaw += ours.yaw;
    shake.roll += ours.roll;
    for (i, v) in ours.offset.iter().enumerate() {
        shake.offset[i] += v;
    }
}

/// Pulls the third-person camera back at a run and in on a hit.
pub(super) fn camera_distance(partial: f32, distance: &mut f32) {
    fm::camera_distance(partial, distance);
}

/// The config file the game writes to `config/fresh_moves-client.toml` when it
/// is missing.
pub(super) fn register_configs(configs: &mut Vec<crate::game::native_configs::NativeConfig>) {
    configs.push(crate::game::native_configs::NativeConfig::new(
        fm::ID,
        crate::game::native_configs::NativeConfigKind::Client,
        &fm::default_toml(),
    ));
}

/// A config file was read (at startup or again after it changed on disk).
pub(super) fn config_loaded(game: &mut Game<'_>, config: &crate::game::native_configs::NativeConfigFile) {
    if config.mod_id != fm::ID {
        return;
    }
    let parsed = fm::load_config(&config.text);
    // Kept for the page's reset button: the file's own values.
    if let Ok(mut file) = FILE_CONFIG.lock() {
        *file = Some(parsed);
    }
    // What the file says is the base; the page's own switches sit on top.
    let mut merged = fm::config();
    merge_saved(game, &mut merged);
    fm::set_config(merged);
    SETTINGS_LOADED.store(true, Ordering::Relaxed);
}

/// The mods list: the row under Options > Mods.
pub(super) fn mods(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: fm::ID.to_owned(),
        title: format!("Fresh Moves {}", fm::VERSION),
        description: fm::DESCRIPTION.to_owned(),
        keys: Vec::new(),
    });
}

/// The buttons of the mod's settings page.
pub(super) fn settings(game: &mut Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id != fm::ID {
        return;
    }
    load_settings_once(game);
    let config = fm::config();
    let on_off = |on: bool| if on { "ON" } else { "OFF" };
    buttons.push(format!("Fresh Moves: {}", on_off(config.master)));
    buttons.push(format!("Preset: {}", config.preset.label()));
    buttons.push(format!("Intensity: {:.0}%", config.intensity * 100.0));
    buttons.push(format!("Speed: {:.0}%", config.speed * 100.0));
    buttons.push(format!("Blend: {:.0} ms", config.blend * 1000.0));
    buttons.push(format!("First person: {}", on_off(config.first_person)));
    buttons.push(format!("Third person: {}", on_off(config.third_person)));
    buttons.push(format!("Head tracking: {}", on_off(config.head_tracking)));
    buttons.push(format!("First-person sway: {}", on_off(config.fp_sway)));
    buttons.push("Reset to the config file".to_owned());
}

/// A click on one of those buttons.
pub(super) fn settings_click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != fm::ID {
        return;
    }
    load_settings_once(game);
    let mut config = fm::config();
    match button {
        0 => config.master = !config.master,
        1 => {
            let preset = config.preset.next();
            config.apply_preset(preset);
        }
        2 => config.intensity = next_of(&INTENSITIES, config.intensity),
        3 => config.speed = next_of(&SPEEDS, config.speed),
        4 => config.blend = next_of(&BLENDS, config.blend),
        5 => config.first_person = !config.first_person,
        6 => config.third_person = !config.third_person,
        7 => config.head_tracking = !config.head_tracking,
        8 => config.fp_sway = !config.fp_sway,
        // The file again, and the game's copies of the switches dropped.
        9 => {
            config = FILE_CONFIG
                .lock()
                .ok()
                .and_then(|file| file.clone())
                .unwrap_or_default();
            clear_saved(game);
        }
        _ => return,
    }
    fm::set_config(config.clone());
    save_settings(game, &config);
}

/// The values the three sliders step through: the same ladder the presets and
/// the file use, so a click never lands on a value the file could not hold.
const INTENSITIES: [f32; 7] = [0.4, 0.6, 0.8, 1.0, 1.2, 1.4, 1.5];
const SPEEDS: [f32; 6] = [0.6, 0.8, 1.0, 1.2, 1.4, 2.0];
const BLENDS: [f32; 7] = [0.0, 0.06, 0.12, 0.18, 0.25, 0.35, 0.6];

/// The next step of a ladder, the first after one that is not on it.
fn next_of(ladder: &[f32], value: f32) -> f32 {
    match ladder.iter().position(|v| (v - value).abs() < 0.005) {
        Some(i) => ladder[(i + 1) % ladder.len()],
        None => ladder[0],
    }
}

// ---------------------------------------------------------------------------
// The game's own storage for a mod's settings: one small file per key, kept by
// the game beside the world's mod list (docs/native-mods.md).
// ---------------------------------------------------------------------------

const KEY_MASTER: &str = "master";
const KEY_PRESET: &str = "preset";
const KEY_INTENSITY: &str = "intensity";
const KEY_SPEED: &str = "speed";
const KEY_BLEND: &str = "blend";
const KEY_FIRST_PERSON: &str = "first_person";
const KEY_THIRD_PERSON: &str = "third_person";
const KEY_HEAD_TRACKING: &str = "head_tracking";
const KEY_FP_SWAY: &str = "fp_sway";

/// The config the file itself gave, kept for the page's reset button.
static FILE_CONFIG: std::sync::Mutex<Option<fm::Config>> = std::sync::Mutex::new(None);
static SETTINGS_LOADED: AtomicBool = AtomicBool::new(false);

/// Reads the page's saved switches once a session, and puts them over the
/// file's values.
fn load_settings_once(game: &Game<'_>) {
    if SETTINGS_LOADED.swap(true, Ordering::Relaxed) {
        return;
    }
    let mut config = fm::config();
    merge_saved(game, &mut config);
    fm::set_config(config);
}

/// Puts the game's saved switches over `config`, where it has them.
fn merge_saved(game: &Game<'_>, config: &mut fm::Config) {
    let read = |key: &str| game.native_setting(fm::ID, key);
    if let Some(v) = read(KEY_MASTER) {
        config.master = v != "off";
    }
    if let Some(v) = read(KEY_PRESET).and_then(|v| fm::Preset::parse(&v)) {
        config.preset = v;
    }
    if let Some(v) = read(KEY_INTENSITY).and_then(|v| v.parse::<f32>().ok()) {
        config.intensity = fm::clamp_f(v, 0.0, 1.5);
    }
    if let Some(v) = read(KEY_SPEED).and_then(|v| v.parse::<f32>().ok()) {
        config.speed = fm::clamp_f(v, 0.5, 2.0);
    }
    if let Some(v) = read(KEY_BLEND).and_then(|v| v.parse::<f32>().ok()) {
        config.blend = fm::clamp_f(v, 0.0, 0.6);
    }
    if let Some(v) = read(KEY_FIRST_PERSON) {
        config.first_person = v != "off";
    }
    if let Some(v) = read(KEY_THIRD_PERSON) {
        config.third_person = v != "off";
    }
    if let Some(v) = read(KEY_HEAD_TRACKING) {
        config.head_tracking = v != "off";
    }
    if let Some(v) = read(KEY_FP_SWAY) {
        config.fp_sway = v != "off";
    }
}

/// Keeps the switches the page owns. The file itself is never written: it is
/// the player's, and the game re-reads it when it changes.
fn save_settings(game: &Game<'_>, config: &fm::Config) {
    let on_off = |on: bool| if on { "on" } else { "off" };
    game.native_set_setting(fm::ID, KEY_MASTER, on_off(config.master));
    game.native_set_setting(fm::ID, KEY_PRESET, config.preset.name());
    game.native_set_setting(fm::ID, KEY_INTENSITY, &format!("{:.2}", config.intensity));
    game.native_set_setting(fm::ID, KEY_SPEED, &format!("{:.2}", config.speed));
    game.native_set_setting(fm::ID, KEY_BLEND, &format!("{:.2}", config.blend));
    game.native_set_setting(fm::ID, KEY_FIRST_PERSON, on_off(config.first_person));
    game.native_set_setting(fm::ID, KEY_THIRD_PERSON, on_off(config.third_person));
    game.native_set_setting(fm::ID, KEY_HEAD_TRACKING, on_off(config.head_tracking));
    game.native_set_setting(fm::ID, KEY_FP_SWAY, on_off(config.fp_sway));
}

/// Drops the page's own switches, so the file speaks alone again.
fn clear_saved(game: &Game<'_>) {
    for key in [
        KEY_MASTER,
        KEY_PRESET,
        KEY_INTENSITY,
        KEY_SPEED,
        KEY_BLEND,
        KEY_FIRST_PERSON,
        KEY_THIRD_PERSON,
        KEY_HEAD_TRACKING,
        KEY_FP_SWAY,
    ] {
        game.native_set_setting(fm::ID, key, "");
    }
}
