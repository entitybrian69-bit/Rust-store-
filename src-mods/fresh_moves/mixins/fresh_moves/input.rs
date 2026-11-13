// native_movement_input (hook "movement_input", version 1): the local player's
// input this tick, read only. Nothing here changes what the player does.
fn fresh_moves_input(
    forward: &mut f32,
    strafe: &mut f32,
    jump: &mut bool,
    sneak: &mut bool,
) {
    crate::game::fresh_moves_native::movement_input(forward, strafe, jump, sneak);
}
