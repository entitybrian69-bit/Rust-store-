// native_config_reloading (hook "config_reloading", version 1): the file was read,
// at startup or again after it changed on disk. Every mod hears every config,
// so the module checks the id.
fn fresh_moves_config_reloaded(
    game: &mut crate::game::Game<'_>,
    config: &crate::game::native_configs::NativeConfigFile,
) {
    crate::game::fresh_moves_native::config_loaded(game, config);
}
