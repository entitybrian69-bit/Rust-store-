//! Volt Lapse settings: one schema, one parser, one validator.
//!
//! Every setting in `config.toml` is described here once (name, type, range or
//! allowed values, default). The same table parses the shipped file, validates
//! `/vl config <key> <value>`, renders the values back to TOML for persistence in
//! the mod's `storage`, and refuses anything it does not know. There are no stubs
//! and no silently ignored keys: an unknown key, a wrong type or an out-of-range
//! value is an error that names the key.

/// What a setting accepts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Bool,
    /// Inclusive integer range.
    Int(i64, i64),
    /// Inclusive float range.
    Float(f64, f64),
    /// Free text, at most this many characters.
    Text(usize),
    /// One of these (snake_case) values.
    Enum(&'static [&'static str]),
    /// List of floats, each within the range, at most this many entries.
    FloatList(f64, f64, usize),
}

impl Kind {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Int(..) => "int",
            Self::Float(..) => "float",
            Self::Text(_) => "string",
            Self::Enum(_) => "enum",
            Self::FloatList(..) => "float list",
        }
    }

    /// What the error tells a player to use.
    pub fn expect(&self) -> String {
        match self {
            Self::Bool => "true or false".into(),
            Self::Int(low, high) => format!("a whole number {low}..={high}"),
            Self::Float(low, high) => format!("a number {low}..={high}"),
            Self::Text(max) => format!("text of at most {max} characters"),
            Self::Enum(values) => format!("one of {}", values.join("|")),
            Self::FloatList(low, high, max) => {
                format!("a list of at most {max} numbers {low}..={high}, like [0.5, 1, 2]")
            }
        }
    }
}

/// One setting.
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub key: &'static str,
    pub kind: Kind,
    /// A TOML literal, parsed by the same code as the file.
    pub default: &'static str,
}

const fn spec(key: &'static str, kind: Kind, default: &'static str) -> Spec {
    Spec {
        key,
        kind,
        default,
    }
}

const THEMES: &[&str] = &["dark", "light", "auto"];
const INTERPOLATIONS: &[&str] = &["linear", "catmull_rom", "bezier"];
const TIMELINE_POSITIONS: &[&str] = &["bottom", "top", "hidden"];
const FILE_NAMINGS: &[&str] = &["timestamp", "world_name", "manual"];
const LOG_LEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];

