// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

// An entity leaving the level.
fn voltlapse_leave(game: &mut crate::game::Game<'_>, dimension: &str, entity: u32, kind: &str) {
    crate::game::voltlapse::entity_leave(game, dimension, entity, kind);
}
