// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

// `/voltlapse ...`, before the server and the game's own commands see it.
fn voltlapse_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::voltlapse::command(game, command, ran);
}
