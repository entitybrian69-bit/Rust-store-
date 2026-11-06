// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! The hook API other mods use.
//!
//! A mod that wants to react to Volt Lapse (add a chapter marker on every kill,
//! push a replay to a server, draw its own timeline) subscribes to the event
//! bus. Subscribers are ordered by priority and then by registration order, so
//! a chain of mods behaves the same way every launch. Every callback runs on the
//! thread that emitted the event, which is the client thread for recording and
//! playback events.

use std::sync::{Arc, Mutex};

use crate::event::Event;

/// What other mods can listen for.
#[derive(Clone, Debug, PartialEq)]
pub enum ApiEvent {
    RecordingStarted { tick_rate: u16, mc_version: String },
    /// One recorded tick, after its events were buffered.
    TickRecorded { tick: u64, events: usize },
    RecordingPaused { tick: u64 },
    RecordingResumed { tick: u64 },
    RecordingStopped { ticks: u64, path: String },
    PlaybackOpened { replay: String, duration_ticks: u64 },
    PlaybackSeeked { tick: u64 },
    PlaybackEnded { tick: u64 },
    /// A gameplay event the recorder captured, for a mod's own counter.
    GameplayRecorded { tick: u64, event: Event },
    RenderStarted { job: String, frames: u64 },
    RenderProgress { job: String, frame: u64, total: u64 },
    RenderFinished { job: String, output: String },
    BookmarkAdded { tick: u64, label: String },
    ConfigChanged { section: String },
    /// A camera path was edited.
    KeyframesChanged { count: usize },
}

impl ApiEvent {
    /// A short name, for logs.
    pub fn name(&self) -> &'static str {
        match self {
            ApiEvent::RecordingStarted { .. } => "recording_started",
            ApiEvent::TickRecorded { .. } => "tick_recorded",
            ApiEvent::RecordingPaused { .. } => "recording_paused",
            ApiEvent::RecordingResumed { .. } => "recording_resumed",
            ApiEvent::RecordingStopped { .. } => "recording_stopped",
            ApiEvent::PlaybackOpened { .. } => "playback_opened",
            ApiEvent::PlaybackSeeked { .. } => "playback_seeked",
            ApiEvent::PlaybackEnded { .. } => "playback_ended",
            ApiEvent::GameplayRecorded { .. } => "gameplay_recorded",
            ApiEvent::RenderStarted { .. } => "render_started",
            ApiEvent::RenderProgress { .. } => "render_progress",
            ApiEvent::RenderFinished { .. } => "render_finished",
            ApiEvent::BookmarkAdded { .. } => "bookmark_added",
            ApiEvent::ConfigChanged { .. } => "config_changed",
            ApiEvent::KeyframesChanged { .. } => "keyframes_changed",
        }
    }

    pub fn matches(&self, filter: ApiFilter) -> bool {
        match filter {
            Filter::All => true,
            Filter::Recording => matches!(
                self,
                ApiEvent::RecordingStarted { .. }
                    | ApiEvent::TickRecorded { .. }
                    | ApiEvent::RecordingPaused { .. }
                    | ApiEvent::RecordingResumed { .. }
                    | ApiEvent::RecordingStopped { .. }
                    | ApiEvent::GameplayRecorded { .. }
            ),
            Filter::Playback => matches!(
                self,
                ApiEvent::PlaybackOpened { .. }
                    | ApiEvent::PlaybackSeeked { .. }
                    | ApiEvent::PlaybackEnded { .. }
                    | ApiEvent::KeyframesChanged { .. }
                    | ApiEvent::BookmarkAdded { .. }
            ),
            Filter::Render => matches!(
                self,
                ApiEvent::RenderStarted { .. } | ApiEvent::RenderProgress { .. } | ApiEvent::RenderFinished { .. }
            ),
        }
    }
}

/// Which events a subscription wants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiFilter {
    All,
    Recording,
    Playback,
    Render,
}

type Handler = std::sync::Arc<dyn Fn(&ApiEvent) + Send + Sync>;

struct Subscriber {
    id: String,
    filter: ApiFilter,
    priority: i32,
    handler: Handler,
}

/// The bus. Cloneable and shareable: the mod owns one, and a registered mod
/// holds the same bus through an `Arc`.
#[derive(Clone, Default)]
pub struct ModApi {
    subscribers: Arc<Mutex<Vec<Subscriber>>>,
}

impl std::fmt::Debug for ModApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.subscribers.lock().map(|list| list.len()).unwrap_or(0);
        write!(f, "ModApi({count} subscribers)")
    }
}

impl ModApi {
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribe. Re-subscribing with the same id replaces the previous handler,
    /// which makes a mod's hot reload safe. Higher priority runs first.
    pub fn subscribe(
        &self,
        id: impl Into<String>,
        filter: ApiFilter,
        priority: i32,
        handler: impl Fn(&ApiEvent) + Send + Sync + 'static,
    ) {
        let id = id.into();
        let mut list = self.subscribers.lock().expect("api lock");
        list.retain(|subscriber| subscriber.id != id);
        list.push(Subscriber { id, filter, priority, handler: std::sync::Arc::new(handler) });
        // Stable ordering: priority descending, then id for determinism.
        list.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
    }

