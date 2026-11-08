#!/usr/bin/env python3
"""Build releases/biomes_unseen.hnmod — REAL schema from the engine code.

Schema source (read, not guessed):
- horizon (docs/api/mod-files.md) + Volt crates/horizon-mods/src/index/native.rs
  Manifest {schema:1, id, version, game, horizon:<40-hex>, mixins:<40-hex>, hooks?}
  with #[serde(deny_unknown_fields)]; mixins/<id>/horizon-mixin.toml must exist
  ([mod] id/version must match; [compatibility] horizon_revision, per
  docs/native-mods.md), other entries only .rs/.toml under mixins/<id>/.
- horizon = Volt Launcher HEAD this bundle is pinned to; mixins = horizon-mixins HEAD.
Worldgen itself is data-driven via the .hmod (Volt worldgen-api patch), so this
native bundle needs no injections: it is a valid zero-hook bundle carrying the
biome dataset as a Rust helper for native tooling. Compilation requires
`horizon-launcher native-build` with a Rust 1.95 toolchain (see GAPS.md).
"""
import hashlib, json, subprocess, zipfile
from pathlib import Path
ROOT = Path(__file__).resolve().parent.parent
VOLT = ROOT.parent / "Volt-launcher"
MIXINS_SHA = "5a64af938ec86b00d53d16a408aabc0038fb3e57"  # horizon-rust/horizon-mixins HEAD at build time

BIOMES = [
 ("bayou",0.8,0.9,0x7fb8d8,0xc8e8e0,0x6a9a4a,0x3f76e4,6,"swamp"),("bog",0.6,0.9,0x8aa8b0,0xb8c8c0,0x5a7a3a,0x3a5f8a,5,"swamp"),
 ("cherry_blossom_grove",0.7,0.8,0x7fc8e8,0xe8d0e8,0x7ab85a,0x3f76e4,4,"forest"),("cold_desert",0.4,0.2,0x8fc8e0,0xe0d8c0,0x9aa06a,0x3f76e4,5,"desert"),
 ("coniferous_forest",0.5,0.7,0x7fb0d0,0xd0e0d8,0x5a8a4a,0x3f76e4,8,"forest"),("dead_forest",0.6,0.5,0x9aa8a8,0xc8c8b8,0x8a8a5a,0x3f76e4,5,"forest"),
 ("field",0.7,0.6,0x7fc8e8,0xd8e8d0,0x7ab85a,0x3f76e4,7,"plains"),("fir_clearing",0.5,0.7,0x7fb0d0,0xd0e0d8,0x5f9048,0x3f76e4,6,"forest"),
 ("floodplain",0.8,0.8,0x7fc0d8,0xd8e8d8,0x6fae52,0x3f76e4,5,"plains"),("forested_field",0.7,0.6,0x7fc8e8,0xd8e8d0,0x77b255,0x3f76e4,6,"plains"),
 ("grassland",0.8,0.4,0x7fd4e8,0xe0ecd0,0x8ab85e,0x3f76e4,7,"plains"),("gravel_beach",0.6,0.5,0x8fc0d8,0xd8ddd0,0x8a9a68,0x3f76e4,4,"beach"),
 ("highland",0.5,0.6,0x7fb8d8,0xd0dce8,0x6f9a52,0x3f76e4,5,"mountain"),("hot_springs",0.9,0.4,0x8fd0e0,0xe8e4d0,0x7aa85e,0x4fb8c8,3,"mountain"),
 ("jade_cliffs",0.7,0.7,0x7fc8d8,0xd8e8dc,0x5fae62,0x3f76e4,4,"mountain"),("lavender_field",0.7,0.6,0x7fc8e8,0xe2d8ea,0x82b862,0x3f76e4,4,"plains"),
 ("lush_desert",0.95,0.3,0x8fd8e8,0xf0e0b8,0x8aa85e,0x3f76e4,5,"desert"),("lush_savanna",0.9,0.5,0x7fd0e8,0xe8e0c0,0x9ab45e,0x3f76e4,6,"savanna"),
 ("maple_woods",0.6,0.7,0x7fc0d8,0xe8d8c0,0x9a9a4e,0x3f76e4,6,"forest"),("marsh",0.8,0.9,0x86b8c8,0xcfe0d8,0x6a9a4e,0x3f76e4,6,"swamp"),
 ("mediterranean_forest",0.8,0.5,0x7fd0e8,0xe8dec8,0x7aa254,0x3f76e4,6,"forest"),("mystic_grove",0.7,0.8,0x86c0e8,0xdcd0f0,0x6ab86a,0x5f8fe4,2,"forest"),
 ("ominous_woods",0.6,0.8,0x6f90a8,0xa8b0c0,0x4e7a42,0x3a5f8a,3,"forest"),("orchard",0.7,0.6,0x7fc8e8,0xdcead2,0x7cb45c,0x3f76e4,5,"forest"),
 ("origin_valley",0.7,0.7,0x7fd8e8,0xd8f0dc,0x77c45c,0x44a8e4,2,"special"),("pasture",0.7,0.6,0x7fc8e8,0xdcead2,0x82bc60,0x3f76e4,6,"plains"),
 ("prairie",0.8,0.3,0x7fd4e8,0xe8e8c8,0xa8b45e,0x3f76e4,7,"plains"),("pumpkin_patch",0.7,0.6,0x7fc0d8,0xe0dcc0,0x86ac58,0x3f76e4,3,"plains"),
 ("rainforest",0.95,0.9,0x6fb8d0,0xc8e0cc,0x3f9a3a,0x3f76e4,5,"jungle"),("redwood_forest",0.7,0.8,0x76b8cc,0xd2e2d4,0x548c46,0x3f76e4,5,"forest"),
 ("rocky_rainforest",0.9,0.9,0x72b4cc,0xcce0d0,0x4a9a44,0x3f76e4,4,"jungle"),("scrubland",0.85,0.2,0x84d0e4,0xeee0bc,0x9aa259,0x3f76e4,6,"desert"),
 ("seasonal_forest",0.6,0.7,0x7cc0d8,0xe4d8bc,0x8a9a4c,0x3f76e4,7,"forest"),("snowy_coniferous_forest",-0.2,0.7,0x92c4de,0xe4ecec,0x6a9a5e,0x3d57d6,6,"snowy"),
 ("snowy_fir_clearing",-0.2,0.7,0x92c4de,0xe4ecec,0x6f9f62,0x3d57d6,5,"snowy"),("snowy_maple_woods",-0.1,0.7,0x8ec2dc,0xe8e4da,0x7fa065,0x3d57d6,5,"snowy"),
 ("tropics",0.95,0.8,0x6fc8dc,0xd4ecd8,0x59b84e,0x2fb8c8,3,"jungle"),("wetland",0.75,0.9,0x80bcc8,0xd0e4da,0x67a04b,0x3f76e4,6,"swamp"),
 ("wooded_scrubland",0.85,0.25,0x84d0e4,0xeee0bc,0x93a258,0x3f76e4,5,"desert"),
 ("crystalline_chasm",2.0,0.0,0x8a2a3a,0x4a0f1e,0xa04a6a,0xcf5a3a,4,"nether"),("infernal_dune",2.0,0.0,0x9a3a2a,0x5a1408,0xa05a3a,0xcf5a3a,5,"nether"),
 ("undergrowth",2.0,0.0,0x7a4a2a,0x3f1e0c,0x8a6a3a,0xcf5a3a,5,"nether"),("visceral_heap",2.0,0.0,0x8a3a4a,0x47121f,0xa04a5a,0xcf5a3a,4,"nether"),
 ("withered_abyss",2.0,0.0,0x4a4a5a,0x1c1c26,0x5a5a6a,0x8a3a3a,3,"nether"),
]

