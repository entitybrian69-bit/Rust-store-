// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

// A click on the mod's settings page.
fn voltlapse_settings_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::voltlapse::settings_click(game, mod_id, button);
}
