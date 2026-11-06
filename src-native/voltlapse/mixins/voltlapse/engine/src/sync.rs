// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! Multiplayer replay sync: watching the same replay with friends.
//!
//! The host owns the transport. Everyone else follows: a member sends its own
//! tick, the host answers with the authoritative state, and a member whose
//! clock drifts past a tolerance seeks. Messages are JSON on the mod's own
//! custom-payload channel, so they travel through the vanilla server untouched.

use crate::json::{self, Json};

/// Tolerance in ticks before a member is corrected.
pub const DRIFT_TOLERANCE_TICKS: u64 = 10;

/// One participant.
#[derive(Clone, Debug, PartialEq)]
pub struct SyncMember {
    pub uuid: String,
    pub name: String,
    /// The tick that member is showing.
    pub tick: u64,
    /// False while the member is still loading the file.
    pub ready: bool,
}

/// What a message does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncKind {
    /// "I am here"; also the answer to a hello.
    Hello,
    /// The host's transport state.
    State,
    /// A member wants to move the transport.
    Seek,
    /// A member added a bookmark everyone should see.
    Bookmark,
    Leave,
}

impl SyncKind {
    pub fn id(self) -> &'static str {
        match self {
            SyncKind::Hello => "hello",
            SyncKind::State => "state",
            SyncKind::Seek => "seek",
            SyncKind::Bookmark => "bookmark",
            SyncKind::Leave => "leave",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "hello" => SyncKind::Hello,
            "state" => SyncKind::State,
            "seek" => SyncKind::Seek,
            "bookmark" => SyncKind::Bookmark,
            "leave" => SyncKind::Leave,
            _ => return None,
        })
    }
}

/// One message on the sync channel.
#[derive(Clone, Debug, PartialEq)]
pub struct SyncMessage {
    pub kind: SyncKind,
    pub sender: String,
    pub tick: u64,
    pub speed: f64,
    pub playing: bool,
    /// Replay file name, so a member can check it has the same recording.
    pub replay: String,
    /// Free-form payload (a bookmark label).
    pub payload: String,
}

impl SyncMessage {
    pub fn hello(sender: impl Into<String>, replay: impl Into<String>) -> Self {
        Self {
            kind: SyncKind::Hello,
            sender: sender.into(),
            tick: 0,
            speed: 1.0,
            playing: false,
            replay: replay.into(),
            payload: String::new(),
        }
    }

    pub fn to_json(&self) -> Json {
        Json::Object(vec![
            ("kind".into(), Json::String(self.kind.id().into())),
            ("sender".into(), Json::String(self.sender.clone())),
            ("tick".into(), Json::int(self.tick as i64)),
            ("speed".into(), Json::Number(self.speed)),
            ("playing".into(), Json::Bool(self.playing)),
            ("replay".into(), Json::String(self.replay.clone())),
            ("payload".into(), Json::String(self.payload.clone())),
        ])
    }

    /// Serialise for the custom payload.
    pub fn encode(&self) -> Vec<u8> {
        self.to_json().to_compact().into_bytes()
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let text = std::str::from_utf8(bytes).ok()?;
        let json = json::parse(text).ok()?;
        Some(Self {
            kind: SyncKind::from_id(json.get("kind").and_then(Json::as_str)?)?,
            sender: json.get("sender").and_then(Json::as_str).unwrap_or("").to_owned(),
            tick: json.get("tick").and_then(Json::as_u64).unwrap_or(0),
            speed: json.get("speed").and_then(Json::as_f64).unwrap_or(1.0).clamp(0.1, 16.0),
            playing: json.get("playing").and_then(Json::as_bool).unwrap_or(false),
            replay: json.get("replay").and_then(Json::as_str).unwrap_or("").to_owned(),
            payload: json.get("payload").and_then(Json::as_str).unwrap_or("").to_owned(),
        })
    }
}

/// The shared session a player hosts or joined.
#[derive(Clone, Debug, PartialEq)]
pub struct SyncSession {
    /// Session id, so a stray message from an old session is ignored.
    pub id: String,
    /// UUID of the host.
    pub host: String,
    /// The replay everyone watches.
    pub replay: String,
    pub tick: u64,
    pub speed: f64,
    pub playing: bool,
    pub members: Vec<SyncMember>,
    /// Messages applied since the session started, for the status line.
    pub applied: u64,
}

impl SyncSession {
    pub fn host(id: impl Into<String>, host: impl Into<String>, replay: impl Into<String>) -> Self {
        let host = host.into();
        Self {
            id: id.into(),
            host: host.clone(),
            replay: replay.into(),
            tick: 0,
            speed: 1.0,
            playing: false,
            members: vec![SyncMember { uuid: host.clone(), name: "host".into(), tick: 0, ready: true }],
            applied: 0,
        }
    }

    pub fn join(&mut self, uuid: impl Into<String>, name: impl Into<String>) {
        let uuid = uuid.into();
        match self.members.iter_mut().find(|member| member.uuid == uuid) {
            Some(member) => {
                member.name = name.into();
                member.ready = true;
            }
            None => self.members.push(SyncMember { uuid, name: name.into(), tick: self.tick, ready: true }),
        }
    }

    pub fn leave(&mut self, uuid: &str) -> bool {
        let before = self.members.len();
        self.members.retain(|member| member.uuid != uuid);
        before != self.members.len()
    }

    pub fn is_host(&self, uuid: &str) -> bool {
        self.host == uuid
    }

