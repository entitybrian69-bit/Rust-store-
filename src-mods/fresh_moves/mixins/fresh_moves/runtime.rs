// Fresh Moves: the animation runtime.
//
// Compiled into `crate::game` as the support module `fresh_moves`
// (`[[module]] name = "fresh_moves"` in `horizon-mixin.toml`). Nothing here
// names a game type: the adapter in `native.rs` reads the game and fills a
// [`Senses`] snapshot, and this module turns snapshots into poses. That keeps
// every easing curve, blend weight, combo rule and config rule testable on its
// own, with the game out of the picture.
//
// Units, as the engine takes them:
//   * body poses: a part's place in model pixels (16 to a block), its rotation
//     in radians, and its scale; the pose is *added* to what vanilla animation
//     left, so walking, sprinting, eating and bow drawing keep their vanilla
//     motion and this only layers on top (docs/cosmetics.md, `BodyPose`).
//   * first person hand: blocks for x/y/z, degrees for pitch/yaw/roll, and a
//     scale multiplier (docs/native-mods.md, `native_render_hand_pose`).
//   * camera shake: degrees for the turns, blocks for the offset, added to the
//     game's own shake (docs/native-mods.md, `native_camera_shake`).
//
// Everything a frame does is allocation free after the first tick for an
// entity, and the per-frame body path takes one mutex and no lock ordering.

// The mod's identity, as `horizon-native.json` and `horizon-mixin.toml` give it.
pub const ID: &str = "fresh_moves";
pub const VERSION: &str = "1.0.0";
pub const DESCRIPTION: &str =
    "Fresh Moves-style fluid player animation plus action-game combat motion, in first and third person.";

// ---------------------------------------------------------------------------
// Small math: no dependencies, no `unsafe`, no panics.
// ---------------------------------------------------------------------------

/// `x` kept inside `lo..=hi`, with a NaN answer of `lo` rather than a panic.
pub fn clamp_f(x: f32, lo: f32, hi: f32) -> f32 {
    if x.is_finite() {
        x.max(lo).min(hi)
    } else {
        lo
    }
}

/// Degrees to radians.
pub fn rad(degrees: f32) -> f32 {
    degrees * core::f32::consts::PI / 180.0
}

/// Smooth start and end: 0 and 1 arrive with no velocity.
pub fn ease_in_out(t: f32) -> f32 {
    let t = clamp_f(t, 0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let u = -2.0 * t + 2.0;
        1.0 - u * u * u / 2.0
    }
}

/// Fast start, soft end: a strike, not a slide.
pub fn ease_out(t: f32) -> f32 {
    let u = 1.0 - clamp_f(t, 0.0, 1.0);
    1.0 - u * u * u
}

/// A bell over `0..=1`: 0 at both ends, 1 at `peak`.
pub fn arc(t: f32, peak: f32) -> f32 {
    let t = clamp_f(t, 0.0, 1.0);
    let peak = clamp_f(peak, 0.05, 0.95);
    if t < peak {
        ease_out(t / peak)
    } else {
        ease_out((1.0 - t) / (1.0 - peak))
    }
}

/// `e^-t` by `half_life` in the same unit as `t`: an impulse dying out.
pub fn decay(t: f32, half_life: f32) -> f32 {
    if !t.is_finite() || t <= 0.0 {
        return 1.0;
    }
    let half_life = half_life.max(0.05);
    (-t * core::f32::consts::LN_2 / half_life).exp()
}

/// A smoothed value moving toward `target` in `dt` seconds, with time constant
/// `tau`. Frame-rate independent: the same `dt` split in two gives the same
/// answer as one step of the whole.
pub fn approach(value: f32, target: f32, tau: f32, dt: f32) -> f32 {
    if !value.is_finite() {
        return target;
    }
    if tau <= 0.0 || dt <= 0.0 {
        return target;
    }
    1.0 - (-dt / tau).exp()
}

/// [`approach`] applied: `value` travels a `k` share of the way to `target`.
pub fn smooth_toward(value: f32, target: f32, tau: f32, dt: f32) -> f32 {
    let k = approach(value, target, tau, dt);
    value + (target - value) * k
}

/// An angle wrapped to `-PI..=PI`.
pub fn wrap_angle(a: f32) -> f32 {
    let mut a = a;
    while a > core::f32::consts::PI {
        a -= core::f32::consts::TAU;
    }
    while a < -core::f32::consts::PI {
        a += core::f32::consts::TAU;
    }
    a
}

// ---------------------------------------------------------------------------
// The animation channels, and the config that switches them.
// ---------------------------------------------------------------------------

/// The channels a player can switch on and off, in the order the config file
/// lists them and the order their blend weights live in.
pub const CHANNELS: [&str; 24] = [
    "idle",
    "walk",
    "sprint",
    "sneak",
    "jump",
    "fall",
    "land",
    "swim",
    "climb",
    "ride",
    "mine",
    "place",
    "eat",
    "drink",
    "bow",
    "shield",
    "item_use",
    "tool_swing",
    "combo",
    "heavy",
    "dodge",
    "sprint_attack",
    "hurt",
    "death",
];

/// Index constants, so the animation code reads as prose.
pub mod ch {
    pub const IDLE: usize = 0;
    pub const WALK: usize = 1;
    pub const SPRINT: usize = 2;
    pub const SNEAK: usize = 3;
    pub const JUMP: usize = 4;
    pub const FALL: usize = 5;
    pub const LAND: usize = 6;
    pub const SWIM: usize = 7;
    pub const CLIMB: usize = 8;
    pub const RIDE: usize = 9;
    pub const MINE: usize = 10;
    pub const PLACE: usize = 11;
    pub const EAT: usize = 12;
    pub const DRINK: usize = 13;
    pub const BOW: usize = 14;
    pub const SHIELD: usize = 15;
    pub const ITEM_USE: usize = 16;
    pub const TOOL_SWING: usize = 17;
    pub const COMBO: usize = 18;
    pub const HEAVY: usize = 19;
    pub const DODGE: usize = 20;
    pub const SPRINT_ATTACK: usize = 21;
    pub const HURT: usize = 22;
    pub const DEATH: usize = 23;
}

/// The three intensities the settings page and the config file offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Subtle,
    Default,
    Action,
}

impl Preset {
    pub fn name(self) -> &'static str {
        match self {
            Preset::Subtle => "subtle",
            Preset::Default => "default",
            Preset::Action => "action",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Preset::Subtle => "Subtle",
            Preset::Default => "Default",
            Preset::Action => "Action",
        }
    }

    pub fn parse(text: &str) -> Option<Preset> {
        match text.trim().trim_matches('"').to_ascii_lowercase().as_str() {
            "subtle" => Some(Preset::Subtle),
            "default" => Some(Preset::Default),
            "action" => Some(Preset::Action),
            _ => None,
        }
    }

    /// The next preset, for the settings page's one button.
    pub fn next(self) -> Preset {
        match self {
            Preset::Subtle => Preset::Default,
            Preset::Default => Preset::Action,
            Preset::Action => Preset::Subtle,
        }
    }

    /// Its strengths: motion amount, and how far a movement pose leans.
    pub fn strengths(self) -> (f32, f32) {
        match self {
            Preset::Subtle => (0.55, 0.65),
            Preset::Default => (1.0, 1.0),
            Preset::Action => (1.35, 1.45),
        }
    }
}

/// Everything the mod can be told to do. The config file fills it; the settings
/// page changes the same fields and keeps them in the game's mod settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// The master switch: off means no pose at all, and any pose of ours is
    /// taken off the model again.
    pub master: bool,
    pub preset: Preset,
    /// Playback rate of the idle/gait/use cycles: 0.5 is half speed.
    pub speed: f32,
    /// How far every offset goes: 0 is nothing, 1.5 is a lot.
    pub intensity: f32,
    /// Seconds a channel takes to fade in or out when it starts or stops.
    pub blend: f32,
    /// First-person hand, item and arm motion.
    pub first_person: bool,
    /// Third-person body poses (the vanilla player model).
    pub third_person: bool,
    /// Turn the torso and shoulders a little toward where the head looks.
    pub head_tracking: bool,
    /// The extra first-person camera sway on top of the game's own bob.
    pub fp_sway: bool,
    /// One switch per [`CHANNELS`] entry.
    pub channels: [bool; CHANNELS.len()],
}

impl Default for Config {
    fn default() -> Self {
        let (intensity, _) = Preset::Default.strengths();
        Self {
            master: true,
            preset: Preset::Default,
            speed: 1.0,
            intensity,
            blend: 0.18,
            first_person: true,
            third_person: true,
            head_tracking: true,
            fp_sway: true,
            channels: [true; CHANNELS.len()],
        }
    }
}

impl Config {
    /// Takes a preset's strengths, leaving the per-animation switches alone:
    /// a player who muted the walk keeps it muted across a preset change.
    pub fn apply_preset(&mut self, preset: Preset) {
        let (intensity, _) = preset.strengths();
        self.preset = preset;
        self.intensity = intensity;
        self.blend = match preset {
            Preset::Subtle => 0.30,
            Preset::Default => 0.18,
            Preset::Action => 0.10,
        };
    }

    /// Whether `channel` may pose: the master switch, this channel and the
    /// person the pose is drawn for.
    pub fn channel_on(&self, channel: usize) -> bool {
        self.master && self.channels.get(channel).copied().unwrap_or(false)
    }