/// Every setting, in the order the README documents them.
pub static SPECS: &[Spec] = &[
    // ---- recording -------------------------------------------------------
    spec("enabled", Kind::Bool, "true"),
    spec("auto_start_on_join", Kind::Bool, "false"),
    spec("auto_start_on_world_load", Kind::Bool, "false"),
    spec("record_interval_ticks", Kind::Int(1, 20), "1"),
    spec("record_chat", Kind::Bool, "true"),
    spec("record_player_inventory", Kind::Bool, "true"),
    spec("record_entity_metadata", Kind::Bool, "false"),
    spec("record_block_changes", Kind::Bool, "true"),
    spec("record_particles", Kind::Bool, "false"),
    spec("record_sounds", Kind::Bool, "false"),
    spec("record_weather", Kind::Bool, "false"),
    spec("record_time_of_day", Kind::Bool, "false"),
    spec("record_dimension_changes", Kind::Bool, "true"),
    spec("record_mob_ai", Kind::Bool, "false"),
    spec("record_item_drops", Kind::Bool, "false"),
    spec("record_projectiles", Kind::Bool, "false"),
    spec("chunk_radius", Kind::Int(0, 16), "8"),
    spec("entity_tracking_radius", Kind::Int(0, 256), "64"),
    spec("max_recording_size_mb", Kind::Int(1, 65536), "2048"),
    spec("max_recording_duration_min", Kind::Int(1, 1440), "120"),
    spec("buffer_flush_interval_sec", Kind::Int(1, 60), "5"),
    spec("compression_level", Kind::Int(0, 9), "6"),
    spec("keyframe_interval_ticks", Kind::Int(1, 72000), "600"),
    spec("capture_fps", Kind::Int(1, 60), "20"),
    // ---- playback --------------------------------------------------------
    spec("default_speed", Kind::Float(0.1, 8.0), "1.0"),
    spec(
        "speed_presets",
        Kind::FloatList(0.1, 8.0, 16),
        "[0.1, 0.25, 0.5, 1, 2, 4, 8]",
    ),
    spec("smooth_camera", Kind::Bool, "true"),
    spec("camera_interpolation", Kind::Enum(INTERPOLATIONS), "\"catmull_rom\""),
    spec("free_camera", Kind::Bool, "false"),
    spec("spectator_lock", Kind::Bool, "false"),
    spec("show_timeline", Kind::Bool, "true"),
    spec("show_keyframes", Kind::Bool, "true"),
    spec("show_path_preview", Kind::Bool, "true"),
    spec("playback_hud", Kind::Bool, "true"),
    spec("hide_hud_in_replay", Kind::Bool, "false"),
    spec("render_distance", Kind::Int(2, 32), "12"),
    spec("time_control", Kind::Bool, "false"),
    spec("weather_control", Kind::Bool, "false"),
    spec("loop_playback", Kind::Bool, "false"),
    spec("skip_to_keyframe_on_load", Kind::Bool, "true"),
    // ---- camera & paths --------------------------------------------------
    spec("keyframe_snap_ticks", Kind::Int(0, 20), "1"),
    spec("path_smoothing", Kind::Float(0.0, 1.0), "0.5"),
    spec("path_tension", Kind::Float(0.0, 1.0), "0.5"),
    spec("fov_override", Kind::Float(30.0, 120.0), "70.0"),
    spec("camera_roll_enabled", Kind::Bool, "false"),
    spec("camera_collision", Kind::Bool, "false"),
    spec("auto_keyframe_on_teleport", Kind::Bool, "true"),
    // ---- storage & files -------------------------------------------------
    spec("replay_folder", Kind::Text(64), "\"replays\""),
    spec("temp_folder", Kind::Text(64), "\"replays/tmp\""),
    spec("file_naming", Kind::Enum(FILE_NAMINGS), "\"timestamp\""),
    spec("auto_delete_after_days", Kind::Int(0, 3650), "0"),
    spec("max_replays_kept", Kind::Int(0, 10000), "0"),
    spec("write_chunk_size_kb", Kind::Int(1, 8192), "1024"),
    spec("atomic_write", Kind::Bool, "true"),
    // ---- UI --------------------------------------------------------------
    spec("theme", Kind::Enum(THEMES), "\"dark\""),
    spec("language", Kind::Text(16), "\"en\""),
    spec("ui_scale", Kind::Float(0.5, 3.0), "1.0"),
    spec("timeline_position", Kind::Enum(TIMELINE_POSITIONS), "\"bottom\""),
    spec("keybind_open_replay_menu", Kind::Text(24), "\"F6\""),
    spec("keybind_toggle_record", Kind::Text(24), "\"F7\""),
    spec("keybind_add_keyframe", Kind::Text(24), "\"K\""),
    spec("keybind_play_pause", Kind::Text(24), "\"Space\""),
    spec("keybind_speed_up", Kind::Text(24), "\"ArrowUp\""),
    spec("keybind_slow_down", Kind::Text(24), "\"ArrowDown\""),
    // ---- advanced --------------------------------------------------------
    spec("debug_logging", Kind::Bool, "false"),
    spec("log_level", Kind::Enum(LOG_LEVELS), "\"info\""),
    spec("metrics_opt_in", Kind::Bool, "false"),
    spec("experimental_features", Kind::Bool, "false"),
    spec("safe_mode", Kind::Bool, "true"),
];

/// A validated value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    List(Vec<f64>),
}

impl Value {
    /// A TOML literal for this value.
    pub fn render(&self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::Int(value) => value.to_string(),
            Self::Float(value) => format_float(*value),
            Self::Text(value) => format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"")),
            Self::List(values) => format!(
                "[{}]",
                values
                    .iter()
                    .map(|v| format_float(*v))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::Text(_) => "string",
            Self::List(_) => "float list",
        }
    }
}

/// `1.0` stays `1.0`, `0.5` stays `0.5`: always a decimal point, never `1e3`.
pub fn format_float(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{value:.1}")
    } else {
        format!("{value}")
    }
}

/// Case and dot-insensitive setting lookup: `Keybind.Open.Replay.Menu` works.
pub fn normalize(key: &str) -> String {
    key.trim()
        .to_ascii_lowercase()
        .replace(['.', '-', ' '], "_")
}

pub fn spec_for(key: &str) -> Option<&'static Spec> {
    let key = normalize(key);
    SPECS.iter().find(|spec| spec.key == key)
}