    pub fn unsubscribe(&self, id: &str) -> bool {
        let mut list = self.subscribers.lock().expect("api lock");
        let before = list.len();
        list.retain(|subscriber| subscriber.id != id);
        before != list.len()
    }

    pub fn subscriber_ids(&self) -> Vec<String> {
        self.subscribers
            .lock()
            .map(|list| list.iter().map(|subscriber| subscriber.id.clone()).collect())
            .unwrap_or_default()
    }

    /// Deliver an event. A panicking handler is caught, so one broken mod
    /// cannot take the game down mid-recording, and a handler may subscribe or
    /// unsubscribe while it runs because the bus holds no lock during a call.
    pub fn emit(&self, event: &ApiEvent) -> usize {
        let handlers: Vec<Handler> = {
            let list = self.subscribers.lock().expect("api lock");
            list.iter()
                .filter(|subscriber| event.matches(subscriber.filter))
                .map(|subscriber| subscriber.handler.clone())
                .collect()
        };
        let mut delivered = 0;
        for handler in handlers {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(event))).is_ok() {
                delivered += 1;
            }
        }
        delivered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn subscribers_receive_matching_events_in_priority_order() {
        let api = ModApi::new();
        let order = Arc::new(Mutex::new(Vec::new()));
        let low = order.clone();
        let high = order.clone();
        api.subscribe("low", ApiFilter::All, 1, move |event| low.lock().unwrap().push(format!("low:{}", event.name())));
        api.subscribe("high", ApiFilter::All, 10, move |event| high.lock().unwrap().push(format!("high:{}", event.name())));
        let delivered = api.emit(&ApiEvent::RecordingStarted { tick_rate: 20, mc_version: "26.3".into() });
        assert_eq!(delivered, 2);
        assert_eq!(*order.lock().unwrap(), vec!["high:recording_started", "low:recording_started"]);
    }

    #[test]
    fn filters_only_deliver_their_kind() {
        let api = ModApi::new();
        let count = Arc::new(AtomicUsize::new(0));
        let counter = count.clone();
        api.subscribe("recording-only", ApiFilter::Recording, 0, move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        api.emit(&ApiEvent::PlaybackSeeked { tick: 10 });
        assert_eq!(count.load(Ordering::SeqCst), 0);
        api.emit(&ApiEvent::TickRecorded { tick: 10, events: 3 });
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn resubscribing_replaces_and_unsubscribing_removes() {
        let api = ModApi::new();
        let first = Arc::new(AtomicUsize::new(0));
        let second = Arc::new(AtomicUsize::new(0));
        let a = first.clone();
        api.subscribe("mod", ApiFilter::All, 0, move |_| {
            a.fetch_add(1, Ordering::SeqCst);
        });
        let b = second.clone();
        api.subscribe("mod", ApiFilter::All, 0, move |_| {
            b.fetch_add(1, Ordering::SeqCst);
        });
        assert_eq!(api.subscriber_ids(), vec!["mod".to_owned()]);
        api.emit(&ApiEvent::PlaybackEnded { tick: 1 });
        assert_eq!(first.load(Ordering::SeqCst), 0);
        assert_eq!(second.load(Ordering::SeqCst), 1);
        assert!(api.unsubscribe("mod"));
        assert!(!api.unsubscribe("mod"));
        assert_eq!(api.emit(&ApiEvent::PlaybackEnded { tick: 2 }), 0);
    }

    #[test]
    fn a_panicking_handler_does_not_stop_the_others() {
        let api = ModApi::new();
        let count = Arc::new(AtomicUsize::new(0));
        api.subscribe("bad", ApiFilter::All, 10, |_| panic!("mod bug"));
        let counter = count.clone();
        api.subscribe("good", ApiFilter::All, 1, move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        let delivered = api.emit(&ApiEvent::ConfigChanged { section: "hud".into() });
        assert_eq!(delivered, 1, "the healthy handler still ran");
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn event_names_and_filters_are_stable() {
        let events = [
            ApiEvent::RecordingStarted { tick_rate: 20, mc_version: "26.3".into() },
            ApiEvent::RenderProgress { job: "a".into(), frame: 1, total: 2 },
            ApiEvent::PlaybackOpened { replay: "a.vrc".into(), duration_ticks: 100 },
        ];
        assert_eq!(events[0].name(), "recording_started");
        assert!(events[1].matches(ApiFilter::Render));
        assert!(!events[1].matches(ApiFilter::Recording));
        assert!(events[2].matches(ApiFilter::Playback));
        assert!(events[2].matches(ApiFilter::All));
    }
}