    /// The lean multiplier: the preset's second strength, scaled by intensity
    /// against the default preset's.
    pub fn lean(&self) -> f32 {
        let (_, lean) = self.preset.strengths();
        lean * self.intensity.max(0.05)
    }

    /// The config file's text for this config: the same format [`parse`] reads.
    ///
    /// [`parse`]: Config::parse
    pub fn to_toml(&self) -> String {
        let mut out = String::with_capacity(2048);
        out.push_str("# Fresh Moves (Horizon native mod) settings.\n");
        out.push_str("# Saved by the game at config/fresh_moves-client.toml.\n");
        out.push_str("# Edit it freely: the file is re-read when it changes on disk.\n");
        out.push_str("# The in-game page (Options > Mods > Fresh Moves) keeps its own\n");
        out.push_str("# copies of these values in the game's mod settings and wins over\n");
        out.push_str("# the file for the switches it shows.\n\n");
        out.push_str("# Master switch. false takes every pose off the player again.\n");
        out.push_str(&format!("master = {}\n", self.master));
        out.push_str("# subtle | default | action\n");
        out.push_str(&format!("preset = \"{}\"\n", self.preset.name()));
        out.push_str("# 0.5 to 2.0: how fast the cycles run.\n");
        out.push_str(&format!("speed = {:.2}\n", self.speed));
        out.push_str("# 0.0 to 1.5: how far every offset goes.\n");
        out.push_str(&format!("intensity = {:.2}\n", self.intensity));
        out.push_str("# 0.0 to 0.60 seconds: how long a channel takes to fade in or out.\n");
        out.push_str(&format!("blend = {:.2}\n", self.blend));
        out.push_str("# First-person hand, item and arm motion.\n");
        out.push_str(&format!("first_person = {}\n", self.first_person));
        out.push_str("# Third-person body poses on the vanilla player model.\n");
        out.push_str(&format!("third_person = {}\n", self.third_person));
        out.push_str("# Turn the torso and shoulders a little toward where the head looks.\n");
        out.push_str(&format!("head_tracking = {}\n", self.head_tracking));
        out.push_str("# The extra first-person camera sway on top of the game's bob.\n");
        out.push_str(&format!("fp_sway = {}\n", self.fp_sway));
        out.push_str("\n[animations]\n");
        for (i, name) in CHANNELS.iter().enumerate() {
            let heading = match i {
                ch::IDLE => Some("\n# movement\n"),
                ch::MINE => Some("\n# actions\n"),
                ch::COMBO => Some("\n# combat\n"),
                ch::HURT => Some("\n# reactions\n"),
                _ => None,
            };
            if let Some(heading) = heading {
                out.push_str(heading);
            }
            out.push_str(&format!("{name} = {}\n", self.channels[i]));
        }
        out
    }

    /// A config from a config file's text. Keys it does not know are skipped,
    /// and a value it cannot read leaves that field at its default, so a
    /// half-written file never stops the mod from starting.
    pub fn parse(text: &str) -> Config {
        let mut config = Config::default();
        let mut section = String::new();
        for raw in text.lines() {
            // A `#` outside a quoted value starts a comment.
            let line = strip_comment(raw);
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                section = name.trim().to_ascii_lowercase();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            if section == "animations" {
                if let Some(at) = CHANNELS.iter().position(|c| *c == key) {
                    if let Some(on) = parse_bool(value) {
                        config.channels[at] = on;
                    }
                }
                continue;
            }
            match key.as_str() {
                "master" => set_bool(&mut config.master, value),
                "preset" => {
                    if let Some(p) = Preset::parse(value) {
                        config.preset = p;
                    }
                }
                "speed" => {
                    if let Some(v) = parse_f32(value) {
                        config.speed = clamp_f(v, 0.5, 2.0);
                    }
                }
                "intensity" => {
                    if let Some(v) = parse_f32(value) {
                        config.intensity = clamp_f(v, 0.0, 1.5);
                    }
                }
                "blend" => {
                    if let Some(v) = parse_f32(value) {
                        config.blend = clamp_f(v, 0.0, 0.6);
                    }
                }
                "first_person" => set_bool(&mut config.first_person, value),
                "third_person" => set_bool(&mut config.third_person, value),
                "head_tracking" => set_bool(&mut config.head_tracking, value),
                "fp_sway" => set_bool(&mut config.fp_sway, value),
                _ => {}
            }
        }
        config
    }
}

/// The text written to `config/fresh_moves-client.toml` when it does not exist.
pub fn default_toml() -> String {
    Config::default().to_toml()
}

fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..i],
            _ => {}
        }
    }
    line
}

fn set_bool(slot: &mut bool, value: &str) {
    if let Some(on) = parse_bool(value) {
        *slot = on;
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim_matches('"').to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => Some(true),
        "false" | "off" | "no" | "0" => Some(false),
        _ => None,
    }
}

fn parse_f32(value: &str) -> Option<f32> {
    let v: f32 = value.trim().trim_matches('"').trim_end_matches('f').parse().ok()?;
    v.is_finite().then_some(v)
}

// ---------------------------------------------------------------------------
// What the adapter tells the runtime about a player each tick.
// ---------------------------------------------------------------------------

/// The item use a player is in the middle of. Vanilla draws its own pose for
/// these; the runtime only needs the kind to pick its first-person additions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseKind {
    None,
    Eat,
    Drink,
    Bow,
    Shield,
    Other,
}

/// One player's state for one tick, read by `native.rs` out of the game.
#[derive(Clone, Copy, Debug)]
pub struct Senses {
    /// The model's clock in ticks (`Mob.age`), the base of every cycle.
    pub age: f32,
    pub on_ground: bool,
    pub sprinting: bool,
    pub crouching: bool,
    pub climbing: bool,
    pub swimming: bool,
    /// Riding a mount, a boat or a minecart.
    pub riding: bool,
    /// Vertical speed in blocks a tick: positive up.
    pub vy: f32,
    /// Horizontal speed in blocks a tick.
    pub speed: f32,
    /// Vanilla's walk cycle (radians of leg swing, and how fast).
    pub walk_position: f32,
    pub walk_speed: f32,
    /// Ticks of the red hurt flash left, and ticks since death.
    pub hurt_time: i32,
    pub death_time: i32,
    /// Vanilla's arm swing: 0 (none) to 1 (started).
    pub swing: f32,
    /// How far into an item use, 0 (not using) to 1 (finished).
    pub use_progress: f32,
    pub use_kind: UseKind,
    /// The head's turn from the body and its pitch, in degrees.
    pub head_yaw: f32,
    pub head_pitch: f32,
    /// This window's own player.
    pub local: bool,
    /// Movement input, for the local player only: -1..=1.
    pub forward: f32,
    pub strafe: f32,
    pub jump_input: bool,
    pub sneak_input: bool,
}

impl Default for Senses {
    fn default() -> Self {
        Self {
            age: 0.0,
            on_ground: true,
            sprinting: false,
            crouching: false,
            climbing: false,
            swimming: false,
            riding: false,
            vy: 0.0,
            speed: 0.0,
            walk_position: 0.0,
            walk_speed: 0.0,
            hurt_time: 0,
            death_time: 0,
            swing: 0.0,
            use_progress: 0.0,
            use_kind: UseKind::None,
            head_yaw: 0.0,
            head_pitch: 0.0,
            local: false,
            forward: 0.0,
            strafe: 0.0,
            jump_input: false,
            sneak_input: false,
        }
    }
}

// ---------------------------------------------------------------------------
// One player's animator: blend weights, timed events and smoothed state.
// ---------------------------------------------------------------------------

/// How long one swing's arc lasts, in ticks. Vanilla's arm swing is 6 ticks;
/// the combo's follow-through reads better a little longer.
const SWING_TICKS: f32 = 9.0;
/// Ticks after a swing starts in which the next one continues the combo.
const COMBO_WINDOW: f32 = 13.0;
/// Ticks a dodge lean lasts.
const DODGE_TICKS: f32 = 12.0;

/// A player's animation state that survives between ticks.
#[derive(Clone, Debug)]
pub struct Animator {
    pub local: bool,
    /// The last tick's snapshot, for the render-time pose.
    pub senses: Senses,
    /// The game clock at the last update, in ticks.
    pub clock: f32,
    /// One blend weight per channel: 0 off, 1 fully in.
    pub weight: [f32; CHANNELS.len()],
    /// The swing combo: which hit (0, 1, 2) and the tick it started.
    pub combo_step: u8,
    pub combo_start: f32,
    pub last_swing: f32,
    /// The last hurt, as a strength and the tick it happened.
    pub hurt_strength: f32,
    pub hurt_start: f32,
    pub last_hurt: i32,
    /// The last landing, as a strength and the tick it happened.
    pub land_strength: f32,
    pub land_start: f32,
    /// The fastest fall of the current airtime, blocks a tick.
    pub peak_fall: f32,
    pub air_start: f32,
    /// The last dodge lean, its tick and which way (sign of the strafe).
    pub dodge_start: f32,
    pub dodge_dir: f32,
    /// Smoothed gait, so a walk that stops does not snap.
    pub gait: f32,
    pub sprint: f32,
    pub sneak: f32,
    pub swim: f32,
    pub climb: f32,
    pub ride: f32,
    pub air: f32,
    /// Smoothed head turn, for the look tracking.
    pub head_yaw: f32,
    pub head_pitch: f32,
    pub look_yaw: f32,
}

