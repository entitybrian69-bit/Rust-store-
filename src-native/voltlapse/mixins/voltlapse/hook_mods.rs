// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

// The Mods page row, with the mod's keys.
fn voltlapse_mods(mods: &mut Vec<crate::mod_api::NativeMod>) {
    crate::game::voltlapse::mods_list(mods);
}