/// Parses one right-hand side (`true`, `7`, `0.5`, `"text"`, `[1, 2]`).
pub fn parse_value(kind: Kind, text: &str) -> Result<Value, String> {
    let text = text.trim();
    match kind {
        Kind::Bool => match text {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err(format!("expected true or false, not {text:?}")),
        },
        Kind::Int(low, high) => {
            let number: i64 = text
                .parse()
                .map_err(|_| format!("expected a whole number {low}..={high}, not {text:?}"))?;
            if number < low || number > high {
                return Err(format!("{number} is outside {low}..={high}"));
            }
            Ok(Value::Int(number))
        }
        Kind::Float(low, high) => {
            let number: f64 = text
                .parse()
                .map_err(|_| format!("expected a number {low}..={high}, not {text:?}"))?;
            if !number.is_finite() || number < low || number > high {
                return Err(format!("{text} is outside {low}..={high}"));
            }
            Ok(Value::Float(number))
        }
        Kind::Text(max) => {
            // Quotes are how the file stores text; on the command line they are
            // optional, so `/vl config replay_folder replays` works too.
            let inner = text
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
                .unwrap_or(text);
            if inner.len() > max {
                return Err(format!("{} characters is over {max}", inner.len()));
            }
            Ok(Value::Text(inner.replace("\\\"", "\"").replace("\\\\", "\\")))
        }
        Kind::Enum(allowed) => {
            let inner = text
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
                .unwrap_or(text);
            let name = normalize(inner);
            if allowed.iter().any(|value| *value == name) {
                Ok(Value::Text(name))
            } else {
                Err(format!("{inner:?} is not one of {}", allowed.join("|")))
            }
        }
        Kind::FloatList(low, high, max) => {
            let inner = text
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
                .ok_or_else(|| format!("expected a list like [0.5, 1, 2], not {text:?}"))?;
            let mut values = Vec::new();
            for item in inner.split(',') {
                let item = item.trim();
                if item.is_empty() {
                    continue;
                }
                let number: f64 = item
                    .parse()
                    .map_err(|_| format!("{item:?} in the list is not a number"))?;
                if !number.is_finite() || number < low || number > high {
                    return Err(format!("{item} is outside {low}..={high}"));
                }
                values.push(number);
            }
            if values.is_empty() {
                return Err("the list is empty".into());
            }
            if values.len() > max {
                return Err(format!("{} entries is over {max}", values.len()));
            }
            Ok(Value::List(values))
        }
    }
}

/// The whole configuration: every spec, in order, with a value.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    values: Vec<Value>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            values: SPECS
                .iter()
                .map(|spec| parse_value(spec.kind, spec.default).expect("a valid default"))
                .collect(),
        }
    }
}

impl Config {
    fn index(key: &str) -> Option<usize> {
        SPECS.iter().position(|spec| spec.key == normalize(key))
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        Self::index(key).map(|i| &self.values[i])
    }

    pub fn bool_of(&self, key: &str) -> bool {
        matches!(self.get(key), Some(Value::Bool(true)))
    }

    pub fn int_of(&self, key: &str) -> i64 {
        match self.get(key) {
            Some(Value::Int(value)) => *value,
            _ => 0,
        }
    }

    pub fn float_of(&self, key: &str) -> f64 {
        match self.get(key) {
            Some(Value::Float(value)) => *value,
            _ => 0.0,
        }
    }

    pub fn text_of(&self, key: &str) -> &str {
        match self.get(key) {
            Some(Value::Text(value)) => value,
            _ => "",
        }
    }

    pub fn list_of(&self, key: &str) -> &[f64] {
        match self.get(key) {
            Some(Value::List(values)) => values,
            _ => &[],
        }
    }

    /// Sets one key from its textual form, validating against its spec.
    pub fn set(&mut self, key: &str, text: &str) -> Result<(), String> {
        let index = Self::index(key).ok_or_else(|| format!("unknown setting {key:?}"))?;
        let value = parse_value(SPECS[index].kind, text)
            .map_err(|error| format!("{}: {error}", SPECS[index].key))?;
        self.values[index] = value;
        Ok(())
    }

    /// Everything back to the shipped defaults.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The file's contents, every key, every value.
    pub fn render(&self) -> String {
        let mut out = String::from(
            "# Volt Lapse configuration. Written by the mod; /vl config <key> <value> edits it.\n",
        );
        for (spec, value) in SPECS.iter().zip(&self.values) {
            out.push_str(spec.key);
            out.push_str(" = ");
            out.push_str(&value.render());
            out.push('\n');
        }
        out
    }