impl Animator {
    pub fn new() -> Self {
        Self {
            local: false,
            senses: Senses::default(),
            clock: 0.0,
            weight: [0.0; CHANNELS.len()],
            combo_step: 0,
            combo_start: -1000.0,
            last_swing: 0.0,
            hurt_strength: 0.0,
            hurt_start: -1000.0,
            last_hurt: 0,
            land_strength: 0.0,
            land_start: -1000.0,
            peak_fall: 0.0,
            air_start: -1000.0,
            dodge_start: -1000.0,
            dodge_dir: 0.0,
            gait: 0.0,
            sprint: 0.0,
            sneak: 0.0,
            swim: 0.0,
            climb: 0.0,
            ride: 0.0,
            air: 0.0,
            head_yaw: 0.0,
            head_pitch: 0.0,
            look_yaw: 0.0,
        }
    }

    /// Forty ticks a second is the tick rate; `dt` is in seconds.
    fn tau(&self, seconds: f32) -> f32 {
        seconds.max(0.008)
    }

    /// Advances one tick: detects the events a pose needs (a landing, a new
    /// swing, a hit) and eases every blend weight and smoothed value toward
    /// what this tick asks for.
    pub fn update(&mut self, senses: &Senses, dt: f32, config: &Config) {
        let previous = self.senses;
        self.local = senses.local;
        self.clock = senses.age;
        self.senses = *senses;

        // --- Events. They are stored with the tick they happened on, so the
        // render can read an envelope at the frame's own clock, not at the
        // last tick's.
        let left_ground = previous.on_ground && !senses.on_ground;
        let landed = !previous.on_ground && senses.on_ground;
        if left_ground {
            self.air_start = senses.age;
            self.peak_fall = 0.0;
        }
        if !senses.on_ground && -senses.vy > self.peak_fall {
            self.peak_fall = -senses.vy;
        }
        if landed {
            // A landing's weight comes from how fast the fall ended: a hop is
            // a blink, a three-block drop is a whole-body compression.
            let strength = clamp_f(self.peak_fall / 0.85, 0.0, 1.4);
            self.land_strength = strength;
            self.land_start = senses.age;
            self.peak_fall = 0.0;
        }
        // A fresh hurt: vanilla resets the red flash to ten ticks, so a rise
        // in it is a hit. The flinch is full strength; its envelope does the
        // shaping.
        if senses.hurt_time > self.last_hurt {
            self.hurt_strength = 1.0;
            self.hurt_start = senses.age;
        }
        self.last_hurt = senses.hurt_time;

        // A swing starts when vanilla's progress drops back to zero. The first
        // hit of a chain is the plain swing; each quick follow-up is the next
        // hit of the combo, and the fourth starts over.
        if senses.swing < 0.05 && self.last_swing > 0.5 {
            if senses.age - self.combo_start <= COMBO_WINDOW {
                self.combo_step = (self.combo_step + 1) % 3;
            } else {
                self.combo_step = 0;
            }
            self.combo_start = senses.age;
        }
        self.last_swing = senses.swing;

        // A dodge lean: sneaking at a run with a sideways push. Visual only,
        // the player's own movement is the game's.
        let dodging = senses.sprinting
            && senses.sneak_input
            && senses.strafe.abs() > 0.35
            && senses.age - self.dodge_start > DODGE_TICKS + 4.0;
        if dodging {
            self.dodge_start = senses.age;
            self.dodge_dir = senses.strafe.signum();
        }

        // --- Smoothed state. `speed`, `gait` and the pose switches are eased so
        // a change of state never snaps.
        let speed = clamp_f(config.speed, 0.5, 2.0);
        let blend = clamp_f(config.blend, 0.0, 0.6);
        // A blend of zero still has to ease, or the weight would snap.
        let fade = if blend > 0.0 { blend * 0.55 } else { 0.02 };
        let tau_move = self.tau(fade / speed);
        self.gait = smooth_toward(
            self.gait,
            clamp_f(senses.walk_speed, 0.0, 1.5),
            tau_move,
            dt,
        );
        let sprint_target = if senses.sprinting && senses.speed > 0.08 { 1.0 } else { 0.0 };
        // A sprint weight that follows the player in, but eases out: leaving a
        // sprint should read as a coast, not a cut.
        self.sprint = smooth_toward(self.sprint, sprint_target, self.tau(0.22), dt);
        self.sneak = smooth_toward(
            self.sneak,
            if senses.crouching { 1.0 } else { 0.0 },
            self.tau(0.18),
            dt,
        );
        self.swim = smooth_toward(
            self.swim,
            if senses.swimming { 1.0 } else { 0.0 },
            self.tau(0.20),
            dt,
        );
        self.climb = smooth_toward(
            self.climb,
            if senses.climbing { 1.0 } else { 0.0 },
            self.tau(0.16),
            dt,
        );
        self.ride = smooth_toward(
            self.ride,
            if senses.riding { 1.0 } else { 0.0 },
            self.tau(0.20),
            dt,
        );
        let air_target = if senses.on_ground || senses.swimming || senses.climbing || senses.riding {
            0.0
        } else {
            1.0
        };
        self.air = smooth_toward(self.air, air_target, self.tau(0.12), dt);

        // Head tracking: the head's own turn, eased, and a slow torso follow.
        let yaw = wrap_angle(rad(senses.head_yaw));
        let pitch = rad(senses.head_pitch);
        self.head_yaw = smooth_toward(self.head_yaw, yaw, self.tau(0.05), dt);
        self.head_pitch = smooth_toward(self.head_pitch, pitch, self.tau(0.07), dt);
        let follow = if config.head_tracking { 0.16 } else { 0.0 };
        self.look_yaw = smooth_toward(self.look_yaw, self.head_yaw * follow, self.tau(0.14), dt);

        // --- Blend weights. Each channel fades in or out over `blend` seconds,
        // which is what makes the transitions read as motion rather than as a
        // model swapping position.
        let fade = if blend > 0.0 { blend } else { 0.02 };
        let tau_blend = self.tau(fade);
        for (i, w) in self.weight.iter_mut().enumerate() {
            let target = if config.channel_on(i) { 1.0 } else { 0.0 };
            *w = smooth_toward(*w, target, tau_blend, dt);
        }
    }

    /// The strength of the landing, `0..=1`, `age` being the render clock.
    pub fn landing(&self, age: f32) -> f32 {
        if self.land_strength <= 0.0 {
            return 0.0;
        }
        let t = age - self.land_start;
        (0.0..=16.0)
            .contains(&t)
            .then(|| self.land_strength * decay(t, 3.2))
            .unwrap_or(0.0)
    }

    /// The strength of the last hurt flinch, `0..=1`.
    pub fn flinch(&self, age: f32) -> f32 {
        if self.hurt_strength <= 0.0 {
            return 0.0;
        }
        let t = age - self.hurt_start;
        (0.0..=12.0)
            .contains(&t)
            .then(|| self.hurt_strength * decay(t, 2.6))
            .unwrap_or(0.0)
    }

    /// The swing arc at `age`: 0 before a swing, 1 at its peak, 0 after.
    fn swing_arc(&self, age: f32) -> f32 {
        let t = age - self.combo_start;
        if !(0.0..=SWING_TICKS).contains(&t) {
            return 0.0;
        }
        let x = t / SWING_TICKS;
        // The strike lands early in the arc and the arm eases back.
        arc(x, 0.32)
    }

    /// The dodge lean at `age`: signed by the strafe, `0..=1` in size.
    fn dodge(&self, age: f32) -> f32 {
        let t = age - self.dodge_start;
        if !(0.0..=DODGE_TICKS).contains(&t) {
            return 0.0;
        }
        self.dodge_dir * arc(t / DODGE_TICKS, 0.35)
    }

    /// Rather than easing each channel in twice, the pose helper takes a
    /// channel's eased weight and the tick's share of it.
    fn w(&self, channel: usize) -> f32 {
        self.weight.get(channel).copied().unwrap_or(0.0)
    }
}

// ---------------------------------------------------------------------------
// The registry: the animators by entity id, the config, and the input.
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

/// How many ticks an entity may go unseen before its animator is dropped.
const FORGET_TICKS: f32 = 260.0;

/// What the movement-input hook last reported, for the local player.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub forward: f32,
    pub strafe: f32,
    pub jump: bool,
    pub sneak: bool,
}

struct Registry {
    config: Config,
    input: Input,
    animators: HashMap<u32, Animator>,
    /// The id of the window's own player, while it has one.
    local: Option<u32>,
    /// The game clock of the last tick, in ticks.
    clock: f32,
    /// Seconds since the previous frame, for the frame-rate smoothing.
    frame_dt: f32,
    /// The first-person pose, smoothed at frame rate: x, y, z, pitch, yaw, roll.
    hand: [f32; 6],
    /// The camera's turn, smoothed at frame rate: pitch, yaw, roll in degrees.
    sway: [f32; 3],
    /// The camera's offset, smoothed at frame rate, in blocks.
    sway_offset: [f32; 3],
    /// The local model's clock, interpolated between ticks for the hand.
    hand_age: f32,
}

static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

fn registry() -> MutexGuard<'static, Registry> {
    let mutex = REGISTRY.get_or_init(|| {
        Mutex::new(Registry {
            config: Config::default(),
            input: Input::default(),
            animators: HashMap::new(),
            local: None,
            clock: 0.0,
            frame_dt: 1.0 / 60.0,
            hand: [0.0; 6],
            sway: [0.0; 3],
            sway_offset: [0.0; 3],
            hand_age: 0.0,
        })
    });
    // A panic anywhere else must not take the animation out for good: the
    // registry holds no invariant a poison could break.
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// The config in force.
pub fn config() -> Config {
    registry().config.clone()
}

