# 🖥️ Installing Horizon `.hmod` & `.hnmod` Mods on PC

This guide covers installing both **Content Mods (`.hmod`)** and **Native Rust Mods (`.hnmod`)** on desktop builds of **[Horizon](https://github.com/horizon-rust/horizon)** (Windows, Linux, and macOS) for Minecraft: Java Edition **26.3**.

---

## 1. Installing `.hmod` Content Mods

All mods in this repository's [`releases/`](../releases/) directory are `.hmod` content mods (`api = "0.2"`).

### Step-by-Step (Manual Folder Install)
1. Download the desired `.hmod` file from [`releases/`](../releases/):
   - [`rust_scrap_armory.hmod`](../releases/rust_scrap_armory.hmod)
   - [`glowing_amethyst.hmod`](../releases/glowing_amethyst.hmod)
   - [`ruby_expansion.hmod`](../releases/ruby_expansion.hmod)
   - [`industrial_pipes.hmod`](../releases/industrial_pipes.hmod)
   - [`dungeon_mobs_plus.hmod`](../releases/dungeon_mobs_plus.hmod)
   - [`buzzing_apiary.hmod`](../releases/buzzing_apiary.hmod)
2. Locate your Horizon game directory (the folder containing `options.txt`).
3. Create a folder named **`horizon-mods`** if it does not already exist:
   ```text
   <game-folder>/
   ├── options.txt
   ├── horizon-mods.txt
   └── horizon-mods/
       ├── rust_scrap_armory.hmod
       ├── glowing_amethyst.hmod
       ├── ruby_expansion.hmod
       ├── industrial_pipes.hmod
       ├── dungeon_mobs_plus.hmod
       └── buzzing_apiary.hmod
   ```
4. Start Horizon, navigate to **Options → Mods**, and flip the switch **ON** next to each mod you want active.
   - Your enabled choices are saved in `horizon-mods.txt` as `mod.<id>:true` (for example, `mod.rust_scrap_armory:true`).
5. **Restart Horizon** so the game builds the block, item, entity, and creative tab registries on startup.

---

## 2. Running a Mod Directly with `horizon-client` (Developer Mode)

You can also test any `.hmod` archive or unpacked source folder from [`src-mods/`](../src-mods/) without copying it into `horizon-mods/`:

```bash
# Run with a packaged .hmod always enabled:
horizon-client --dev-mod releases/rust_scrap_armory.hmod

# Or run using cargo-horizon-mod from an unpacked source folder:
cd src-mods/rust_scrap_armory
cargo horizon-mod run
```

---

## 3. Multiplayer & LAN Server Sync (`horizon:mod_list`)

All `.hmod` packages in this store declare `side = "both"` in `horizon-mod.toml`:
- During the connection handshake (`horizon:mod_list`), a Horizon server sends the **id**, **version**, and **SHA-256 hash** of every enabled `side = "both"` `.hmod`.
- Every connecting PC or Android player must have the exact same `.hmod` files enabled, or the server will refuse the connection and list which mod is missing or mismatched.
- Client-only mods (`side = "client"`, such as HUDs or minimaps) never block joining a server.

---

## 4. Installing `.hnmod` Native Rust Mods (PC Only)

Native mods (`.hnmod`) are [`horizon-mixins`](https://github.com/horizon-rust/horizon-mixins) source bundles that patch Horizon's Rust source code and are compiled into a custom client binary by the Horizon Launcher:

1. Place `.hnmod` files into `<game-folder>/horizon-native-mods/`.
2. Open **Horizon Launcher** (or run `horizon-launcher native-build <path> --trust-native-code`) to compile and launch the native-modded client.
