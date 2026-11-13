// native_config_loading (hook "config_loading", version 1): the file was read,
// at startup or again after it changed on disk. Every mod hears every config,
// so the module checks the id.
fn fresh_moves_config_loaded(
    game: &mut crate::game::Game<'_>,
    config: &crate::game::native_configs::NativeConfigFile,
) {
    crate::game::fresh_moves_native::config_loaded(game, config);
}
