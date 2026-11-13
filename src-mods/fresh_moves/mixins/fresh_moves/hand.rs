// native_render_hand_pose (hook "render_hand_pose", version 1): the additive
// first-person hand, item, map and empty-arm pose, for each visible hand.
fn fresh_moves_hand(
    swing: f32,
    equip: f32,
    x: &mut f32,
    y: &mut f32,
    z: &mut f32,
    pitch: &mut f32,
    yaw: &mut f32,
    roll: &mut f32,
    scale: &mut f32,
) {
    crate::game::fresh_moves_native::hand_pose(swing, equip, x, y, z, pitch, yaw, roll, scale);
}
