# Biomes Unseen — Parity Checklist vs Biomes O' Plenty

Legend: DONE = registered & verified in the loadable package; ADAPTED = same role via an API 0.2-proven template; UNSUPPORTED = capability absent from Horizon API 0.2 in this repo (see GAPS.md). No entry is marked DONE unless `scripts/test_biomes_unseen.py` + `build_hmods.py --check-only` pass for it.

## Wood sets (log/planks/leaves/sapling, log->4 planks recipe)
| cherry_log / cherry_planks / cherry_leaves / cherry_sapling | DONE (.hmod) |
| dead_log / dead_planks / dead_leaves / dead_sapling | DONE (.hmod) |
| fir_log / fir_planks / fir_leaves / fir_sapling | DONE (.hmod) |
| hellbark_log / hellbark_planks / hellbark_leaves / hellbark_sapling | DONE (.hmod) |
| jacaranda_log / jacaranda_planks / jacaranda_leaves / jacaranda_sapling | DONE (.hmod) |
| magic_log / magic_planks / magic_leaves / magic_sapling | DONE (.hmod) |
| mahogany_log / mahogany_planks / mahogany_leaves / mahogany_sapling | DONE (.hmod) |
| maple_log / maple_planks / maple_leaves / maple_sapling | DONE (.hmod) |
| palm_log / palm_planks / palm_leaves / palm_sapling | DONE (.hmod) |
| redwood_log / redwood_planks / redwood_leaves / redwood_sapling | DONE (.hmod) |
| umbran_log / umbran_planks / umbran_leaves / umbran_sapling | DONE (.hmod) |
| willow_log / willow_planks / willow_leaves / willow_sapling | DONE (.hmod) |

## Plants & flowers
| lavender | DONE (.hmod, flower-template block) |
| orange_cosmos | DONE (.hmod, flower-template block) |
| pink_daffodil | DONE (.hmod, flower-template block) |
| pink_hibiscus | DONE (.hmod, flower-template block) |
| white_anemone | DONE (.hmod, flower-template block) |
| wildflower | DONE (.hmod, flower-template block) |
| violet | DONE (.hmod, flower-template block) |
| burning_blossom | DONE (.hmod, flower-template block) |
| glowshroom | DONE (.hmod, flower-template block) |
| waterlily_pad | DONE (.hmod, flower-template block) |
| cattail | DONE (.hmod, flower-template block) |
| rose | DONE (.hmod, flower-template block) |

## Terrain / gem blocks & items
| Entry | Status |
|---|---|
| ash_block, brimstone, flesh_block, porous_flesh, origin_grass_block, mud_brick_block | DONE (.hmod) |
| tanzanite_ore, tanzanite_block, tanzanite, tanzanite_sword, tanzanite_pickaxe | ADAPTED — legacy BOP gem tier recreated on emerald/diamond templates (modern BOP dropped gems) |
| mudball, origin_acorn, 12 saplings | DONE (.hmod items) |
| Full per-wood stairs/slab/fence/door/button sets, boats, signs | UNSUPPORTED — no proven registry example for those shapes beyond the block-template system used; not faked |
| Sounds, particles | UNSUPPORTED — blocks reuse vanilla sound groups via `properties.sounds` (proven field); no custom sound/particle registry in API 0.2 examples |

