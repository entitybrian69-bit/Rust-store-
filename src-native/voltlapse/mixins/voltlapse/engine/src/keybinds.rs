// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! Rebindable keys.
//!
//! Vanilla key names are used throughout (`key.keyboard.f6`, `key.mouse.middle`),
//! so a binding is stored exactly as the game's own options store one and the
//! options screen can show the right label for it. Every action has a
//! localisation key (`voltlapse.keybind.<action>`), which is the only
//! user-facing string this module produces.

use crate::json::Json;

/// Every action Volt Lapse binds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    StartStopRecording,
    PauseResumeRecording,
    OpenViewer,
    OpenCameraEditor,
    ToggleFreeCam,
    AddKeyframe,
    Screenshot,
    ToggleHud,
    SpeedUp,
    SpeedDown,
    PlayPause,
    SeekForward,
    SeekBackward,
    ToggleLoop,
    RenderLastPath,
}

impl Action {
    pub const ALL: [Action; 15] = [
        Action::StartStopRecording,
        Action::PauseResumeRecording,
        Action::OpenViewer,
        Action::OpenCameraEditor,
        Action::ToggleFreeCam,
        Action::AddKeyframe,
        Action::Screenshot,
        Action::ToggleHud,
        Action::SpeedUp,
        Action::SpeedDown,
        Action::PlayPause,
        Action::SeekForward,
        Action::SeekBackward,
        Action::ToggleLoop,
        Action::RenderLastPath,
    ];

    /// The id used in config files and in the localisation key.
    pub fn id(self) -> &'static str {
        match self {
            Action::StartStopRecording => "start_stop_recording",
            Action::PauseResumeRecording => "pause_resume_recording",
            Action::OpenViewer => "open_viewer",
            Action::OpenCameraEditor => "open_camera_editor",
            Action::ToggleFreeCam => "toggle_free_cam",
            Action::AddKeyframe => "add_keyframe",
            Action::Screenshot => "screenshot",
            Action::ToggleHud => "toggle_hud",
            Action::SpeedUp => "speed_up",
            Action::SpeedDown => "speed_down",
            Action::PlayPause => "play_pause",
            Action::SeekForward => "seek_forward",
            Action::SeekBackward => "seek_backward",
            Action::ToggleLoop => "toggle_loop",
            Action::RenderLastPath => "render_last_path",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.id() == id)
    }

    /// The translation key for the action's name.
    pub fn translation_key(self) -> String {
        format!("voltlapse.keybind.{}", self.id())
    }

    /// The default binding, as a vanilla key name; `None` means unbound.
    pub fn default_binding(self) -> Option<&'static str> {
        Some(match self {
            Action::StartStopRecording => "key.keyboard.f6",
            Action::PauseResumeRecording => "key.keyboard.f7",
            Action::OpenViewer => "key.keyboard.f8",
            Action::OpenCameraEditor => "key.keyboard.f9",
            Action::ToggleFreeCam => "key.keyboard.f10",
            Action::AddKeyframe => "key.keyboard.k",
            Action::Screenshot => "key.keyboard.f2",
            Action::ToggleHud => "key.keyboard.f1",
            Action::SpeedUp => "key.keyboard.equal",
            Action::SpeedDown => "key.keyboard.minus",
            Action::PlayPause => "key.keyboard.space",
            Action::SeekForward => "key.keyboard.right",
            Action::SeekBackward => "key.keyboard.left",
            Action::ToggleLoop => "key.keyboard.l",
            Action::RenderLastPath => "key.keyboard.f11",
        })
    }
}

/// One key press the engine sees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyEvent {
    /// Vanilla key name, e.g. `key.keyboard.f6`.
    pub key: &'static str,
    pub pressed: bool,
    /// GLFW modifier bits: shift 1, control 2, alt 4, super 8.
    pub modifiers: u8,
    /// True while a key is held.
    pub repeat: bool,
}

/// A set of bindings, one per action.
#[derive(Clone, Debug, PartialEq)]
pub struct Keybinds {
    bindings: Vec<(Action, Option<String>)>,
}

impl Default for Keybinds {
    fn default() -> Self {
        Self {
            bindings: Action::ALL
                .into_iter()
                .map(|action| (action, action.default_binding().map(str::to_owned)))
                .collect(),
        }
    }
}

impl Keybinds {
    /// The key bound to an action.
    pub fn binding(&self, action: Action) -> Option<&str> {
        self.bindings
            .iter()
            .find(|(bound, _)| *bound == action)
            .and_then(|(_, key)| key.as_deref())
    }

    /// Bind an action, replacing the previous key.
    pub fn bind(&mut self, action: Action, key: Option<&str>) {
        let key = key.map(str::to_owned);
        match self.bindings.iter_mut().find(|(bound, _)| *bound == action) {
            Some(slot) => slot.1 = key,
            None => self.bindings.push((action, key)),
        }
    }

    /// Which actions a key triggers.
    pub fn actions_for(&self, key: &str) -> Vec<Action> {
        self.bindings
            .iter()
            .filter(|(_, bound)| bound.as_deref() == Some(key))
            .map(|(action, _)| *action)
            .collect()
    }

    /// Two actions bound to one key; the settings screen warns about these.
    pub fn conflicts(&self) -> Vec<(String, Vec<Action>)> {
        let mut keys: Vec<String> = Vec::new();
        for (_, key) in &self.bindings {
            if let Some(key) = key {
                if !keys.contains(key) {
                    keys.push(key.clone());
                }
            }
        }
        keys.into_iter()
            .filter_map(|key| {
                let actions = self.actions_for(&key);
                (actions.len() > 1).then_some((key, actions))
            })
            .collect()
    }