/// Applies a config file's text. Answers the config it left in force.
pub fn load_config(text: &str) -> Config {
    let parsed = Config::parse(text);
    let mut reg = registry();
    reg.config = parsed.clone();
    parsed
}

/// Replaces the config, as the settings page does.
pub fn set_config(config: Config) {
    registry().config = config;
}

/// The movement input the hook last reported, for the local player.
pub fn set_input(forward: f32, strafe: f32, jump: bool, sneak: bool) {
    let mut reg = registry();
    reg.input = Input {
        forward: clamp_f(forward, -1.0, 1.0),
        strafe: clamp_f(strafe, -1.0, 1.0),
        jump,
        sneak,
    };
}

/// One tick for the player with entity `id`. The adapter calls this for the
/// window's own player and for every other player it can see.
pub fn tick_player(id: u32, senses: Senses) {
    let mut reg = registry();
    let mut senses = senses;
    if senses.local {
        senses.forward = reg.input.forward;
        senses.strafe = reg.input.strafe;
        senses.jump_input = reg.input.jump;
        senses.sneak_input = reg.input.sneak;
    }
    let config = reg.config.clone();
    // A tick is 1/20 s; the adapter calls this once per tick.
    let dt = 0.05;
    reg.clock = senses.age;
    {
        let animator = reg.animators.entry(id).or_insert_with(Animator::new);
        animator.update(&senses, dt, &config);
    }
    if senses.local {
        reg.local = Some(id);
        reg.hand_age = senses.age;
    }
    // Drop the animators of players who have not been seen for a while: a
    // server's join list changes and the map must not grow with it.
    let clock = reg.clock;
    reg.animators
        .retain(|_, a| a.local || clock - a.clock < FORGET_TICKS);
}

/// Forgets every animator: a level change, or a connection that ended.
pub fn forget_all() {
    let mut reg = registry();
    reg.animators.clear();
    reg.local = None;
    reg.hand = [0.0; 6];
    reg.sway = [0.0; 3];
    reg.sway_offset = [0.0; 3];
}

/// Whether a pose of ours belongs on a player at all: the master switch and
/// third person are both on. The adapter asks before installing one, and again
/// before taking one off a model it installed earlier.
pub fn poses_enabled() -> bool {
    let reg = registry();
    reg.config.master && reg.config.third_person
}

/// Whether the first-person additions may run.
pub fn first_person_enabled() -> bool {
    let reg = registry();
    reg.config.master && reg.config.first_person
}


/// The body offsets for `id` at `age`, or None when it poses nothing.
///
/// The offsets are added to whatever the engine's own animation left in
/// `poses`; the scale entries are multiplied. `true` means at least one part
/// changed, which is what the engine wants to hear.
pub fn body_pose(id: u32, age: f32, poses: &mut [[f32; 9]; 6]) -> bool {
    let reg = registry();
    let Some(animator) = reg.animators.get(&id) else {
        return false;
    };
    if !reg.config.master || !reg.config.third_person {
        return false;
    }
    let offsets = body_offsets(animator, &reg.config, age);
    let mut touched = false;
    for (i, off) in offsets.iter().enumerate() {
        let mut any = false;
        for k in 0..3 {
            if off[k] != 0.0 {
                poses[i][k] += off[k];
                any = true;
            }
        }
        for k in 3..6 {
            if off[k] != 0.0 {
                poses[i][k] += off[k];
                any = true;
            }
        }
        for k in 6..9 {
            if off[k] != 1.0 {
                poses[i][k] *= off[k];
                any = true;
            }
        }
        touched |= any;
    }
    touched
}

// ---------------------------------------------------------------------------
// Third person: the body.
// ---------------------------------------------------------------------------

/// Part indices, matching `horizon_hooks::players::BODY_PARTS`.
const HEAD: usize = 0;
const BODY: usize = 1;
const RIGHT_ARM: usize = 2;
const LEFT_ARM: usize = 3;
const RIGHT_LEG: usize = 4;
const LEFT_LEG: usize = 5;

/// Additive offsets: place in pixels, rotation in radians, scale as a factor
/// (1 is unchanged).
fn identity_offsets() -> [[f32; 9]; 6] {
    [[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0]; 6]
}

