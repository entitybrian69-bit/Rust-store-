// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! HUD and overlay composition.
//!
//! Three things share this module: the in-replay HUD (health, hunger, crosshair,
//! debug text, name tags — each toggleable), the cinematic overlays a creator
//! adds to a render (title cards, lower thirds, a watermark, a stopwatch), and
//! the fade in/out envelope that makes either appear smoothly. Layout is pure
//! arithmetic on a screen rectangle, so the same overlays draw identically on a
//! phone screen and in an 8K render.

use crate::json::Json;

/// A corner or edge of the screen an overlay hangs from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Anchor {
    pub fn id(self) -> &'static str {
        match self {
            Anchor::TopLeft => "top_left",
            Anchor::TopCenter => "top_center",
            Anchor::TopRight => "top_right",
            Anchor::CenterLeft => "center_left",
            Anchor::Center => "center",
            Anchor::CenterRight => "center_right",
            Anchor::BottomLeft => "bottom_left",
            Anchor::BottomCenter => "bottom_center",
            Anchor::BottomRight => "bottom_right",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "top_left" => Anchor::TopLeft,
            "top_center" | "top" => Anchor::TopCenter,
            "top_right" => Anchor::TopRight,
            "center_left" => Anchor::CenterLeft,
            "center" => Anchor::Center,
            "center_right" => Anchor::CenterRight,
            "bottom_left" => Anchor::BottomLeft,
            "bottom_center" | "bottom" => Anchor::BottomCenter,
            "bottom_right" => Anchor::BottomRight,
            _ => return None,
        })
    }

    /// The fraction of free space on each axis: 0 (start), 0.5 (middle), 1 (end).
    pub fn factors(self) -> (f32, f32) {
        let x = match self {
            Anchor::TopLeft | Anchor::CenterLeft | Anchor::BottomLeft => 0.0,
            Anchor::TopCenter | Anchor::Center | Anchor::BottomCenter => 0.5,
            Anchor::TopRight | Anchor::CenterRight | Anchor::BottomRight => 1.0,
        };
        let y = match self {
            Anchor::TopLeft | Anchor::TopCenter | Anchor::TopRight => 0.0,
            Anchor::CenterLeft | Anchor::Center | Anchor::CenterRight => 0.5,
            Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => 1.0,
        };
        (x, y)
    }
}

/// A rectangle in GUI pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    /// Place a `width`×`height` box in a screen, anchored with a pixel offset.
    pub fn anchored(anchor: Anchor, offset: (f32, f32), screen: (f32, f32), size: (f32, f32)) -> Rect {
        let (fx, fy) = anchor.factors();
        Rect {
            x: (screen.0 - size.0) * fx + offset.0,
            y: (screen.1 - size.1) * fy + offset.1,
            width: size.0,
            height: size.1,
        }
    }

    /// Shrink to a centred sub-rect for letterboxing: bars top and bottom.
    pub fn letterboxed(&self, aspect: f32) -> (Rect, f32) {
        let current = if self.height > 0.0 { self.width / self.height } else { aspect };
        if aspect <= 0.0 || current <= 0.0 {
            return (*self, 0.0);
        }
        if aspect > current {
            // Wider than the screen: bars above and below.
            let height = self.width / aspect;
            let bar = (self.height - height) * 0.5;
            (Rect::new(self.x, self.y + bar, self.width, height), bar)
        } else {
            // Taller: bars left and right (a vertical video from a wide render).
            let width = self.height * aspect;
            let bar = (self.width - width) * 0.5;
            (Rect::new(self.x + bar, self.y, width, self.height), bar)
        }
    }
}

/// A keyframe on an overlay's own little timeline: opacity, scale and offset
/// can animate independently of the camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayKeyframe {
    pub tick: u64,
    pub opacity: f32,
    pub scale: f32,
    pub offset: (f32, f32),
}

