# Changelog
## 1.0.0 — 2026-11-08
- Initial release: 56 blocks / 17 items / 16 recipes (12 wood sets, 12 plants, nether & terrain blocks, tanzanite tier), creative tab, loot, tags, en_us lang, original procedural textures.
- `.hnmod` source bundle with 44 biome climate/color/weight definitions (std-only Rust).
- docs: PARITY.md, GAPS.md. Known limitation: biome/worldgen registration not exposed by Horizon API 0.2 in this repo.
## 1.0.1 — 2026-11-08
- Fixed Android import: Volt's importer required mod.wasm for every .hmod, rejecting all content-only mods (incl. all 6 pre-existing repo mods). Importer fixed in Volt (branch fix-android-content-mod-import) to match Package::read: manifest at ZIP root AND (mod.wasm OR assets//data/ content). No packaging change was needed or made — the .hmod layout already matches the spec; version bumped for the new release.
- CI: scripts/test_android_import.py (port of Volt content_files::validate) runs in GitHub Actions against all releases/*.hmod.
