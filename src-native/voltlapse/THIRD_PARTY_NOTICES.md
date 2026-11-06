# Third-party notices

Volt Lapse is licensed under the GNU Lesser General Public License, version 3 or
later (SPDX `LGPL-3.0-or-later`). The full text is in `LICENSE`; every source
file carries a four-line SPDX header naming the same licence.

## Replay Mod

Volt Lapse's feature design and file-format shape follow **Replay Mod**
(Copyright © 2014–2026 Johni0702 and the Replay Mod contributors), which is
licensed under the GNU Lesser General Public License, version 3.

What that means here:

* The *ideas* — a zip replay with `metaData.json` and a separate stream entry,
  camera paths with keyframes, markers, thumbnails, a render queue — are
  Replay Mod's, and are credited as such in `NOTICE`, the crate documentation
  and the banner of every file that implements them.
* No Replay Mod source code is copied into this repository. The engine is an
  independent implementation of those ideas against Horizon's native API.
* Volt Lapse's replay format is **not** Replay Mod's: `.vrc` holds this
  engine's tick stream, `.mcpr` holds Minecraft packets. The metadata keeps
  Replay Mod's field names where the meaning is identical, so tooling that
  reads `metaData.json` can read ours.

## Horizon and horizon-mixins

* **Horizon** (the game client Volt Lapse is a mod for) is MIT licensed. Volt
  Lapse links against its hook points, which are public API.
* **horizon-mixins** by levzzz5154, MIT licensed, is the tool that stages and
  injects native mods. Volt Lapse ships bundles for it and links nothing from
  it; the native manifest records the revision it was built with.

## The LGPL text

`LICENSE` is the verbatim LGPL-3.0 text from the SPDX license list
(`spdx/license-list-data`), not from any downstream copy, so the wording is
unmodified. `mixins/voltlapse/LICENSE` is the same file, shipped inside the
package next to `NOTICE` so a recipient of the `.hnmod` has both.

## Everything else

No other third-party code is included. The build tool
(`tools/build.py`) and the tests use only the Python standard library; the
engine uses only the Rust standard library and the Horizon API.
