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
- [🎬 Fresh Moves — Native Player Animation (`.hnmod`)](#-fresh-moves--native-player-animation-hnmod)
- [📖 Guides & Documentation](#-guides--documentation)
  - [Android Installation Guide (`docs/INSTALL_ANDROID.md`)](docs/INSTALL_ANDROID.md)
  - [PC Installation Guide (`docs/INSTALL_PC.md`)](docs/INSTALL_PC.md)
  - [Create & Submit `.hmod` Mods (`docs/CREATE_HMOD.md`)](docs/CREATE_HMOD.md)
  - [Upstream Horizon & Community Mod Directory (`catalog/COMMUNITY_MODS.md`)](catalog/COMMUNITY_MODS.md)
  - [Fresh Moves: design, config and install (`docs/FRESH_MOVES.md`)](docs/FRESH_MOVES.md)

---

## 🎬 Fresh Moves — Player Animation (`.hmod` + `.hnmod`)

**[`releases/fresh_moves-1.0.0.hmod`](releases/fresh_moves-1.0.0.hmod)** — a **single `.hmod`** carrying the
animation, and its code-only twin **[`releases/fresh_moves-1.0.0.hnmod`](releases/fresh_moves-1.0.0.hnmod)**. It gives
every player model a full animation set: idle breathing and weight shift, walk,
sprint, sneak, jump, fall, land, swim, climb and ride; mine, place, eat, drink,
bow, shield and tool use; a three-hit sword combo, heavy attack, dodge, sprint
attack, hurt and death; eased blending, head tracking and a subtle first-person
camera sway — in **both first and third person**, on **PC and Android**.

| | |
| :--- | :--- |
| **Files** | `fresh_moves-1.0.0.hmod` (combined: content manifest + native code) and `fresh_moves-1.0.0.hnmod` (code only) |
| **Bundle id / version** | `fresh_moves` `1.0.0`, native manifest schema `1` |
| **Pinned engine** | Horizon `26.3`, `horizon` `a30ed9e4…`, `mixins` `5a64af93…`, hook API `397` |
| **Hooks** | 12 named hook points, none cancellable — it cannot skip another mod's code |
| **Config** | `config/fresh_moves-client.toml` + in-game page (Options → Mods → Fresh Moves) |
| **Validate** | `python3 scripts/build_hnmod.py --check-only` |

Install: drop the `.hmod` (or the `.hnmod`) in the launcher's
`horizon-native-mods/` on PC; import the `.hnmod` in Starling on Android — then
launch. Full detail, the config reference and the multi-mod rules are in
**[`docs/FRESH_MOVES.md`](docs/FRESH_MOVES.md)**.

A `.hmod` alone cannot animate a player — Horizon's content API has one capability
(`registries`) and nothing in it poses a model. Horizon does read a `.hmod` that
carries **both** halves in one archive (`horizon_mods::index::native::content`,
`launcher/src/native_mods/source.rs`), which is exactly what
`fresh_moves-1.0.0.hmod` is: `horizon-mod.toml` beside `horizon-native.json` and
`mixins/fresh_moves/`, installed as one file and built with its content
snapshotted. The `.hnmod` is the same bundle without the content manifest.

---

## 📦 Ready-to-Download `.hmod` Catalog

All packages below use **Horizon Content API `0.2`** (`capabilities = ["registries"]`, `side = "both"`) and run out of the box on **PC** (Windows, Linux, macOS) and **Android** (`arm64-v8a` / `x86_64`).

| 
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

## 🧩 Community WASM HMOD project

**Volt Survival Suite** is a client-side code mod maintained in the [Volt Launcher source](https://github.com/entitybrian69-bit/Volt-launcher/tree/android/mods/community/volt-survival-suite). Unlike the content-only mods below, it contains a WebAssembly component and uses Horizon's sandboxed mod SDK.

Features: live survival HUD, coordinates and facing, health/hunger, world time and dimension, target-block inspection, persistent HUD toggle, and the `/kit`, `/where`, `/inspect`, and `/time` commands.

Its [GitHub Actions build](https://github.com/entitybrian69-bit/Volt-launcher/actions/workflows/build-volt-survival-suite.yml) runs tests, creates the component, packages the `.hmod`, and uploads a workflow artifact. The artifact is a CI build, not yet a signed registry release; wait for a successful run before installing it.

Volt also includes the [HMOD Doctor](https://github.com/entitybrian69-bit/Volt-launcher/blob/android/docs/hmod-doctor.md), a diagnostic CLI for checking manifests, code/content package types, content validation and duplicate mod IDs.

---

## 📖 Guides & Documentation

- 📱 **[Installing `.hmod` Mods on Android (`docs/INSTALL_ANDROID.md`)](docs/INSTALL_ANDROID.md)**
- 🖥️ **[Installing `.hmod` & `.hnmod` Mods on PC (`docs/INSTALL_PC.md`)](docs/INSTALL_PC.md)**
- 🧑‍💻 **[How to Create & Package `.hmod` Mods (`docs/CREATE_HMOD.md`)](docs/CREATE_HMOD.md)**
- 🌐 **[Upstream Horizon & Community Mod Directory (`catalog/COMMUNITY_MODS.md`)](catalog/COMMUNITY_MODS.md)**

---

## 📄 License

Released under the [MIT License](LICENSE). All mods in this repository are free to download, share, and remix.