/// A text card: title, lower third, subtitle, stopwatch label.
#[derive(Clone, Debug, PartialEq)]
pub struct TextCard {
    pub id: String,
    pub text: String,
    pub anchor: Anchor,
    pub offset: (f32, f32),
    /// Text scale, 1 is vanilla's font.
    pub scale: f32,
    /// ARGB colour.
    pub color: u32,
    pub shadow: bool,
    /// Ticks over which the card fades in at `start` and out at `end`.
    pub fade_in: u64,
    pub fade_out: u64,
    pub start: u64,
    pub end: Option<u64>,
    /// Animation keyframes; empty means "constant opacity 1".
    pub keyframes: Vec<OverlayKeyframe>,
}

impl Default for TextCard {
    fn default() -> Self {
        Self {
            id: String::new(),
            text: String::new(),
            anchor: Anchor::BottomCenter,
            offset: (0.0, -40.0),
            scale: 1.0,
            color: 0xFFFF_FFFF,
            shadow: true,
            fade_in: 10,
            fade_out: 10,
            start: 0,
            end: None,
            keyframes: Vec::new(),
        }
    }
}

/// A card with its envelope applied, ready to draw.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedCard {
    pub id: String,
    pub text: String,
    pub rect: Rect,
    pub scale: f32,
    pub color: u32,
    pub shadow: bool,
    /// 0..=1, multiplied into the colour's alpha when drawing.
    pub opacity: f32,
}

impl TextCard {
    /// Opacity at a tick, from the fade envelope and any keyframes.
    pub fn opacity_at(&self, tick: u64) -> f32 {
        let end = self.end.unwrap_or(u64::MAX);
        if tick < self.start || tick > end {
            return 0.0;
        }
        let mut opacity = 1.0f32;
        if self.fade_in > 0 {
            let elapsed = tick - self.start;
            if elapsed < self.fade_in {
                opacity *= elapsed as f32 / self.fade_in as f32;
            }
        }
        if let Some(end) = self.end {
            if self.fade_out > 0 && tick + self.fade_out > end {
                let remaining = end.saturating_sub(tick) as f32 / self.fade_out as f32;
                opacity *= remaining.clamp(0.0, 1.0);
            }
        }
        if !self.keyframes.is_empty() {
            opacity *= sample_keyframes(&self.keyframes, tick).0;
        }
        opacity.clamp(0.0, 1.0)
    }

    /// Resolve to a drawable box, or `None` when it is fully transparent.
    pub fn resolve(&self, tick: u64, screen: (f32, f32), char_width: f32, line_height: f32) -> Option<ResolvedCard> {
        let opacity = self.opacity_at(tick);
        if opacity <= 0.001 {
            return None;
        }
        let keyframed = if self.keyframes.is_empty() { None } else { Some(sample_keyframes(&self.keyframes, tick)) };
        let scale = self.scale * keyframed.map(|k| k.1).unwrap_or(1.0);
        let offset = match keyframed {
            Some((_, _, offset)) => (self.offset.0 + offset.0, self.offset.1 + offset.1),
            None => self.offset,
        };
        // Width from the longest line, so multi-line titles centre correctly.
        let lines: Vec<&str> = self.text.lines().collect();
        let width = lines.iter().map(|line| line.chars().count()).max().unwrap_or(0) as f32
            * char_width
            * scale;
        let height = lines.len().max(1) as f32 * line_height * scale;
        Some(ResolvedCard {
            id: self.id.clone(),
            text: self.text.clone(),
            rect: Rect::anchored(self.anchor, offset, screen, (width, height)),
            scale,
            color: self.color,
            shadow: self.shadow,
            opacity,
        })
    }

