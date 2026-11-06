//! What a recording holds, and its on-disk (well, in-`storage`) form.
//!
//! The legacy WebAssembly server API has no filesystem and no packet stream
//! (`TODO(api)`: see README), so a recording is the *server-observable* state a
//! server mod can see, sampled per tick and turned into a compact, versioned text
//! stream. It is plain Rust: no SDK calls, unit-tested on the host.
//!
//! Records: `M` the name, `F`/`P` a frame and its players, `R` a run of frames
//! identical to the one before it (`compression_level`), `C` chat, `B` a block
//! change, `K` a camera keyframe, `S` one block of a keyframe's snapshot.

use std::fmt::Write as _;

pub const FORMAT: &str = "VL1";

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSample {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub game_mode: String,
    pub health: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub tick: u64,
    pub dimension: String,
    pub players: Vec<PlayerSample>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatLine {
    pub tick: u64,
    pub player: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockChange {
    pub tick: u64,
    pub placed: bool,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub state: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe {
    pub tick: u64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// One block of a keyframe's snapshot around the player.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub keyframe: u32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub state: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recording {
    pub name: String,
    pub created_tick: u64,
    pub frames: Vec<Frame>,
    pub chat: Vec<ChatLine>,
    pub blocks: Vec<BlockChange>,
    pub keyframes: Vec<Keyframe>,
    pub cells: Vec<Cell>,
}

/// Space, newline, tab and backslash survive a line-based format.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            ' ' => out.push_str("\\s"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn write_frame(out: &mut String, frame: &Frame) {
    let _ = writeln!(
        out,
        "F {} {} {}",
        frame.tick,
        escape(&frame.dimension),
        frame.players.len()
    );
    for player in &frame.players {
        let _ = writeln!(
            out,
            "P {} {} {} {} {} {}",
            escape(&player.name),
            player.x,
            player.y,
            player.z,
            escape(&player.game_mode),
            player.health
        );
    }
}

/// The shortest run `render_with` collapses, from `compression_level`.
fn min_run_for(level: i64) -> u64 {
    if level <= 0 {
        u64::MAX
    } else {
        (10 - level).clamp(1, 9) as u64
    }
}

impl Recording {
    pub fn new(name: impl Into<String>, created_tick: u64) -> Self {
        Self {
            name: name.into(),
            created_tick,
            ..Self::default()
        }
    }

    /// The tick the recording starts at: the first frame, else the creation tick.
    pub fn start_tick(&self) -> u64 {
        self.frames.first().map_or(self.created_tick, |f| f.tick)
    }

    /// The tick it ends at: the last frame or event.
    pub fn end_tick(&self) -> u64 {
        let frame = self.frames.last().map_or(0, |f| f.tick);
        let chat = self.chat.last().map_or(0, |c| c.tick);
        let block = self.blocks.last().map_or(0, |b| b.tick);
        let key = self.keyframes.last().map_or(0, |k| k.tick);
        frame.max(chat).max(block).max(key).max(self.created_tick)
    }

    pub fn duration_ticks(&self) -> u64 {
        self.end_tick().saturating_sub(self.start_tick())
    }

    /// A one-line description for `/vl list`.
    pub fn summary(&self) -> String {
        format!(
            "{}: {} frames, {} chat, {} block changes, {} keyframes, {} cells, {} ticks",
            self.name,
            self.frames.len(),
            self.chat.len(),
            self.blocks.len(),
            self.keyframes.len(),
            self.cells.len(),
            self.duration_ticks()
        )
    }

    pub fn render(&self) -> String {
        self.render_with(0)
    }

    /// The stream, with runs of identical consecutive frames collapsed into
    /// `R` records when `level` says so: 0 writes every frame, and higher
    /// levels collapse shorter runs (level 9 collapses any repeat).
    pub fn render_with(&self, level: i64) -> String {
        let mut out = String::new();
        out.push_str(FORMAT);
        out.push('\n');
        let _ = writeln!(out, "M {} {}", escape(&self.name), self.created_tick);
        let min_run = min_run_for(level);
        let mut index = 0usize;
        while index < self.frames.len() {
            let head = &self.frames[index];
            let mut run = 1usize;
            while index + run < self.frames.len() {
                let candidate = &self.frames[index + run];
                let repeats = candidate.dimension == head.dimension
                    && candidate.players == head.players
                    && candidate.tick == head.tick + run as u64;
                if !repeats {
                    break;
                }
                run += 1;
            }
            if run > 1 && run as u64 >= min_run {
                write_frame(&mut out, head);
                let _ = writeln!(out, "R {} {}", head.tick + 1, run - 1);
            } else {
                for offset in 0..run {
                    write_frame(&mut out, &self.frames[index + offset]);
                }
            }
            index += run;
        }
        for line in &self.chat {
            let _ = writeln!(
                out,
                "C {} {} {}",
                line.tick,
                escape(&line.player),
                escape(&line.text)
            );
        }
        for block in &self.blocks {
            let _ = writeln!(
                out,
                "B {} {} {} {} {} {}",
                block.tick,
                if block.placed { "p" } else { "b" },
                block.x,
                block.y,
                block.z,
                escape(&block.state)
            );
        }
        for key in &self.keyframes {
            let _ = writeln!(out, "K {} {} {} {}", key.tick, key.x, key.y, key.z);
        }
        for cell in &self.cells {
            let _ = writeln!(
                out,
                "S {} {} {} {} {}",
                cell.keyframe,
                cell.x,
                cell.y,
                cell.z,
                escape(&cell.state)
            );
        }
        out
    }

    /// Parses what [`render`](Self::render) wrote. Every failure names its line.
    pub fn parse(source: &str) -> Result<Self, String> {
        let mut lines = source.lines().enumerate();
        match lines.next() {
            Some((_, first)) if first.trim() == FORMAT => {}
            _ => return Err(format!("not a Volt Lapse recording (want {FORMAT})")),
        }
        let mut recording = Self::default();
        let mut pending_players = 0usize;
        for (index, raw) in lines {
            let number = index + 1;
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            let fields: Vec<&str> = line.split(' ').collect();
            let bad = |what: &str| format!("line {number}: expected {what}");
            match fields[0] {
                "M" => {
                    if fields.len() != 3 {
                        return Err(bad("M <name> <tick>"));
                    }
                    recording.name = unescape(fields[1]);
                    recording.created_tick = fields[2].parse().map_err(|_| bad("a tick"))?;
                }
                "F" => {
                    if fields.len() != 4 {
                        return Err(bad("F <tick> <dimension> <players>"));
                    }
                    let tick = fields[1].parse().map_err(|_| bad("a tick"))?;
                    let dimension = unescape(fields[2]);
                    pending_players = fields[3].parse().map_err(|_| bad("a player count"))?;
                    recording.frames.push(Frame {
                        tick,
                        dimension,
                        players: Vec::new(),
                    });
                }
                "P" => {
                    if fields.len() != 7 || pending_players == 0 {
                        return Err(bad("P <name> <x> <y> <z> <mode> <health> after F"));
                    }
                    let player = PlayerSample {
                        name: unescape(fields[1]),
                        x: fields[2].parse().map_err(|_| bad("an x"))?,
                        y: fields[3].parse().map_err(|_| bad("a y"))?,
                        z: fields[4].parse().map_err(|_| bad("a z"))?,
                        game_mode: unescape(fields[5]),
                        health: fields[6].parse().map_err(|_| bad("a health"))?,
                    };
                    pending_players -= 1;
                    recording
                        .frames
                        .last_mut()
                        .expect("a frame precedes its players")
                        .players
                        .push(player);
                }
                "R" => {
                    if fields.len() != 3 {
                        return Err(bad("R <first tick> <count>"));
                    }
                    let first: u64 = fields[1].parse().map_err(|_| bad("a tick"))?;
                    let count: u64 = fields[2].parse().map_err(|_| bad("a count"))?;
                    let last = recording
                        .frames
                        .last()
                        .cloned()
                        .ok_or_else(|| bad("R only after an F"))?;
                    for step in 0..count {
                        recording.frames.push(Frame {
                            tick: first + step,
                            ..last.clone()
                        });
                    }
                }
                "C" => {
                    if fields.len() != 4 {
                        return Err(bad("C <tick> <player> <text>"));
                    }
                    recording.chat.push(ChatLine {
                        tick: fields[1].parse().map_err(|_| bad("a tick"))?,
                        player: unescape(fields[2]),
                        text: unescape(fields[3]),
                    });
                }
                "B" => {
                    if fields.len() != 7 {
                        return Err(bad("B <tick> <p|b> <x> <y> <z> <state>"));
                    }
                    recording.blocks.push(BlockChange {
                        tick: fields[1].parse().map_err(|_| bad("a tick"))?,
                        placed: match fields[2] {
                            "p" => true,
                            "b" => false,
                            _ => return Err(bad("p or b")),
                        },
                        x: fields[3].parse().map_err(|_| bad("an x"))?,
                        y: fields[4].parse().map_err(|_| bad("a y"))?,
                        z: fields[5].parse().map_err(|_| bad("a z"))?,
                        state: unescape(fields[6]),
                    });
                }
                "K" => {
                    if fields.len() != 5 {
                        return Err(bad("K <tick> <x> <y> <z>"));
                    }
                    recording.keyframes.push(Keyframe {
                        tick: fields[1].parse().map_err(|_| bad("a tick"))?,
                        x: fields[2].parse().map_err(|_| bad("an x"))?,
                        y: fields[3].parse().map_err(|_| bad("a y"))?,
                        z: fields[4].parse().map_err(|_| bad("a z"))?,
                    });
                }
                "S" => {
                    if fields.len() != 6 {
                        return Err(bad("S <keyframe> <x> <y> <z> <state>"));
                    }
                    recording.cells.push(Cell {
                        keyframe: fields[1].parse().map_err(|_| bad("a keyframe"))?,
                        x: fields[2].parse().map_err(|_| bad("an x"))?,
                        y: fields[3].parse().map_err(|_| bad("a y"))?,
                        z: fields[4].parse().map_err(|_| bad("a z"))?,
                        state: unescape(fields[5]),
                    });
                }
                other => return Err(format!("line {number}: unknown record {other:?}")),
            }
        }
        if pending_players > 0 {
            return Err(format!("{pending_players} player line(s) missing after F"));
        }
        Ok(recording)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Recording {
        let mut recording = Recording::new("world name with spaces", 42);
        recording.frames.push(Frame {
            tick: 100,
            dimension: "minecraft:the_nether".into(),
            players: vec![
                PlayerSample {
                    name: "Alex".into(),
                    x: 1.5,
                    y: 64.0,
                    z: -3.25,
                    game_mode: "survival".into(),
                    health: 19.5,
                },
                PlayerSample {
                    name: "Steve".into(),
                    x: 0.0,
                    y: 70.0,
                    z: 0.0,
                    game_mode: "creative".into(),
                    health: 20.0,
                },
            ],
        });
        recording.chat.push(ChatLine {
            tick: 110,
            player: "Alex".into(),
            text: "hello there  **two  spaces**".into(),
        });
        recording.blocks.push(BlockChange {
            tick: 120,
            placed: true,
            x: 1,
            y: 64,
            z: 2,
            state: "minecraft:oak_log[axis=y]".into(),
        });
        recording.blocks.push(BlockChange {
            tick: 130,
            placed: false,
            x: 1,
            y: 64,
            z: 2,
            state: "minecraft:air".into(),
        });
        recording.keyframes.push(Keyframe {
            tick: 140,
            x: 1.5,
            y: 64.0,
            z: -3.25,
        });
        recording.cells.push(Cell {
            keyframe: 0,
            x: 2,
            y: 64,
            z: 2,
            state: "minecraft:stone".into(),
        });
        recording
    }

    #[test]
    fn everything_round_trips_including_spaces() {
        let recording = sample();
        let text = recording.render();
        let parsed = Recording::parse(&text).unwrap();
        assert_eq!(recording, parsed);
        assert_eq!(
            parsed.chat[0].text,
            "hello there  **two  spaces**",
            "spaces and repeats survive"
        );
        assert_eq!(parsed.frames[0].players.len(), 2);
    }

    #[test]
    fn a_recording_reports_its_span() {
        let recording = sample();
        assert_eq!(recording.start_tick(), 100);
        assert_eq!(recording.end_tick(), 140);
        assert_eq!(recording.duration_ticks(), 40);
        assert!(recording.summary().contains("2 block changes"));
    }

    #[test]
    fn malformed_input_is_refused_with_a_line() {
        let cases = [
            ("", "not a Volt Lapse recording"),
            ("VL1\nX 1\n", "unknown record"),
            ("VL1\nF 1 overworld\n", "F <tick>"),
            ("VL1\nF 1 overworld 2\n", "player line(s) missing"),
            ("VL1\nB 1 q 0 0 0 stone\n", "p or b"),
            ("VL1\nP 1 0 0 0 survival 20\n", "after F"),
        ];
        for (source, expected) in cases {
            let error = Recording::parse(source).unwrap_err();
            assert!(error.contains(expected), "{source:?}: {error}");
        }
    }

    #[test]
    fn compression_collapses_runs_of_identical_frames() {
        let mut recording = Recording::new("runs", 0);
        for tick in 10..=19 {
            recording.frames.push(Frame {
                tick,
                dimension: "minecraft:overworld".into(),
                players: vec![PlayerSample {
                    name: "Alex".into(),
                    x: 0.5,
                    y: 64.0,
                    z: 0.5,
                    game_mode: "survival".into(),
                    health: 20.0,
                }],
            });
        }
        let plain = recording.render_with(0);
        assert!(!plain.contains("\nR "), "level 0 writes every frame");
        let packed = recording.render_with(9);
        assert!(packed.len() < plain.len(), "{packed}");
        assert!(packed.contains("\nR 11 9\n"), "{packed}");
        assert_eq!(Recording::parse(&packed).unwrap(), recording);
        assert_eq!(Recording::parse(&plain).unwrap(), recording);
        // A frame that differs is not folded into the run.
        let mut moved = recording.clone();
        moved.frames[5].players[0].x = 4.0;
        let text = moved.render_with(9);
        assert_eq!(Recording::parse(&text).unwrap(), moved);
        assert!(text.matches("\nR ").count() == 2, "{text}");
    }

    #[test]
    fn an_empty_recording_is_valid() {
        let text = Recording::new("empty", 7).render();
        let parsed = Recording::parse(&text).unwrap();
        assert_eq!(parsed.name, "empty");
        assert_eq!(parsed.start_tick(), 7);
        assert_eq!(parsed.end_tick(), 7);
        assert_eq!(parsed.duration_ticks(), 0);
    }
}
