# Biomes Unseen (biomes_unseen) v1.0.0
A Biomes O' Plenty-style expansion for Horizon (Minecraft 26.3 Rust rewrite), built only on Horizon API 0.2 registry types proven by this repo's example mods. 56 blocks, 17 items, 16 recipes: 12 wood sets (fir, redwood, mahogany, jacaranda, palm, willow, dead, magic, umbran, hellbark, cherry, maple), 12 flowers/plants, nether flesh/brimstone/ash, origin grass, mud bricks, and the tanzanite gem tier.

## Install — PC (.hmod, recommended)
1. Download `releases/biomes_unseen.hmod`. 2. Copy it into `<game-folder>/horizon-mods/`. 3. Start Horizon → Options → Mods → toggle **Biomes Unseen** ON. 4. Restart Horizon.
## Install — Android (.hmod)
64-bit Horizon Android (arm64-v8a/x86_64, Android 9+): Mods tab → paste the raw GitHub URL of `releases/biomes_unseen.hmod` (or copy the file into `horizon-mods/` via Files) → toggle ON → restart the game. **Do not use the .hnmod on Android** — it is PC source-only.
## Install — PC native (.hnmod, experimental/unverified)
Copy `releases/biomes_unseen.hnmod` to `<game-folder>/horizon-native-mods/` and build with `horizon-launcher native-build <path> --trust-native-code`. It carries the 44-biome climate dataset in Rust; biome *generation* requires an engine biome hook that API 0.2 in this repo does not expose (see GAPS.md).
## Config
.hmod API 0.2 exposes no config capability; per-biome toggles/weights live in the `Config` struct in the .hnmod Rust source. Content is obtainable via the "Biomes Unseen" creative tab, crafting (log→planks, tanzanite gear, mud bricks), and block drops.
## Build & verify (zero-dependency Python 3)
```bash
python3 scripts/generate_biomes_unseen.py
python3 scripts/build_hmods.py            # regenerates + validates all .hmod, updates catalog
python3 scripts/build_hnmod.py            # builds releases/biomes_unseen.hnmod
python3 scripts/test_biomes_unseen.py     # registry/asset/lang/loot/recipe completeness
python3 scripts/build_hmods.py --check-only
```
