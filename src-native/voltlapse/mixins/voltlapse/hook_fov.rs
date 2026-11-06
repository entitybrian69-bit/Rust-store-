// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

// The world's field of view: a camera path may animate it.
fn voltlapse_fov(game: &crate::game::Game<'_>, partial: f32, camera: [f64; 3], degrees: &mut f32) {
    crate::game::voltlapse::fov(game, partial, camera, degrees);
}
