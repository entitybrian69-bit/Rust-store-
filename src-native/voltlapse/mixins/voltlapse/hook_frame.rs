// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

// Once a frame: advance the replay transport and the render loop.
fn voltlapse_frame(game: &mut crate::game::Game<'_>, elapsed: std::time::Duration) {
    crate::game::voltlapse::frame(game, elapsed);
}
