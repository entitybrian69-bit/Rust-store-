// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! The mod's own translation table.
//!
//! Horizon resolves the game's own keys; a mod's screens are drawn by the mod,
//! so it carries the strings itself. Every label the overlay and the chat
//! messages use goes through [`Catalog`], which reads the language file shipped
//! in `assets/voltlapse/lang/<locale>.json` and falls back to English, then to
//! the key, so a missing translation shows English instead of nothing.
//!
//! The key list is [`DEFAULTS`]; a language file may hold a subset (the rest
//! fall back) and extra keys (ignored). `ui.*` labels are drawn on the screens,
//! `toast.*` are the short messages, `cmd.*` the command replies.

use crate::json::{self, Json};

/// `(key, English)` for every string the mod draws itself.
pub const DEFAULTS: [(&str, &str); 65] = [
    ("ui.title", "Volt Lapse"),
    ("ui.close", "Close"),
    ("ui.replays", "Replays"),
    ("ui.camera", "Camera"),
    ("ui.render", "Render"),
    ("ui.settings", "Settings"),
    ("ui.help", "Help"),
    ("ui.tab.recording", "Recording"),
    ("ui.tab.playback", "Playback"),
    ("ui.tab.rendering", "Rendering"),
    ("ui.tab.camera", "Camera"),
    ("ui.tab.hud", "HUD"),
    ("ui.tab.storage", "Storage"),
    ("ui.replay.empty", "No replays yet: press F6, or run /voltlapse record start."),
    ("ui.replay.play", "Play"),
    ("ui.replay.export", "Export"),
    ("ui.replay.delete", "Delete"),
    ("ui.replay.refresh", "Refresh"),
    ("ui.replay.unreadable", "unreadable"),
    ("ui.camera.empty", "No camera path yet: press F11 to add a keyframe."),
    ("ui.camera.add", "Add"),
    ("ui.camera.copy", "Copy"),
    ("ui.camera.delete", "Delete"),
    ("ui.camera.clipboard", "Clipboard"),
    ("ui.camera.paste", "Paste"),
    ("ui.camera.snap", "Snap"),
    ("ui.camera.hint", "Click a marker, then the timeline, to move it."),
    ("ui.render.start", "Render"),
    ("ui.render.cancel", "Cancel"),
    ("ui.render.resolution", "Resolution"),
    ("ui.render.fps", "Frame rate"),
    ("ui.render.format", "Format"),
    ("ui.render.aa", "Anti-aliasing"),
    ("ui.render.motion_blur", "Motion blur"),
    ("ui.render.dof", "Depth of field"),
    ("ui.render.tilt_shift", "Tilt-shift"),
    ("ui.render.vignette", "Vignette"),
    ("ui.render.lut", "LUT"),
    ("ui.render.alpha", "Alpha"),
    ("ui.render.multi_pass", "Multi-pass"),
    ("ui.render.projection", "Projection"),
    ("ui.render.preview", "preview"),
    ("ui.settings.reset", "Reset section"),
    ("ui.settings.save", "Save"),
    ("ui.help.done", "Got it"),
    ("ui.status.replays", "replays"),
    ("ui.status.keyframes", "keyframes"),
    ("ui.status.no_render", "No render running"),
    ("hud.rec", "REC"),
    ("hud.paused", "PAUSED"),
    ("hud.loop", "loop"),
    ("toast.recording", "Recording to {0}"),
    ("toast.saved", "Saved {0}"),
    ("toast.recording_paused", "Recording paused"),
    ("toast.recording_resumed", "Recording resumed"),
    ("toast.keyframe_added", "Keyframe added"),
    ("toast.replay_deleted", "Replay deleted"),
    ("toast.settings_saved", "Settings saved"),
    ("toast.render_done", "Rendered {0}"),
    ("toast.render_cancelled", "Render cancelled"),
    ("toast.recovered", "A crashed recording was recovered"),
    ("toast.ffmpeg_missing", "ffmpeg not found; frames kept in {0}"),
    ("cmd.unknown", "unknown subcommand '{0}'"),
    ("cmd.usage", "usage: /voltlapse <record|play|render|config|keyframe|bookmark|list|info|export|share|help>"),
    ("cmd.no_replay", "No replay called {0}"),
];

/// A loaded language file: the English defaults with a locale's overrides.
#[derive(Clone, Debug, PartialEq)]
pub struct Catalog {
    pub locale: String,
    entries: Vec<(String, String)>,
}

impl Default for Catalog {
    fn default() -> Self {
        Self::english()
    }
}

impl Catalog {
    /// English only: every key resolves to the text in [`DEFAULTS`].
    pub fn english() -> Self {
        Self {
            locale: "en_us".to_owned(),
            entries: DEFAULTS.iter().map(|(key, text)| ((*key).to_owned(), (*text).to_owned())).collect(),
        }
    }