/// The whole body pose for one player at `age`.
fn body_offsets(a: &Animator, config: &Config, age: f32) -> [[f32; 9]; 6] {
    let mut off = identity_offsets();
    let s = &a.senses;
    let speed = clamp_f(config.speed, 0.5, 2.0);
    let lean = clamp_f(config.lean(), 0.0, 2.0);
    // One second of ticks: every cycle below is written in ticks and divided
    // by this, so the speed switch moves them all together.
    let t = age * speed;

    // The walk cycle as vanilla has it: `cos(pos * 0.6662) * 1.4 * speed`
    // radians for a quadruped's legs, so the humanoid's own is `sin` of the
    // same phase. Using it keeps our secondary motion in step with the legs.
    let phase = s.walk_position * 0.6662;
    let gait = clamp_f(a.gait, 0.0, 1.5);
    let sprint = clamp_f(a.sprint, 0.0, 1.0);
    let sneak = clamp_f(a.sneak, 0.0, 1.0);
    let air = clamp_f(a.air, 0.0, 1.0);
    let swim = clamp_f(a.swim, 0.0, 1.0);
    let climb = clamp_f(a.climb, 0.0, 1.0);
    let ride = clamp_f(a.ride, 0.0, 1.0);
    let swing = a.swing_arc(age);
    let dodge = a.dodge(age);
    let land = a.landing(age);
    let flinch = a.flinch(age);

    // --- Idle: breathing and a slow weight shift. Always on (it is what keeps
    // a standing player from looking like a statue), scaled by its channel.
    let w_idle = a.w(ch::IDLE) * lean;
    if w_idle > 0.001 {
        let breath = (t * 0.115).sin();
        let shift = (t * 0.043).sin();
        let shift2 = (t * 0.043 + 1.7).sin();
        off[BODY][1] += 0.22 * breath * w_idle;
        off[BODY][3] += 0.010 * breath * w_idle;
        off[BODY][4] += 0.022 * shift * w_idle;
        off[BODY][5] += 0.014 * shift2 * w_idle;
        off[BODY][7] += 0.004 * breath * w_idle;
        off[HEAD][3] += -0.012 * breath * w_idle;
        off[HEAD][4] += -0.030 * shift * w_idle;
        off[HEAD][5] += 0.010 * shift2 * w_idle;
        off[RIGHT_ARM][5] += 0.012 * breath * w_idle;
        off[LEFT_ARM][5] += -0.012 * breath * w_idle;
    }

    // --- Walk and sprint: the leanness of a body that is going somewhere, on
    // top of vanilla's leg and arm swing rather than instead of it.
    let w_walk = a.w(ch::WALK) * lean * (1.0 - 0.55 * sprint);
    let w_sprint = a.w(ch::SPRINT) * lean;
    if (gait > 0.01 || sprint > 0.01) && (w_walk > 0.001 || w_sprint > 0.001) {
        let bob = (phase * 2.0).sin();
        // A body leaning into its own input, not only into its speed: holding
        // forward on a slow walk still looks like walking.
        let lean_walk = 0.055 * gait * w_walk + 0.012 * s.forward.abs() * w_walk;
        let lean_sprint = 0.230 * sprint * w_sprint;
        off[BODY][3] += lean_walk + lean_sprint;
        off[BODY][1] += (0.30 * bob) * (gait * w_walk + sprint * w_sprint);
        off[BODY][4] += 0.055 * phase.sin() * gait * w_walk;
        off[BODY][5] += -0.045 * phase.sin() * gait * w_walk;
        off[HEAD][3] += -(lean_walk * 0.55 + lean_sprint * 0.65);
        off[HEAD][1] += -0.35 * bob * gait * w_walk;
        off[HEAD][4] += -0.040 * phase.sin() * gait * w_walk;
        // Arms counter-swing a little wider than vanilla's, and swing back
        // toward the body line at a sprint.
        let arm = (0.10 * gait * w_walk + 0.26 * sprint * w_sprint) * phase.sin();
        off[RIGHT_ARM][3] += -arm;
        off[LEFT_ARM][3] += arm;
        off[RIGHT_ARM][5] += 0.05 * sprint * w_sprint;
        off[LEFT_ARM][5] += -0.05 * sprint * w_sprint;
        // Legs: a sprint reaches further than vanilla's stride.
        off[RIGHT_LEG][3] += -0.05 * sprint * phase.sin() * w_sprint;
        off[LEFT_LEG][3] += 0.05 * sprint * phase.sin() * w_sprint;
    }

    // --- Sneak: vanilla crouches the model; this hunches the shoulders over
    // it and keeps the head up, which is the part vanilla leaves out.
    // A jump pressed but not yet taken: the model gathers a little first.
    if s.jump_input && s.on_ground {
        let gather = a.w(ch::JUMP) * lean;
        off[BODY][1] -= 0.35 * gather;
        off[RIGHT_LEG][3] -= 0.10 * gather;
        off[LEFT_LEG][3] -= 0.10 * gather;
        off[RIGHT_ARM][3] += 0.06 * gather;
        off[LEFT_ARM][3] += 0.06 * gather;
    }

    let w_sneak = a.w(ch::SNEAK) * lean;
    if sneak > 0.01 && w_sneak > 0.001 {
        off[BODY][3] += 0.085 * sneak * w_sneak;
        off[BODY][4] += 0.070 * sneak * w_sneak;
        off[HEAD][3] += -0.075 * sneak * w_sneak;
        off[RIGHT_ARM][3] += -0.090 * sneak * w_sneak;
        off[LEFT_ARM][3] += -0.090 * sneak * w_sneak;
        off[RIGHT_ARM][5] += 0.050 * sneak * w_sneak;
        off[LEFT_ARM][5] += -0.050 * sneak * w_sneak;
    }

    // --- Air: a jump tucks and opens the arms, a fall spreads them and looks
    // down. Both fade with the airtime's own smoothing.
    if air > 0.01 {
        let rising = s.vy > 0.06;
        let w_jump = a.w(ch::JUMP) * lean;
        let w_fall = a.w(ch::FALL) * lean;
        let air_t = clamp_f((age - a.air_start) / 8.0, 0.0, 1.0);
        let rise = ease_out(air_t);
        if rising && w_jump > 0.001 {
            off[BODY][3] += -0.070 * air * rise * w_jump;
            off[RIGHT_ARM][5] += 0.230 * air * rise * w_jump;
            off[LEFT_ARM][5] += -0.230 * air * rise * w_jump;
            off[RIGHT_LEG][3] += -0.160 * air * rise * w_jump;
            off[LEFT_LEG][3] += -0.160 * air * rise * w_jump;
        } else if w_fall > 0.001 {
            let spread = clamp_f(-s.vy / 0.6, 0.0, 1.0);
            off[BODY][3] += 0.100 * air * w_fall;
            off[RIGHT_ARM][5] += (0.300 + 0.180 * spread) * air * w_fall;
            off[LEFT_ARM][5] += -(0.300 + 0.180 * spread) * air * w_fall;
            off[RIGHT_LEG][3] += -0.100 * air * spread * w_fall;
            off[LEFT_LEG][3] += 0.080 * air * spread * w_fall;
            off[HEAD][3] += 0.060 * air * spread * w_fall;
        }
    }

    // --- Landing: the compression a fall buys. Legs take it first, the torso
    // follows, the arms come out for balance.
    let w_land = a.w(ch::LAND) * lean;
    if land > 0.01 && w_land > 0.001 {
        let l = land * w_land;
        off[BODY][1] += -1.15 * l;
        off[BODY][3] += 0.170 * l;
        off[RIGHT_LEG][3] += -0.330 * l;
        off[LEFT_LEG][3] += -0.330 * l;
        off[RIGHT_ARM][5] += 0.220 * l;
        off[LEFT_ARM][5] += -0.220 * l;
        off[HEAD][3] += 0.090 * l;
        off[BODY][7] += 0.020 * l;
    }

    // --- Swim, climb and ride: the three cases vanilla either leaves to the
    // player's pitch or handles with a fixed pose.
    let w_swim = a.w(ch::SWIM) * lean;
    if swim > 0.01 && w_swim > 0.001 {
        let stroke = (t * 0.24).sin();
        off[RIGHT_ARM][3] += -0.420 * (0.5 + 0.5 * stroke) * swim * w_swim;
        off[LEFT_ARM][3] += -0.420 * (0.5 - 0.5 * stroke) * swim * w_swim;
        off[RIGHT_ARM][5] += 0.240 * swim * w_swim;
        off[LEFT_ARM][5] += -0.240 * swim * w_swim;
        off[HEAD][3] += -0.120 * swim * w_swim;
    }
    let w_climb = a.w(ch::CLIMB) * lean;
    if climb > 0.01 && w_climb > 0.001 {
        let reach = (t * 0.30).sin();
        off[RIGHT_ARM][3] += (-0.900 + 0.350 * reach) * climb * w_climb;
        off[LEFT_ARM][3] += (-0.900 - 0.350 * reach) * climb * w_climb;
        off[RIGHT_LEG][3] += -0.300 * climb * w_climb;
        off[LEFT_LEG][3] += -0.300 * climb * w_climb;
        off[BODY][3] += 0.140 * climb * w_climb;
    }
    let w_ride = a.w(ch::RIDE) * lean;
    if ride > 0.01 && w_ride > 0.001 {
        off[RIGHT_LEG][5] += 0.300 * ride * w_ride;
        off[LEFT_LEG][5] += -0.300 * ride * w_ride;
        off[RIGHT_LEG][3] += -0.420 * ride * w_ride;
        off[LEFT_LEG][3] += -0.420 * ride * w_ride;
        off[BODY][3] += -0.040 * ride * w_ride;
    }

    // --- The swing chain. Vanilla animates the arm; this adds the shoulder,
    // the torso twist and the follow-through that turn a swing into a hit.
    let heavy = a.combo_step == 2;
    let w_combo = if heavy {
        a.w(ch::HEAVY) * lean
    } else {
        a.w(ch::COMBO) * lean
    };
    if swing > 0.001 && w_combo > 0.001 {
        let c = swing * w_combo;
        match a.combo_step {
            // A downward diagonal from the right shoulder.
            0 => {
                off[BODY][4] += -0.300 * c;
                off[BODY][3] += 0.090 * c;
                off[RIGHT_ARM][5] += 0.340 * c;
                off[RIGHT_ARM][3] += -0.220 * c;
                off[LEFT_ARM][4] += 0.120 * c;
            }
            // A backhand that carries the turn the other way.
            1 => {
                off[BODY][4] += 0.320 * c;
                off[BODY][3] += 0.050 * c;
                off[RIGHT_ARM][5] += -0.420 * c;
                off[RIGHT_ARM][3] += -0.140 * c;
                off[LEFT_ARM][4] += -0.140 * c;
            }
            // The overhead: both arms, the whole body behind it.
            _ => {
                off[BODY][3] += 0.240 * c;
                off[BODY][1] += -0.700 * c;
                off[RIGHT_ARM][3] += -0.520 * c;
                off[LEFT_ARM][3] += -0.420 * c;
                off[RIGHT_ARM][5] += 0.120 * c;
                off[LEFT_ARM][5] += -0.120 * c;
                off[HEAD][3] += 0.110 * c;
                off[RIGHT_LEG][3] += -0.120 * c;
                off[LEFT_LEG][3] += -0.120 * c;
            }
        }
    }

    // Mining and placing: the body follows the arm instead of standing still
    // behind it. Small, because vanilla already swings the arm hard.
    let w_mine = a.w(ch::MINE) * lean;
    let w_place = a.w(ch::PLACE) * lean;
    let w_tool = a.w(ch::TOOL_SWING) * lean;
    if swing > 0.001 {
        let follow = swing * (w_mine * 0.30 + w_tool * 0.18 + w_place * 0.10);
        off[BODY][3] += 0.070 * follow;
        off[HEAD][3] += 0.050 * follow;
        off[RIGHT_ARM][5] += 0.090 * follow;
    }

    // --- Sprint attack: a wide, low cut that only reads at a run.
    let w_sprint_attack = a.w(ch::SPRINT_ATTACK) * lean;
    if swing > 0.001 && sprint > 0.4 && w_sprint_attack > 0.001 {
        let c = swing * sprint * w_sprint_attack;
        off[BODY][3] += 0.140 * c;
        off[BODY][4] += -0.260 * c;
        off[RIGHT_ARM][5] += 0.300 * c;
    }

    // --- Dodge: a lean and a tuck, alive for a moment. Purely a pose: the
    // player's own momentum is the game's.
    let w_dodge = a.w(ch::DODGE) * lean;
    if dodge.abs() > 0.001 && w_dodge > 0.001 {
        let d = dodge * w_dodge;
        off[BODY][5] += 0.520 * d;
        off[BODY][3] += 0.240 * d.abs();
        off[BODY][1] += -0.450 * d.abs();
        off[RIGHT_LEG][3] += -0.520 * d.abs();
        off[LEFT_LEG][3] += -0.380 * d.abs();
        off[RIGHT_ARM][5] += 0.300 * d.abs();
        off[LEFT_ARM][5] += -0.300 * d.abs();
        off[HEAD][5] += -0.200 * d;
    }

    // --- Hurt and death: reactions, not actions. The flinch pulls the chest
    // back and the head away; death lets the limbs go, on top of vanilla's own
    // fall over twenty ticks.
    let w_hurt = a.w(ch::HURT) * lean;
    if flinch > 0.001 && w_hurt > 0.001 {
        let f = flinch * w_hurt;
        off[BODY][3] += -0.240 * f;
        off[BODY][4] += 0.100 * f;
        off[HEAD][3] += 0.220 * f;
        off[HEAD][4] += 0.120 * f;
        off[RIGHT_ARM][5] += 0.180 * f;
        off[LEFT_ARM][5] += -0.180 * f;
    }
    let w_death = a.w(ch::DEATH) * lean;
    if s.death_time > 0 && w_death > 0.001 {
        let d = clamp_f(s.death_time as f32 / 18.0, 0.0, 1.0) * w_death;
        off[RIGHT_ARM][3] += 0.420 * d;
        off[LEFT_ARM][3] += 0.360 * d;
        off[RIGHT_ARM][5] += -0.260 * d;
        off[LEFT_ARM][5] += 0.260 * d;
        off[HEAD][3] += 0.180 * d;
        off[RIGHT_LEG][5] += 0.120 * d;
        off[LEFT_LEG][5] += -0.120 * d;
    }

    // --- Look tracking: the torso turns a little toward the head, and the
    // head keeps a sliver of its own lag so the neck reads as weight.
    let w_look = a.w(ch::IDLE) * lean;
    if config.head_tracking && w_look > 0.001 {
        off[BODY][4] += a.look_yaw * w_look;
        off[HEAD][4] += -a.look_yaw * 0.55 * w_look;
        off[BODY][3] += clamp_f(a.head_pitch, -1.0, 1.0) * 0.045 * w_look;
        off[HEAD][5] += -clamp_f(a.head_pitch, -1.0, 1.0) * 0.030 * w_look;
    }

    // Nothing may leave the model in a shape the engine cannot draw.
    for part in off.iter_mut() {
        for k in 0..3 {
            part[k] = clamp_f(part[k], -6.0, 6.0);
        }
        for k in 3..6 {
            part[k] = clamp_f(part[k], -1.5, 1.5);
        }
        for k in 6..9 {
            part[k] = clamp_f(part[k], 0.5, 1.5);
        }
    }
    off
}

