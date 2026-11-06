# 📱 Installing Horizon `.hmod` Mods on Android

This guide explains how to install and enable `.hmod` mods on **[Horizon Android](https://github.com/Vasysik/horizon-android)** (the native Rust/Vulkan client for Minecraft: Java Edition **26.3**).

---

## 1. Requirements

| APK Architecture | Minimum Android | `.hmod` Support |
| :--- | :--- | :--- |
| **`arm64-v8a` (64-bit ARM)** | Android 9+ (API 28) | ✅ **Full `.hmod` support** |
| **`x86_64` (64-bit Intel/AMD)** | Android 9+ (API 28) | ✅ **Full `.hmod` support** |
| `armeabi-v7a` / `x86` (32-bit) | Android 8.1+ (API 27) | ⚠️ Legacy experimental build (omits 64-bit `.hmod` runtime) |

1. Install the 64-bit APK (`arm64-v8a` for almost all modern phones/tablets) from [Vasysik/horizon-android Releases](https://github.com/Vasysik/horizon-android/releases).
2. Open Horizon Android and tap **Download Minecraft 26.3** (~500 MiB initial resource verification) if you haven't already.

---

## 2. Method A: Direct HTTPS `.hmod` URL Import (Recommended)

Horizon Android's built-in launcher lets you import `.hmod` packages directly via an HTTPS link:

1. Open **Horizon Android** to the main **Launcher** screen.
2. Tap the **Mods** tab.
3. Paste one of the direct HTTPS `.hmod` URLs below into the URL import field:

   - **Rust Scrap & Base Armory (`rust_scrap_armory.hmod`):**
     ```text
     https://raw.githubusercontent.com/entitybrian69-bit/Rust-store-/main/releases/rust_scrap_armory.hmod
     ```
   - **Glowing Amethyst (`glowing_amethyst.hmod`):**
     ```text
     https://raw.githubusercontent.com/entitybrian69-bit/Rust-store-/main/releases/glowing_amethyst.hmod
     ```
   - **Ruby Expansion (`ruby_expansion.hmod`):**
     ```text
     https://raw.githubusercontent.com/entitybrian69-bit/Rust-store-/main/releases/ruby_expansion.hmod
     ```
   - **Industrial Pipes & Grates (`industrial_pipes.hmod`):**
     ```text
     https://raw.githubusercontent.com/entitybrian69-bit/Rust-store-/main/releases/industrial_pipes.hmod
     ```
   - **Dungeon Mobs & Frost Magic (`dungeon_mobs_plus.hmod`):**
     ```text
     https://raw.githubusercontent.com/entitybrian69-bit/Rust-store-/main/releases/dungeon_mobs_plus.hmod
     ```
   - **Buzzing Apiary (`buzzing_apiary.hmod`):**
     ```text
     https://raw.githubusercontent.com/entitybrian69-bit/Rust-store-/main/releases/buzzing_apiary.hmod
     ```

4. Tap **Import / Download**.
5. Go to the **Installed** tab in the Launcher and toggle the mod **ON** (newly imported packages start disabled by default).
6. Launch the game (**Play**), open **Options → Mods**, verify the switch is turned **ON**, and restart the game if the mod adds new blocks, items, mob effects, or entity types.

---

## 3. Method B: Importing `.hmod` Files via Android System Files App

Recent builds of Horizon Android expose its `mods` directory inside the Android system **Files** app:

1. Download any `.hmod` file from the [`releases/`](../releases/) folder onto your phone.
2. Close Horizon Android completely.
3. Open the Android **Files** app (Google Files / DocumentsUI), open the left sidebar, and select **Horizon Android**.
4. Copy your downloaded `.hmod` file into the **`mods`** folder (`horizon-mods/`).
5. Launch **Horizon Android**, go to **Options → Mods**, and toggle the mod **ON**.

---

## 4. Troubleshooting on Android

- **Mod shows "Restart required" in `Options → Mods`:**
  - Mods that only change textures/recipes reload immediately, while mods with `capabilities = ["registries"]` (adding blocks, items, mob effects, entity types, or menus) load at game startup. Toggle the mod **ON** while outside a world, then close and reopen the game.
- **Why won't `.hnmod` or Fabric/Forge `.jar` files load on Android?**
  - `.hnmod` files are source-only Rust mixins that require desktop `cargo` compilation by `horizon-launcher`. They cannot be compiled on-device on Android unless baked into a custom APK build.
  - Java `.jar` mods (Fabric, Forge, NeoForge) target the JVM and do not run on Horizon's native Rust engine. Use `.hmod` packages instead.
