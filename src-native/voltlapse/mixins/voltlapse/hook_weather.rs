// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

// Weather starting or stopping.
fn voltlapse_weather(
    game: &mut crate::game::Game<'_>,
    dimension: &str,
    raining: bool,
    thundering: bool,
    cancel: &mut bool,
) {
    crate::game::voltlapse::weather_change(game, dimension, raining, thundering, cancel);
}
