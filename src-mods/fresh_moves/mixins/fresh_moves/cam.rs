// native_camera_distance (hook "camera_distance", version 1): the third-person
// camera's distance, pulled back at a run and in on a hit. First person never
// asks for it.
fn fresh_moves_camera(partial: f32, distance: &mut f32) {
    crate::game::fresh_moves_native::camera_distance(partial, distance);
}
