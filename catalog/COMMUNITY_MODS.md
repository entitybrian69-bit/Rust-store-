# 🌐 Upstream Horizon & Community Mod Directory

In addition to the ready-to-download `.hmod` packages hosted directly in [`releases/`](../releases/), here are the official and community resources for the **Horizon Rust Minecraft Engine** ecosystem.

---

## 1. Core Engine & Clients (PC & Android)

| Project | Platform | Description | Link |
| :--- | :---: | :--- | :--- |
| **Horizon (`horizon-rust/horizon`)** | 🖥️ Linux, macOS, Windows | Minecraft: Java Edition **26.3** rewritten from scratch in Rust with 100% vanilla parity checks and `.hmod` / `.hnmod` mod support. | [github.com/horizon-rust/horizon](https://github.com/horizon-rust/horizon) |
| **Horizon Android (`Vasysik/horizon-android`)** | 📱 Android 9+ (`arm64-v8a`, `x86_64`) | Native Android Vulkan port of Horizon 26.3 with editable touch schemes and direct HTTPS `.hmod` mod import. | [github.com/Vasysik/horizon-android](https://github.com/Vasysik/horizon-android) |
| **Horizon Official Registry (`horizon-rust/horizon-mods`)** | 🖥️ PC / 📱 Android | Official reviewed registry of `.hmod` mods, `.hnmod` native mods, `.hpack` modpacks, and resource packs. | [github.com/horizon-rust/horizon-mods](https://github.com/horizon-rust/horizon-mods) |

---

## 2. Official Horizon Native Mods (`horizon-rust/horizon-mods/tree/main/native`)

These official native mods ship as `.hnmod` source bundles in [`horizon-rust/horizon-mods`](https://github.com/horizon-rust/horizon-mods/tree/main/native) for PC builds:

| Mod Name | Type | Description |
| :--- | :---: | :--- |
| **Zoom** | `.hnmod` (Client) | Smooth optic zoom hotkey with mouse sensitivity scaling. |
| **FPS Counter** | `.hnmod` (Client) | Lightweight on-screen frame rate and frame-time HUD overlay. |
| **Keystrokes** | `.hnmod` (Client) | Displays WASD, mouse buttons, and CPS on the HUD. |
| **Minimap** | `.hnmod` (Client) | Live top-down terrain minimap and waypoint display. |
| **Recipes** | `.hnmod` (Client) | In-game recipe viewer and item usage browser (JEI/REI style). |
| **Coordinates HUD** | `.hnmod` (Client) | Compact XYZ coordinates, biome, and facing direction overlay. |

---

## 3. Modding Tools & Specifications

- **[`cargo-horizon-mod`](https://github.com/horizon-rust/horizon/blob/main/docs/mods.md#making-a-content-mod)** — Official CLI tool to scaffold (`new`), package (`build`), run (`run`), and verify (`check`) `.hmod` and `.hnmod` packages.
- **[`horizon-mixins`](https://github.com/horizon-rust/horizon-mixins)** — Compile-time Rust mixin system for native `.hnmod` mods.
- **[Mod File Format Specification (`docs/api/mod-files.md`)](https://github.com/horizon-rust/horizon/blob/main/docs/api/mod-files.md)** — Complete specification of `.hmod` and `.hnmod` ZIP rules, manifests, namespaces, and size limits.
