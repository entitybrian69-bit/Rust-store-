//! Quick Place — a legacy Horizon **WebAssembly server mod** (API 0.3).
//!
//! The block a player just placed becomes the seed of a small sweep: the free
//! neighbours the configured `placement_mode` names are filled with the same block
//! state, up to `max_blocks_per_sweep`. Only positions the host reports as
//! `minecraft:air` are written; a non-air neighbour, another fluid, or a position in
//! an unloaded chunk is left exactly as it is.
//!
//! Everything that decides *which* positions to try lives in [`planner`], which has
//! no SDK calls and is unit-tested on the host. This file is only the wiring: the
//! `block_placed` hook, the air check, `world.set-blocks`, and the command.
//!
//! API gaps are marked `TODO(api)` where they bite: no yaw, no sneaking, no
//! replaceability predicate, no atomic air-only write (see README.md).

mod config;
mod planner;

pub use config::{parse as parse_config, Config, MAX_SWEEP};
pub use planner::{plan, Axis, Mode, Pos, Request, MAX_CANDIDATES, MAX_REACH};

use horizon_mod_sdk::server::{
    players, world, Args, BlockChange, BlockPos, Player, Registration, ServerMod,
};
use horizon_mod_sdk::{info, warn};

/// How far `line` and `pillar` run before the per-sweep cap is applied.
const REACH: i32 = 3;
/// The only state treated as free. Other air-like states (`cave_air`, `void_air`)
/// and fluids are left alone: API 0.3 has no `is_replaceable` predicate
/// (`TODO(api)`), so anything that is not exactly this is treated as occupied.
const FREE: &str = "minecraft:air";
const COMMAND: &str = "quickplace";

struct QuickPlace {
    config: Config,
    /// Blocks this session has added, for `/quickplace status`.
    placed: u64,
}

impl ServerMod for QuickPlace {
    fn init() -> Result<(Self, Registration), String> {
        let config = parse_config(include_str!("../config.toml"))?;
        if config.require_sneaking {
            // TODO(api): the legacy server `Player` record carries no sneaking
            // state, so this cannot be enforced. Say so instead of pretending.
            warn("require_sneaking = true is not supported by API 0.3 (no sneak state): ignoring it");
        }
        info(&format!(
            "Quick Place {}: enabled={} mode={} limit={} (air-only)",
            env!("CARGO_PKG_VERSION"),
            config.enabled,
            config.placement_mode.name(),
            config.max_blocks_per_sweep
        ));
        Ok((
            Self {
                config,
                placed: 0,
            },
            Registration::new().command(COMMAND).on_blocks(),
        ))
    }

    /// `/quickplace` — report the settings; `/quickplace <mode>` — switch mode for
    /// this session.
    fn command(
        &mut self,
        _player: &Player,
        name: &str,
        args: &Args,
    ) -> Result<Option<String>, String> {
        if name != COMMAND {
            return Err(format!("Quick Place owns no command called {name}"));
        }
        let text = args.text().trim();
        if text.is_empty() || text == "status" {
            return Ok(Some(format!(
                "Quick Place: enabled={}, mode={}, limit={}, placed={}",
                self.config.enabled,
                self.config.placement_mode.name(),
                self.config.max_blocks_per_sweep,
                self.placed
            )));
        }
        match Mode::parse(text) {
            Some(mode) => {
                self.config.placement_mode = mode;
                Ok(Some(format!("Quick Place: mode {}", mode.name())))
            }
            None => Err("Quick Place: mode must be pad, line, pillar or cube".into()),
        }
    }

    /// A player placed `state` at `pos`: fill the free neighbours with it.
    fn block_placed(&mut self, player: &Player, pos: BlockPos, state: &str) {
        if !self.config.enabled || self.config.max_blocks_per_sweep == 0 {
            return;
        }
        let request = Request {
            origin: Pos::new(pos.x, pos.y, pos.z),
            // TODO(api): the server `Player` has a position but no yaw, so `line`
            // derives its direction from where the player stands (planner::line_step).
            player: Pos::new(
                player.x.floor() as i32,
                player.y.floor() as i32,
                player.z.floor() as i32,
            ),
            mode: self.config.placement_mode,
            axis: Axis::Auto,
            reach: REACH,
            limit: self.config.max_blocks_per_sweep,
        };

        let mut changes: Vec<BlockChange> = Vec::new();
        for target in plan(&request) {
            // Air-only, checked immediately before queueing: never overwrite a block
            // that is there, and never touch one in a chunk that is not loaded.
            if world::get_block(&player.dimension, target.x, target.y, target.z).as_deref()
                != Some(FREE)
            {
                continue;
            }
            changes.push(BlockChange {
                x: target.x,
                y: target.y,
                z: target.z,
                state: state.to_string(),
            });
            if changes.len() >= self.config.max_blocks_per_sweep as usize {
                break;
            }
        }
        if changes.is_empty() {
            return;
        }

        let count = changes.len();
        // TODO(api): `set-blocks` queues writes that the host applies later, so the
        // air check above is best-effort, not atomic. The API offers no compare-and-set
        // or air-only placement. See README "API gaps".
        match world::set_blocks(&player.dimension, &changes) {
            Ok(()) => {
                self.placed += count as u64;
                let _ = players::tell(
                    &player.id,
                    &format!("Quick Place: placed {count} block(s) with {} mode", self.config.placement_mode.name()),
                );
            }
            Err(error) => warn(&format!("set-blocks refused: {error}")),
        }
    }
}

horizon_mod_sdk::export_server_mod!(QuickPlace);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_config_is_the_one_that_loads() {
        let config = parse_config(include_str!("../config.toml")).unwrap();
        assert_eq!(config.placement_mode, Mode::Pad);
        assert_eq!(config.max_blocks_per_sweep, 4);
        assert!(!config.require_sneaking);
        assert!(config.enabled);
    }

    #[test]
    fn a_pad_sweep_over_air_stops_at_the_limit() {
        // The pure planner: no host, no SDK. The hook's air filter runs on top.
        let request = Request {
            origin: Pos::new(10, 70, -4),
            player: Pos::new(10, 70, -8),
            mode: Mode::Pad,
            axis: Axis::Auto,
            reach: REACH,
            limit: 3,
        };
        let planned = plan(&request);
        assert_eq!(planned.len(), 3);
        assert_eq!(planned[0], Pos::new(10, 70, -5));
        assert!(!planned.contains(&Pos::new(10, 70, -4)));
    }
}