    /// Parses a whole file. Sections (`[recording]`) are cosmetic: keys are flat
    /// and must be unique. A missing key keeps its default.
    pub fn parse(source: &str) -> Result<Self, String> {
        let mut config = Self::default();
        let mut seen: Vec<String> = Vec::new();
        for (index, raw) in source.lines().enumerate() {
            let line = index + 1;
            let mut text = match raw.find('#') {
                Some(comment) => &raw[..comment],
                None => raw,
            }
            .trim();
            if text.is_empty() {
                continue;
            }
            if text.starts_with('[') {
                if !text.ends_with(']') {
                    return Err(format!("config.toml line {line}: unterminated section header"));
                }
                continue;
            }
            let (key, value) = text
                .split_once('=')
                .ok_or_else(|| format!("config.toml line {line}: expected `key = value`"))?;
            text = key.trim();
            let spec = spec_for(text)
                .ok_or_else(|| format!("config.toml line {line}: unknown setting {text:?}"))?;
            if seen.iter().any(|k| k == spec.key) {
                return Err(format!("config.toml line {line}: {} is set twice", spec.key));
            }
            let parsed = parse_value(spec.kind, value)
                .map_err(|error| format!("config.toml line {line}: {}: {error}", spec.key))?;
            let position = SPECS
                .iter()
                .position(|candidate| candidate.key == spec.key)
                .expect("a spec from SPECS");
            config.values[position] = parsed;
            seen.push(spec.key.to_string());
        }
        Ok(config)
    }

    /// `key = value` lines for every setting whose value is not the default.
    pub fn changed(&self) -> Vec<String> {
        let defaults = Self::default();
        SPECS
            .iter()
            .zip(&self.values)
            .zip(&defaults.values)
            .filter(|((_, value), (_, default))| value != default)
            .map(|((spec, value), _)| format!("{} = {}", spec.key, value.render()))
            .collect()
    }
}

/// One line per setting: `key = value` plus its type.
pub fn summary(config: &Config) -> Vec<String> {
    SPECS
        .iter()
        .zip(&config.values)
        .map(|(spec, value)| {
            format!("{} = {} ({})", spec.key, value.render(), value.type_name())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_is_valid_and_round_trips() {
        let config = Config::default();
        let rendered = config.render();
        let parsed = Config::parse(&rendered).unwrap();
        assert_eq!(config, parsed);
        assert_eq!(SPECS.len(), 69, "every documented setting exists once");
        for spec in SPECS {
            assert!(
                config.get(spec.key).is_some(),
                "{} has a value",
                spec.key
            );
        }
    }

    #[test]
    fn shipped_config_matches_the_defaults() {
        let shipped = include_str!("../config.toml");
        assert_eq!(Config::parse(shipped).unwrap(), Config::default());
    }

    #[test]
    fn types_and_ranges_are_enforced_with_clear_errors() {
        for (key, text, expected) in [
            ("record_interval_ticks", "21", "outside 1..=20"),
            ("record_interval_ticks", "fast", "whole number"),
            ("capture_fps", "0", "outside 1..=60"),
            ("enabled", "yes", "true or false"),
            ("camera_interpolation", "\"spline\"", "not one of"),
            ("path_tension", "1.5", "outside 0..=1"),
            ("speed_presets", "[1, 9]", "outside 0.1..=8"),
            ("speed_presets", "[]", "empty"),
            ("theme", "midnight", "not one of"),
            ("nonexistent_setting", "1", "unknown setting"),
        ] {
            let error = Config::default().set(key, text).unwrap_err();
            assert!(error.contains(expected), "{key}={text}: {error}");
        }
    }

    #[test]
    fn setting_is_normalised_and_persisted() {
        let mut config = Config::default();
        config.set("Record.Interval.Ticks", "5").unwrap();
        assert_eq!(config.int_of("record_interval_ticks"), 5);
        config
            .set("camera_interpolation", "\"bezier\"")
            .unwrap();
        assert_eq!(config.text_of("camera_interpolation"), "bezier");
        config.set("speed_presets", "[0.5, 1, 2]").unwrap();
        // Quotes are optional on the command line.
        config.set("replay_folder", "replays").unwrap();
        assert_eq!(config.text_of("replay_folder"), "replays");
        assert!(config
            .set("replay_folder", &"x".repeat(300))
            .unwrap_err()
            .contains("over"));
        assert_eq!(config.list_of("speed_presets"), &[0.5, 1.0, 2.0]);
        let changed = config.changed();
        assert!(changed.contains(&"record_interval_ticks = 5".to_string()));
        config.reset();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn the_file_parser_rejects_bad_lines_and_keeps_defaults_for_missing_keys() {
        assert!(Config::parse("not a pair").unwrap_err().contains("key = value"));
        assert!(Config::parse("mystery = 1").unwrap_err().contains("unknown setting"));
        assert!(Config::parse("[recording]\nenabled = true\nenabled = false")
            .unwrap_err()
            .contains("set twice"));
        // Sections are cosmetic and missing keys keep their defaults.
        let parsed = Config::parse("[recording]\nenabled = false\n").unwrap();
        assert!(!parsed.bool_of("enabled"));
        assert_eq!(parsed.int_of("capture_fps"), 20);
    }
}
