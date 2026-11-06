// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

// A key changed state: run the mod's bindings.
fn voltlapse_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::voltlapse::key(game, key, pressed);
}
