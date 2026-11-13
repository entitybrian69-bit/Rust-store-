// native_frame (hook "frame", version 1): once a frame, advance the frame-rate
// smoothing the first-person hand and the camera use.
fn fresh_moves_frame(elapsed: std::time::Duration) {
    crate::game::fresh_moves_native::frame(elapsed);
}
