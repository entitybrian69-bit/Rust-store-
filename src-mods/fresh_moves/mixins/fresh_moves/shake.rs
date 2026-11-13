// native_camera_shake (hook "camera_shake", version 1): our sway added to the
// game's own shake, never set in its place, so another mod's shake survives.
fn fresh_moves_shake(
    partial: f32,
    shake: &mut crate::game::native_hooks::NativeShake,
) {
    crate::game::fresh_moves_native::camera_shake(partial, shake);
}