// ---------------------------------------------------------------------------
// First person: the hand, the item, the camera.
// ---------------------------------------------------------------------------

/// The first-person hand pose the adapter adds to the engine's own:
/// blocks for the place, degrees for the turns, a factor for the scale.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HandPose {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
    pub scale: f32,
}

impl HandPose {
    fn neutral() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            pitch: 0.0,
            yaw: 0.0,
            roll: 0.0,
            scale: 1.0,
        }
    }

    /// Adds `other` in, multiplying the scale: two mods' poses compose.
    pub fn add(&mut self, other: &HandPose) {
        self.x += other.x;
        self.y += other.y;
        self.z += other.z;
        self.pitch += other.pitch;
        self.yaw += other.yaw;
        self.roll += other.roll;
        self.scale *= other.scale;
    }

    /// Keeps every value inside the range the hook accepts (offsets within
    /// -16..=16 blocks, angles within -3600..=3600 degrees, scale within
    /// 0.01..=100), so the game never has to drop one of ours.
    pub fn sanitize(&mut self) {
        self.x = clamp_f(self.x, -8.0, 8.0);
        self.y = clamp_f(self.y, -8.0, 8.0);
        self.z = clamp_f(self.z, -8.0, 8.0);
        self.pitch = clamp_f(self.pitch, -360.0, 360.0);
        self.yaw = clamp_f(self.yaw, -360.0, 360.0);
        self.roll = clamp_f(self.roll, -360.0, 360.0);
        self.scale = clamp_f(self.scale, 0.2, 4.0);
    }
}

/// The camera sway the adapter adds to the game's shake: degrees and blocks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shake {
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
    pub offset: [f32; 3],
}

impl Shake {
    /// Adds `other` in: two mods' shakes add up, as the hook asks.
    pub fn add(&mut self, other: &Shake) {
        self.pitch += other.pitch;
        self.yaw += other.yaw;
        self.roll += other.roll;
        for (i, v) in self.offset.iter_mut().enumerate() {
            *v += other.offset[i];
        }
    }

    pub fn sanitize(&mut self) {
        self.pitch = clamp_f(self.pitch, -10.0, 10.0);
        self.yaw = clamp_f(self.yaw, -10.0, 10.0);
        self.roll = clamp_f(self.roll, -10.0, 10.0);
        for v in self.offset.iter_mut() {
            *v = clamp_f(*v, -0.5, 0.5);
        }
    }
}

/// Advances the frame-rate smoothing the hand and the camera use. Called once a
/// frame with the seconds since the previous one.
pub fn frame(elapsed: f32) {
    let dt = clamp_f(elapsed, 0.0, 0.25);
    let mut reg = registry();
    let frame_dt = if dt > 0.0 { dt } else { 1.0 / 60.0 };
    reg.frame_dt = frame_dt;
    let speed = clamp_f(reg.config.speed, 0.5, 2.0);
    // The local model's clock moves on between ticks; the hand's cycles are
    // timed on it, so they do not step at the tick rate.
    reg.hand_age += frame_dt * 20.0 * speed;
    let config = reg.config.clone();
    let hand_age = reg.hand_age;

    // Read the targets while the animator map is borrowed, then let go of it
    // before the smoothed values are written back.
    let mut hand_t = [0.0f32; 6];
    let mut sway_t = [0.0f32; 3];
    let mut offset_t = [0.0f32; 3];
    if let Some(local) = reg.local {
        if let Some(a) = reg.animators.get(&local) {
            hand_t = hand_target(a, &config, hand_age);
            sway_t = camera_target(a, &config, hand_age);
            offset_t = camera_offset_target(a, &config, hand_age);
        }
    }

    // 40 ms to settle: quick enough to feel attached, slow enough to turn a
    // tick-rate event into a frame-rate motion. The camera lags a little more,
    // because a head does.
    for i in 0..6 {
        reg.hand[i] = smooth_toward(reg.hand[i], hand_t[i], 0.040, frame_dt);
    }
    for i in 0..3 {
        reg.sway[i] = smooth_toward(reg.sway[i], sway_t[i], 0.085, frame_dt);
        reg.sway_offset[i] = smooth_toward(reg.sway_offset[i], offset_t[i], 0.075, frame_dt);
    }
}

/// The hand's target pose for the local player, before frame smoothing.
fn hand_target(a: &Animator, config: &Config, age: f32) -> [f32; 6] {
    let mut pose = HandPose::neutral();
    if !config.master || !config.first_person {
        return [0.0; 6];
    }
    let s = &a.senses;
    let lean = clamp_f(config.lean(), 0.0, 2.0);
    let t = age;
    let phase = s.walk_position * 0.6662;
    let gait = clamp_f(a.gait, 0.0, 1.4);
    let sprint = clamp_f(a.sprint, 0.0, 1.0);
    let air = ease_in_out(clamp_f(a.air, 0.0, 1.0));

    // --- Walking: half of the vanilla bob's scale, in the places vanilla does
    // not bob, so it reads as weight rather than as a second bob.
    let w_walk = a.w(ch::WALK) * lean;
    if gait > 0.01 && w_walk > 0.001 {
        let g = gait * w_walk;
        pose.x += 0.0075 * (phase * 2.0).cos() * g;
        pose.y += 0.0055 * (phase * 2.0).sin() * g;
        pose.z += 0.0035 * (phase * 4.0).sin() * g;
        pose.roll += 0.55 * (phase).sin() * g;
        pose.yaw += 0.30 * (phase * 2.0).cos() * g;
    }

    // --- Sprinting: a hard forward cant, the hand carried lower and out.
    let w_sprint = a.w(ch::SPRINT) * lean;
    if sprint > 0.01 && w_sprint > 0.001 {
        let sp = sprint * w_sprint;
        pose.y -= 0.0130 * sp;
        pose.z += 0.0090 * sp;
        pose.x += 0.0060 * sp;
        pose.roll += 1.30 * phase.sin() * sp;
        pose.pitch += -1.60 * sp;
        pose.yaw += 0.90 * phase.cos() * sp;
    }

    // --- Air: the arms come up in a jump and the hand trails on the way down.
    let w_air = (a.w(ch::JUMP) + a.w(ch::FALL)) * 0.5 * lean;
    if air > 0.01 && w_air > 0.001 {
        let rising = s.vy > 0.06;
        let dir = if rising { 1.0 } else { -1.0 };
        pose.y += 0.0110 * dir * air * w_air;
        pose.pitch += -2.60 * dir * air * w_air;
        pose.roll += 1.10 * dir * air * w_air;
    }

    // --- Sword and tool swings: the wrist and the follow-through, never the
    // whole arc (vanilla's own swing is already in the pose this adds to).
    let swing = a.swing_arc(age);
    if swing > 0.001 {
        let heavy = a.combo_step == 2;
        let w_combo = if heavy {
            a.w(ch::HEAVY) * lean
        } else {
            a.w(ch::COMBO) * lean
        };
        if w_combo > 0.001 {
            let c = swing * w_combo;
            let dir = if a.combo_step == 1 { -1.0 } else { 1.0 };
            pose.roll += 7.5 * c * dir;
            pose.pitch += -5.0 * c;
            pose.yaw += 3.5 * c * dir;
            pose.x += 0.006 * c * dir;
            if heavy {
                pose.y -= 0.010 * c;
                pose.pitch += -4.0 * c;
            }
        }
        let w_mine = a.w(ch::MINE) * lean;
        let w_tool = a.w(ch::TOOL_SWING) * lean;
        let w_place = a.w(ch::PLACE) * lean;
        let follow = swing * (w_mine + w_tool + w_place);
        if follow > 0.001 {
            pose.roll += 2.5 * follow;
            pose.pitch += -2.0 * follow;
        }
    }

    // --- Using an item over time. The game poses the item itself (eat, drink,
    // bow, spear, brush), so this is only the part it leaves out: the arm's
    // weight while it is held, and the shake of a bow drawn to its end.
    let kind_weight = match s.use_kind {
        UseKind::Eat => a.w(ch::EAT),
        UseKind::Drink => a.w(ch::DRINK),
        UseKind::Bow => a.w(ch::BOW),
        UseKind::Shield => a.w(ch::SHIELD),
        UseKind::Other => a.w(ch::ITEM_USE),
        UseKind::None => 0.0,
    } * lean;
    let use_progress = clamp_f(s.use_progress, 0.0, 1.0);
    let held = use_progress * kind_weight;
    if held > 0.001 {
        let tremor = (t * 0.9).sin() * 0.10 + (t * 2.3).sin() * 0.04;
        pose.y += 0.0040 * held;
        pose.roll += 0.55 * held + tremor * 0.60;
        pose.pitch += -0.80 * held;
        if s.use_kind == UseKind::Bow {
            // A drawn bow is not still: the shake grows with the draw.
            let draw = use_progress * use_progress;
            pose.roll += (t * 3.1).sin() * 0.90 * draw * kind_weight;
            pose.pitch += (t * 2.7).cos() * 0.50 * draw * kind_weight;
        }
        if s.use_kind == UseKind::Shield {
            pose.y += 0.0060 * held;
            pose.roll += 1.20 * held;
        }
    }

    // The hand settles while the game's own use pose plays: this only stops
    // our sway from fighting it.
    let damp = 1.0 - 0.65 * use_progress;
    pose.x *= damp;
    pose.y *= damp;
    pose.z *= damp;
    pose.roll *= damp;
    pose.yaw *= damp;

    // --- A hurt flinch reaches even the hand.
    let flinch = a.flinch(age) * a.w(ch::HURT) * lean;
    if flinch > 0.001 {
        pose.pitch += 3.2 * flinch;
        pose.roll += 2.4 * flinch;
        pose.y += 0.008 * flinch;
    }

    // --- The camera sway switch also calms the hand: they are one system.
    if !config.fp_sway {
        pose.roll *= 0.45;
        pose.x *= 0.6;
        pose.y *= 0.6;
    }

    pose.sanitize();
    [pose.x, pose.y, pose.z, pose.pitch, pose.yaw, pose.roll]
}

