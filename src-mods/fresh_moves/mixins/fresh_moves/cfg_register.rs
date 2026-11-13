// native_register_configs (hook "register_configs", version 1): the mod's own
// config file, `config/fresh_moves-client.toml`, written when it is missing.
fn fresh_moves_configs(configs: &mut Vec<crate::game::native_configs::NativeConfig>) {
    crate::game::fresh_moves_native::register_configs(configs);
}
