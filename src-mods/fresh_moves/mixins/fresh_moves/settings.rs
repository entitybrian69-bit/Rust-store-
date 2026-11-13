// native_settings (hook "settings", version 1): the buttons of the mod's page,
// asked again each time it opens and after each click.
fn fresh_moves_settings(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    buttons: &mut Vec<String>,
) {
    crate::game::fresh_moves_native::settings(game, mod_id, buttons);
}