/// The camera's target turn for the local player, in degrees.
fn camera_target(a: &Animator, config: &Config, age: f32) -> [f32; 3] {
    if !config.master || !config.fp_sway || !config.first_person {
        return [0.0; 3];
    }
    let s = &a.senses;
    let lean = clamp_f(config.lean(), 0.0, 2.0);
    let phase = s.walk_position * 0.6662;
    let gait = clamp_f(a.gait, 0.0, 1.4);
    let sprint = clamp_f(a.sprint, 0.0, 1.0);
    let t = age;

    let mut pitch = 0.0;
    let mut yaw = 0.0;
    let mut roll = 0.0;

    let w_walk = a.w(ch::WALK) * lean;
    if gait > 0.01 && w_walk > 0.001 {
        let g = gait * w_walk;
        // A quarter of a degree: enough to feel the ground, not enough to aim
        // differently.
        pitch += 0.32 * (phase * 2.0).sin() * g;
        roll += 0.26 * phase.sin() * g;
        yaw += 0.10 * (phase * 2.0).cos() * g;
    }
    let w_sprint = a.w(ch::SPRINT) * lean;
    if sprint > 0.01 && w_sprint > 0.001 {
        let sp = sprint * w_sprint;
        roll += 0.55 * phase.sin() * sp;
        pitch += 0.25 * (phase * 2.0).cos() * sp;
    }
    let land = a.landing(age) * a.w(ch::LAND) * lean;
    if land > 0.001 {
        pitch += 1.35 * land;
        roll += 0.45 * land * (t * 1.7).sin();
    }
    let swing = a.swing_arc(age);
    let w_combo = a.w(ch::COMBO) * lean;
    if swing > 0.001 && w_combo > 0.001 {
        pitch += -0.55 * swing * w_combo;
        let turn = if a.combo_step == 1 { -1.0 } else { 1.0 };
        yaw += 0.30 * swing * w_combo * turn;
    }
    let flinch = a.flinch(age) * a.w(ch::HURT) * lean;
    if flinch > 0.001 {
        pitch += 1.10 * flinch;
        roll += 0.80 * flinch;
    }
    let dodge = a.dodge(age) * a.w(ch::DODGE) * lean;
    if dodge.abs() > 0.001 {
        roll += 1.20 * dodge;
    }

    [
        clamp_f(pitch, -6.0, 6.0),
        clamp_f(yaw, -4.0, 4.0),
        clamp_f(roll, -6.0, 6.0),
    ]
}

/// The camera's target offset in blocks (right, up, back).
fn camera_offset_target(a: &Animator, config: &Config, age: f32) -> [f32; 3] {
    if !config.master || !config.fp_sway || !config.first_person {
        return [0.0; 3];
    }
    let lean = clamp_f(config.lean(), 0.0, 2.0);
    let phase = a.senses.walk_position * 0.6662;
    let gait = clamp_f(a.gait, 0.0, 1.4);
    let land = a.landing(age) * a.w(ch::LAND) * lean;
    let air = clamp_f(a.air, 0.0, 1.0);
    let mut offset = [0.0f32; 3];
    offset[1] += 0.0085 * (phase * 2.0).sin() * gait * a.w(ch::WALK) * lean;
    offset[1] -= 0.075 * land;
    offset[1] += 0.020 * air * (a.w(ch::FALL)) * lean;
    offset
}

/// The hand pose the adapter adds for one of the two hands. `swing` and `equip`
/// are the engine's own, and are used to keep our additions from fighting a
/// very fast swing or an item swap.
pub fn hand_pose(swing: f32, equip: f32, out: &mut HandPose) {
    // A fast swing's own motion is already large; ours steps back from it, and
    // an item swap's dip leaves no room for a sway of our own.
    let busy = 1.0 - 0.55 * clamp_f(swing, 0.0, 1.0);
    let swapping = 1.0 - 0.70 * clamp_f(equip, 0.0, 1.0);
    let scale = busy * swapping;
    let reg = registry();
    let smoothed = reg.hand;
    drop(reg);

    let ours = HandPose {
        x: smoothed[0] * scale,
        y: smoothed[1] * scale,
        z: smoothed[2] * scale,
        pitch: smoothed[3] * scale,
        yaw: smoothed[4] * scale,
        roll: smoothed[5] * scale,
        scale: 1.0,
    };
    out.add(&ours);
    out.sanitize();
}

/// The camera shake the adapter adds: our smoothed sway plus the impulses the
/// pose's events left, which are read at the frame's clock.
pub fn camera_shake(partial: f32, out: &mut Shake) {
    let reg = registry();
    let config = reg.config.clone();
    if !config.master || !config.fp_sway {
        return;
    }
    let sway = reg.sway;
    let offset = reg.sway_offset;
    drop(reg);

    // The partial tick only matters for the offsets' phase; the sway itself is
    // already smoothed at frame rate.
    let wobble = clamp_f(partial, 0.0, 1.0);
    let ours = Shake {
        pitch: sway[0],
        yaw: sway[1],
        roll: sway[2],
        offset: [
            offset[0],
            offset[1] * (0.85 + 0.15 * wobble),
            offset[2],
        ],
    };
    out.add(&ours);
    out.sanitize();
}

/// The third-person camera distance multiplier: the camera pulls back at a run
/// and leans in on a hit. First person never asks.
pub fn camera_distance(partial: f32, distance: &mut f32) {
    let reg = registry();
    let config = reg.config.clone();
    if !config.master || !config.third_person {
        return;
    }
    let Some(local) = reg.local else {
        return;
    };
    let Some(a) = reg.animators.get(&local) else {
        return;
    };
    let age = a.senses.age + clamp_f(partial, 0.0, 1.0);
    let sprint = clamp_f(a.sprint, 0.0, 1.0) * a.w(ch::SPRINT);
    let swing = a.swing_arc(age) * a.w(ch::COMBO);
    let land = a.landing(age) * a.w(ch::LAND);
    let mut factor = 1.0;
    factor += 0.055 * sprint;
    factor -= 0.035 * swing;
    factor -= 0.030 * land;
    let wished = *distance * clamp_f(factor, 0.80, 1.10);
    if wished.is_finite() && wished >= 0.0 {
        *distance = wished;
    }
}

