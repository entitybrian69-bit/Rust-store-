// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

// A world opening or closing: auto-start, auto-stop, crash recovery.
fn voltlapse_lifecycle(
    game: &mut crate::game::Game<'_>,
    context: &crate::mod_api::NativeLifecycleContext,
    phase: crate::game::native_lifecycle::WorldPhase,
    error_text: &str,
) {
    crate::game::voltlapse::lifecycle(game, context, phase, error_text);
}
