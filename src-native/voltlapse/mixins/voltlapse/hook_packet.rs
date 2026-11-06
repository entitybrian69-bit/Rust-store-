// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

// Every decoded play packet: full-fidelity capture, and the sound and particle
// packets become first-class events.
fn voltlapse_packet(
    game: &mut crate::game::Game<'_>,
    packet: &crate::mod_api::ClientboundPlayPacket,
) {
    crate::game::voltlapse::packet(game, packet);
}