    /// The state message the host broadcasts.
    pub fn state_message(&self, sender: &str) -> SyncMessage {
        SyncMessage {
            kind: SyncKind::State,
            sender: sender.to_owned(),
            tick: self.tick,
            speed: self.speed,
            playing: self.playing,
            replay: self.replay.clone(),
            payload: String::new(),
        }
    }

    /// Apply a message. Only the host's `State` moves the transport; a member's
    /// `Seek` is a request the host answers with a new state.
    pub fn apply(&mut self, message: &SyncMessage) -> bool {
        if message.replay != self.replay && message.kind != SyncKind::Hello {
            return false;
        }
        match message.kind {
            SyncKind::Hello => {
                self.join(message.sender.clone(), message.sender.clone());
                self.applied += 1;
                true
            }
            SyncKind::State => {
                if !self.is_host(&message.sender) {
                    return false;
                }
                self.tick = message.tick;
                self.speed = message.speed.clamp(0.1, 16.0);
                self.playing = message.playing;
                self.applied += 1;
                true
            }
            SyncKind::Seek => {
                if let Some(member) = self.members.iter_mut().find(|member| member.uuid == message.sender) {
                    member.tick = message.tick;
                }
                if self.is_host(&message.sender) {
                    self.tick = message.tick;
                    self.applied += 1;
                }
                true
            }
            SyncKind::Bookmark => {
                self.applied += 1;
                true
            }
            SyncKind::Leave => {
                self.leave(&message.sender);
                self.applied += 1;
                true
            }
        }
    }

    /// Whether a member at `local_tick` is far enough behind to be corrected.
    pub fn needs_correction(&self, local_tick: u64) -> bool {
        self.tick.abs_diff(local_tick) > DRIFT_TOLERANCE_TICKS
    }

    /// One line for the HUD: `3 watching · 12:04 · 2.0x`.
    pub fn status_line(&self, tick_rate: u16) -> String {
        format!(
            "{} watching · {} · {:.1}x{}",
            self.members.len(),
            crate::hud::format_duration(self.tick, tick_rate),
            self.speed,
            if self.playing { "" } else { " · paused" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> SyncSession {
        SyncSession::host("session-1", "host-uuid", "clutch.vrc")
    }

    #[test]
    fn a_hosted_session_starts_with_the_host() {
        let session = session();
        assert_eq!(session.members.len(), 1);
        assert!(session.is_host("host-uuid"));
        assert!(!session.is_host("other"));
    }

    #[test]
    fn members_join_and_leave() {
        let mut session = session();
        session.join("alex", "Alex");
        session.join("steve", "Steve");
        session.join("alex", "Alex2");
        assert_eq!(session.members.len(), 3);
        assert_eq!(session.members[1].name, "Alex2");
        assert!(session.leave("steve"));
        assert!(!session.leave("nobody"));
        assert_eq!(session.members.len(), 2);
    }

    #[test]
    fn only_the_host_moves_the_transport() {
        let mut session = session();
        session.join("alex", "Alex");
        let host_state = SyncMessage { kind: SyncKind::State, sender: "host-uuid".into(), tick: 500, speed: 2.0, playing: true, replay: "clutch.vrc".into(), payload: String::new() };
        assert!(session.apply(&host_state));
        assert_eq!(session.tick, 500);
        assert!(session.playing);

        let fake_state = SyncMessage { sender: "alex".into(), tick: 10, ..host_state.clone() };
        assert!(!session.apply(&fake_state), "a member cannot move the transport");
        assert_eq!(session.tick, 500);
    }

    #[test]
    fn a_seek_records_the_members_position() {
        let mut session = session();
        session.join("alex", "Alex");
        let seek = SyncMessage { kind: SyncKind::Seek, sender: "alex".into(), tick: 42, speed: 1.0, playing: false, replay: "clutch.vrc".into(), payload: String::new() };
        assert!(session.apply(&seek));
        assert_eq!(session.members[1].tick, 42);
        assert_eq!(session.tick, 0, "the host's transport did not move");
    }

    #[test]
    fn messages_for_another_replay_are_ignored() {
        let mut session = session();
        let other = SyncMessage { kind: SyncKind::State, sender: "host-uuid".into(), tick: 10, speed: 1.0, playing: true, replay: "other.vrc".into(), payload: String::new() };
        assert!(!session.apply(&other));
        let hello = SyncMessage::hello("alex", "other.vrc");
        assert!(session.apply(&hello), "a hello is always answered");
    }

    #[test]
    fn drift_beyond_the_tolerance_needs_correction() {
        let mut session = session();
        session.tick = 100;
        assert!(!session.needs_correction(105));
        assert!(!session.needs_correction(100 + DRIFT_TOLERANCE_TICKS));
        assert!(session.needs_correction(100 + DRIFT_TOLERANCE_TICKS + 1));
        assert!(session.needs_correction(0));
    }

    #[test]
    fn messages_round_trip_through_the_wire() {
        let message = SyncMessage { kind: SyncKind::Bookmark, sender: "alex".into(), tick: 77, speed: 0.5, playing: true, replay: "a.vrc".into(), payload: "clutch".into() };
        let decoded = SyncMessage::decode(&message.encode()).expect("decodes");
        assert_eq!(decoded, message);
        assert!(SyncMessage::decode(b"not json").is_none());
    }

    #[test]
    fn the_status_line_reads_like_the_hud() {
        let mut session = session();
        session.join("alex", "Alex");
        session.tick = 240;
        session.speed = 2.0;
        assert_eq!(session.status_line(20), "2 watching · 00:12.00 · 2.0x · paused");
    }
}
