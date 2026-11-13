// native_tick (hook "tick", version 1): once a client tick, animate every
// player model the window can see. The work is in the support module, so this
// stays a binding and nothing else.
fn fresh_moves_tick(game: &mut crate::game::Game<'_>) {
    crate::game::fresh_moves_native::tick(game);
}
