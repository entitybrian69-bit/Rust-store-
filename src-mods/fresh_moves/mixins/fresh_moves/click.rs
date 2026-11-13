// native_settings_click (hook "settings_click", version 1): a click on one of
// the page's buttons.
fn fresh_moves_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::fresh_moves_native::settings_click(game, mod_id, button);
}
