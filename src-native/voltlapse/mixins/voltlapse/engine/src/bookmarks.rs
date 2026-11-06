// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! Bookmarks: named moments in a replay.
//!
//! A bookmark is a tick, a label and a colour, and the list keeps itself sorted
//! so "next bookmark" and "previous bookmark" are a binary search. Bookmarks are
//! stored in the replay's `metaData.json`, so they travel with the file.

use crate::json::Json;

/// One marked moment.
#[derive(Clone, Debug, PartialEq)]
pub struct Bookmark {
    pub tick: u64,
    pub label: String,
    /// ARGB colour for the timeline marker.
    pub color: u32,
    /// Long-form note shown in the browser.
    pub note: String,
}

impl Bookmark {
    pub fn new(tick: u64, label: impl Into<String>) -> Self {
        Self { tick, label: label.into(), color: 0xFF_4C8DFF, note: String::new() }
    }

    pub fn to_json(&self) -> Json {
        Json::Object(vec![
            ("tick".into(), Json::int(self.tick as i64)),
            ("label".into(), Json::String(self.label.clone())),
            ("color".into(), Json::int(self.color as i64)),
            ("note".into(), Json::String(self.note.clone())),
        ])
    }

    pub fn from_json(json: &Json) -> Option<Self> {
        Some(Self {
            tick: json.get("tick").and_then(Json::as_u64)?,
            label: json.get("label").and_then(Json::as_str).unwrap_or("bookmark").to_owned(),
            color: json.get("color").and_then(Json::as_u64).unwrap_or(0xFF_4C8DFF) as u32,
            note: json.get("note").and_then(Json::as_str).unwrap_or("").to_owned(),
        })
    }
}

/// A sorted set of bookmarks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bookmarks {
    items: Vec<Bookmark>,
}

impl Bookmarks {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Bookmark> {
        self.items.iter()
    }

    pub fn as_slice(&self) -> &[Bookmark] {
        &self.items
    }

    /// Add a bookmark; a second bookmark on the same tick replaces the first.
    pub fn add(&mut self, bookmark: Bookmark) {
        match self.items.iter_mut().find(|existing| existing.tick == bookmark.tick) {
            Some(existing) => *existing = bookmark,
            None => {
                let at = self.items.partition_point(|b| b.tick <= bookmark.tick);
                self.items.insert(at, bookmark);
            }
        }
    }

    pub fn remove(&mut self, tick: u64) -> Option<Bookmark> {
        let at = self.items.iter().position(|b| b.tick == tick)?;
        Some(self.items.remove(at))
    }

    pub fn rename(&mut self, tick: u64, label: &str) -> bool {
        match self.items.iter_mut().find(|b| b.tick == tick) {
            Some(bookmark) => {
                bookmark.label = label.to_owned();
                true
            }
            None => false,
        }
    }

    /// The bookmark nearest a tick, within `tolerance` ticks.
    pub fn nearest(&self, tick: u64, tolerance: u64) -> Option<&Bookmark> {
        let index = self.items.partition_point(|b| b.tick < tick);
        let before = index.checked_sub(1).and_then(|i| self.items.get(i));
        let after = self.items.get(index);
        match (before, after) {
            (Some(before), Some(after)) => {
                let before_distance = tick - before.tick;
                let after_distance = after.tick - tick;
                if before_distance <= after_distance && before_distance <= tolerance {
                    Some(before)
                } else if after_distance <= tolerance {
                    Some(after)
                } else {
                    None
                }
            }
            (Some(before), None) => (tick - before.tick <= tolerance).then_some(before),
            (None, Some(after)) => (after.tick - tick <= tolerance).then_some(after),
            (None, None) => None,
        }
    }

    /// The next bookmark after a tick.
    pub fn next_after(&self, tick: u64) -> Option<&Bookmark> {
        self.items.get(self.items.partition_point(|b| b.tick <= tick))
    }

    /// The previous bookmark before a tick.
    pub fn previous_before(&self, tick: u64) -> Option<&Bookmark> {
        let index = self.items.partition_point(|b| b.tick < tick);
        index.checked_sub(1).and_then(|i| self.items.get(i))
    }

    pub fn to_json(&self) -> Json {
        Json::Array(self.items.iter().map(Bookmark::to_json).collect())
    }

    pub fn from_json(json: &Json) -> Self {
        let mut bookmarks = Bookmarks::new();
        if let Some(items) = json.as_array() {
            for item in items {
                if let Some(bookmark) = Bookmark::from_json(item) {
                    bookmarks.add(bookmark);
                }
            }
        }
        bookmarks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> Bookmarks {
        let mut bookmarks = Bookmarks::new();
        bookmarks.add(Bookmark::new(100, "clutch"));
        bookmarks.add(Bookmark::new(0, "start"));
        bookmarks.add(Bookmark::new(50, "fight"));
        bookmarks
    }

    #[test]
    fn bookmarks_stay_sorted() {
        let bookmarks = list();
        assert_eq!(bookmarks.iter().map(|b| b.tick).collect::<Vec<_>>(), vec![0, 50, 100]);
        assert_eq!(bookmarks.len(), 3);
    }

    #[test]
    fn adding_twice_on_a_tick_replaces() {
        let mut bookmarks = list();
        bookmarks.add(Bookmark::new(50, "boss fight"));
        assert_eq!(bookmarks.len(), 3);
        assert_eq!(bookmarks.nearest(50, 0).unwrap().label, "boss fight");
    }

    #[test]
    fn navigation_finds_the_next_and_previous() {
        let bookmarks = list();
        assert_eq!(bookmarks.next_after(10).unwrap().tick, 50);
        assert_eq!(bookmarks.next_after(100), None);
        assert_eq!(bookmarks.previous_before(60).unwrap().tick, 50);
        assert_eq!(bookmarks.previous_before(0), None);
        assert_eq!(bookmarks.nearest(48, 5).unwrap().tick, 50);
        assert_eq!(bookmarks.nearest(80, 5), None);
        assert_eq!(bookmarks.nearest(80, 30).unwrap().tick, 100);
    }

    #[test]
    fn rename_and_remove() {
        let mut bookmarks = list();
        assert!(bookmarks.rename(50, "big fight"));
        assert!(!bookmarks.rename(999, "nope"));
        assert_eq!(bookmarks.remove(50).unwrap().label, "big fight");
        assert!(bookmarks.remove(50).is_none());
        assert_eq!(bookmarks.len(), 2);
    }

    #[test]
    fn json_round_trips() {
        let mut bookmarks = list();
        let mut first = Bookmark::new(7, "note");
        first.note = "remember this".into();
        first.color = 0xFF00FF00;
        bookmarks.add(first);
        let back = Bookmarks::from_json(&bookmarks.to_json());
        assert_eq!(back, bookmarks);
        assert_eq!(Bookmarks::from_json(&Json::Null).len(), 0);
    }
}
