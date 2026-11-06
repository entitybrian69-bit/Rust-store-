// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

// The command names the chat box colours, completes and lists.
fn voltlapse_commands(game: &mut crate::game::Game<'_>, commands: &mut Vec<&'static str>) {
    crate::game::voltlapse::commands(game, commands);
}