    /// The actions a press triggers, in the order they should run. A repeat
    /// never fires a one-shot action twice.
    pub fn pressed(&self, event: KeyEvent) -> Vec<Action> {
        if !event.pressed {
            return Vec::new();
        }
        let mut actions = self.actions_for(event.key);
        if event.repeat {
            actions.retain(|action| matches!(action, Action::SeekForward | Action::SeekBackward));
        }
        actions
    }

    /// Restore one action's default.
    pub fn reset(&mut self, action: Action) {
        self.bind(action, action.default_binding());
    }

    /// Restore every default.
    pub fn reset_all(&mut self) {
        *self = Keybinds::default();
    }

    /// Whether a key name is a vanilla key or mouse name.
    pub fn is_valid_key(key: &str) -> bool {
        (key.starts_with("key.keyboard.") && key.len() > "key.keyboard.".len())
            || (key.starts_with("key.mouse.") && key.len() > "key.mouse.".len())
    }

    pub fn to_json(&self) -> Json {
        let mut entries = Vec::new();
        for (action, key) in &self.bindings {
            entries.push((
                action.id().to_owned(),
                key.as_ref().map(|key| Json::String(key.clone())).unwrap_or(Json::Null),
            ));
        }
        Json::Object(entries)
    }

    /// Read bindings, keeping defaults for anything missing or invalid.
    pub fn from_json(json: &Json) -> Self {
        let mut keybinds = Keybinds::default();
        if let Some(entries) = json.as_object() {
            for (id, value) in entries {
                let Some(action) = Action::from_id(id) else {
                    continue;
                };
                match value {
                    Json::Null => keybinds.bind(action, None),
                    Json::String(key) if Keybinds::is_valid_key(key) => keybinds.bind(action, Some(key)),
                    // A hand-edited typo keeps the default instead of unbinding.
                    _ => {}
                }
            }
        }
        keybinds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: &'static str) -> KeyEvent {
        KeyEvent { key, pressed: true, modifiers: 0, repeat: false }
    }

    #[test]
    fn defaults_are_unique_and_valid() {
        let keybinds = Keybinds::default();
        assert_eq!(keybinds.conflicts(), Vec::new(), "defaults must not collide");
        for action in Action::ALL {
            let key = keybinds.binding(action).expect("bound by default");
            assert!(Keybinds::is_valid_key(key), "{key} is a vanilla key name");
        }
        assert_eq!(keybinds.binding(Action::StartStopRecording), Some("key.keyboard.f6"));
    }

    #[test]
    fn pressing_a_bound_key_returns_its_actions() {
        let keybinds = Keybinds::default();
        assert_eq!(keybinds.pressed(press("key.keyboard.f6")), vec![Action::StartStopRecording]);
        assert_eq!(keybinds.pressed(KeyEvent { pressed: false, ..press("key.keyboard.f6") }), Vec::<Action>::new());
        assert_eq!(keybinds.pressed(press("key.keyboard.z")), Vec::<Action>::new());
    }

    #[test]
    fn repeats_only_fire_continuous_actions() {
        let keybinds = Keybinds::default();
        let repeat = KeyEvent { repeat: true, ..press("key.keyboard.f6") };
        assert!(keybinds.pressed(repeat).is_empty());
        let seek = KeyEvent { repeat: true, ..press("key.keyboard.right") };
        assert_eq!(keybinds.pressed(seek), vec![Action::SeekForward]);
    }

    #[test]
    fn rebinding_detects_conflicts_and_can_reset() {
        let mut keybinds = Keybinds::default();
        keybinds.bind(Action::Screenshot, Some("key.keyboard.f6"));
        let conflicts = keybinds.conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].0, "key.keyboard.f6");
        assert_eq!(conflicts[0].1, vec![Action::StartStopRecording, Action::Screenshot]);
        keybinds.reset(Action::Screenshot);
        assert!(keybinds.conflicts().is_empty());
        keybinds.bind(Action::Screenshot, None);
        assert_eq!(keybinds.binding(Action::Screenshot), None);
        keybinds.reset_all();
        assert_eq!(keybinds.binding(Action::Screenshot), Some("key.keyboard.f2"));
    }

    #[test]
    fn json_round_trips_and_ignores_bad_keys() {
        let mut keybinds = Keybinds::default();
        keybinds.bind(Action::StartStopRecording, Some("key.mouse.middle"));
        keybinds.bind(Action::PlayPause, None);
        let back = Keybinds::from_json(&keybinds.to_json());
        assert_eq!(back, keybinds);

        let malformed = crate::json::parse(r#"{"screenshot":"keyboard.f2","start_stop_recording":"key.mouse.middle","nonsense":"key.keyboard.q"}"#).unwrap();
        let repaired = Keybinds::from_json(&malformed);
        assert_eq!(repaired.binding(Action::Screenshot), Some("key.keyboard.f2"), "typo keeps the default");
        assert_eq!(repaired.binding(Action::StartStopRecording), Some("key.mouse.middle"));
    }

    #[test]
    fn every_action_has_a_translation_key() {
        for action in Action::ALL {
            assert_eq!(action.translation_key(), format!("voltlapse.keybind.{}", action.id()));
            assert_eq!(Action::from_id(action.id()), Some(action));
        }
        assert_eq!(Action::from_id("nope"), None);
    }
}
