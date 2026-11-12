# Volt Survival Suite — community code HMOD

Source project: https://github.com/entitybrian69-bit/Volt-launcher/tree/android/mods/community/volt-survival-suite

Build workflow: https://github.com/entitybrian69-bit/Volt-launcher/actions/workflows/build-volt-survival-suite.yml

Volt Survival Suite is a client-side WebAssembly mod for Horizon. It adds a live survival HUD
(coordinates, facing, health, hunger, dimension and world time), a six-block target inspector,
a persistent HUD toggle, and the /kit, /where, /inspect and /time commands.

This is a code HMOD containing horizon-mod.toml and mod.wasm, not a Java .jar, texture pack
or content-only HMOD. It uses only game-read, hud, chat and storage capabilities.

The workflow runs unit tests, builds the WASM module, componentizes it with wasm-tools,
packages the component as a ZIP-compatible .hmod, validates the archive and uploads the
installable file as a GitHub Actions artifact. The artifact is not a signed Horizon registry
release; only install it after the workflow reports success.

Install on supported 64-bit Horizon Android:
1. Open the workflow link and select a successful run.
2. Download and extract the volt-survival-suite-hmod artifact.
3. Import volt_survival_suite-0.1.0.hmod through Mods > Import file (or HTTPS import).
4. Enable Volt Survival Suite under Options > Mods and restart the game.

The mod cannot read inventory, armor, biome or entity state through the APIs it requests.
