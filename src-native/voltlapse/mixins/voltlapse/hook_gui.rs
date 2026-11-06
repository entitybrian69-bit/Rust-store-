// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

// After the game's own GUI: the replay browser, the camera editor, the render
// settings and the settings tabs.
fn voltlapse_gui(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    gui_hidden: bool,
    focused: bool,
) {
    crate::game::voltlapse::gui_post(game, batch, gui_hidden, focused);
}