    /// Load a language file (`assets/voltlapse/lang/<locale>.json`).
    ///
    /// The file is a flat object of `key: text`. Unknown keys are dropped and a
    /// missing key keeps its English text, so a partial translation is usable.
    pub fn parse(locale: &str, text: &str) -> Result<Self, json::JsonError> {
        let json = json::parse(text)?;
        Ok(Self::from_json(locale, &json))
    }

    pub fn from_json(locale: &str, json: &Json) -> Self {
        let mut catalog = Self::english();
        catalog.locale = locale.to_owned();
        if let Some(fields) = json.as_object() {
            for (key, value) in fields {
                if let Some(text) = value.as_str() {
                    catalog.set(key, text);
                }
            }
        }
        catalog
    }

    /// Replace one string.
    pub fn set(&mut self, key: &str, text: &str) {
        match self.entries.iter_mut().find(|(existing, _)| existing == key) {
            Some((_, slot)) => *slot = text.to_owned(),
            None => self.entries.push((key.to_owned(), text.to_owned())),
        }
    }

    /// The text for a key: the locale's, else English, else the key itself.
    pub fn text(&self, key: &str) -> &str {
        match self.entries.iter().find(|(existing, _)| existing == key) {
            Some((_, text)) => text,
            None => DEFAULTS
                .iter()
                .find(|(existing, _)| *existing == key)
                .map(|(_, text)| *text)
                .unwrap_or(key),
        }
    }

    /// The text with `{0}`, `{1}`… replaced, in order.
    pub fn format(&self, key: &str, args: &[&str]) -> String {
        let mut text = self.text(key).to_owned();
        for (index, arg) in args.iter().enumerate() {
            text = text.replace(&format!("{{{index}}}"), arg);
        }
        text
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// How many keys this catalog knows (translations included).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The locale a language code selects, with Horizon's fallback rules.
pub fn locale_for(code: &str) -> &'static str {
    match code {
        "de_de" | "de" => "de_de",
        "ne_np" | "ne" => "ne_np",
        "es_es" | "es" => "es_es",
        _ => "en_us",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_holds_every_key() {
        let catalog = Catalog::english();
        for (key, text) in DEFAULTS {
            assert_eq!(catalog.text(key), text, "{key}");
        }
        assert_eq!(catalog.len(), DEFAULTS.len());
        assert_eq!(catalog.locale(), "en_us");
    }

    #[test]
    fn a_translation_overrides_and_a_missing_key_falls_back() {
        let text = r#"{"ui.title": "Volt Lapse (Deutsch)", "ui.replay.play": "Abspielen", "unknown.key": "x"}"#;
        let catalog = Catalog::parse("de_de", text).expect("parses");
        assert_eq!(catalog.text("ui.title"), "Volt Lapse (Deutsch)");
        assert_eq!(catalog.text("ui.replay.play"), "Abspielen");
        assert_eq!(catalog.text("ui.camera.add"), "Add", "untranslated keys stay English");
        assert_eq!(catalog.text("not.a.key"), "not.a.key", "an unknown key is its own name");
        assert_eq!(catalog.locale(), "de_de");
    }

    #[test]
    fn placeholders_are_filled_in_order() {
        let catalog = Catalog::english();
        assert_eq!(catalog.format("toast.recording", &["Replay 1.vrc"]), "Recording to Replay 1.vrc");
        assert_eq!(catalog.format("toast.saved", &["a.vrc"]), "Saved a.vrc");
        assert_eq!(catalog.format("ui.title", &[]), "Volt Lapse");
    }

    #[test]
    fn a_broken_language_file_is_an_error_not_a_panic() {
        assert!(Catalog::parse("de_de", "{not json").is_err());
        assert!(Catalog::parse("de_de", "[]").is_ok(), "an array simply translates nothing");
    }

    #[test]
    fn locale_codes_map_to_the_shipped_files() {
        assert_eq!(locale_for("de_de"), "de_de");
        assert_eq!(locale_for("de"), "de_de");
        assert_eq!(locale_for("ne_np"), "ne_np");
        assert_eq!(locale_for("fr_fr"), "en_us");
        assert_eq!(locale_for(""), "en_us");
    }

    #[test]
    fn the_shipped_language_files_hold_the_default_keys() {
        // Shipping a language file missing a key is a translation bug, not a
        // crash: the English text shows. The test keeps them complete anyway.
        for (locale, text) in [
            ("en_us", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../lang/en_us.json"))),
            ("de_de", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../lang/de_de.json"))),
            ("ne_np", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../lang/ne_np.json"))),
        ] {
            let catalog = Catalog::parse(locale, text).expect("the language file parses");
            for (key, _) in DEFAULTS {
                assert!(
                    catalog.text(key) != *key,
                    "{locale} is missing {key}"
                );
            }
        }
    }
}
