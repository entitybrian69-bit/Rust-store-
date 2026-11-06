// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

// Once a tick: sample the world into the recording and keep its clock.
fn voltlapse_tick(game: &mut crate::game::Game<'_>) {
    crate::game::voltlapse::tick(game);
}
