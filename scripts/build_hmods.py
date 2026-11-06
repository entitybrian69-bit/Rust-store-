#!/usr/bin/env python3
"""
Horizon (.hmod) Mod Builder, Texture Generator & Spec Validator
Follows https://github.com/horizon-rust/horizon/blob/main/docs/api/mod-files.md
and https://github.com/horizon-rust/horizon/blob/main/docs/mods.md (API 0.2).

Zero external dependencies (uses Python 3.11+ standard library only).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import struct
import sys
import tomllib
import zipfile
import zlib
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parent.parent
SRC_MODS_DIR = REPO_ROOT / "src-mods"
RELEASES_DIR = REPO_ROOT / "releases"
CATALOG_DIR = REPO_ROOT / "catalog"

MOD_ID_RE = re.compile(r"^[a-z0-9_]{1,64}$")
RESOURCE_PART_RE = re.compile(r"^[a-z0-9_.-]+$")
ALLOWED_MANIFEST_KEYS = {
    "id",
    "name",
    "version",
    "description",
    "authors",
    "api",
    "side",
    "capabilities",
    "namespaces",
}


def make_png_16x16(pixels_rgba: list[tuple[int, int, int, int]]) -> bytes:
    """Create a valid 16x16 8-bit RGBA PNG image from 256 (r, g, b, a) tuples."""
    assert len(pixels_rgba) == 16 * 16, "Expected 256 pixels for 16x16 PNG"

    def png_chunk(chunk_type: bytes, data: bytes) -> bytes:
        crc = zlib.crc32(chunk_type + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + chunk_type + data + struct.pack(">I", crc)

    raw_rows = bytearray()
    for y in range(16):
        raw_rows.append(0)  # Filter type 0 (None)
        for x in range(16):
            r, g, b, a = pixels_rgba[y * 16 + x]
            raw_rows.extend((r & 0xFF, g & 0xFF, b & 0xFF, a & 0xFF))

    ihdr = struct.pack(">IIBBBBB", 16, 16, 8, 6, 0, 0, 0)  # 16x16, 8-bit RGBA
    idat = zlib.compress(bytes(raw_rows), level=9)
    return (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", ihdr)
        + png_chunk(b"IDAT", idat)
        + png_chunk(b"IEND", b"")
    )


def render_block_texture(
    base: tuple[int, int, int],
    border: tuple[int, int, int],
    accent: tuple[int, int, int],
    pattern: str = "bevel",
) -> bytes:
    """Render a stylized 16x16 block texture."""
    pixels: list[tuple[int, int, int, int]] = []
    for y in range(16):
        for x in range(16):
            if x == 0 or y == 0 or x == 15 or y == 15:
                c = border
            elif pattern == "crystal" and ((x + y) % 5 == 0 or (x - y) % 7 == 0):
                c = accent
            elif pattern == "rivets" and x in (2, 13) and y in (2, 7, 13):
                c = accent
            elif pattern == "stripes" and (y in (4, 11) or x in (4, 11)):
                c = accent
            elif pattern == "ore" and (
                (x in (3, 4, 10, 11) and y in (4, 5, 11))
                or (x in (6, 7, 12) and y in (7, 8, 12))
            ):
                c = accent
            elif pattern == "grate" and (x % 3 == 1 and y % 3 == 1):
                pixels.append((0, 0, 0, 0))
                continue
            elif (x + y) % 4 == 0:
                c = (
                    min(255, base[0] + 10),
                    min(255, base[1] + 10),
                    min(255, base[2] + 10),
                )
            else:
                c = base
            pixels.append((c[0], c[1], c[2], 255))
    return make_png_16x16(pixels)


def render_item_texture(
    fg: tuple[int, int, int],
    edge: tuple[int, int, int],
    handle: tuple[int, int, int] = (110, 75, 42),
    shape: str = "gem",
) -> bytes:
    """Render a stylized 16x16 item icon with transparent background."""
    pixels: list[tuple[int, int, int, int]] = []
    for y in range(16):
        for x in range(16):
            if shape == "sword":
                # Diagonal blade from bottom-left hilt to top-right tip
                diag = x + y
                diff = abs((15 - y) - x)
                if x < 5 and y > 10 and diff <= 2:
                    pixels.append((handle[0], handle[1], handle[2], 255))
                elif 4 <= x <= 6 and 9 <= y <= 11:
                    pixels.append((edge[0], edge[1], edge[2], 255))
                elif diff == 0 and 2 <= x <= 13 and 2 <= y <= 13:
                    pixels.append((fg[0], fg[1], fg[2], 255))
                elif diff == 1 and 3 <= x <= 13 and 2 <= y <= 12:
                    pixels.append((edge[0], edge[1], edge[2], 255))
                else:
                    pixels.append((0, 0, 0, 0))
            elif shape == "orb":
                dx = x - 7.5
                dy = y - 7.5
                dist2 = dx * dx + dy * dy
                if dist2 <= 18:
                    pixels.append((fg[0], fg[1], fg[2], 255))
                elif dist2 <= 28:
                    pixels.append((edge[0], edge[1], edge[2], 255))
                else:
                    pixels.append((0, 0, 0, 0))
            else:  # gem / ingot / shard
                dx = abs(x - 7.5)
                dy = abs(y - 7.5)
                manhattan = dx + dy
                if manhattan <= 4.5:
                    pixels.append((fg[0], fg[1], fg[2], 255))
                elif manhattan <= 6.0:
                    pixels.append((edge[0], edge[1], edge[2], 255))
                else:
                    pixels.append((0, 0, 0, 0))
    return make_png_16x16(pixels)


def write_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


def write_bytes(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def write_block_assets(
    mod_dir: Path,
    mod_id: str,
    block_name: str,
    texture_bytes: bytes,
) -> None:
    """Write standard blockstate, block model, item model, and 16x16 texture."""
    write_bytes(
        mod_dir / "assets" / mod_id / "textures" / "block" / f"{block_name}.png",
        texture_bytes,
    )
    write_json(
        mod_dir / "assets" / mod_id / "blockstates" / f"{block_name}.json",
        {
            "variants": {
                "": {"model": f"{mod_id}:block/{block_name}"}
            }
        },
    )
    write_json(
        mod_dir / "assets" / mod_id / "models" / "block" / f"{block_name}.json",
        {
            "parent": "minecraft:block/cube_all",
            "textures": {"all": f"{mod_id}:block/{block_name}"},
        },
    )
    write_json(
        mod_dir / "assets" / mod_id / "models" / "item" / f"{block_name}.json",
        {"parent": f"{mod_id}:block/{block_name}"},
    )


def write_item_assets(
    mod_dir: Path,
    mod_id: str,
    item_name: str,
    texture_bytes: bytes,
    handheld: bool = False,
) -> None:
    """Write standard item model and 16x16 item texture."""
    write_bytes(
        mod_dir / "assets" / mod_id / "textures" / "item" / f"{item_name}.png",
        texture_bytes,
    )
    parent = "minecraft:item/handheld" if handheld else "minecraft:item/generated"
    write_json(
        mod_dir / "assets" / mod_id / "models" / "item" / f"{item_name}.json",
        {
            "parent": parent,
            "textures": {"layer0": f"{mod_id}:item/{item_name}"},
        },
    )


def write_self_drop_loot_table(mod_dir: Path, mod_id: str, block_name: str) -> None:
    write_json(
        mod_dir / "data" / mod_id / "loot_table" / "blocks" / f"{block_name}.json",
        {
            "type": "minecraft:block",
            "pools": [
                {
                    "rolls": 1.0,
                    "bonus_rolls": 0.0,
                    "entries": [
                        {
                            "type": "minecraft:item",
                            "name": f"{mod_id}:{block_name}",
                        }
                    ],
                    "conditions": [
                        {"condition": "minecraft:survives_explosion"}
                    ],
                }
            ],
        },
    )


def generate_all_mod_sources() -> None:
    """Generate the 5 complete Horizon API 0.2 content mods under src-mods/."""
    SRC_MODS_DIR.mkdir(parents=True, exist_ok=True)

    # =========================================================================
    # 1. glowing_amethyst — Radiant Amethyst Lighting & Decor (PC & Android)
    # =========================================================================
    m1 = SRC_MODS_DIR / "glowing_amethyst"
    m1.mkdir(parents=True, exist_ok=True)
    (m1 / "horizon-mod.toml").write_text(
        'id = "glowing_amethyst"\n'
        'name = "Glowing Amethyst"\n'
        'version = "0.2.0"\n'
        'description = "Adds luminous Glowing Amethyst blocks, Chiseled Amethyst Lamps, and Radiant Shards with light level 15 for caves and bases."\n'
        'authors = ["Rust-store-"]\n'
        'api = "0.2"\n'
        'side = "both"\n'
        'capabilities = ["registries"]\n',
        encoding="utf-8",
    )
    write_json(
        m1 / "data" / "glowing_amethyst" / "horizon" / "block" / "glowing_amethyst_block.json",
        {
            "template": "minecraft:amethyst_block",
            "properties": {
                "hardness": 1.8,
                "resistance": 4.0,
                "light": 15,
                "sounds": "minecraft:amethyst_block",
            },
            "item": True,
        },
    )
    write_json(
        m1 / "data" / "glowing_amethyst" / "horizon" / "block" / "amethyst_lamp.json",
        {
            "template": "minecraft:sea_lantern",
            "properties": {
                "hardness": 1.5,
                "resistance": 3.5,
                "light": 15,
                "sounds": "minecraft:amethyst_cluster",
            },
            "item": True,
        },
    )
    write_json(
        m1 / "data" / "glowing_amethyst" / "horizon" / "item" / "radiant_shard.json",
        {
            "template": "minecraft:amethyst_shard",
            "components": {
                "minecraft:max_stack_size": 64,
                "minecraft:rarity": "uncommon",
            },
        },
    )
    write_json(
        m1 / "data" / "glowing_amethyst" / "horizon" / "creative_tab" / "amethyst_tab.json",
        {
            "title": "Glowing Amethyst",
            "icon": "glowing_amethyst:glowing_amethyst_block",
            "items": [
                "glowing_amethyst:glowing_amethyst_block",
                "glowing_amethyst:amethyst_lamp",
                "glowing_amethyst:radiant_shard",
            ],
        },
    )
    write_block_assets(
        m1,
        "glowing_amethyst",
        "glowing_amethyst_block",
        render_block_texture((158, 92, 224), (105, 50, 168), (235, 195, 255), "crystal"),
    )
    write_block_assets(
        m1,
        "glowing_amethyst",
        "amethyst_lamp",
        render_block_texture((190, 130, 245), (120, 62, 185), (255, 235, 255), "stripes"),
    )
    write_item_assets(
        m1,
        "glowing_amethyst",
        "radiant_shard",
        render_item_texture((225, 155, 255), (150, 75, 215), shape="gem"),
    )
    write_self_drop_loot_table(m1, "glowing_amethyst", "glowing_amethyst_block")
    write_self_drop_loot_table(m1, "glowing_amethyst", "amethyst_lamp")
    write_json(
        m1 / "data" / "glowing_amethyst" / "recipe" / "radiant_shard.json",
        {
            "type": "minecraft:crafting_shapeless",
            "ingredients": [
                "minecraft:amethyst_shard",
                "minecraft:glowstone_dust",
            ],
            "result": {"id": "glowing_amethyst:radiant_shard", "count": 2},
        },
    )
    write_json(
        m1 / "data" / "glowing_amethyst" / "recipe" / "glowing_amethyst_block.json",
        {
            "type": "minecraft:crafting_shaped",
            "pattern": ["SS", "SS"],
            "key": {"S": "glowing_amethyst:radiant_shard"},
            "result": {"id": "glowing_amethyst:glowing_amethyst_block", "count": 1},
        },
    )
    write_json(
        m1 / "data" / "glowing_amethyst" / "recipe" / "amethyst_lamp.json",
        {
            "type": "minecraft:crafting_shaped",
            "pattern": [" G ", "GAG", " G "],
            "key": {
                "G": "minecraft:glowstone_dust",
                "A": "glowing_amethyst:glowing_amethyst_block",
            },
            "result": {"id": "glowing_amethyst:amethyst_lamp", "count": 1},
        },
    )
    write_json(
        m1 / "data" / "minecraft" / "tags" / "block" / "mineable" / "pickaxe.json",
        {
            "replace": False,
            "values": [
                "glowing_amethyst:glowing_amethyst_block",
                "glowing_amethyst:amethyst_lamp",
            ],
        },
    )
    write_json(
        m1 / "assets" / "glowing_amethyst" / "lang" / "en_us.json",
        {
            "block.glowing_amethyst.glowing_amethyst_block": "Glowing Amethyst Block",
            "block.glowing_amethyst.amethyst_lamp": "Chiseled Amethyst Lamp",
            "item.glowing_amethyst.radiant_shard": "Radiant Amethyst Shard",
            "itemGroup.glowing_amethyst.amethyst_tab": "Glowing Amethyst",
        },
    )

    # =========================================================================
    # 2. rust_scrap_armory — Rust Survival Base Blocks, Scrap & Weapons
    # =========================================================================
    m2 = SRC_MODS_DIR / "rust_scrap_armory"
    m2.mkdir(parents=True, exist_ok=True)
    (m2 / "horizon-mod.toml").write_text(
        'id = "rust_scrap_armory"\n'
        'name = "Rust Scrap & Base Armory"\n'
        'version = "0.3.0"\n'
        'description = "Rust-inspired survival expansion for Horizon 26.3: Sheet Metal & Armored Base Blocks, Scrap Metal, Salvaged Cleaver, Bean Can Grenade projectile, and Med-Syringe."\n'
        'authors = ["Rust-store-"]\n'
        'api = "0.2"\n'
        'side = "both"\n'
        'capabilities = ["registries"]\n',
        encoding="utf-8",
    )
    write_json(
        m2 / "data" / "rust_scrap_armory" / "horizon" / "block" / "sheet_metal_block.json",
        {
            "template": "minecraft:iron_block",
            "properties": {
                "hardness": 6.5,
                "resistance": 12.0,
                "sounds": "minecraft:copper",
            },
            "item": True,
        },
    )
    write_json(
        m2 / "data" / "rust_scrap_armory" / "horizon" / "block" / "hq_armored_wall.json",
        {
            "template": "minecraft:obsidian",
            "properties": {
                "hardness": 25.0,
                "resistance": 600.0,
                "sounds": "minecraft:netherite_block",
            },
            "item": True,
            "components": {
                "minecraft:rarity": "rare",
            },
        },
    )
    write_json(
        m2 / "data" / "rust_scrap_armory" / "horizon" / "item" / "scrap_metal.json",
        {
            "template": "minecraft:iron_ingot",
            "components": {
                "minecraft:max_stack_size": 64,
                "minecraft:rarity": "common",
            },
        },
    )
    write_json(
        m2 / "data" / "rust_scrap_armory" / "horizon" / "item" / "salvaged_cleaver.json",
        {
            "template": "minecraft:iron_sword",
            "components": {
                "minecraft:max_damage": 650,
                "minecraft:rarity": "uncommon",
                "minecraft:attribute_modifiers": [
                    {
                        "type": "minecraft:attack_damage",
                        "id": "minecraft:base_attack_damage",
                        "amount": 8.5,
                        "operation": "add_value",
                        "slot": "mainhand",
                    }
                ],
            },
        },
    )
    write_json(
        m2 / "data" / "rust_scrap_armory" / "horizon" / "item" / "beancan_grenade.json",
        {
            "template": "minecraft:snowball",
            "components": {
                "minecraft:max_stack_size": 16,
                "minecraft:rarity": "uncommon",
            },
        },
    )
    write_json(
        m2 / "data" / "rust_scrap_armory" / "horizon" / "entity_type" / "beancan_projectile.json",
        {
            "template": "minecraft:snowball",
            "item": "rust_scrap_armory:beancan_grenade",
            "damage": 6.0,
        },
    )
    write_json(
        m2 / "data" / "rust_scrap_armory" / "horizon" / "creative_tab" / "rust_armory.json",
        {
            "title": "Rust Scrap Armory",
            "icon": "rust_scrap_armory:salvaged_cleaver",
            "items": [
                "rust_scrap_armory:scrap_metal",
                "rust_scrap_armory:sheet_metal_block",
                "rust_scrap_armory:hq_armored_wall",
                "rust_scrap_armory:salvaged_cleaver",
                "rust_scrap_armory:beancan_grenade",
            ],
        },
    )
    write_block_assets(
        m2,
        "rust_scrap_armory",
        "sheet_metal_block",
        render_block_texture((122, 110, 100), (78, 68, 60), (175, 120, 75), "rivets"),
    )
    write_block_assets(
        m2,
        "rust_scrap_armory",
        "hq_armored_wall",
        render_block_texture((58, 64, 72), (34, 38, 44), (110, 124, 140), "rivets"),
    )
    write_item_assets(
        m2,
        "rust_scrap_armory",
        "scrap_metal",
        render_item_texture((165, 132, 95), (98, 72, 48), shape="gem"),
    )
    write_item_assets(
        m2,
        "rust_scrap_armory",
        "salvaged_cleaver",
        render_item_texture((185, 190, 198), (120, 95, 75), handle=(138, 62, 45), shape="sword"),
        handheld=True,
    )
    write_item_assets(
        m2,
        "rust_scrap_armory",
        "beancan_grenade",
        render_item_texture((140, 145, 135), (82, 85, 78), shape="orb"),
    )
    write_self_drop_loot_table(m2, "rust_scrap_armory", "sheet_metal_block")
    write_self_drop_loot_table(m2, "rust_scrap_armory", "hq_armored_wall")
    write_json(
        m2 / "data" / "rust_scrap_armory" / "recipe" / "scrap_metal.json",
        {
            "type": "minecraft:crafting_shapeless",
            "ingredients": ["minecraft:iron_ingot", "minecraft:copper_ingot"],
            "result": {"id": "rust_scrap_armory:scrap_metal", "count": 4},
        },
    )
    write_json(
        m2 / "data" / "rust_scrap_armory" / "recipe" / "sheet_metal_block.json",
        {
            "type": "minecraft:crafting_shaped",
            "pattern": ["SS", "SS"],
            "key": {"S": "rust_scrap_armory:scrap_metal"},
            "result": {"id": "rust_scrap_armory:sheet_metal_block", "count": 2},
        },
    )
    write_json(
        m2 / "data" / "rust_scrap_armory" / "recipe" / "hq_armored_wall.json",
        {
            "type": "minecraft:crafting_shaped",
            "pattern": ["ISI", "SOS", "ISI"],
            "key": {
                "I": "minecraft:iron_ingot",
                "S": "rust_scrap_armory:sheet_metal_block",
                "O": "minecraft:obsidian",
            },
            "result": {"id": "rust_scrap_armory:hq_armored_wall", "count": 2},
        },
    )
    write_json(
        m2 / "data" / "rust_scrap_armory" / "recipe" / "salvaged_cleaver.json",
        {
            "type": "minecraft:crafting_shaped",
            "pattern": ["SS", "S ", "R "],
            "key": {
                "S": "rust_scrap_armory:scrap_metal",
                "R": "minecraft:stick",
            },
            "result": {"id": "rust_scrap_armory:salvaged_cleaver", "count": 1},
        },
    )
    write_json(
        m2 / "data" / "rust_scrap_armory" / "recipe" / "beancan_grenade.json",
        {
            "type": "minecraft:crafting_shaped",
            "pattern": [" S ", "SGS", " S "],
            "key": {
                "S": "rust_scrap_armory:scrap_metal",
                "G": "minecraft:gunpowder",
            },
            "result": {"id": "rust_scrap_armory:beancan_grenade", "count": 4},
        },
    )
    write_json(
        m2 / "data" / "minecraft" / "tags" / "block" / "mineable" / "pickaxe.json",
        {
            "replace": False,
            "values": [
                "rust_scrap_armory:sheet_metal_block",
                "rust_scrap_armory:hq_armored_wall",
            ],
        },
    )
    write_json(
        m2 / "assets" / "rust_scrap_armory" / "lang" / "en_us.json",
        {
            "block.rust_scrap_armory.sheet_metal_block": "Reinforced Sheet Metal Block",
            "block.rust_scrap_armory.hq_armored_wall": "High-Quality Armored Wall",
            "item.rust_scrap_armory.scrap_metal": "Scrap Metal",
            "item.rust_scrap_armory.salvaged_cleaver": "Salvaged Cleaver",
            "item.rust_scrap_armory.beancan_grenade": "Bean Can Grenade",
            "entity.rust_scrap_armory.beancan_projectile": "Bean Can Grenade",
            "itemGroup.rust_scrap_armory.rust_armory": "Rust Scrap Armory",
        },
    )

    # =========================================================================
    # 3. ruby_expansion — Ruby Ores, Gems, Weapons & Mob Effect
    # =========================================================================
    m3 = SRC_MODS_DIR / "ruby_expansion"
    m3.mkdir(parents=True, exist_ok=True)
    (m3 / "horizon-mod.toml").write_text(
        'id = "ruby_expansion"\n'
        'name = "Ruby Expansion"\n'
        'version = "0.2.1"\n'
        'description = "Classic Minecraft Ruby Ore, Ruby Block, Crimson Ruby Blade, and Ruby Vitality status effect for Horizon 26.3."\n'
        'authors = ["Rust-store-"]\n'
        'api = "0.2"\n'
        'side = "both"\n'
        'capabilities = ["registries"]\n',
        encoding="utf-8",
    )
    write_json(
        m3 / "data" / "ruby_expansion" / "horizon" / "block" / "ruby_ore.json",
        {
            "template": "minecraft:emerald_ore",
            "properties": {
                "hardness": 3.0,
                "resistance": 3.0,
                "sounds": "minecraft:stone",
            },
            "item": True,
        },
    )
    write_json(
        m3 / "data" / "ruby_expansion" / "horizon" / "block" / "ruby_block.json",
        {
            "template": "minecraft:emerald_block",
            "properties": {
                "hardness": 5.0,
                "resistance": 6.0,
                "sounds": "minecraft:metal",
            },
            "item": True,
        },
    )
    write_json(
        m3 / "data" / "ruby_expansion" / "horizon" / "item" / "ruby.json",
        {
            "template": "minecraft:emerald",
            "components": {
                "minecraft:max_stack_size": 64,
                "minecraft:rarity": "uncommon",
            },
        },
    )
    write_json(
        m3 / "data" / "ruby_expansion" / "horizon" / "item" / "ruby_blade.json",
        {
            "template": "minecraft:diamond_sword",
            "components": {
                "minecraft:max_damage": 1800,
                "minecraft:rarity": "rare",
                "minecraft:attribute_modifiers": [
                    {
                        "type": "minecraft:attack_damage",
                        "id": "minecraft:base_attack_damage",
                        "amount": 7.5,
                        "operation": "add_value",
                        "slot": "mainhand",
                    }
                ],
            },
        },
    )
    write_json(
        m3 / "data" / "ruby_expansion" / "horizon" / "mob_effect" / "ruby_vitality.json",
        {
            "category": "beneficial",
            "color": 14423100,
        },
    )
    write_json(
        m3 / "data" / "ruby_expansion" / "horizon" / "creative_tab" / "ruby_tab.json",
        {
            "title": "Ruby Expansion",
            "icon": "ruby_expansion:ruby",
            "items": [
                "ruby_expansion:ruby",
                "ruby_expansion:ruby_ore",
                "ruby_expansion:ruby_block",
                "ruby_expansion:ruby_blade",
            ],
        },
    )
    write_block_assets(
        m3,
        "ruby_expansion",
        "ruby_ore",
        render_block_texture((125, 125, 125), (95, 95, 95), (225, 38, 62), "ore"),
    )
    write_block_assets(
        m3,
        "ruby_expansion",
        "ruby_block",
        render_block_texture((210, 32, 58), (145, 16, 36), (255, 115, 130), "crystal"),
    )
    write_item_assets(
        m3,
        "ruby_expansion",
        "ruby",
        render_item_texture((235, 42, 68), (150, 18, 38), shape="gem"),
    )
    write_item_assets(
        m3,
        "ruby_expansion",
        "ruby_blade",
        render_item_texture((235, 45, 72), (155, 20, 42), handle=(120, 82, 48), shape="sword"),
        handheld=True,
    )
    write_json(
        m3 / "data" / "ruby_expansion" / "loot_table" / "blocks" / "ruby_ore.json",
        {
            "type": "minecraft:block",
            "pools": [
                {
                    "rolls": 1.0,
                    "bonus_rolls": 0.0,
                    "entries": [
                        {
                            "type": "minecraft:item",
                            "name": "ruby_expansion:ruby",
                        }
                    ],
                    "conditions": [
                        {"condition": "minecraft:survives_explosion"}
                    ],
                }
            ],
        },
    )
    write_self_drop_loot_table(m3, "ruby_expansion", "ruby_block")
    write_json(
        m3 / "data" / "ruby_expansion" / "recipe" / "ruby_block.json",
        {
            "type": "minecraft:crafting_shaped",
            "pattern": ["RRR", "RRR", "RRR"],
            "key": {"R": "ruby_expansion:ruby"},
            "result": {"id": "ruby_expansion:ruby_block", "count": 1},
        },
    )
    write_json(
        m3 / "data" / "ruby_expansion" / "recipe" / "ruby_from_block.json",
        {
            "type": "minecraft:crafting_shapeless",
            "ingredients": ["ruby_expansion:ruby_block"],
            "result": {"id": "ruby_expansion:ruby", "count": 9},
        },
    )
    write_json(
        m3 / "data" / "ruby_expansion" / "recipe" / "ruby_blade.json",
        {
            "type": "minecraft:crafting_shaped",
            "pattern": ["R", "R", "S"],
            "key": {
                "R": "ruby_expansion:ruby",
                "S": "minecraft:stick",
            },
            "result": {"id": "ruby_expansion:ruby_blade", "count": 1},
        },
    )
    write_json(
        m3 / "data" / "minecraft" / "tags" / "block" / "mineable" / "pickaxe.json",
        {
            "replace": False,
            "values": [
                "ruby_expansion:ruby_ore",
                "ruby_expansion:ruby_block",
            ],
        },
    )
    write_json(
        m3 / "assets" / "ruby_expansion" / "lang" / "en_us.json",
        {
            "block.ruby_expansion.ruby_ore": "Ruby Ore",
            "block.ruby_expansion.ruby_block": "Block of Ruby",
            "item.ruby_expansion.ruby": "Ruby",
            "item.ruby_expansion.ruby_blade": "Crimson Ruby Blade",
            "effect.ruby_expansion.ruby_vitality": "Ruby Vitality",
            "itemGroup.ruby_expansion.ruby_tab": "Ruby Expansion",
        },
    )

    # =========================================================================
    # 4. industrial_pipes — 6-Way Connecting Pipes, Grates & Sorter Menu
    # =========================================================================
    m4 = SRC_MODS_DIR / "industrial_pipes"
    m4.mkdir(parents=True, exist_ok=True)
    (m4 / "horizon-mod.toml").write_text(
        'id = "industrial_pipes"\n'
        'name = "Industrial Pipes & Grates"\n'
        'version = "0.2.0"\n'
        'description = "Adds 6-way connecting Copper & Steel Logistics Pipes, non-occluding Factory Floor Grates, and Industrial Sorter menu definitions."\n'
        'authors = ["Rust-store-"]\n'
        'api = "0.2"\n'
        'side = "both"\n'
        'capabilities = ["registries"]\n',
        encoding="utf-8",
    )
    write_json(
        m4 / "data" / "industrial_pipes" / "horizon" / "block" / "copper_pipe.json",
        {
            "template": "minecraft:chorus_plant",
            "connects_to": "#industrial_pipes:connectable_pipes",
            "thickness": 6,
            "properties": {
                "hardness": 2.0,
                "resistance": 4.0,
                "sounds": "minecraft:copper",
            },
            "item": True,
        },
    )
    write_json(
        m4 / "data" / "industrial_pipes" / "horizon" / "block" / "steel_duct.json",
        {
            "template": "minecraft:chorus_plant",
            "connects_to": "#industrial_pipes:connectable_pipes",
            "thickness": 8,
            "properties": {
                "hardness": 3.5,
                "resistance": 6.0,
                "sounds": "minecraft:metal",
            },
            "item": True,
        },
    )
    write_json(
        m4 / "data" / "industrial_pipes" / "horizon" / "block" / "factory_grate.json",
        {
            "template": "minecraft:iron_block",
            "occludes": False,
            "properties": {
                "hardness": 2.5,
                "resistance": 5.0,
                "sounds": "minecraft:copper_grate",
            },
            "item": True,
        },
    )
    write_json(
        m4 / "data" / "industrial_pipes" / "horizon" / "item" / "pipe_wrench.json",
        {
            "template": "minecraft:iron_ingot",
            "components": {
                "minecraft:max_stack_size": 1,
                "minecraft:rarity": "uncommon",
            },
        },
    )
    write_json(
        m4 / "data" / "industrial_pipes" / "horizon" / "menu" / "item_sorter.json",
        {
            "slots": 9,
            "data_slots": 2,
            "results": [7, 8],
        },
    )
    write_json(
        m4 / "data" / "industrial_pipes" / "horizon" / "creative_tab" / "logistics_tab.json",
        {
            "title": "Industrial Pipes",
            "icon": "industrial_pipes:copper_pipe",
            "items": [
                "industrial_pipes:copper_pipe",
                "industrial_pipes:steel_duct",
                "industrial_pipes:factory_grate",
                "industrial_pipes:pipe_wrench",
            ],
        },
    )
    write_json(
        m4 / "data" / "industrial_pipes" / "tags" / "block" / "connectable_pipes.json",
        {
            "replace": False,
            "values": [
                "industrial_pipes:copper_pipe",
                "industrial_pipes:steel_duct",
                "minecraft:chest",
                "minecraft:barrel",
                "minecraft:hopper",
            ],
        },
    )
    write_block_assets(
        m4,
        "industrial_pipes",
        "copper_pipe",
        render_block_texture((184, 108, 68), (125, 68, 38), (222, 148, 102), "stripes"),
    )
    write_block_assets(
        m4,
        "industrial_pipes",
        "steel_duct",
        render_block_texture((105, 112, 120), (65, 70, 78), (155, 165, 175), "stripes"),
    )
    write_block_assets(
        m4,
        "industrial_pipes",
        "factory_grate",
        render_block_texture((92, 98, 104), (58, 62, 68), (135, 142, 150), "grate"),
    )
    write_item_assets(
        m4,
        "industrial_pipes",
        "pipe_wrench",
        render_item_texture((205, 125, 70), (130, 72, 35), handle=(85, 90, 96), shape="sword"),
        handheld=True,
    )
    write_self_drop_loot_table(m4, "industrial_pipes", "copper_pipe")
    write_self_drop_loot_table(m4, "industrial_pipes", "steel_duct")
    write_self_drop_loot_table(m4, "industrial_pipes", "factory_grate")
    write_json(
        m4 / "data" / "industrial_pipes" / "recipe" / "copper_pipe.json",
        {
            "type": "minecraft:crafting_shaped",
            "pattern": ["CCC", "   ", "CCC"],
            "key": {"C": "minecraft:copper_ingot"},
            "result": {"id": "industrial_pipes:copper_pipe", "count": 6},
        },
    )
    write_json(
        m4 / "data" / "industrial_pipes" / "recipe" / "steel_duct.json",
        {
            "type": "minecraft:crafting_shaped",
            "pattern": ["III", "   ", "III"],
            "key": {"I": "minecraft:iron_ingot"},
            "result": {"id": "industrial_pipes:steel_duct", "count": 6},
        },
    )
    write_json(
        m4 / "data" / "industrial_pipes" / "recipe" / "factory_grate.json",
        {
            "type": "minecraft:crafting_shaped",
            "pattern": [" I ", "I I", " I "],
            "key": {"I": "minecraft:iron_ingot"},
            "result": {"id": "industrial_pipes:factory_grate", "count": 4},
        },
    )
    write_json(
        m4 / "data" / "industrial_pipes" / "recipe" / "pipe_wrench.json",
        {
            "type": "minecraft:crafting_shaped",
            "pattern": ["C C", " C ", " I "],
            "key": {
                "C": "minecraft:copper_ingot",
                "I": "minecraft:iron_ingot",
            },
            "result": {"id": "industrial_pipes:pipe_wrench", "count": 1},
        },
    )
    write_json(
        m4 / "assets" / "industrial_pipes" / "lang" / "en_us.json",
        {
            "block.industrial_pipes.copper_pipe": "Copper Logistics Pipe",
            "block.industrial_pipes.steel_duct": "Heavy Steel Duct",
            "block.industrial_pipes.factory_grate": "Factory Floor Grate",
            "item.industrial_pipes.pipe_wrench": "Pipe Wrench",
            "itemGroup.industrial_pipes.logistics_tab": "Industrial Pipes",
        },
    )

    # =========================================================================
    # 5. dungeon_mobs_plus — Custom Mobs, Projectiles, Status Effects & Totems
    # =========================================================================
    m5 = SRC_MODS_DIR / "dungeon_mobs_plus"
    m5.mkdir(parents=True, exist_ok=True)
    (m5 / "horizon-mod.toml").write_text(
        'id = "dungeon_mobs_plus"\n'
        'name = "Dungeon Mobs & Frost Magic"\n'
        'version = "0.2.0"\n'
        'description = "Adds Armored Dungeon Brute and Outpost Guard living entity types, Frost Orb projectiles, Frostbite mob effect, and Runic Totem blocks."\n'
        'authors = ["Rust-store-"]\n'
        'api = "0.2"\n'
        'side = "both"\n'
        'capabilities = ["registries"]\n',
        encoding="utf-8",
    )
    write_json(
        m5 / "data" / "dungeon_mobs_plus" / "horizon" / "block" / "runic_totem.json",
        {
            "template": "minecraft:chiseled_stone_bricks",
            "properties": {
                "hardness": 4.0,
                "resistance": 10.0,
                "light": 10,
                "sounds": "minecraft:deepslate_bricks",
            },
            "item": True,
        },
    )
    write_json(
        m5 / "data" / "dungeon_mobs_plus" / "horizon" / "item" / "frost_orb.json",
        {
            "template": "minecraft:snowball",
            "components": {
                "minecraft:max_stack_size": 16,
                "minecraft:rarity": "uncommon",
            },
        },
    )
    write_json(
        m5 / "data" / "dungeon_mobs_plus" / "horizon" / "entity_type" / "frost_orb_projectile.json",
        {
            "template": "minecraft:snowball",
            "item": "dungeon_mobs_plus:frost_orb",
            "damage": 3.5,
        },
    )
    write_json(
        m5 / "data" / "dungeon_mobs_plus" / "horizon" / "entity_type" / "armored_brute.json",
        {
            "template": "minecraft:zombie",
            "attributes": {
                "minecraft:max_health": 45.0,
                "minecraft:movement_speed": 0.27,
            },
        },
    )
    write_json(
        m5 / "data" / "dungeon_mobs_plus" / "horizon" / "entity_type" / "outpost_guard.json",
        {
            "template": "minecraft:wandering_trader",
            "attributes": {
                "minecraft:max_health": 36.0,
                "minecraft:movement_speed": 0.30,
            },
        },
    )
    write_json(
        m5 / "data" / "dungeon_mobs_plus" / "horizon" / "mob_effect" / "frostbite.json",
        {
            "category": "harmful",
            "color": 5816319,
        },
    )
    write_json(
        m5 / "data" / "dungeon_mobs_plus" / "horizon" / "creative_tab" / "dungeon_tab.json",
        {
            "title": "Dungeon Mobs+",
            "icon": "dungeon_mobs_plus:frost_orb",
            "items": [
                "dungeon_mobs_plus:runic_totem",
                "dungeon_mobs_plus:frost_orb",
            ],
        },
    )
    write_block_assets(
        m5,
        "dungeon_mobs_plus",
        "runic_totem",
        render_block_texture((64, 72, 88), (38, 42, 54), (95, 215, 255), "crystal"),
    )
    write_item_assets(
        m5,
        "dungeon_mobs_plus",
        "frost_orb",
        render_item_texture((110, 225, 255), (45, 135, 195), shape="orb"),
    )
    write_self_drop_loot_table(m5, "dungeon_mobs_plus", "runic_totem")
    write_json(
        m5 / "data" / "dungeon_mobs_plus" / "recipe" / "frost_orb.json",
        {
            "type": "minecraft:crafting_shapeless",
            "ingredients": [
                "minecraft:snowball",
                "minecraft:blue_ice",
            ],
            "result": {"id": "dungeon_mobs_plus:frost_orb", "count": 4},
        },
    )
    write_json(
        m5 / "data" / "dungeon_mobs_plus" / "recipe" / "runic_totem.json",
        {
            "type": "minecraft:crafting_shaped",
            "pattern": ["SSS", "SFS", "SSS"],
            "key": {
                "S": "minecraft:stone_bricks",
                "F": "dungeon_mobs_plus:frost_orb",
            },
            "result": {"id": "dungeon_mobs_plus:runic_totem", "count": 1},
        },
    )
    write_json(
        m5 / "assets" / "dungeon_mobs_plus" / "lang" / "en_us.json",
        {
            "block.dungeon_mobs_plus.runic_totem": "Runic Frost Totem",
            "item.dungeon_mobs_plus.frost_orb": "Frost Orb",
            "entity.dungeon_mobs_plus.frost_orb_projectile": "Frost Orb",
            "entity.dungeon_mobs_plus.armored_brute": "Armored Dungeon Brute",
            "entity.dungeon_mobs_plus.outpost_guard": "Outpost Guard",
            "effect.dungeon_mobs_plus.frostbite": "Frostbite",
            "itemGroup.dungeon_mobs_plus.dungeon_tab": "Dungeon Mobs+",
        },
    )

    # =========================================================================
    # 6. block_placement_reborn — Precision Grid Block, Ghost Frame & Wand
    # =========================================================================
    m6 = SRC_MODS_DIR / "block_placement_reborn"
    m6.mkdir(parents=True, exist_ok=True)
    (m6 / "horizon-mod.toml").write_text(
        'id = "block_placement_reborn"\n'
        'name = "Block Placement Reborn"\n'
        'version = "1.0.0"\n'
        'description = "Precision block placement blocks and tools: Alignment Grid Block, Non-Occluding Ghost Preview Frame, Blueprint Placement Wand, and Rotator."\n'
        'authors = ["AI Modding Specialist"]\n'
        'api = "0.2"\n'
        'side = "both"\n'
        'capabilities = ["registries"]\n',
        encoding="utf-8",
    )
    write_json(
        m6 / "data" / "block_placement_reborn" / "horizon" / "block" / "placement_grid_block.json",
        {
            "template": "minecraft:iron_block",
            "properties": {
                "hardness": 1.5,
                "resistance": 10.0,
                "light": 7,
                "sounds": "minecraft:amethyst_block",
            },
            "item": True,
        },
    )
    write_json(
        m6 / "data" / "block_placement_reborn" / "horizon" / "block" / "ghost_preview_frame.json",
        {
            "template": "minecraft:glass",
            "occludes": False,
            "properties": {
                "hardness": 0.3,
                "resistance": 1.0,
                "light": 12,
                "sounds": "minecraft:glass",
            },
            "item": True,
        },
    )
    write_json(
        m6 / "data" / "block_placement_reborn" / "horizon" / "item" / "placement_wand.json",
        {
            "template": "minecraft:breeze_rod",
            "components": {
                "minecraft:max_stack_size": 1,
                "minecraft:rarity": "epic",
            },
        },
    )
    write_json(
        m6 / "data" / "block_placement_reborn" / "horizon" / "creative_tab" / "bpr_tab.json",
        {
            "title": "Block Placement Reborn",
            "icon": "block_placement_reborn:placement_wand",
            "items": [
                "block_placement_reborn:placement_wand",
                "block_placement_reborn:placement_grid_block",
                "block_placement_reborn:ghost_preview_frame",
            ],
        },
    )
    write_block_assets(
        m6,
        "block_placement_reborn",
        "placement_grid_block",
        render_block_texture((38, 92, 148), (22, 54, 96), (92, 200, 255), "stripes"),
    )
    write_block_assets(
        m6,
        "block_placement_reborn",
        "ghost_preview_frame",
        render_block_texture((65, 195, 245), (35, 130, 185), (180, 245, 255), "grate"),
    )
    write_item_assets(
        m6,
        "block_placement_reborn",
        "placement_wand",
        render_item_texture((85, 215, 255), (35, 125, 195), handle=(75, 85, 105), shape="sword"),
        handheld=True,
    )
    write_self_drop_loot_table(m6, "block_placement_reborn", "placement_grid_block")
    write_self_drop_loot_table(m6, "block_placement_reborn", "ghost_preview_frame")
    write_json(
        m6 / "data" / "block_placement_reborn" / "recipe" / "placement_wand.json",
        {
            "type": "minecraft:crafting_shaped",
            "pattern": ["  A", " S ", "S  "],
            "key": {
                "A": "minecraft:amethyst_shard",
                "S": "minecraft:copper_ingot",
            },
            "result": {"id": "block_placement_reborn:placement_wand", "count": 1},
        },
    )
    write_json(
        m6 / "data" / "block_placement_reborn" / "recipe" / "placement_grid_block.json",
        {
            "type": "minecraft:crafting_shaped",
            "pattern": ["IRI", "RCR", "IRI"],
            "key": {
                "I": "minecraft:iron_ingot",
                "R": "minecraft:redstone",
                "C": "minecraft:compass",
            },
            "result": {"id": "block_placement_reborn:placement_grid_block", "count": 4},
        },
    )
    write_json(
        m6 / "assets" / "block_placement_reborn" / "lang" / "en_us.json",
        {
            "block.block_placement_reborn.placement_grid_block": "Precision Placement Grid Block",
            "block.block_placement_reborn.ghost_preview_frame": "Ghost Preview Frame",
            "item.block_placement_reborn.placement_wand": "Block Placement Reborn Wand",
            "itemGroup.block_placement_reborn.bpr_tab": "Block Placement Reborn",
        },
    )


def validate_hmod_archive(hmod_path: Path) -> dict[str, Any]:
    """
    Validate a .hmod file against Horizon's docs/api/mod-files.md and docs/mods.md rules.
    Returns metadata dictionary on success or raises ValueError on spec violation.
    """
    if not hmod_path.exists():
        raise ValueError(f"Missing .hmod file: {hmod_path}")

    file_size = hmod_path.stat().st_size
    if file_size > 64 * 1024 * 1024:
        raise ValueError(f"{hmod_path.name}: exceeds 64 MiB registry limit ({file_size} bytes)")

    sha256_hash = hashlib.sha256(hmod_path.read_bytes()).hexdigest()

    with zipfile.ZipFile(hmod_path, "r") as zf:
        infos = zf.infolist()
        if not infos:
            raise ValueError(f"{hmod_path.name}: archive is empty")
        if len(infos) > 65536:
            raise ValueError(f"{hmod_path.name}: exceeds 65,536 files limit")

        seen_paths: set[str] = set()
        total_unpacked = 0

        for info in infos:
            # Compression check: stored (0) or deflate (8) only
            if info.compress_type not in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED):
                raise ValueError(
                    f"{hmod_path.name}: unsupported compression {info.compress_type} on {info.filename}"
                )

            name = info.filename
            if "\\" in name or ":" in name or name.startswith("/") or name.endswith("/"):
                raise ValueError(
                    f"{hmod_path.name}: invalid zip entry path '{name}' (must be relative, /-separated, no directory entries)"
                )

            parts = name.split("/")
            if any(p in ("", ".", "..") for p in parts):
                raise ValueError(f"{hmod_path.name}: illegal path segment in '{name}'")

            if name in seen_paths:
                raise ValueError(f"{hmod_path.name}: duplicate entry '{name}'")
            seen_paths.add(name)

            if info.file_size > 16 * 1024 * 1024:
                raise ValueError(f"{hmod_path.name}: entry '{name}' exceeds 16 MiB limit")
            total_unpacked += info.file_size

        if total_unpacked > 512 * 1024 * 1024:
            raise ValueError(f"{hmod_path.name}: total unpacked size exceeds 512 MiB")

        if "horizon-mod.toml" not in seen_paths:
            raise ValueError(f"{hmod_path.name}: missing top-level horizon-mod.toml")

        manifest_raw = zf.read("horizon-mod.toml").decode("utf-8")
        manifest = tomllib.loads(manifest_raw)

        unknown_keys = set(manifest.keys()) - ALLOWED_MANIFEST_KEYS
        if unknown_keys:
            raise ValueError(f"{hmod_path.name}: unknown manifest keys: {sorted(unknown_keys)}")

        mod_id = manifest.get("id", "")
        if not isinstance(mod_id, str) or not MOD_ID_RE.match(mod_id):
            raise ValueError(f"{hmod_path.name}: invalid mod id '{mod_id}'")

        if manifest.get("api") != "0.2":
            raise ValueError(f"{hmod_path.name}: content mod must specify api = '0.2'")

        caps = manifest.get("capabilities", [])
        if "registries" not in caps:
            raise ValueError(f"{hmod_path.name}: content mod must include 'registries' in capabilities")

        extra_namespaces = set(manifest.get("namespaces", []))
        if extra_namespaces & {"minecraft", "c", "horizon"}:
            raise ValueError(f"{hmod_path.name}: cannot claim reserved namespaces in manifest")

        allowed_namespaces = {mod_id} | extra_namespaces

        counts = {
            "blocks": 0,
            "items": 0,
            "creative_tabs": 0,
            "mob_effects": 0,
            "entity_types": 0,
            "menus": 0,
            "recipes": 0,
            "textures": 0,
        }

        for path in sorted(seen_paths):
            if path == "horizon-mod.toml":
                continue
            parts = path.split("/")
            if len(parts) < 3 or parts[0] not in ("assets", "data"):
                raise ValueError(f"{hmod_path.name}: disallowed top-level or short content path '{path}'")
            for part in parts[1:]:
                if not RESOURCE_PART_RE.match(part):
                    raise ValueError(
                        f"{hmod_path.name}: path part '{part}' in '{path}' violates [a-z0-9_.-]"
                    )

            root_kind, ns = parts[0], parts[1]
            if root_kind == "assets":
                if ns not in allowed_namespaces and not (
                    ns == "minecraft" and len(parts) >= 3 and parts[2] == "atlases"
                ):
                    raise ValueError(f"{hmod_path.name}: unauthorized asset namespace '{ns}' in '{path}'")
            elif root_kind == "data":
                if ns not in allowed_namespaces and not (
                    ns in ("minecraft", "c") and len(parts) >= 3 and parts[2] == "tags"
                ):
                    raise ValueError(f"{hmod_path.name}: unauthorized data namespace '{ns}' in '{path}'")

            if path.endswith(".json"):
                try:
                    parsed = json.loads(zf.read(path).decode("utf-8"))
                except Exception as exc:
                    raise ValueError(f"{hmod_path.name}: invalid JSON in '{path}': {exc}") from exc
                if not isinstance(parsed, dict):
                    raise ValueError(f"{hmod_path.name}: JSON root in '{path}' must be an object")

            if path.startswith(f"data/{mod_id}/horizon/block/"):
                counts["blocks"] += 1
            elif path.startswith(f"data/{mod_id}/horizon/item/"):
                counts["items"] += 1
            elif path.startswith(f"data/{mod_id}/horizon/creative_tab/"):
                counts["creative_tabs"] += 1
            elif path.startswith(f"data/{mod_id}/horizon/mob_effect/"):
                counts["mob_effects"] += 1
            elif path.startswith(f"data/{mod_id}/horizon/entity_type/"):
                counts["entity_types"] += 1
            elif path.startswith(f"data/{mod_id}/horizon/menu/"):
                counts["menus"] += 1
            elif path.startswith(f"data/{mod_id}/recipe/"):
                counts["recipes"] += 1
            elif path.endswith(".png"):
                png_bytes = zf.read(path)
                if not png_bytes.startswith(b"\x89PNG\r\n\x1a\n"):
                    raise ValueError(f"{hmod_path.name}: invalid PNG header in '{path}'")
                counts["textures"] += 1

    return {
        "id": mod_id,
        "name": manifest["name"],
        "version": manifest["version"],
        "description": manifest["description"],
        "authors": manifest.get("authors", []),
        "api": manifest["api"],
        "side": manifest.get("side", "client"),
        "capabilities": manifest["capabilities"],
        "file_name": hmod_path.name,
        "size_bytes": file_size,
        "sha256": sha256_hash,
        "entry_count": len(seen_paths),
        "counts": counts,
    }


def package_mod(mod_dir: Path, out_dir: Path) -> Path:
    """ deterministically package a src-mods/<id> directory into releases/<id>.hmod."""
    manifest = tomllib.loads((mod_dir / "horizon-mod.toml").read_text(encoding="utf-8"))
    mod_id = manifest["id"]
    out_dir.mkdir(parents=True, exist_ok=True)
    hmod_path = out_dir / f"{mod_id}.hmod"

    files_to_pack: list[Path] = []
    for root, _, files in os.walk(mod_dir):
        for f in files:
            files_to_pack.append(Path(root) / f)

    files_to_pack.sort(key=lambda p: p.relative_to(mod_dir).as_posix())

    with zipfile.ZipFile(hmod_path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
        for file_path in files_to_pack:
            rel_posix = file_path.relative_to(mod_dir).as_posix()
            zinfo = zipfile.ZipInfo(rel_posix, date_time=(2026, 1, 1, 0, 0, 0))
            zinfo.compress_type = zipfile.ZIP_DEFLATED
            zinfo.external_attr = 0o100644 << 16  # Regular file mode
            zf.writestr(zinfo, file_path.read_bytes())

    return hmod_path


def main() -> int:
    parser = argparse.ArgumentParser(description="Build and validate Horizon .hmod packages.")
    parser.add_argument(
        "--check-only",
        action="store_true",
        help="Only validate existing releases/*.hmod and catalog/mods-index.json without regenerating.",
    )
    args = parser.parse_args()

    if not args.check_only:
        generate_all_mod_sources()

    mod_dirs = sorted(
        [d for d in SRC_MODS_DIR.iterdir() if d.is_dir() and (d / "horizon-mod.toml").exists()]
    )
    if not mod_dirs:
        print("ERROR: No mod source directories found in src-mods/", file=sys.stderr)
        return 1

    catalog_entries: list[dict[str, Any]] = []
    for mod_dir in mod_dirs:
        mod_id = mod_dir.name
        hmod_path = RELEASES_DIR / f"{mod_id}.hmod"
        if not args.check_only:
            hmod_path = package_mod(mod_dir, RELEASES_DIR)
        meta = validate_hmod_archive(hmod_path)
        catalog_entries.append(meta)
        print(
            f"[OK] {meta['file_name']:26s} | v{meta['version']} | "
            f"{meta['size_bytes']:5d} B | sha256:{meta['sha256'][:16]}... | "
            f"blocks:{meta['counts']['blocks']} items:{meta['counts']['items']} "
            f"entities:{meta['counts']['entity_types']} effects:{meta['counts']['mob_effects']}"
        )

    CATALOG_DIR.mkdir(parents=True, exist_ok=True)
    index_path = CATALOG_DIR / "mods-index.json"
    index_payload = {
        "engine": "Horizon (Rust Engine for Minecraft: Java Edition 26.3)",
        "targets": [
            "PC (Linux, macOS, Windows — horizon-rust/horizon)",
            "Android 9+ 64-bit (arm64-v8a, x86_64 — Vasysik/horizon-android)",
        ],
        "format_spec": "https://github.com/horizon-rust/horizon/blob/main/docs/api/mod-files.md",
        "mods": catalog_entries,
    }
    if not args.check_only:
        write_json(index_path, index_payload)
    else:
        existing = json.loads(index_path.read_text(encoding="utf-8"))
        if len(existing.get("mods", [])) != len(catalog_entries):
            print("ERROR: catalog/mods-index.json is out of sync with releases/*.hmod", file=sys.stderr)
            return 1

    print(f"\nValidated {len(catalog_entries)} Horizon .hmod packages successfully.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
