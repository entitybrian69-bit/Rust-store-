// native_mods (hook "mods", version 1): the mod's row in Options > Mods.
fn fresh_moves_mods(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::fresh_moves_native::mods(mods);
}
