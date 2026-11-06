//! Legacy Horizon WASM prototype. The server hook only previews candidates.
//! See README for why queued, unconditional writes are deliberately disabled.
mod planner;
pub use planner::{facing_from_yaw, plan, Facing, Mode, Pos};

use horizon_mod_sdk::server::{players, world, BlockPos, Player, Registration, ServerMod};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    enabled: bool,
    max_blocks_per_sweep: u32,
    placement_mode: String,
    facing: String,
}

impl Config {
    fn load(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|e| format!("invalid config: {e}"))?;
        if config.max_blocks_per_sweep > 64 {
            return Err("max_blocks_per_sweep must be 0..=64".into());
        }
        config.mode()?;
        config.facing()?;
        Ok(config)
    }
    fn mode(&self) -> Result<Mode, String> {
        match self.placement_mode.as_str() {
            "line" => Ok(Mode::Line), "plane" => Ok(Mode::Plane),
            _ => Err("placement_mode must be line or plane".into()),
        }
    }
    fn facing(&self) -> Result<Facing, String> {
        match self.facing.as_str() {
            "north" => Ok(Facing::North), "east" => Ok(Facing::East),
            "south" => Ok(Facing::South), "west" => Ok(Facing::West),
            _ => Err("facing must be north, east, south, or west".into()),
        }
    }
}

struct QuickPlace { config: Config }

impl ServerMod for QuickPlace {
    fn init() -> Result<(Self, Registration), String> {
        Ok((Self { config: Config::load(include_str!("../config.toml"))? }, Registration::new().on_blocks()))
    }

    fn block_placed(&mut self, player: &Player, pos: BlockPos, _state: &str) {
        if !self.config.enabled || self.config.max_blocks_per_sweep == 0 { return; }
        // TODO(api): Server Player exposes no rotation or sneaking. The fixed
        // configured facing is intentional; sneaking cannot be required.
        let facing = match self.config.facing() { Ok(f) => f, Err(_) => return };
        let mode = match self.config.mode() { Ok(m) => m, Err(_) => return };
        let candidates = plan(Pos { x: pos.x, y: pos.y, z: pos.z }, facing, mode,
            self.config.max_blocks_per_sweep,
            |p| world::get_block(&player.dimension, p.x, p.y, p.z).as_deref() == Some("minecraft:air"));
        // TODO(api): set_blocks is queued and unconditional. No compare-and-set
        // exists, so even an air check can overwrite a later non-air block.
        // Refuse all writes until the host offers an atomic air-only operation.
        let _ = players::tell(&player.id, &format!(
            "Quick Place preview: {} air target(s); no blocks placed (no atomic air-only write).",
            candidates.len()));
    }
}

horizon_mod_sdk::export_server_mod!(QuickPlace);

#[cfg(test)]
mod config_tests {
    use super::Config;
    #[test]
    fn defaults_are_valid() {
        let c = Config::load(include_str!("../config.toml")).unwrap();
        assert!(c.enabled);
        assert_eq!(c.max_blocks_per_sweep, 3);
        assert_eq!(c.facing, "south");
        assert_eq!(c.placement_mode, "line");
    }
    #[test]
    fn invalid_config_fails_closed() {
        let base = include_str!("../config.toml");
        assert!(Config::load(&base.replace("= 3", "= 65")).is_err());
        assert!(Config::load(&base.replace("\"south\"", "\"diagonal\"")).is_err());
        assert!(Config::load(&base.replace("\"line\"", "\"circle\"")).is_err());
        assert!(Config::load(&format!("{base}\nunknown = true\n")).is_err());
    }
}