// ---------------------------------------------------------------------------
// Tests. The runtime names no game type, so these run anywhere `cargo test`
// can build the file.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn senses() -> Senses {
        Senses {
            local: true,
            age: 100.0,
            walk_position: 2.0,
            walk_speed: 1.0,
            ..Senses::default()
        }
    }

    fn tick(animator: &mut Animator, senses: &Senses, config: &Config, times: usize) {
        for _ in 0..times {
            animator.update(senses, 0.05, config);
        }
    }

    #[test]
    fn the_default_config_is_the_default_preset() {
        let config = Config::default();
        assert!(config.master);
        assert!(config.first_person && config.third_person);
        assert_eq!(config.preset, Preset::Default);
        assert!((config.intensity - 1.0).abs() < 1e-6);
        assert!(config.channels.iter().all(|c| *c));
    }

    #[test]
    fn a_config_survives_a_trip_through_the_file() {
        let mut config = Config::default();
        config.master = false;
        config.preset = Preset::Action;
        config.apply_preset(Preset::Action);
        config.speed = 1.4;
        config.intensity = 1.25;
        config.blend = 0.05;
        config.channels[ch::EAT] = false;
        config.channels[ch::DEATH] = false;
        let text = config.to_toml();
        let back = Config::parse(&text);
        assert_eq!(back, config);
    }

    #[test]
    fn a_broken_file_leaves_the_defaults_standing() {
        let text = "master = maybe\nspeed = \"fast\"\nintensity = 99\n[animations]\nwalk = nim\nunknown = false\n";
        let config = Config::parse(text);
        assert!(config.master);
        assert_eq!(config.speed, 1.0);
        assert!((config.intensity - 1.5).abs() < 1e-6);
        assert!(config.channels[ch::WALK]);
    }

    #[test]
    fn comments_and_case_are_read_the_way_a_player_writes_them() {
        let text = "# mine\nPreset = \"Action\"  # trailing\nSPEED = 1.2\n\n[Animations]\nCOMBAT_UNKNOWN = false\ncombo = false\n";
        let config = Config::parse(text);
        assert_eq!(config.preset, Preset::Action);
        assert!((config.speed - 1.2).abs() < 1e-6);
        assert!(!config.channels[ch::COMBO]);
        assert!(config.channels[ch::HEAVY]);
    }

    #[test]
    fn a_preset_keeps_the_switches_it_does_not_own() {
        let mut config = Config::default();
        config.channels[ch::SNEAK] = false;
        config.apply_preset(Preset::Subtle);
        assert!(!config.channels[ch::SNEAK]);
        assert!(config.intensity < 1.0);
        assert_eq!(config.preset, Preset::Subtle);
        assert_eq!(Preset::Subtle.next(), Preset::Default);
        assert_eq!(Preset::Action.next(), Preset::Subtle);
    }

    #[test]
    fn the_easings_reach_their_ends() {
        assert!((ease_in_out(0.0) - 0.0).abs() < 1e-6);
        assert!((ease_in_out(1.0) - 1.0).abs() < 1e-6);
        assert!((ease_out(0.0) - 0.0).abs() < 1e-6);
        assert!((ease_out(1.0) - 1.0).abs() < 1e-6);
        assert!((arc(0.0, 0.32) - 0.0).abs() < 1e-6);
        assert!((arc(1.0, 0.32) - 0.0).abs() < 1e-6);
        assert!(arc(0.32) > 0.95);
    }

    #[test]
    fn a_moving_value_settles_where_it_is_going() {
        let mut v = 0.0;
        for _ in 0..200 {
            v = smooth_toward(v, 1.0, 0.1, 1.0 / 60.0);
        }
        assert!((v - 1.0).abs() < 0.01);
        // Splitting the same time in two does not change the answer much.
        let one = smooth_toward(0.0, 1.0, 0.1, 0.05);
        let mut two = smooth_toward(0.0, 1.0, 0.1, 0.025);
        two = smooth_toward(two, 1.0, 0.1, 0.025);
        assert!((one - two).abs() < 0.01);
    }

    #[test]
    fn a_channel_fades_in_over_its_blend_time() {
        let config = Config::default();
        let mut a = Animator::new();
        tick(&mut a, &senses(), &config, 1);
        let first = a.w(ch::WALK);
        tick(&mut a, &senses(), &config, 4);
        let later = a.w(ch::WALK);
        assert!(first > 0.0 && first < 0.6, "first tick {first}");
        assert!(later > first && later < 1.0, "later {later}");
    }

    #[test]
    fn a_switched_off_channel_fades_out() {
        let mut config = Config::default();
        let mut a = Animator::new();
        tick(&mut a, &senses(), &config, 40);
        assert!(a.w(ch::WALK) > 0.95);
        config.channels[ch::WALK] = false;
        tick(&mut a, &senses(), &config, 40);
        assert!(a.w(ch::WALK) < 0.05);
    }

    /// One swing, as the engine reports it: progress rises, then falls back to
    /// zero, and the drop is what starts a new hit.
    fn swing(a: &mut Animator, s: &mut Senses, config: &Config) {
        s.swing = 0.9;
        a.update(s, 0.05, config);
        s.swing = 0.0;
        a.update(s, 0.05, config);
    }

    #[test]
    fn the_combo_counts_three_hits_and_starts_over() {
        let config = Config::default();
        let mut a = Animator::new();
        let mut s = senses();
        for step in 0..4u8 {
            swing(&mut a, &mut s, &config);
            assert_eq!(a.combo_step, step % 3, "step {step}");
            // Eight ticks later: still inside the combo window.
            s.age += 8.0;
        }
    }

    #[test]
    fn a_slow_swing_starts_the_combo_over() {
        let config = Config::default();
        let mut a = Animator::new();
        let mut s = senses();
        swing(&mut a, &mut s, &config);
        swing(&mut a, &mut s, &config);
        assert_eq!(a.combo_step, 1);
        // Well past the window: the next hit is the first hit again.
        s.age += 40.0;
        swing(&mut a, &mut s, &config);
        assert_eq!(a.combo_step, 0);
    }

    #[test]
    fn a_landing_pushes_and_then_lets_go() {
        let config = Config::default();
        let mut a = Animator::new();
        let mut s = senses();
        s.on_ground = true;
        a.update(&s, 0.05, &config);
        s.on_ground = false;
        s.vy = -0.8;
        s.age += 4.0;
        a.update(&s, 0.05, &config);
        s.on_ground = true;
        s.vy = 0.0;
        s.age += 2.0;
        a.update(&s, 0.05, &config);
        assert!(a.landing(s.age) > 0.5, "{}", a.landing(s.age));
        assert!(a.landing(s.age + 40.0) < 0.01);
    }

    #[test]
    fn a_hurt_flinch_is_strongest_when_it_lands() {
        let config = Config::default();
        let mut a = Animator::new();
        let mut s = senses();
        a.update(&s, 0.05, &config);
        s.hurt_time = 10;
        s.age += 1.0;
        a.update(&s, 0.05, &config);
        let at_once = a.flinch(s.age);
        assert!(at_once > 0.3);
        assert!(a.flinch(s.age + 20.0) < 0.01);
    }

    #[test]
    fn the_body_pose_stays_inside_what_the_model_can_draw() {
        let config = Config::default();
        let mut a = Animator::new();
        let mut s = senses();
        s.sprinting = true;
        s.speed = 0.2;
        s.on_ground = false;
        s.vy = -1.4;
        s.hurt_time = 8;
        s.death_time = 4;
        s.swing = 0.6;
        s.use_progress = 1.0;
        s.head_yaw = 120.0;
        s.head_pitch = 60.0;
        tick(&mut a, &s, &config, 200);
        let off = body_offsets(&a, &config, s.age);
        for part in &off {
            for k in 0..3 {
                assert!(part[k].is_finite() && part[k].abs() <= 6.0);
            }
            for k in 3..6 {
                assert!(part[k].is_finite() && part[k].abs() <= 1.5);
            }
            for k in 6..9 {
                assert!(part[k].is_finite() && part[k] >= 0.5 && part[k] <= 1.5);
            }
        }
    }

    #[test]
    fn the_walk_moves_the_arms_the_way_the_legs_go() {
        let config = Config::default();
        let mut a = Animator::new();
        let mut s = senses();
        tick(&mut a, &s, &config, 60);
        // A quarter of the cycle in: the arms are at their widest.
        s.walk_position = core::f32::consts::FRAC_PI_2 / 0.6662;
        tick(&mut a, &s, &config, 40);
        let off = body_offsets(&a, &config, s.age);
        // The arms are on opposite sides of the phase, and cancel.
        assert!(off[RIGHT_ARM][3].abs() > 0.001, "right {}", off[RIGHT_ARM][3]);
        assert!(off[LEFT_ARM][3].abs() > 0.001, "left {}", off[LEFT_ARM][3]);
        assert!((off[RIGHT_ARM][3] + off[LEFT_ARM][3]).abs() < 0.02);
    }

    #[test]
    fn a_pose_of_ours_is_added_and_leaves_what_it_finds() {
        let config = Config::default();
        let mut a = Animator::new();
        let mut s = senses();
        tick(&mut a, &s, &config, 80);
        let off = body_offsets(&a, &config, s.age);
        let mut poses = [[0.0f32, 0.0, 0.0, 0.1, 0.2, 0.3, 1.0, 1.0, 1.0]; 6];
        let before = poses;
        for (i, o) in off.iter().enumerate() {
            for k in 0..6 {
                poses[i][k] += o[k];
            }
            for k in 6..9 {
                poses[i][k] *= o[k];
            }
        }
        // Nothing written is further from vanilla's than the offsets allow.
        for i in 0..6 {
            for k in 3..6 {
                assert!((poses[i][k] - before[i][k]).abs() <= 1.5 + 1e-3);
            }
        }
    }

    #[test]
    fn a_hand_pose_keeps_to_what_the_hook_takes() {
        // The hook's own limits: offsets within -16..=16 blocks, angles within
        // -3600..=3600 degrees, scale within 0.01..=100.
        let mut pose = HandPose {
            x: 99.0,
            y: -99.0,
            z: 0.5,
            pitch: 5000.0,
            yaw: -12000.0,
            roll: 900.0,
            scale: 40.0,
        };
        pose.sanitize();
        assert!(pose.x.abs() <= 16.0 && pose.y.abs() <= 16.0);
        assert!(pose.pitch.abs() <= 3600.0 && pose.yaw.abs() <= 3600.0);
        assert!((0.01..=100.0).contains(&pose.scale));

        // Two poses compose: the offsets add and the scales multiply.
        let mut a = HandPose::neutral();
        a.add(&HandPose {
            x: 0.25,
            scale: 1.5,
            ..HandPose::neutral()
        });
        a.add(&HandPose {
            x: 0.25,
            scale: 1.5,
            ..HandPose::neutral()
        });
        assert!((a.x - 0.5).abs() < 1e-6);
        assert!((a.scale - 2.25).abs() < 1e-6);
    }

    #[test]
    fn a_camera_sway_never_exceeds_what_the_game_clamps_to() {
        let mut shake = Shake::default();
        shake.pitch = 40.0;
        shake.offset = [3.0, -2.0, 1.0];
        shake.sanitize();
        assert!(shake.pitch <= 10.0 && shake.pitch >= -10.0);
        for v in shake.offset {
            assert!(v.abs() <= 0.5);
        }
    }
}
