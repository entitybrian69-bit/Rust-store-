// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! `/voltlapse <record|play|render|config|...>`: parsing, completion and the
//! messages the chat shows.
//!
//! The parser is pure: it turns a line of text into a [`Command`] and never
//! touches the game. Every user-visible string is a translation key, so the mod
//! ships one language file and every message is localisable.

use crate::config::Section;

/// What the command line asked for.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// `/voltlapse record [start|stop|pause|resume|stats]`
    Record { action: RecordAction },
    /// `/voltlapse play [name] [at <time>] [speed <x>]`
    Play { replay: Option<String>, at_seconds: Option<f64>, speed: Option<f64> },
    /// `/voltlapse render [start|queue|cancel]`
    Render { action: RenderAction },
    /// `/voltlapse config <section> <key> [value]` or `<section> reset`
    Config { section: Section, key: Option<String>, value: Option<String> },
    /// `/voltlapse keyframe <add|clear|list|delete> [tick]`
    Keyframe { action: KeyframeAction },
    /// `/voltlapse bookmark <add|remove|list> [label]`
    Bookmark { action: BookmarkAction, label: Option<String> },
    /// `/voltlapse list [query]` — the replay browser in chat.
    List { query: String },
    /// `/voltlapse info` — the replay being played.
    Info,
    /// `/voltlapse export <name>`
    Export { name: String },
    /// `/voltlapse share` — multiplayer replay sync.
    Share { action: ShareAction },
    /// `/voltlapse help`
    Help,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordAction {
    /// Toggle: start when idle, stop when recording.
    Toggle,
    Start,
    Stop,
    Pause,
    Resume,
    /// Write a recovery checkpoint now.
    Save,
    Stats,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderAction {
    Start,
    /// Render everything queued.
    Queue,
    Cancel,
    Status,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyframeAction {
    /// Add at the current tick.
    Add,
    /// Add at an explicit tick.
    AddAt(u64),
    Delete(u64),
    Clear,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BookmarkAction {
    Add,
    Remove(u64),
    List,
    Next,
    Previous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShareAction {
    Host,
    Join,
    Leave,
    Status,
}

/// Why a command line was refused.
#[derive(Clone, Debug, PartialEq)]
pub enum CommandError {
    /// The verb is not one of ours.
    Unknown(String),
    /// A verb needs an argument it did not get.
    MissingArgument(&'static str),
    /// A number did not parse, or was out of range.
    BadNumber(String),
    /// A `mm:ss` timestamp did not parse.
    BadTime(String),
    /// The section name is not one of the config's.
    UnknownSection(String),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::Unknown(verb) => write!(f, "unknown subcommand '{verb}'"),
            CommandError::MissingArgument(name) => write!(f, "missing argument <{name}>"),
            CommandError::BadNumber(text) => write!(f, "'{text}' is not a number"),
            CommandError::BadTime(text) => write!(f, "'{text}' is not a time (use mm:ss.d)"),
            CommandError::UnknownSection(section) => write!(f, "unknown settings section '{section}'"),
        }
    }
}

impl std::error::Error for CommandError {}

/// The root command name, without a slash.
pub const ROOT: &str = "voltlapse";

/// Parse the arguments that follow `/voltlapse`.
pub fn parse(args: &str) -> Result<Command, CommandError> {
    let mut words = args.split_whitespace();
    let Some(verb) = words.next() else {
        return Ok(Command::Help);
    };
    let verb = verb.to_ascii_lowercase();
    let rest: Vec<&str> = words.collect();
    match verb.as_str() {
        "help" | "?" => Ok(Command::Help),
        "record" | "rec" => Ok(Command::Record {
            action: match rest.first().map(|word| word.to_ascii_lowercase()).as_deref() {
                None | Some("toggle") => RecordAction::Toggle,
                Some("start") => RecordAction::Start,
                Some("stop") => RecordAction::Stop,
                Some("pause") => RecordAction::Pause,
                Some("resume") => RecordAction::Resume,
                Some("save") => RecordAction::Save,
                Some("stats") => RecordAction::Stats,
                Some(other) => return Err(CommandError::Unknown(format!("record {other}"))),
            },
        }),
        "play" | "watch" => {
            let mut replay = None;
            let mut at_seconds = None;
            let mut speed = None;
            let mut index = 0;
            while index < rest.len() {
                match rest[index].to_ascii_lowercase().as_str() {
                    "at" | "seek" => {
                        let value = rest.get(index + 1).ok_or(CommandError::MissingArgument("time"))?;
                        at_seconds = Some(parse_time(value)?);
                        index += 2;
                    }
                    "speed" => {
                        let value = rest.get(index + 1).ok_or(CommandError::MissingArgument("speed"))?;
                        let parsed: f64 = value.parse().map_err(|_| CommandError::BadNumber(value.to_string()))?;
                        speed = Some(parsed.clamp(0.1, 16.0));
                        index += 2;
                    }
                    other => {
                        replay = Some(other.to_owned());
                        index += 1;
                    }
                }
            }
            Ok(Command::Play { replay, at_seconds, speed })
        }
        "render" => Ok(Command::Render {
            action: match rest.first().map(|word| word.to_ascii_lowercase()).as_deref() {
                None | Some("start") => RenderAction::Start,
                Some("queue") => RenderAction::Queue,
                Some("cancel") | Some("stop") => RenderAction::Cancel,
                Some("status") => RenderAction::Status,
                Some(other) => return Err(CommandError::Unknown(format!("render {other}"))),
            },
        }),
        "config" | "settings" => {
            let section_name = rest.first().ok_or(CommandError::MissingArgument("section"))?;
            let section = Section::from_id(&section_name.to_ascii_lowercase())
                .ok_or_else(|| CommandError::UnknownSection((*section_name).to_owned()))?;
            let key = rest.get(1).map(|key| (*key).to_owned());
            let value = rest.get(2).map(|value| (*value).to_owned());
            Ok(Command::Config { section, key, value })
        }
        "keyframe" | "kf" => {
            let action = match rest.first().map(|word| word.to_ascii_lowercase()).as_deref() {
                None | Some("add") => match rest.get(1) {
                    Some(tick) => KeyframeAction::AddAt(parse_tick(tick)?),
                    None => KeyframeAction::Add,
                },
                Some("delete") | Some("remove") => {
                    let tick = rest.get(1).ok_or(CommandError::MissingArgument("tick"))?;
                    KeyframeAction::Delete(parse_tick(tick)?)
                }
                Some("clear") => KeyframeAction::Clear,
                Some("list") => KeyframeAction::List,
                Some(other) => return Err(CommandError::Unknown(format!("keyframe {other}"))),
            };
            Ok(Command::Keyframe { action })
        }
        "bookmark" | "mark" => {
            let action = match rest.first().map(|word| word.to_ascii_lowercase()).as_deref() {
                None | Some("add") => BookmarkAction::Add,
                Some("remove") | Some("delete") => {
                    let tick = rest.get(1).ok_or(CommandError::MissingArgument("tick"))?;
                    BookmarkAction::Remove(parse_tick(tick)?)
                }
                Some("list") => BookmarkAction::List,
                Some("next") => BookmarkAction::Next,
                Some("previous") | Some("prev") => BookmarkAction::Previous,
                Some(other) => return Err(CommandError::Unknown(format!("bookmark {other}"))),
            };
            Ok(Command::Bookmark { action, label: rest.get(1).map(|label| (*label).to_owned()) })
        }
        "list" | "browse" => Ok(Command::List { query: rest.join(" ") }),
        "info" | "status" => Ok(Command::Info),
        "export" | "share-file" => {
            let name = rest.first().ok_or(CommandError::MissingArgument("name"))?;
            Ok(Command::Export { name: (*name).to_owned() })
        }
        "share" | "sync" => Ok(Command::Share {
            action: match rest.first().map(|word| word.to_ascii_lowercase()).as_deref() {
                Some("join") => ShareAction::Join,
                Some("leave") | Some("stop") => ShareAction::Leave,
                Some("status") => ShareAction::Status,
                _ => ShareAction::Host,
            },
        }),
        other => Err(CommandError::Unknown(other.to_owned())),
    }
}

fn parse_tick(text: &str) -> Result<u64, CommandError> {
    text.parse::<u64>().map_err(|_| CommandError::BadNumber(text.to_owned()))
}

/// `12`, `1:30`, `1:30.5` or `1:02:03` to seconds.
pub fn parse_time(text: &str) -> Result<f64, CommandError> {
    let parts: Vec<&str> = text.split(':').collect();
    if parts.len() > 3 || parts.iter().any(|part| part.is_empty()) {
        return Err(CommandError::BadTime(text.to_owned()));
    }
    let mut seconds = 0.0;
    for part in &parts {
        let value: f64 = part.parse().map_err(|_| CommandError::BadTime(text.to_owned()))?;
        if value < 0.0 {
            return Err(CommandError::BadTime(text.to_owned()));
        }
        seconds = seconds * 60.0 + value;
    }
    Ok(seconds)
}

/// The verb list, for tab completion and `/help`.
pub const VERBS: [(&str, &str); 11] = [
    ("record", "start, stop, pause, resume or show recording stats"),
    ("play", "open a recorded replay, optionally at a time and speed"),
    ("render", "render the active camera path to a video file"),
    ("config", "read or change a setting"),
    ("keyframe", "add, list or clear camera keyframes"),
    ("bookmark", "mark and navigate moments"),
    ("list", "list saved replays"),
    ("info", "what is recording or playing right now"),
    ("export", "copy a replay into the exports folder"),
    ("share", "host or join a shared replay session"),
    ("help", "this list"),
];

/// The words that follow `verb`, for tab completion.
pub fn complete(verb: Option<&str>, partial: &str) -> Vec<String> {
    let candidates: Vec<&str> = match verb {
        None => VERBS.iter().map(|(verb, _)| *verb).collect(),
        Some("record") => vec!["start", "stop", "pause", "resume", "save", "stats"],
        Some("render") => vec!["start", "queue", "cancel", "status"],
        Some("keyframe") => vec!["add", "delete", "clear", "list"],
        Some("bookmark") => vec!["add", "remove", "list", "next", "previous"],
        Some("config") => vec!["recording", "playback", "rendering", "camera", "hud", "keybinds", "storage", "performance", "ui"],
        Some("share") => vec!["host", "join", "leave", "status"],
        _ => Vec::new(),
    };
    candidates
        .into_iter()
        .filter(|candidate| candidate.starts_with(partial))
        .map(str::to_owned)
        .collect()
}

/// A message for the chat: a translation key and its arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub key: String,
    pub args: Vec<String>,
}

impl Message {
    pub fn new(key: impl Into<String>) -> Self {
        Self { key: key.into(), args: Vec::new() }
    }

    pub fn with(key: impl Into<String>, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self { key: key.into(), args: args.into_iter().map(Into::into).collect() }
    }

    /// Fill the key's placeholders (`{0}`, `{1}`) for a log line or a test.
    pub fn render(&self) -> String {
        let mut text = self.key.clone();
        for (index, arg) in self.args.iter().enumerate() {
            text = text.replace(&format!("{{{index}}}"), arg);
        }
        text
    }
}

impl Command {
    /// The message describing what the command will do, before it runs.
    pub fn acknowledgement(&self) -> Message {
        match self {
            Command::Record { action } => Message::new(format!("voltlapse.command.record.{}", match action {
                RecordAction::Toggle => "toggle",
                RecordAction::Start => "start",
                RecordAction::Stop => "stop",
                RecordAction::Pause => "pause",
                RecordAction::Resume => "resume",
                RecordAction::Save => "save",
                RecordAction::Stats => "stats",
            })),
            Command::Play { replay, .. } => Message::with(
                "voltlapse.command.play",
                [replay.clone().unwrap_or_else(|| "latest".to_owned())],
            ),
            Command::Render { action } => Message::new(format!("voltlapse.command.render.{}", match action {
                RenderAction::Start => "start",
                RenderAction::Queue => "queue",
                RenderAction::Cancel => "cancel",
                RenderAction::Status => "status",
            })),
            Command::Config { section, key, value } => match (key, value) {
                (None, _) => Message::with("voltlapse.command.config.show", [section.id()]),
                (Some(key), None) => Message::with("voltlapse.command.config.get", [section.id(), key.clone()]),
                (Some(key), Some(value)) => Message::with("voltlapse.command.config.set", [section.id(), key.clone(), value.clone()]),
            },
            Command::Keyframe { action } => Message::new(match action {
                KeyframeAction::Add | KeyframeAction::AddAt(_) => "voltlapse.command.keyframe.add",
                KeyframeAction::Delete(_) => "voltlapse.command.keyframe.delete",
                KeyframeAction::Clear => "voltlapse.command.keyframe.clear",
                KeyframeAction::List => "voltlapse.command.keyframe.list",
            }),
            Command::Bookmark { action, .. } => Message::new(match action {
                BookmarkAction::Add => "voltlapse.command.bookmark.add",
                BookmarkAction::Remove(_) => "voltlapse.command.bookmark.remove",
                BookmarkAction::List => "voltlapse.command.bookmark.list",
                BookmarkAction::Next => "voltlapse.command.bookmark.next",
                BookmarkAction::Previous => "voltlapse.command.bookmark.previous",
            }),
            Command::List { query } if query.is_empty() => Message::new("voltlapse.command.list"),
            Command::List { query } => Message::with("voltlapse.command.list.query", [query.clone()]),
            Command::Info => Message::new("voltlapse.command.info"),
            Command::Export { name } => Message::with("voltlapse.command.export", [name.clone()]),
            Command::Share { action } => Message::new(format!("voltlapse.command.share.{}", match action {
                ShareAction::Host => "host",
                ShareAction::Join => "join",
                ShareAction::Leave => "leave",
                ShareAction::Status => "status",
            })),
            Command::Help => Message::new("voltlapse.command.help"),
        }
    }
}

/// Error message for a refused command line.
pub fn error_message(error: &CommandError) -> Message {
    match error {
        CommandError::Unknown(verb) => Message::with("voltlapse.command.error.unknown", [verb.clone()]),
        CommandError::MissingArgument(name) => Message::with("voltlapse.command.error.missing", [(*name).to_owned()]),
        CommandError::BadNumber(text) => Message::with("voltlapse.command.error.number", [text.clone()]),
        CommandError::BadTime(text) => Message::with("voltlapse.command.error.time", [text.clone()]),
        CommandError::UnknownSection(section) => Message::with("voltlapse.command.error.section", [section.clone()]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_and_help_lines_show_help() {
        assert_eq!(parse(""), Ok(Command::Help));
        assert_eq!(parse("help"), Ok(Command::Help));
        assert_eq!(parse("?"), Ok(Command::Help));
    }

    #[test]
    fn record_verbs() {
        assert_eq!(parse("record"), Ok(Command::Record { action: RecordAction::Toggle }));
        assert_eq!(parse("record start"), Ok(Command::Record { action: RecordAction::Start }));
        assert_eq!(parse("REC pause"), Ok(Command::Record { action: RecordAction::Pause }));
        assert_eq!(parse("rec stats"), Ok(Command::Record { action: RecordAction::Stats }));
        assert!(matches!(parse("record explode"), Err(CommandError::Unknown(_))));
    }

    #[test]
    fn play_options_in_any_order() {
        assert_eq!(
            parse("play speed 2 at 1:30 name.vrc"),
            Ok(Command::Play { replay: Some("name.vrc".into()), at_seconds: Some(90.0), speed: Some(2.0) })
        );
        assert_eq!(parse("play"), Ok(Command::Play { replay: None, at_seconds: None, speed: None }));
        // Speed is clamped to the transport's range.
        assert_eq!(
            parse("play speed 100"),
            Ok(Command::Play { replay: None, at_seconds: None, speed: Some(16.0) })
        );
        assert!(matches!(parse("play at"), Err(CommandError::MissingArgument("time"))));
    }

    #[test]
    fn times_parse_in_every_shape() {
        assert_eq!(parse_time("12").unwrap(), 12.0);
        assert_eq!(parse_time("1:30").unwrap(), 90.0);
        assert_eq!(parse_time("1:02:03").unwrap(), 3_723.0);
        assert_eq!(parse_time("0:00.5").unwrap(), 0.5);
        assert!(parse_time("1:2:3:4").is_err());
        assert!(parse_time("abc").is_err());
        assert!(parse_time("").is_err());
    }

    #[test]
    fn config_sections_and_values() {
        assert_eq!(
            parse("config rendering width 2560"),
            Ok(Command::Config { section: Section::Rendering, key: Some("width".into()), value: Some("2560".into()) })
        );
        assert_eq!(
            parse("config hud"),
            Ok(Command::Config { section: Section::Hud, key: None, value: None })
        );
        assert_eq!(
            parse("config render"),
            Ok(Command::Config { section: Section::Rendering, key: None, value: None })
        );
        assert!(matches!(parse("config nonsense"), Err(CommandError::UnknownSection(_))));
        assert!(matches!(parse("config"), Err(CommandError::MissingArgument("section"))));
    }

    #[test]
    fn keyframes_and_bookmarks() {
        assert_eq!(parse("keyframe"), Ok(Command::Keyframe { action: KeyframeAction::Add }));
        assert_eq!(parse("kf add 120"), Ok(Command::Keyframe { action: KeyframeAction::AddAt(120) }));
        assert_eq!(parse("keyframe delete 5"), Ok(Command::Keyframe { action: KeyframeAction::Delete(5) }));
        assert!(matches!(parse("keyframe delete"), Err(CommandError::MissingArgument("tick"))));
        assert!(matches!(parse("keyframe delete x"), Err(CommandError::BadNumber(_))));
        assert_eq!(
            parse("bookmark add clutch"),
            Ok(Command::Bookmark { action: BookmarkAction::Add, label: Some("clutch".into()) })
        );
        assert_eq!(parse("bookmark next"), Ok(Command::Bookmark { action: BookmarkAction::Next, label: None }));
    }

    #[test]
    fn other_verbs_and_errors() {
        assert_eq!(parse("list clutch"), Ok(Command::List { query: "clutch".into() }));
        assert_eq!(parse("info"), Ok(Command::Info));
        assert_eq!(parse("export clip"), Ok(Command::Export { name: "clip".into() }));
        assert_eq!(parse("share join"), Ok(Command::Share { action: ShareAction::Join }));
        assert!(matches!(parse("export"), Err(CommandError::MissingArgument("name"))));
        assert_eq!(parse("nope"), Err(CommandError::Unknown("nope".into())));
    }

    #[test]
    fn completion_filters_by_prefix() {
        assert_eq!(complete(None, "re"), vec!["record", "render"]);
        assert_eq!(complete(Some("record"), "s"), vec!["start", "stop", "save", "stats"]);
        assert!(complete(Some("play"), "").is_empty());
        assert_eq!(complete(Some("config"), "hud"), vec!["hud"]);
    }

    #[test]
    fn messages_carry_translation_keys_and_arguments() {
        let command = parse("play clip.vrc").unwrap();
        assert_eq!(command.acknowledgement().render(), "voltlapse.command.play");
        let message = Command::Config { section: Section::Hud, key: Some("crosshair".into()), value: Some("off".into()) }
            .acknowledgement();
        assert_eq!(message.args, vec!["hud", "crosshair", "off"]);
        let error = error_message(&CommandError::MissingArgument("time"));
        assert_eq!(error.render(), "voltlapse.command.error.missing");
        assert_eq!(Message::with("voltlapse.x", ["a", "b"]).render(), "voltlapse.x");
    }

    #[test]
    fn every_verb_has_a_help_line() {
        assert_eq!(VERBS.len(), 11);
        for (verb, help) in VERBS {
            assert!(!help.is_empty());
            assert!(parse(verb).is_ok() || matches!(parse(verb), Err(CommandError::MissingArgument(_))), "{verb}");
        }
    }
}
