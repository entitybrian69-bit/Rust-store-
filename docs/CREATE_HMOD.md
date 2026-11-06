# 🧑‍💻 Creating, Building & Validating Horizon `.hmod` Mods

This guide explains how to build new `.hmod` mods for **Horizon (Minecraft 26.3 Rust Engine)** and submit them to **Rust-store-** using 100% free tools.

---

## 1. Anatomy of a `.hmod` File

According to Horizon's [`docs/api/mod-files.md`](https://github.com/horizon-rust/horizon/blob/main/docs/api/mod-files.md), a `.hmod` is a standard ZIP archive (using `stored` or `deflate` compression) with `/`-separated relative paths and no directory entries:

```text
my_mod.hmod (ZIP archive)
├── horizon-mod.toml
├── assets/
│   └── my_mod/
│       ├── blockstates/
│       │   └── my_block.json
│       ├── lang/
│       │   └── en_us.json
│       ├── models/
│       │   ├── block/
│       │   │   └── my_block.json
│       │   └── item/
│       │       ├── my_block.json
│       │       └── my_item.json
│       └── textures/
│           ├── block/
│           │   └── my_block.png
│           └── item/
│               └── my_item.png
└── data/
    ├── minecraft/
    │   └── tags/
    │       └── block/
    │           └── mineable/
    │               └── pickaxe.json
    └── my_mod/
        ├── horizon/
        │   ├── block/
        │   │   └── my_block.json
        │   ├── item/
        │   │   └── my_item.json
        │   ├── creative_tab/
        │   │   └── main_tab.json
        │   ├── mob_effect/
        │   │   └── custom_effect.json
        │   ├── entity_type/
        │   │   └── custom_mob.json
        │   └── menu/
        │       └── custom_menu.json
        ├── loot_table/
        │   └── blocks/
        │       └── my_block.json
        └── recipe/
            └── my_block.json
```

---

## 2. The `horizon-mod.toml` Manifest

Every content mod requires a top-level `horizon-mod.toml` with no unknown keys:

```toml
id = "my_mod"                # Must match ^[a-z0-9_]{1,64}$
name = "My Mod"
version = "0.1.0"
description = "Adds custom blocks and items to Horizon 26.3."
authors = ["YourName"]
api = "0.2"                  # Required for content registries
side = "both"                # "both", "client", or "server"
capabilities = ["registries"]
```

---

## 3. Declaring Blocks, Items, Entities & Effects (API `0.2`)

### Custom Block (`data/<id>/horizon/block/<name>.json`)
Copies a vanilla block template and overrides specific properties:
```json
{
  "template": "minecraft:iron_block",
  "properties": {
    "hardness": 4.0,
    "resistance": 8.0,
    "light": 7,
    "sounds": "minecraft:amethyst_block"
  },
  "item": true
}
```
- **6-Way Connecting Pipes:** Use `"template": "minecraft:chorus_plant"`, `"connects_to": "#<id>:<tag>"`, and optional `"thickness": 6` (2 to 14 pixels).
- **Non-Occluding Blocks (Grates/Panels):** Set `"occludes": false`.

### Custom Item / Weapon (`data/<id>/horizon/item/<name>.json`)
Copies a vanilla item template and patches 26.3 data components:
```json
{
  "template": "minecraft:iron_sword",
  "components": {
    "minecraft:max_damage": 900,
    "minecraft:rarity": "uncommon",
    "minecraft:attribute_modifiers": [
      {
        "type": "minecraft:attack_damage",
        "id": "minecraft:base_attack_damage",
        "amount": 7.5,
        "operation": "add_value",
        "slot": "mainhand"
      }
    ]
  }
}
```

### Custom Projectile or Living Mob (`data/<id>/horizon/entity_type/<name>.json`)
```json
{
  "template": "minecraft:snowball",
  "item": "my_mod:throwing_star",
  "damage": 4.0
}
```
Or a living mob (`minecraft:zombie`, `minecraft:villager`, or `minecraft:wandering_trader`):
```json
{
  "template": "minecraft:zombie",
  "attributes": {
    "minecraft:max_health": 40.0,
    "minecraft:movement_speed": 0.3
  }
}
```

---

## 4. Building & Checking Mods

### Option A: Using This Repo's Zero-Dependency Python Builder (`scripts/build_hmods.py`)
Place your mod folder under `src-mods/<id>/` and run:
```bash
python3 scripts/build_hmods.py
```
This automatically packages `releases/<id>.hmod`, validates every path, manifest key, JSON file, and PNG texture against Horizon's rules, and updates `catalog/mods-index.json` with the new SHA-256 hash.

### Option B: Using Upstream `cargo-horizon-mod`
If you have Rust installed, you can also use Horizon's official CLI:
```bash
cargo install --git https://github.com/horizon-rust/horizon cargo-horizon-mod
cargo horizon-mod check releases/rust_scrap_armory.hmod
```
