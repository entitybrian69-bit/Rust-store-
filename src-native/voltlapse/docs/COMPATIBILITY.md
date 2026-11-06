# Compatibility

`compatibility.json` at the bundle root is the machine-readable version of this
page. It is kept in sync with `horizon-native.json` (the revision pins) and is
checked by `tools/build.py`'s tests.

## Verified against

| | |
| --- | --- |
| Game | Minecraft Java **26.3** (Horizon client) |
| Horizon commit | `41f822b6` (tested) |
| horizon-mixins | `5a64af93` |
| Native hook API | **410** |

The bundle hooks only Horizon's *named* hook points, never a raw function
target, and declares `"hooks": 410` instead of pinning to a commit. Hook points
are only ever added and each addition raises `NATIVE_HOOKS_API`, so Volt Lapse
builds on any Horizon whose hook API is 410 or later. `horizon` in the manifest
records where it was tested; the launcher may build it on another commit after
asking.

### Hook points used

| Hook point | API |
| --- | --- |
| `gui_render_post` | 410 |
| `client_packet` | 408 |
| `client_chat_received` | 403 |
| `camera_pose` | 402 |
| `compute_fov` | 362 |
| `frame`, `tick`, `hud`, `key`, `mods?`, `settings`, `command`, … | 410 (they are inside the same file and carried along) |

`hooks: 410` is the maximum, which is what the manifest wants.

## Minecraft versions

Volt Lapse is a client mod: it records what the client already has, so the
*server* can be any version the Horizon client can join — vanilla, proxies,
ViaVersion setups, a 1.8 anarchy server — and nothing needs installing on the
server side. The list in `compatibility.json` (`server_versions`) is what has
been exercised, not a range of game versions the mod supports: the client is
26.3, full stop.

Replays are self-contained. A `.vrc` recorded on one machine plays on any other
running a Volt Lapse build that understands format version 1.

## Loaders

Volt Lapse does not use Forge, Fabric, Quilt or NeoForge, and it does not need
them: it is a Horizon native mod compiled into the game. Mixing it with Java
mod loaders is not applicable; mixing it with Horizon content mods is fine.

## Shaders and graphics mods

| Thing | Status | Notes |
| --- | --- | --- |
| OptiFine shaders | supported | Volt Lapse reads the camera after OptiFine's placement, so a cinematic camera still wins. |
| Iris / Sodium packs | supported | The renderer captures the post-shader frame. |
| Horizon shader packs (`native_shader_params`) | supported | Frame capture happens after the pack's pass. |
| Custom GUI scale / wide resolutions | supported | The viewer lays itself out in GUI space and clamps to the screen. |
| Vulkan vs OpenGL backends | same | The engine works in GUI space and reads the finished frame; no backend-specific call is made. |

## Servers and Realms

* **Single player**: recorded with the integrated server; no host needed.
* **Realms**: recorded like any server — the mod never asks the server for
  anything.
* **Servers with anti-cheat**: Volt Lapse sends no packets on its own. The only
  packets it can send are the ones a chat command produces, and it never
  automates movement. Check the server's rules anyway: recording is usually
  fine, automating playback on a live server is not, and playback is
  client-side only (it re-renders captured state, it does not rewind the
  server).
* **`/voltlapse share`**: hosted through a session the mod opens locally; a
  server plugin is not required and cannot help.

## Known limitations

* Replay Mod `.mcpr` files cannot be imported: the two formats store different
  streams (packets vs ticks). `NOTICE` says so; the export path writes `.vrc`
  (and can copy the metadata out for tools that read Replay Mod metadata).
* Rendering needs a window to render in: it runs on the client's render thread,
  so it cannot run headless or on a server.
* Android builds are supported, but on-device rendering is limited by memory;
  see `INSTALL_ANDROID.md`.
* A replay recorded with a newer format/opcode version is refused with a
  message rather than partially decoded.