## Biomes (climate/colors/weight = data shipped in .hnmod Rust source; generation UNSUPPORTED in .hmod)
| Biome | Temp | Downfall | Sky | Fog | Grass | Water | Weight | Category | Status |
|---|---|---|---|---|---|---|---|---|---|
| biomes_unseen:bayou | 0.8 | 0.9 | #7FB8D8 | #C8E8E0 | #6A9A4A | #3F76E4 | 6 | swamp | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:bog | 0.6 | 0.9 | #8AA8B0 | #B8C8C0 | #5A7A3A | #3A5F8A | 5 | swamp | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:cherry_blossom_grove | 0.7 | 0.8 | #7FC8E8 | #E8D0E8 | #7AB85A | #3F76E4 | 4 | forest | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:cold_desert | 0.4 | 0.2 | #8FC8E0 | #E0D8C0 | #9AA06A | #3F76E4 | 5 | desert | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:coniferous_forest | 0.5 | 0.7 | #7FB0D0 | #D0E0D8 | #5A8A4A | #3F76E4 | 8 | forest | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:dead_forest | 0.6 | 0.5 | #9AA8A8 | #C8C8B8 | #8A8A5A | #3F76E4 | 5 | forest | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:field | 0.7 | 0.6 | #7FC8E8 | #D8E8D0 | #7AB85A | #3F76E4 | 7 | plains | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:fir_clearing | 0.5 | 0.7 | #7FB0D0 | #D0E0D8 | #5F9048 | #3F76E4 | 6 | forest | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:floodplain | 0.8 | 0.8 | #7FC0D8 | #D8E8D8 | #6FAE52 | #3F76E4 | 5 | plains | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:forested_field | 0.7 | 0.6 | #7FC8E8 | #D8E8D0 | #77B255 | #3F76E4 | 6 | plains | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:grassland | 0.8 | 0.4 | #7FD4E8 | #E0ECD0 | #8AB85E | #3F76E4 | 7 | plains | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:gravel_beach | 0.6 | 0.5 | #8FC0D8 | #D8DDD0 | #8A9A68 | #3F76E4 | 4 | beach | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:highland | 0.5 | 0.6 | #7FB8D8 | #D0DCE8 | #6F9A52 | #3F76E4 | 5 | mountain | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:hot_springs | 0.9 | 0.4 | #8FD0E0 | #E8E4D0 | #7AA85E | #4FB8C8 | 3 | mountain | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:jade_cliffs | 0.7 | 0.7 | #7FC8D8 | #D8E8DC | #5FAE62 | #3F76E4 | 4 | mountain | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:lavender_field | 0.7 | 0.6 | #7FC8E8 | #E2D8EA | #82B862 | #3F76E4 | 4 | plains | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:lush_desert | 0.95 | 0.3 | #8FD8E8 | #F0E0B8 | #8AA85E | #3F76E4 | 5 | desert | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:lush_savanna | 0.9 | 0.5 | #7FD0E8 | #E8E0C0 | #9AB45E | #3F76E4 | 6 | savanna | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:maple_woods | 0.6 | 0.7 | #7FC0D8 | #E8D8C0 | #9A9A4E | #3F76E4 | 6 | forest | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:marsh | 0.8 | 0.9 | #86B8C8 | #CFE0D8 | #6A9A4E | #3F76E4 | 6 | swamp | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:mediterranean_forest | 0.8 | 0.5 | #7FD0E8 | #E8DEC8 | #7AA254 | #3F76E4 | 6 | forest | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:mystic_grove | 0.7 | 0.8 | #86C0E8 | #DCD0F0 | #6AB86A | #5F8FE4 | 2 | forest | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:ominous_woods | 0.6 | 0.8 | #6F90A8 | #A8B0C0 | #4E7A42 | #3A5F8A | 3 | forest | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:orchard | 0.7 | 0.6 | #7FC8E8 | #DCEAD2 | #7CB45C | #3F76E4 | 5 | forest | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:origin_valley | 0.7 | 0.7 | #7FD8E8 | #D8F0DC | #77C45C | #44A8E4 | 2 | special | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:pasture | 0.7 | 0.6 | #7FC8E8 | #DCEAD2 | #82BC60 | #3F76E4 | 6 | plains | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:prairie | 0.8 | 0.3 | #7FD4E8 | #E8E8C8 | #A8B45E | #3F76E4 | 7 | plains | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:pumpkin_patch | 0.7 | 0.6 | #7FC0D8 | #E0DCC0 | #86AC58 | #3F76E4 | 3 | plains | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:rainforest | 0.95 | 0.9 | #6FB8D0 | #C8E0CC | #3F9A3A | #3F76E4 | 5 | jungle | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:redwood_forest | 0.7 | 0.8 | #76B8CC | #D2E2D4 | #548C46 | #3F76E4 | 5 | forest | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:rocky_rainforest | 0.9 | 0.9 | #72B4CC | #CCE0D0 | #4A9A44 | #3F76E4 | 4 | jungle | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:scrubland | 0.85 | 0.2 | #84D0E4 | #EEE0BC | #9AA259 | #3F76E4 | 6 | desert | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:seasonal_forest | 0.6 | 0.7 | #7CC0D8 | #E4D8BC | #8A9A4C | #3F76E4 | 7 | forest | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:snowy_coniferous_forest | -0.2 | 0.7 | #92C4DE | #E4ECEC | #6A9A5E | #3D57D6 | 6 | snowy | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:snowy_fir_clearing | -0.2 | 0.7 | #92C4DE | #E4ECEC | #6F9F62 | #3D57D6 | 5 | snowy | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:snowy_maple_woods | -0.1 | 0.7 | #8EC2DC | #E8E4DA | #7FA065 | #3D57D6 | 5 | snowy | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:tropics | 0.95 | 0.8 | #6FC8DC | #D4ECD8 | #59B84E | #2FB8C8 | 3 | jungle | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:wetland | 0.75 | 0.9 | #80BCC8 | #D0E4DA | #67A04B | #3F76E4 | 6 | swamp | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:wooded_scrubland | 0.85 | 0.25 | #84D0E4 | #EEE0BC | #93A258 | #3F76E4 | 5 | desert | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:crystalline_chasm | 2.0 | 0.0 | #8A2A3A | #4A0F1E | #A04A6A | #CF5A3A | 4 | nether | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:infernal_dune | 2.0 | 0.0 | #9A3A2A | #5A1408 | #A05A3A | #CF5A3A | 5 | nether | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:undergrowth | 2.0 | 0.0 | #7A4A2A | #3F1E0C | #8A6A3A | #CF5A3A | 5 | nether | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:visceral_heap | 2.0 | 0.0 | #8A3A4A | #47121F | #A04A5A | #CF5A3A | 4 | nether | UNSUPPORTED in .hmod / data in .hnmod |
| biomes_unseen:withered_abyss | 2.0 | 0.0 | #4A4A5A | #1C1C26 | #5A5A6A | #8A3A3A | 3 | nether | UNSUPPORTED in .hmod / data in .hnmod |

## Worldgen / misc
| Entry | Status |
|---|---|
| Biome registration, configured/placed features, tree generation, ore veins, surface rules | UNSUPPORTED — no biome/worldgen registry type exists in any example mod or doc in this repo |
| Fog/sky/grass/water biome colors | ADAPTED — exact values carried in .hnmod `BIOMES` table; cannot be applied by .hmod |
| Config (per-biome toggle/weight) | ADAPTED — `Config` in .hnmod Rust source; .hmod API 0.2 exposes no config capability |
| Recipes, loot tables, mineable tags, creative tab, lang | DONE (.hmod, verified) |