    pub fn to_json(&self) -> Json {
        Json::Object(vec![
            ("id".into(), Json::String(self.id.clone())),
            ("text".into(), Json::String(self.text.clone())),
            ("anchor".into(), Json::String(self.anchor.id().into())),
            ("offset_x".into(), Json::Number(self.offset.0 as f64)),
            ("offset_y".into(), Json::Number(self.offset.1 as f64)),
            ("scale".into(), Json::Number(self.scale as f64)),
            ("color".into(), Json::int(self.color as i64)),
            ("shadow".into(), Json::Bool(self.shadow)),
            ("fade_in".into(), Json::int(self.fade_in as i64)),
            ("fade_out".into(), Json::int(self.fade_out as i64)),
            ("start".into(), Json::int(self.start as i64)),
            ("end".into(), self.end.map(|e| Json::int(e as i64)).unwrap_or(Json::Null)),
            (
                "keyframes".into(),
                Json::Array(
                    self.keyframes
                        .iter()
                        .map(|k| {
                            Json::Object(vec![
                                ("tick".into(), Json::int(k.tick as i64)),
                                ("opacity".into(), Json::Number(k.opacity as f64)),
                                ("scale".into(), Json::Number(k.scale as f64)),
                                ("offset_x".into(), Json::Number(k.offset.0 as f64)),
                                ("offset_y".into(), Json::Number(k.offset.1 as f64)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }

    pub fn from_json(json: &Json) -> Option<Self> {
        let mut card = TextCard::default();
        card.id = json.get("id").and_then(Json::as_str).unwrap_or("card").to_owned();
        card.text = json.get("text").and_then(Json::as_str).unwrap_or("").to_owned();
        card.anchor = json.get("anchor").and_then(Json::as_str).and_then(Anchor::from_id).unwrap_or(Anchor::BottomCenter);
        card.offset = (
            json.get("offset_x").and_then(Json::as_f64).unwrap_or(0.0) as f32,
            json.get("offset_y").and_then(Json::as_f64).unwrap_or(-40.0) as f32,
        );
        card.scale = json.get("scale").and_then(Json::as_f64).unwrap_or(1.0).clamp(0.1, 20.0) as f32;
        card.color = json.get("color").and_then(Json::as_u64).unwrap_or(0xFFFF_FFFF) as u32;
        card.shadow = json.get("shadow").and_then(Json::as_bool).unwrap_or(true);
        card.fade_in = json.get("fade_in").and_then(Json::as_u64).unwrap_or(10);
        card.fade_out = json.get("fade_out").and_then(Json::as_u64).unwrap_or(10);
        card.start = json.get("start").and_then(Json::as_u64).unwrap_or(0);
        card.end = json.get("end").and_then(Json::as_u64);
        if let Some(items) = json.get("keyframes").and_then(Json::as_array) {
            for item in items {
                card.keyframes.push(OverlayKeyframe {
                    tick: item.get("tick").and_then(Json::as_u64).unwrap_or(0),
                    opacity: item.get("opacity").and_then(Json::as_f64).unwrap_or(1.0).clamp(0.0, 1.0) as f32,
                    scale: item.get("scale").and_then(Json::as_f64).unwrap_or(1.0).clamp(0.1, 20.0) as f32,
                    offset: (
                        item.get("offset_x").and_then(Json::as_f64).unwrap_or(0.0) as f32,
                        item.get("offset_y").and_then(Json::as_f64).unwrap_or(0.0) as f32,
                    ),
                });
            }
            card.keyframes.sort_by_key(|k| k.tick);
        }
        Some(card)
    }
}

/// Sample overlay keyframes: (opacity, scale, offset).
fn sample_keyframes(keyframes: &[OverlayKeyframe], tick: u64) -> (f32, f32, (f32, f32)) {
    match keyframes.len() {
        0 => (1.0, 1.0, (0.0, 0.0)),
        1 => {
            let k = keyframes[0];
            (k.opacity, k.scale, k.offset)
        }
        _ => {
            let index = keyframes.partition_point(|k| k.tick <= tick).saturating_sub(1);
            let a = keyframes[index];
            let b = keyframes[(index + 1).min(keyframes.len() - 1)];
            if b.tick == a.tick || tick >= b.tick {
                return (b.opacity, b.scale, b.offset);
            }
            let t = (tick.saturating_sub(a.tick)) as f32 / (b.tick - a.tick) as f32;
            (
                a.opacity + (b.opacity - a.opacity) * t,
                a.scale + (b.scale - a.scale) * t,
                (a.offset.0 + (b.offset.0 - a.offset.0) * t, a.offset.1 + (b.offset.1 - a.offset.1) * t),
            )
        }
    }
}

/// An image watermark or logo.
#[derive(Clone, Debug, PartialEq)]
pub struct Watermark {
    /// File under `config/voltlapse/overlays/` or an absolute path.
    pub image: String,
    pub anchor: Anchor,
    pub offset: (f32, f32),
    pub width: f32,
    pub height: f32,
    pub opacity: f32,
    pub start: u64,
    pub end: Option<u64>,
}

impl Default for Watermark {
    fn default() -> Self {
        Self {
            image: "voltlapse_watermark.png".into(),
            anchor: Anchor::BottomRight,
            offset: (-6.0, -6.0),
            width: 64.0,
            height: 32.0,
            opacity: 0.75,
            start: 0,
            end: None,
        }
    }
}

impl Watermark {
    /// Opacity at a tick (0 outside its range).
    pub fn opacity_at(&self, tick: u64) -> f32 {
        let end = self.end.unwrap_or(u64::MAX);
        if tick < self.start || tick > end {
            0.0
        } else {
            self.opacity.clamp(0.0, 1.0)
        }
    }

    pub fn rect(&self, screen: (f32, f32)) -> Rect {
        Rect::anchored(self.anchor, self.offset, screen, (self.width, self.height))
    }
}

/// Which parts of the game HUD a replay shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudSettings {
    pub health: bool,
    pub hunger: bool,
    pub crosshair: bool,
    pub debug: bool,
    pub player_names: bool,
    pub entity_names: bool,
    pub timer: bool,
    /// Show the recording indicator while recording.
    pub recording_indicator: bool,
    pub indicator_anchor: Anchor,
    pub indicator_color: u32,
    /// Hide the whole HUD (F1).
    pub master_visible: bool,
    /// Draw the transport bar (play state, speed, time) in a replay.
    pub transport: bool,
}

impl Default for HudSettings {
    fn default() -> Self {
        Self {
            health: true,
            hunger: true,
            crosshair: true,
            debug: false,
            player_names: true,
            entity_names: false,
            timer: false,
            recording_indicator: true,
            indicator_anchor: Anchor::TopRight,
            indicator_color: 0xFFFF_3B30,
            master_visible: true,
            transport: true,
        }
    }
}

impl HudSettings {
    /// Whether any world HUD element is drawn.
    pub fn shows_world_hud(&self) -> bool {
        self.master_visible && (self.health || self.hunger || self.crosshair)
    }

    /// Cycle a toggle by index, as the settings page's buttons do.
    pub fn toggle(&mut self, index: usize) -> bool {
        match index {
            0 => self.health = !self.health,
            1 => self.hunger = !self.hunger,
            2 => self.crosshair = !self.crosshair,
            3 => self.debug = !self.debug,
            4 => self.player_names = !self.player_names,
            5 => self.entity_names = !self.entity_names,
            6 => self.timer = !self.timer,
            7 => self.transport = !self.transport,
            8 => self.recording_indicator = !self.recording_indicator,
            _ => return false,
        }
        true
    }

    /// The settings page's labels, in `toggle` order.
    pub fn labels(&self) -> Vec<String> {
        let on = |value: bool| if value { "ON" } else { "OFF" };
        vec![
            format!("Health and armour: {}", on(self.health)),
            format!("Hunger: {}", on(self.hunger)),
            format!("Crosshair: {}", on(self.crosshair)),
            format!("Debug screen: {}", on(self.debug)),
            format!("Player name tags: {}", on(self.player_names)),
            format!("Entity name tags: {}", on(self.entity_names)),
            format!("Timer: {}", on(self.timer)),
            format!("Transport bar: {}", on(self.transport)),
            format!("Recording indicator: {}", on(self.recording_indicator)),
        ]
    }
}

/// Everything an overlay render draws above the world.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HudOverlay {
    pub cards: Vec<TextCard>,
    pub watermark: Option<Watermark>,
    pub settings: HudSettings,
    /// Ticks shown by the stopwatch overlay.
    pub timer_start: u64,
}

impl HudOverlay {
    /// Add a title card / lower third, replacing one with the same id.
    pub fn add_card(&mut self, card: TextCard) {
        match self.cards.iter_mut().find(|existing| existing.id == card.id) {
            Some(existing) => *existing = card,
            None => self.cards.push(card),
        }
    }

    pub fn remove_card(&mut self, id: &str) -> bool {
        let before = self.cards.len();
        self.cards.retain(|card| card.id != id);
        before != self.cards.len()
    }

    /// Resolve every visible card for a frame, ordered back to front.
    pub fn resolved(&self, tick: u64, screen: (f32, f32), char_width: f32, line_height: f32) -> Vec<ResolvedCard> {
        if !self.settings.master_visible {
            return Vec::new();
        }
        self.cards
            .iter()
            .filter_map(|card| card.resolve(tick, screen, char_width, line_height))
            .collect()
    }

    /// The stopwatch text (`01:23.45`), when the timer overlay is on.
    pub fn timer_text(&self, tick: u64, tick_rate: u16) -> String {
        format_duration(tick.saturating_sub(self.timer_start), tick_rate)
    }

    pub fn to_json(&self) -> Json {
        Json::Object(vec![
            ("cards".into(), Json::Array(self.cards.iter().map(TextCard::to_json).collect())),
            (
                "watermark".into(),
                match &self.watermark {
                    Some(watermark) => Json::Object(vec![
                        ("image".into(), Json::String(watermark.image.clone())),
                        ("anchor".into(), Json::String(watermark.anchor.id().into())),
                        ("offset_x".into(), Json::Number(watermark.offset.0 as f64)),
                        ("offset_y".into(), Json::Number(watermark.offset.1 as f64)),
                        ("width".into(), Json::Number(watermark.width as f64)),
                        ("height".into(), Json::Number(watermark.height as f64)),
                        ("opacity".into(), Json::Number(watermark.opacity as f64)),
                    ]),
                    None => Json::Null,
                },
            ),
            ("timer_start".into(), Json::int(self.timer_start as i64)),
        ])
    }

    pub fn from_json(json: &Json) -> Self {
        let mut overlay = HudOverlay::default();
        if let Some(items) = json.get("cards").and_then(Json::as_array) {
            for item in items {
                if let Some(card) = TextCard::from_json(item) {
                    overlay.add_card(card);
                }
            }
        }
        if let Some(watermark) = json.get("watermark").filter(|w| **w != Json::Null) {
            let mut value = Watermark::default();
            value.image = watermark.get("image").and_then(Json::as_str).unwrap_or("").to_owned();
            value.anchor = watermark.get("anchor").and_then(Json::as_str).and_then(Anchor::from_id).unwrap_or(Anchor::BottomRight);
            value.offset = (
                watermark.get("offset_x").and_then(Json::as_f64).unwrap_or(-6.0) as f32,
                watermark.get("offset_y").and_then(Json::as_f64).unwrap_or(-6.0) as f32,
            );
            value.width = watermark.get("width").and_then(Json::as_f64).unwrap_or(64.0) as f32;
            value.height = watermark.get("height").and_then(Json::as_f64).unwrap_or(32.0) as f32;
            value.opacity = watermark.get("opacity").and_then(Json::as_f64).unwrap_or(0.75).clamp(0.0, 1.0) as f32;
            overlay.watermark = Some(value);
        }
        overlay.timer_start = json.get("timer_start").and_then(Json::as_u64).unwrap_or(0);
        overlay
    }
}

/// `mm:ss.cc` for a tick count, as the replay browser and the timer show it.
pub fn format_duration(ticks: u64, tick_rate: u16) -> String {
    let tick_rate = tick_rate.max(1) as u64;
    let total_centis = ticks.saturating_mul(100) / tick_rate;
    let minutes = total_centis / 6_000;
    let seconds = (total_centis / 100) % 60;
    let centis = total_centis % 100;
    if minutes >= 60 {
        format!("{}:{:02}:{:02}.{:02}", minutes / 60, minutes % 60, seconds, centis)
    } else {
        format!("{minutes:02}:{seconds:02}.{centis:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_format_with_centiseconds_and_hours() {
        assert_eq!(format_duration(0, 20), "00:00.00");
        assert_eq!(format_duration(20, 20), "00:01.00");
        assert_eq!(format_duration(2_441, 20), "02:02.05");
        assert_eq!(format_duration(20 * 3_723, 20), "1:02:03.00");
        assert_eq!(format_duration(10, 0), "00:01.00", "a zero tick rate is treated as 1");
    }

    #[test]
    fn anchors_place_boxes_in_the_expected_corners() {
        let screen = (100.0, 50.0);
        let size = (10.0, 10.0);
        let top_left = Rect::anchored(Anchor::TopLeft, (0.0, 0.0), screen, size);
        assert_eq!((top_left.x, top_left.y), (0.0, 0.0));
        let center = Rect::anchored(Anchor::Center, (0.0, 0.0), screen, size);
        assert_eq!((center.x, center.y), (45.0, 20.0));
        let bottom_right = Rect::anchored(Anchor::BottomRight, (-5.0, -5.0), screen, size);
        assert_eq!((bottom_right.x, bottom_right.y), (85.0, 35.0));
        assert_eq!(Anchor::from_id("bottom-right"), Some(Anchor::BottomRight));
        assert_eq!(Anchor::from_id("nope"), None);
    }

    #[test]
    fn letterboxing_centres_and_reports_bars() {
        let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let (inner, bar) = screen.letterboxed(1920.0 / 816.0); // 2.35:1
        assert!((inner.height - 816.0).abs() < 0.5, "height {}", inner.height);
        assert!((bar - 132.0).abs() < 0.5, "bar {bar}");
        let (inner, bar) = screen.letterboxed(1080.0 / 1920.0);
        assert!((inner.width - 607.5).abs() < 0.5);
        assert!(bar > 0.0);
        // The native aspect changes nothing.
        let (same, bar) = screen.letterboxed(1920.0 / 1080.0);
        assert_eq!(bar, 0.0);
        assert_eq!(same, screen);
    }

    #[test]
    fn a_card_fades_in_and_out() {
        let card = TextCard {
            text: "Volt Lapse".into(),
            start: 10,
            end: Some(60),
            fade_in: 10,
            fade_out: 20,
            ..TextCard::default()
        };
        assert_eq!(card.opacity_at(5), 0.0);
        assert!((card.opacity_at(15) - 0.5).abs() < 0.01);
        assert!((card.opacity_at(30) - 1.0).abs() < 0.01);
        assert!((card.opacity_at(50) - 0.5).abs() < 0.01);
        assert_eq!(card.opacity_at(61), 0.0);
        assert!(card.resolve(30, (100.0, 100.0), 6.0, 9.0).is_some());
        assert!(card.resolve(5, (100.0, 100.0), 6.0, 9.0).is_none());
    }

    #[test]
    fn cards_without_an_end_never_fade_out() {
        let card = TextCard { text: "lower third".into(), start: 0, end: None, fade_out: 10, ..TextCard::default() };
        assert!((card.opacity_at(10_000) - 1.0).abs() < 0.01);
    }

    #[test]
    fn overlay_keyframes_animate_opacity_and_scale() {
        let card = TextCard {
            text: "zoom".into(),
            start: 0,
            keyframes: vec![
                OverlayKeyframe { tick: 0, opacity: 0.0, scale: 1.0, offset: (0.0, 0.0) },
                OverlayKeyframe { tick: 20, opacity: 1.0, scale: 2.0, offset: (10.0, 0.0) },
            ],
            ..TextCard::default()
        };
        assert!((card.opacity_at(0) - 0.0).abs() < 0.01);
        assert!((card.opacity_at(10) - 0.5).abs() < 0.02);
        let resolved = card.resolve(20, (200.0, 100.0), 6.0, 9.0).unwrap();
        assert!((resolved.scale - 2.0).abs() < 0.01);
    }

    #[test]
    fn overlay_layout_centres_multiline_text() {
        let card = TextCard { text: "line one\nlonger line two".into(), anchor: Anchor::TopCenter, offset: (0.0, 0.0), ..TextCard::default() };
        let resolved = card.resolve(100, (200.0, 100.0), 5.0, 9.0).unwrap();
        // The longest line is 15 characters; at 5 px that is 75, centred in 200.
        assert!((resolved.rect.width - 75.0).abs() < 0.01);
        assert!((resolved.rect.x - 62.5).abs() < 0.01);
        assert!((resolved.rect.height - 18.0).abs() < 0.01);
    }

    #[test]
    fn the_master_switch_hides_everything() {
        let mut overlay = HudOverlay::default();
        overlay.add_card(TextCard { text: "hello".into(), ..TextCard::default() });
        assert_eq!(overlay.resolved(0, (100.0, 100.0), 6.0, 9.0).len(), 1);
        overlay.settings.master_visible = false;
        assert!(overlay.resolved(0, (100.0, 100.0), 6.0, 9.0).is_empty());
    }

    #[test]
    fn hud_toggles_and_labels_line_up() {
        let mut settings = HudSettings::default();
        assert_eq!(settings.labels().len(), 9);
        assert!(settings.toggle(3));
        assert!(settings.debug);
        assert!(!settings.toggle(99));
        settings.master_visible = false;
        assert!(!settings.shows_world_hud());
        assert!(settings.labels()[3].ends_with("ON"));
    }

    #[test]
    fn watermark_visibility_follows_its_range() {
        let watermark = Watermark { start: 10, end: Some(20), ..Watermark::default() };
        assert_eq!(watermark.opacity_at(9), 0.0);
        assert!((watermark.opacity_at(15) - 0.75).abs() < 1e-6);
        assert_eq!(watermark.opacity_at(21), 0.0);
        let rect = watermark.rect((1920.0, 1080.0));
        assert_eq!(rect.x, 1920.0 - 64.0 - 6.0);
    }

    #[test]
    fn timer_text_counts_from_its_start() {
        let mut overlay = HudOverlay::default();
        overlay.timer_start = 40;
        assert_eq!(overlay.timer_text(80, 20), "00:02.00");
        assert_eq!(overlay.timer_text(40, 20), "00:00.00");
    }

    #[test]
    fn overlays_round_trip_through_json() {
        let mut overlay = HudOverlay::default();
        overlay.add_card(TextCard {
            id: "intro".into(),
            text: "Chapter I".into(),
            anchor: Anchor::Center,
            scale: 2.0,
            keyframes: vec![OverlayKeyframe { tick: 5, opacity: 1.0, scale: 1.0, offset: (0.0, 0.0) }],
            ..TextCard::default()
        });
        overlay.watermark = Some(Watermark::default());
        overlay.timer_start = 12;
        let back = HudOverlay::from_json(&overlay.to_json());
        assert_eq!(back.cards.len(), 1);
        assert_eq!(back.cards[0].id, "intro");
        assert_eq!(back.cards[0].anchor, Anchor::Center);
        assert_eq!(back.cards[0].keyframes.len(), 1);
        assert!(back.watermark.is_some());
        assert_eq!(back.timer_start, 12);
        assert!(back.remove_card("intro"));
        assert!(!back.remove_card("intro"));
    }
}
