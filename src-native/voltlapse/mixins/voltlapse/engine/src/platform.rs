// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! PC and Android differences in one place.
//!
//! Horizon Android is the same engine on a phone: the same replay format, the
//! same camera paths, the same renderer. What differs is memory, how a player
//! reaches a control, and where files may live. Volt Lapse picks defaults from
//! the platform and lets the config override them.

use crate::config::Config;
use crate::perf::PerformanceSettings;

/// Where the mod is running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    /// Desktop Horizon (Windows, Linux, macOS).
    Pc,
    /// Horizon Android, arm64-v8a or x86_64.
    Android,
}

impl Platform {
    /// The platform this build is compiled for, with an environment override so
    /// a desktop can test the phone's policy.
    pub fn detect() -> Platform {
        if let Ok(forced) = std::env::var("VOLTLAPSE_PLATFORM") {
            if forced.eq_ignore_ascii_case("android") {
                return Platform::Android;
            }
            if forced.eq_ignore_ascii_case("pc") {
                return Platform::Pc;
            }
        }
        if cfg!(target_os = "android") {
            Platform::Android
        } else {
            Platform::Pc
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Platform::Pc => "pc",
            Platform::Android => "android",
        }
    }

    /// The file a native mod is packaged as for this platform.
    pub fn package_extension(self) -> &'static str {
        match self {
            Platform::Pc => "hnmod",
            Platform::Android => "hmod",
        }
    }

    /// Whether the interface should offer touch-sized controls.
    pub fn touch_first(self) -> bool {
        self == Platform::Android
    }

    /// Default export size for a phone render.
    pub fn default_render_size(self) -> (u32, u32) {
        match self {
            Platform::Pc => (1920, 1080),
            Platform::Android => (1280, 720),
        }
    }

    /// Threads a capture or render may use before the config is read.
    pub fn default_threads(self) -> usize {
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        match self {
            Platform::Pc => cores,
            Platform::Android => (cores / 2).max(1),
        }
    }

    /// Apply the platform's defaults to a freshly loaded config, without
    /// overriding anything the player chose.
    pub fn apply_defaults(self, config: &mut Config) {
        if self == Platform::Android {
            config.performance.low_memory = true;
            config.performance = config.performance.clone().sanitized();
            config.rendering.width = self.default_render_size().0;
            config.rendering.height = self.default_render_size().1;
            config.rendering = config.rendering.clone().sanitized();
            if config.ui.theme.id() != "light" {
                config.ui.theme = crate::config::Theme::Dark;
            }
        }
        if config.performance.render_threads == 0 {
            config.performance.render_threads = self.default_threads();
        }
    }

    /// A short hint shown in the onboarding tutorial.
    pub fn onboarding_hint(self) -> &'static str {
        match self {
            Platform::Pc => "voltlapse.tutorial.pc",
            Platform::Android => "voltlapse.tutorial.android",
        }
    }

    /// Extensions the replay browser should list.
    pub fn replay_extensions(self) -> [&'static str; 2] {
        // Both platforms read the same `.vrc` files, and `.mcpr` files exported
        // by Replay Mod open too (their packet stream is not decoded, but the
        // metadata and the browser entry work).
        ["vrc", "mcpr"]
    }
}

/// Does this machine look like it can run an 8K render? Used only to warn.
pub fn can_render_8k(platform: Platform, free_memory_mb: u64) -> bool {
    match platform {
        Platform::Android => false,
        Platform::Pc => free_memory_mb > 8 * 1024,
    }
}

/// The performance settings a platform suggests when the config has none.
pub fn suggested_performance(platform: Platform) -> PerformanceSettings {
    let base = PerformanceSettings::default();
    match platform {
        Platform::Pc => base.sanitized(),
        Platform::Android => PerformanceSettings { low_memory: true, ..base }.sanitized(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_extensions_match_the_deliverables() {
        assert_eq!(Platform::Pc.package_extension(), "hnmod");
        assert_eq!(Platform::Android.package_extension(), "hmod");
        assert_eq!(Platform::Android.id(), "android");
        assert!(Platform::Android.touch_first());
        assert!(!Platform::Pc.touch_first());
    }

    #[test]
    fn android_defaults_are_conservative() {
        let mut config = Config::default();
        Platform::Android.apply_defaults(&mut config);
        assert!(config.performance.low_memory);
        assert!(config.performance.buffer_mb <= 16);
        assert_eq!((config.rendering.width, config.rendering.height), (1280, 720));
        assert!(config.performance.render_threads >= 1);
    }

    #[test]
    fn pc_keeps_full_quality_defaults() {
        let mut config = Config::default();
        Platform::Pc.apply_defaults(&mut config);
        assert!(!config.performance.low_memory);
        assert_eq!((config.rendering.width, config.rendering.height), (1920, 1080));
    }

    #[test]
    fn a_phone_cannot_render_8k_but_a_desktop_can() {
        assert!(!can_render_8k(Platform::Android, 64 * 1024));
        assert!(can_render_8k(Platform::Pc, 16 * 1024));
        assert!(!can_render_8k(Platform::Pc, 1024));
    }

    #[test]
    fn suggestions_respect_the_platform() {
        assert!(suggested_performance(Platform::Android).low_memory);
        assert!(!suggested_performance(Platform::Pc).low_memory);
        assert_eq!(Platform::Android.replay_extensions(), ["vrc", "mcpr"]);
        assert!(Platform::Android.onboarding_hint().starts_with("voltlapse.tutorial."));
    }

    #[test]
    fn detection_is_stable() {
        let platform = Platform::detect();
        assert_eq!(platform, Platform::detect());
        std::env::set_var("VOLTLAPSE_PLATFORM", "android");
        assert_eq!(Platform::detect(), Platform::Android);
        std::env::set_var("VOLTLAPSE_PLATFORM", "pc");
        assert_eq!(Platform::detect(), Platform::Pc);
        std::env::remove_var("VOLTLAPSE_PLATFORM");
    }
}
