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

## 📖 Guides & Documentation

- 📱 **[Installing `.hmod` Mods on Android (`docs/INSTALL_ANDROID.md`)](docs/INSTALL_ANDROID.md)**
- 🖥️ **[Installing `.hmod` & `.hnmod` Mods on PC (`docs/INSTALL_PC.md`)](docs/INSTALL_PC.md)**
- 🧑‍💻 **[How to Create & Package `.hmod` Mods (`docs/CREATE_HMOD.md`)](docs/CREATE_HMOD.md)**
- 🌐 **[Upstream Horizon & Community Mod Directory (`catalog/COMMUNITY_MODS.md`)](catalog/COMMUNITY_MODS.md)**

---

## 📄 License

Released under the [MIT License](LICENSE). All mods in this repository are free to download, share, and remix.
