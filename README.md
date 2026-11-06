# 🦀 Rust-store- — Horizon Minecraft Engine `.hmod` Mod Store (PC & Android)

[![Validate Horizon .hmod Packages](https://github.com/entitybrian69-bit/Rust-store-/actions/workflows/blank.yml/badge.svg)](https://github.com/entitybrian69-bit/Rust-store-/actions/workflows/blank.yml)
![Target Engine](https://img.shields.io/badge/Engine-Horizon%20Rust%20(MC%2026.3)-dea584)
![Platforms](https://img.shields.io/badge/Platforms-PC%20%7C%20Android%2064--bit-2ea44f)
![Cost](https://img.shields.io/badge/Price-100%25%20Free%20(%240)-brightgreen)

Welcome to **Rust-store-** — a **100% free ($0)**, open-source GitHub repository catalog and `.hmod` store for **[Horizon](https://github.com/horizon-rust/horizon)** (Minecraft: Java Edition **26.3** rewritten from scratch in Rust) and **[Horizon Android](https://github.com/Vasysik/horizon-android)** (the native Rust/Vulkan Android port).

Every `.hmod` in this repository is pre-packaged, verified against the official [Horizon Mod File Spec (`docs/api/mod-files.md`)](https://github.com/horizon-rust/horizon/blob/main/docs/api/mod-files.md), pinned by **SHA-256**, and works on **both PC and Android** at zero cost.

---

## 📚 Quick Navigation

- [📦 Ready-to-Download `.hmod` Catalog](#-ready-to-download-hmod-catalog)
- [📲 One-Copy Direct HTTPS URLs for Horizon Android](#-one-copy-direct-https-urls-for-horizon-android)
- [🔍 Detailed Mod Breakdowns](#-detailed-mod-breakdowns)
- [🧭 Platform Compatibility Matrix (`.hmod` vs `.hnmod`)](#-platform-compatibility-matrix-hmod-vs-hnmod)
- [📖 Guides & Documentation](#-guides--documentation)
  - [Android Installation Guide (`docs/INSTALL_ANDROID.md`)](docs/INSTALL_ANDROID.md)
  - [PC Installation Guide (`docs/INSTALL_PC.md`)](docs/INSTALL_PC.md)
  - [Create & Submit `.hmod` Mods (`docs/CREATE_HMOD.md`)](docs/CREATE_HMOD.md)
  - [Upstream Horizon & Community Mod Directory (`catalog/COMMUNITY_MODS.md`)](catalog/COMMUNITY_MODS.md)

---

## 📦 Ready-to-Download `.hmod` Catalog

All packages below use **Horizon Content API `0.2`** (`capabilities = ["registries"]`, `side = "both"`) and run out of the box on **PC** (Windows, Linux, macOS) and **Android** (`arm64-v8a` / `x86_64`).

| Mod Name & ID | Version | Platforms | Adds | File Size | Download (`.hmod`) | Source Folder |
| :--- | :---: | :---: | :--- | :---: | :---: | :---: |
| **Block Placement Reborn**<br>`block_placement_reborn` | `1.0.0` | 🖥️ PC<br>📱 Android | Precision Placement Grid Block, Non-Occluding Ghost Preview Frame, Blueprint Placement Wand, plus Cross-Platform Java/Bedrock/Rust Core | `6,457 B` | [**Download `.hmod`**](releases/block_placement_reborn.hmod) | [`src-mods/block_placement_reborn`](src-mods/block_placement_reborn/) |
| **Rust Scrap & Base Armory**<br>`rust_scrap_armory` | `0.3.0` | 🖥️ PC<br>📱 Android | Sheet Metal & HQ Armored Walls, Scrap Metal, Salvaged Cleaver, Bean Can Grenade (Thrown Entity) | `9,592 B` | [**Download `.hmod`**](releases/rust_scrap_armory.hmod) | [`src-mods/rust_scrap_armory`](src-mods/rust_scrap_armory/) |
| **Glowing Amethyst**<br>`glowing_amethyst` | `0.2.0` | 🖥️ PC<br>📱 Android | Glowing Amethyst Block (Light 15), Chiseled Amethyst Lamp, Radiant Amethyst Shard | `6,655 B` | [**Download `.hmod`**](releases/glowing_amethyst.hmod) | [`src-mods/glowing_amethyst`](src-mods/glowing_amethyst/) |
| **Ruby Expansion**<br>`ruby_expansion` | `0.2.1` | 🖥️ PC<br>📱 Android | Ruby Ore, Block of Ruby, Ruby Gem, Crimson Ruby Blade, Ruby Vitality Status Effect | `7,319 B` | [**Download `.hmod`**](releases/ruby_expansion.hmod) | [`src-mods/ruby_expansion`](src-mods/ruby_expansion/) |
| **Industrial Pipes & Grates**<br>`industrial_pipes` | `0.2.0` | 🖥️ PC<br>📱 Android | 6-Way Connecting Copper Pipe & Heavy Steel Duct, Non-Occluding Factory Grate, Pipe Wrench, Sorter Menu | `8,852 B` | [**Download `.hmod`**](releases/industrial_pipes.hmod) | [`src-mods/industrial_pipes`](src-mods/industrial_pipes/) |
| **Dungeon Mobs & Frost Magic**<br>`dungeon_mobs_plus` | `0.2.0` | 🖥️ PC<br>📱 Android | Armored Dungeon Brute & Outpost Guard Mobs, Frost Orb Projectile, Frostbite Effect, Runic Frost Totem | `5,391 B` | [**Download `.hmod`**](releases/dungeon_mobs_plus.hmod) | [`src-mods/dungeon_mobs_plus`](src-mods/dungeon_mobs_plus/) |
| **Buzzing Apiary**<br>`buzzing_apiary` | `0.1.0` | 🖥️ PC<br>📱 Android | Apiary Hives, Beeswax Bricks, Honey Lamps (Light 15), Beeswax, Honeyed Bread, Beekeeper's Smoker, Bee Swarm Mob, Pollination Effect, Apiary Extractor Menu | `11,772 B` | [**Download `.hmod`**](releases/buzzing_apiary.hmod) | [`src-mods/buzzing_apiary`](src-mods/buzzing_apiary/) |

> **Machine-Readable Index:** [`catalog/mods-index.json`](catalog/mods-index.json) contains the full manifest metadata, entry counts, and SHA-256 hashes for all hosted `.hmod` files.

---

## 📲 One-Copy Direct HTTPS URLs for Horizon Android

In **Horizon Android** (`Vasysik/horizon-android` 64-bit APK), open **Launcher → Mods** and paste any of the direct HTTPS `.hmod` URLs below, then enable the package in **Installed** and **Options → Mods**:

### 0. Block Placement Reborn (`block_placement_reborn.hmod`)
```text
https://raw.githubusercontent.com/entitybrian69-bit/Rust-store-/main/releases/block_placement_reborn.hmod
```
- **SHA-256:** `b18242e470da87c0af0ad9285804c3d6ca3a9e73898f291e9b6cdf1c594685a4`

### 1. Rust Scrap & Base Armory (`rust_scrap_armory.hmod`)
```text
https://raw.githubusercontent.com/entitybrian69-bit/Rust-store-/main/releases/rust_scrap_armory.hmod
```
- **SHA-256:** `90bd06844c733ce3327bb44d98a7fe03b4fa942f7686082e517c839c50d05a1f`

### 2. Glowing Amethyst (`glowing_amethyst.hmod`)
```text
https://raw.githubusercontent.com/entitybrian69-bit/Rust-store-/main/releases/glowing_amethyst.hmod
```
- **SHA-256:** `4fd4efb7504c5ed74b17234023ef7603deb4a2ae3afae5f6797813d79c24d8a5`

### 3. Ruby Expansion (`ruby_expansion.hmod`)
```text
https://raw.githubusercontent.com/entitybrian69-bit/Rust-store-/main/releases/ruby_expansion.hmod
```
- **SHA-256:** `f2bb8de7c9dca0e297e5ee1aad5da239dc89d14d7efcbfa37e563cbe31b93531`

### 4. Industrial Pipes & Grates (`industrial_pipes.hmod`)
```text
https://raw.githubusercontent.com/entitybrian69-bit/Rust-store-/main/releases/industrial_pipes.hmod
```
- **SHA-256:** `c8beb67dbb1f78a79eb7b0f4512041ebfaad55b03d78588175c0ff135552ec6e`

### 5. Dungeon Mobs & Frost Magic (`dungeon_mobs_plus.hmod`)
```text
https://raw.githubusercontent.com/entitybrian69-bit/Rust-store-/main/releases/dungeon_mobs_plus.hmod
```
- **SHA-256:** `f331e42bdf7610f689e192e2ebcc175072d18f911bf2af35f35fe3beb23f00fb`

### 6. Buzzing Apiary (`buzzing_apiary.hmod`)
```text
https://raw.githubusercontent.com/entitybrian69-bit/Rust-store-/main/releases/buzzing_apiary.hmod
```
- **SHA-256:** `fbab71ff01e781b316038d87c738b503d4f9a62ef8087afdce58e987c4712677`

---

## 🔍 Detailed Mod Breakdowns

### 🛡️ 1. Rust Scrap & Base Armory (`rust_scrap_armory`)
Brings classic Rust base-building and scrap survival mechanics into Horizon 26.3:
- **Blocks:**
  - `rust_scrap_armory:sheet_metal_block` — *Reinforced Sheet Metal Block* (`hardness: 6.5`, `resistance: 12.0`, copper rivets texture)
  - `rust_scrap_armory:hq_armored_wall` — *High-Quality Armored Wall* (`hardness: 25.0`, `resistance: 600.0`, blast-resistant raid defense)
- **Items & Weapons:**
  - `rust_scrap_armory:scrap_metal` — Crafted from Iron + Copper ingots; base currency/material for Rust recipes
  - `rust_scrap_armory:salvaged_cleaver` — Heavy melee weapon (`max_damage: 650`, `+8.5 attack_damage`)
  - `rust_scrap_armory:beancan_grenade` — Throwable explosive (`max_stack_size: 16`)
- **Projectile Entity Type:**
  - `rust_scrap_armory:beancan_projectile` — Thrown projectile (`minecraft:snowball` template) dealing `6.0` impact damage
- **Creative Tab:** *Rust Scrap Armory* (`rust_scrap_armory:rust_armory`)

---

### 💜 2. Glowing Amethyst (`glowing_amethyst`)
Crystal lighting blocks and shards based on Horizon's content mod registry system:
- **Blocks:**
  - `glowing_amethyst:glowing_amethyst_block` — *Glowing Amethyst Block* (`light: 15`, `hardness: 1.8`, chime sounds)
  - `glowing_amethyst:amethyst_lamp` — *Chiseled Amethyst Lamp* (`light: 15`, `hardness: 1.5`)
- **Items:**
  - `glowing_amethyst:radiant_shard` — *Radiant Amethyst Shard* (crafted from Amethyst Shard + Glowstone Dust)
- **Creative Tab:** *Glowing Amethyst* (`glowing_amethyst:amethyst_tab`)

---

### ♦️ 3. Ruby Expansion (`ruby_expansion`)
Restores the classic Minecraft Ruby gem tier with weapons and status effects:
- **Blocks:**
  - `ruby_expansion:ruby_ore` — Drops `ruby_expansion:ruby` when mined
  - `ruby_expansion:ruby_block` — Compact storage block for 9 Rubies
- **Items & Weapons:**
  - `ruby_expansion:ruby` — Uncommon gemstone
  - `ruby_expansion:ruby_blade` — *Crimson Ruby Blade* (`max_damage: 1800`, `+7.5 attack_damage`, `rare` tier)
- **Status Effect:**
  - `ruby_expansion:ruby_vitality` — Beneficial status effect (`/effect give @s ruby_expansion:ruby_vitality 30`)
- **Creative Tab:** *Ruby Expansion* (`ruby_expansion:ruby_tab`)

---

### ⚙️ 4. Industrial Pipes & Grates (`industrial_pipes`)
Showcases Horizon API `0.2` 6-way pipe connection blocks (`minecraft:chorus_plant` template), non-occluding grates, and custom container menus:
- **Blocks:**
  - `industrial_pipes:copper_pipe` — 6-way connecting pipe (`thickness: 6`, connects to `#industrial_pipes:connectable_pipes` including Chests, Barrels, Hoppers, and other pipes)
  - `industrial_pipes:steel_duct` — Heavy 6-way duct (`thickness: 8`)
  - `industrial_pipes:factory_grate` — See-through floor grate (`occludes: false`)
- **Items & Menus:**
  - `industrial_pipes:pipe_wrench` — Handheld utility wrench
  - `industrial_pipes:item_sorter` — 9-slot custom Horizon `MenuType` (`slots: 9`, `data_slots: 2`, `results: [7, 8]`)
- **Creative Tab:** *Industrial Pipes* (`industrial_pipes:logistics_tab`)

---

### ❄️ 5. Dungeon Mobs & Frost Magic (`dungeon_mobs_plus`)
Adds custom living mob types, frost projectiles, and harmful status effects:
- **Living & Thrown Entity Types:**
  - `dungeon_mobs_plus:armored_brute` — Heavy Zombie variant with `45.0` Max Health (`/summon dungeon_mobs_plus:armored_brute`)
  - `dungeon_mobs_plus:outpost_guard` — Guard NPC built on `minecraft:wandering_trader` with `36.0` Max Health (`/summon dungeon_mobs_plus:outpost_guard`)
  - `dungeon_mobs_plus:frost_orb_projectile` — Thrown Frost Orb dealing `3.5` damage on hit
- **Blocks, Items & Status Effects:**
  - `dungeon_mobs_plus:runic_totem` — Glowing dungeon totem block (`light: 10`)
  - `dungeon_mobs_plus:frost_orb` — Throwable Frost Orb item
  - `dungeon_mobs_plus:frostbite` — Harmful frost status effect (`/effect give @s dungeon_mobs_plus:frostbite 30`)
- **Creative Tab:** *Dungeon Mobs+* (`dungeon_mobs_plus:dungeon_tab`)

---

### 🐝 6. Buzzing Apiary (`buzzing_apiary`)
A cozy, non-combat beekeeping expansion that turns Honeycomb into a full building and farming loop:
- **Blocks:**
  - `buzzing_apiary:apiary_hive` — *Apiary Hive* (`hardness: 2.0`, `resistance: 3.0`, `light: 3`, wood sounds)
  - `buzzing_apiary:beeswax_bricks` — *Beeswax Bricks* (`hardness: 1.2`, `resistance: 6.0`, decorative honeycomb-brick palette)
  - `buzzing_apiary:honey_lamp` — *Honey Lamp* (`light: 15`, `hardness: 0.6`, full-bright honey lighting)
- **Items:**
  - `buzzing_apiary:beeswax` — Crafted from 2 Honeycomb; core material for every recipe in the mod
  - `buzzing_apiary:honeyed_bread` — *Honeyed Bread* food item (`max_stack_size: 16`, `uncommon` tier)
  - `buzzing_apiary:beekeeper_smoker` — Handheld utility tool (`max_damage: 128`) for calming hives
- **Entity Type:**
  - `buzzing_apiary:bee_swarm` — Tame living Bee variant (`10.0` Max Health, `0.3` movement speed — `/summon buzzing_apiary:bee_swarm`)
- **Status Effect:**
  - `buzzing_apiary:pollination` — Beneficial status effect, honey-gold `#FFC30F` particle color (`/effect give @s buzzing_apiary:pollination 30`)
- **Menu:**
  - `buzzing_apiary:apiary_extractor` — 27-slot Horizon `MenuType` (`slots: 27`, `data_slots: 3`, `results: [25, 26]`) for honey extraction
- **Creative Tab:** *Buzzing Apiary* (`buzzing_apiary:apiary_tab`)

---

## 🧭 Platform Compatibility Matrix (`.hmod` vs `.hnmod`)

Horizon supports two primary mod formats across PC and Android:

| Mod Format | Extension | Contents | PC (`horizon-rust/horizon`) | Android (`Vasysik/horizon-android`) |
| :--- | :---: | :--- | :---: | :---: |
| **Content Mod (API `0.2`)** | `.hmod` | `horizon-mod.toml` + `assets/` + `data/` (blocks, items, recipes, tags, loot, creative tabs, mob effects, entity types, menus) | ✅ Supported (`horizon-mods/`) | ✅ Supported on 64-bit (`arm64-v8a`, `x86_64`) via HTTPS URL or Files app |
| **Native Mixin / Trait Mod** | `.hnmod` | `horizon-native.json` + `mixins/<id>/` (Rust source code compiled into the client) | ✅ Built by `horizon-launcher` (`horizon-native-mods/`) | ⚠️ Requires custom compiled APK (source-only `.hnmod` cannot compile on-device) |
| **Java Fabric / Forge / NeoForge** | `.jar` | Java bytecode & Mixins | ❌ Java code does not run (assets/data can be ported via `horizon-port-pack`) | ❌ Not supported |

---

## 🛠️ Building & Validating Locally ($0 Tooling)

This repository includes a zero-dependency Python 3 builder and spec validator (`scripts/build_hmods.py`) that enforces every rule from Horizon's [`docs/api/mod-files.md`](https://github.com/horizon-rust/horizon/blob/main/docs/api/mod-files.md):

```bash
# Regenerate all mod assets, pack releases/*.hmod, and update catalog/mods-index.json
python3 scripts/build_hmods.py

# Validate existing releases/*.hmod without modifying files (used in GitHub Actions CI)
python3 scripts/build_hmods.py --check-only
```

---

## 📖 Guides & Documentation

- 📱 **[Installing `.hmod` Mods on Android (`docs/INSTALL_ANDROID.md`)](docs/INSTALL_ANDROID.md)**
- 🖥️ **[Installing `.hmod` & `.hnmod` Mods on PC (`docs/INSTALL_PC.md`)](docs/INSTALL_PC.md)**
- 🧑‍💻 **[How to Create & Package `.hmod` Mods (`docs/CREATE_HMOD.md`)](docs/CREATE_HMOD.md)**
- 🌐 **[Upstream Horizon & Community Mod Directory (`catalog/COMMUNITY_MODS.md`)](catalog/COMMUNITY_MODS.md)**

---

## 📄 License

Released under the [MIT License](LICENSE). All mods in this repository are free to download, share, and remix.
