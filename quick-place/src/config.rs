//! `config.toml`, parsed with no dependencies.
//!
//! The legacy WebAssembly API has no filesystem access, so the file is compiled
//! in with `include_str!` and this parser is the only reader. It accepts the
//! small subset the file uses — `key = true|false|integer|"string"`, `#` comments,
//! blank lines — and fails closed: an unknown key, a repeated key, a missing key
//! or a value that does not parse is an error, so a typo can never be ignored.

use crate::planner::Mode;

/// The hard cap on `max_blocks_per_sweep`; the host takes up to 4096 per call,
/// but a building aid has no reason to exceed this in one sweep.
pub const MAX_SWEEP: u32 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// `false`: the mod loads, registers its command and does nothing on placement.
    pub enabled: bool,
    /// How many blocks one placement may add (0..=64).
    pub max_blocks_per_sweep: u32,
    /// Parsed and reported, but **inert**: the legacy server `Player` record has
    /// no sneaking field, so this cannot be honoured (see README, `TODO(api)`).
    pub require_sneaking: bool,
    pub placement_mode: Mode,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            max_blocks_per_sweep: 4,
            require_sneaking: false,
            placement_mode: Mode::Pad,
        }
    }
}

/// Where a value sits in the file, for error messages.
struct Value {
    text: String,
    line: usize,
}

/// The `key = value` pairs of a file, in order.
fn pairs(source: &str) -> Result<Vec<(String, Value)>, String> {
    let mut out = Vec::new();
    for (index, raw) in source.lines().enumerate() {
        let line_number = index + 1;
        let line = match raw.find('#') {
            Some(comment) => &raw[..comment],
            None => raw,
        }
        .trim();
        if line.is_empty() {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("config.toml line {line_number}: expected `key = value`"))?;
        let key = key.trim();
        if key.is_empty() {
            return Err(format!("config.toml line {line_number}: empty key"));
        }
        out.push((
            key.to_string(),
            Value {
                text: value.trim().to_string(),
                line: line_number,
            },
        ));
    }
    Ok(out)
}

fn boolean(key: &str, value: &Value) -> Result<bool, String> {
    match value.text.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!(
            "config.toml line {}: {key} = {other}: expected true or false",
            value.line
        )),
    }
}

fn integer(key: &str, value: &Value) -> Result<u32, String> {
    value
        .text
        .parse::<u32>()
        .map_err(|_| format!("config.toml line {}: {key}: expected a whole number", value.line))
}

fn string<'a>(key: &str, value: &'a Value) -> Result<&'a str, String> {
    let text = value.text.as_str();
    text.strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .filter(|inner| !inner.contains('"'))
        .ok_or_else(|| {
            format!(
                "config.toml line {}: {key}: expected a \"quoted string\"",
                value.line
            )
        })
}

/// Parses and validates a whole `config.toml`.
pub fn parse(source: &str) -> Result<Config, String> {
    let mut config = Config::default();
    let mut seen: Vec<String> = Vec::new();
    for (key, value) in pairs(source)? {
        if seen.contains(&key) {
            return Err(format!(
                "config.toml line {}: {key} is set twice",
                value.line
            ));
        }
        match key.as_str() {
            "enabled" => config.enabled = boolean(&key, &value)?,
            "max_blocks_per_sweep" => config.max_blocks_per_sweep = integer(&key, &value)?,
            "require_sneaking" => config.require_sneaking = boolean(&key, &value)?,
            "placement_mode" => {
                let text = string(&key, &value)?;
                config.placement_mode = Mode::parse(text).ok_or_else(|| {
                    format!(
                        "config.toml line {}: placement_mode must be pad, line, pillar or cube, not {text:?}",
                        value.line
                    )
                })?;
            }
            other => {
                return Err(format!(
                    "config.toml line {}: unknown setting {other}",
                    value.line
                ))
            }
        }
        seen.push(key);
    }
    for required in [
        "enabled",
        "max_blocks_per_sweep",
        "require_sneaking",
        "placement_mode",
    ] {
        if !seen.iter().any(|key| key == required) {
            return Err(format!("config.toml: {required} is missing"));
        }
    }
    if config.max_blocks_per_sweep > MAX_SWEEP {
        return Err(format!(
            "config.toml: max_blocks_per_sweep must be 0..={MAX_SWEEP}, not {}",
            config.max_blocks_per_sweep
        ));
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIPPED: &str = include_str!("../config.toml");

    #[test]
    fn the_shipped_config_parses_and_matches_its_documented_defaults() {
        let config = parse(SHIPPED).unwrap();
        assert!(config.enabled);
        assert_eq!(config.max_blocks_per_sweep, 4);
        assert!(!config.require_sneaking);
        assert_eq!(config.placement_mode, Mode::Pad);
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let config = parse(
            "# a leading comment\n\nenabled = false # trailing\nmax_blocks_per_sweep = 0\n\
             require_sneaking = true\nplacement_mode = \"cube\"\n",
        )
        .unwrap();
        assert!(!config.enabled);
        assert_eq!(config.max_blocks_per_sweep, 0);
        assert!(config.require_sneaking);
        assert_eq!(config.placement_mode, Mode::Cube);
    }

    #[test]
    fn bad_settings_fail_closed() {
        for (source, expected) in [
            (
                SHIPPED.replace("max_blocks_per_sweep = 4", "max_blocks_per_sweep = 65"),
                "0..=64",
            ),
            (
                SHIPPED.replace("placement_mode = \"pad\"", "placement_mode = \"circle\""),
                "pad, line, pillar or cube",
            ),
            (
                SHIPPED.replace("enabled = true", "enabled = yes"),
                "expected true or false",
            ),
            (format!("{SHIPPED}extra = 1\n"), "unknown setting extra"),
            (
                SHIPPED.replace("enabled = true\n", ""),
                "enabled is missing",
            ),
            (
                SHIPPED.replace("enabled = true", "enabled = true\nenabled = false"),
                "set twice",
            ),
        ] {
            let error = parse(&source).unwrap_err();
            assert!(
                error.contains(expected),
                "expected {expected:?} in {error:?}"
            );
        }
    }
}
