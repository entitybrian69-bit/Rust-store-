#!/usr/bin/env python3
"""Build releases/biomes_unseen.hnmod (PC native source bundle).

Documented .hnmod contents (docs/INSTALL_PC.md): horizon-native.json +
mixins/<id>/ Rust source compiled by horizon-launcher on PC.
No horizon-mixins trait/macro names exist anywhere in this repo, so the Rust
source deliberately uses ONLY std + its own types (no invented Horizon API).
It carries the real biome/climate/feature data a native build needs; the
missing engine hook is tracked in docs/biomes_unseen/GAPS.md.
Android cannot compile .hnmod on-device; use biomes_unseen.hmod there.
"""
import hashlib, json, zipfile
from pathlib import Path
ROOT = Path(__file__).resolve().parent.parent

# name, temp, downfall, sky, fog, grass, water, weight, category
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
 # Nether
 ("crystalline_chasm",2.0,0.0,0x8a2a3a,0x4a0f1e,0xa04a6a,0xcf5a3a,4,"nether"),("infernal_dune",2.0,0.0,0x9a3a2a,0x5a1408,0xa05a3a,0xcf5a3a,5,"nether"),
 ("undergrowth",2.0,0.0,0x7a4a2a,0x3f1e0c,0x8a6a3a,0xcf5a3a,5,"nether"),("visceral_heap",2.0,0.0,0x8a3a4a,0x47121f,0xa04a5a,0xcf5a3a,4,"nether"),
 ("withered_abyss",2.0,0.0,0x4a4a5a,0x1c1c26,0x5a5a6a,0x8a3a3a,3,"nether"),
]

def rust():
    rows = ",\n    ".join(
        f'BiomeDef {{ id: "biomes_unseen:{n}", temperature: {t}, downfall: {d}, sky_color: 0x{s:06X}, fog_color: 0x{f:06X}, grass_color: 0x{g:06X}, water_color: 0x{w:06X}, weight: {wt}, category: "{c}" }}'
        for n,t,d,s,f,g,w,wt,c in BIOMES)
    return f'''//! Biomes Unseen — biome/climate data for a Horizon native (.hnmod) build.
//! std-only on purpose: no horizon-mixins trait names exist in the host repo,
//! so this file invents none. See docs/biomes_unseen/GAPS.md.
#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiomeDef {{
    pub id: &'static str, pub temperature: f32, pub downfall: f32,
    pub sky_color: u32, pub fog_color: u32, pub grass_color: u32,
    pub water_color: u32, pub weight: u32, pub category: &'static str,
}}

/// Per-biome toggles/weights (the configurable surface for a native build).
#[derive(Debug, Clone)]
pub struct Config {{ pub enabled: Vec<(&'static str, bool)>, pub weight_scale: f32 }}
impl Default for Config {{
    fn default() -> Self {{ Self {{ enabled: BIOMES.iter().map(|b| (b.id, true)).collect(), weight_scale: 1.0 }} }}
}}

pub static BIOMES: &[BiomeDef] = &[
    {rows}
];

pub fn enabled_biomes(cfg: &Config) -> Vec<&'static BiomeDef> {{
    BIOMES.iter().filter(|b| cfg.enabled.iter().find(|(id, _)| id == &b.id).map(|(_, e)| *e).unwrap_or(true)).collect()
}}

#[cfg(test)]
mod tests {{
    use super::*;
    #[test] fn ids_unique_and_namespaced() {{
        let mut ids: Vec<&str> = BIOMES.iter().map(|b| b.id).collect();
        let n = ids.len(); ids.sort_unstable(); ids.dedup();
        assert_eq!(n, ids.len()); assert!(ids.iter().all(|i| i.starts_with("biomes_unseen:")));
    }}
    #[test] fn default_config_enables_all() {{ assert_eq!(enabled_biomes(&Config::default()).len(), BIOMES.len()); }}
}}
'''

def main():
    manifest = {"id":"biomes_unseen","name":"Biomes Unseen","version":"1.0.0","side":"client",
      "description":"Native source bundle: biome/climate/feature data (std-only Rust). PC horizon-launcher only; Android use biomes_unseen.hmod.",
      "contents":["horizon-native.json","mixins/biomes_unseen/src/lib.rs","mixins/biomes_unseen/Cargo.toml"]}
    files = {
      "horizon-native.json": json.dumps(manifest, indent=2)+"\n",
      "mixins/biomes_unseen/Cargo.toml": '[package]\nname = "biomes_unseen"\nversion = "1.0.0"\nedition = "2021"\n\n[lib]\npath = "src/lib.rs"\n',
      "mixins/biomes_unseen/src/lib.rs": rust(),
    }
    out = ROOT/"releases/biomes_unseen.hnmod"; out.parent.mkdir(exist_ok=True)
    with zipfile.ZipFile(out,"w",zipfile.ZIP_DEFLATED,compresslevel=9) as zf:
        for name in sorted(files):
            zi = zipfile.ZipInfo(name, date_time=(2026,1,1,0,0,0)); zi.compress_type=zipfile.ZIP_DEFLATED; zi.external_attr=0o100644<<16
            zf.writestr(zi, files[name])
    print(out, out.stat().st_size, hashlib.sha256(out.read_bytes()).hexdigest(), f"biomes={len(BIOMES)}")

if __name__=="__main__": main()