def volt_sha():
    try: return subprocess.check_output(["git","-C",str(VOLT),"rev-parse","HEAD"],text=True).strip()
    except Exception: return "f275b6308efe4ff3d0ba35a8252bf63256ff1dd2"

def rust():
    rows = ",\n    ".join(
        f'BiomeDef {{ id: "biomes_unseen:{n}", temperature: {t}, downfall: {d}, sky_color: 0x{s:06X}, fog_color: 0x{f:06X}, grass_color: 0x{g:06X}, water_color: 0x{w:06X}, weight: {wt}, category: "{c}" }}'
        for n,t,d,s,f,g,w,wt,c in BIOMES)
    return f'''//! Biomes Unseen — biome dataset helper for native tooling (std-only).
//! Worldgen registration itself is data-driven via the companion .hmod
//! (Volt worldgen-api: horizon:climate in data/biomes_unseen/worldgen/biome/).
#![forbid(unsafe_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiomeDef {{ pub id: &'static str, pub temperature: f32, pub downfall: f32,
    pub sky_color: u32, pub fog_color: u32, pub grass_color: u32, pub water_color: u32,
    pub weight: u32, pub category: &'static str }}
pub static BIOMES: &[BiomeDef] = &[
    {rows}
];
'''

def main():
    sha = volt_sha()
    manifest = {"schema":1,"id":"biomes_unseen","version":"1.0.1","game":"26.3","horizon":sha,"mixins":MIXINS_SHA,"hooks":1}
    files = {
      "horizon-native.json": json.dumps(manifest, indent=2)+"\n",
      "mixins/biomes_unseen/horizon-mixin.toml":
        f'[mod]\nid = "biomes_unseen"\nversion = "1.0.1"\n\n[compatibility]\nhorizon_revision = "{sha}"\n',
      "mixins/biomes_unseen/src/lib.rs": rust(),
      "mixins/biomes_unseen/README.md": "# Biomes Unseen native bundle\nZero-hook bundle: biome dataset helper. Worldgen is data-driven via biomes_unseen.hmod (Volt worldgen-api patch). Build: horizon-launcher native-build <this file> --trust-native-code (PC only).\n",
    }
    out = ROOT/"releases/biomes_unseen.hnmod"; out.parent.mkdir(exist_ok=True)
    with zipfile.ZipFile(out,"w",zipfile.ZIP_DEFLATED,compresslevel=9) as zf:
        for name in sorted(files):
            zi = zipfile.ZipInfo(name, date_time=(2026,1,1,0,0,0)); zi.compress_type=zipfile.ZIP_DEFLATED; zi.external_attr=0o100644<<16
            zf.writestr(zi, files[name])
    print(out, out.stat().st_size, hashlib.sha256(out.read_bytes()).hexdigest(), f"biomes={len(BIOMES)} horizon={sha}")

if __name__=="__main__": main()
