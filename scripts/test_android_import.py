#!/usr/bin/env python3
"""CI mirror of Volt Android's import validation for .hmod files.

Faithful Python port of `content_files::validate(path, Kind::Mods, name, ..)`
in Volt-Launcher `crates/horizon-client/src/content_files.rs` (branch
fix-android-content-mod-import), which is the exact function the Android
importer runs (`crates/horizon-client/src/android/documents.rs`: "Import
failed: {error}"). The Rust unit tests in that file cover the same cases;
this port exists so this repo's CI can check every released .hmod against
the importer's rules without a Rust toolchain.

Rules ported (Mods kind): plain .hmod file name; file <= 32 MiB; <= 32768
entries; no empty/control/backslash/colon/absolute/./.. paths; no duplicate
paths; no symlinks; expanded total <= 64 MiB; and the ZIP-root requirement:
horizon-mod.toml at the root AND (mod.wasm OR any assets/ or data/ file) —
content-only mods are legal per horizon-mods `Package::read`.
Exit 1 if any releases/*.hmod is rejected.
"""
import sys, zipfile
from pathlib import Path
ROOT = Path(__file__).resolve().parent.parent

def plain_name(name):
    return (name and len(name) <= 180 and not name.startswith('.')
            and not any(c in name for c in '/\\:\0') and not any(ord(c) < 32 for c in name))

def validate_mods(path: Path) -> str | None:
    """None = accepted (like Ok(..)); str = the importer's rejection reason."""
    name = path.name
    if not plain_name(name) or not name.lower().endswith(".hmod"):
        return "Choose a ZIP pack, .hshader shader pack or sandboxed .hmod matching this folder"
    if path.stat().st_size > 32 << 20: return "Horizon mod exceeds 32 MiB"
    try: zf = zipfile.ZipFile(path)
    except Exception as e: return f"zip error: {e}"
    infos = zf.infolist()
    if len(infos) > 32768: return "Archive contains too many files"
    names, total, manifest, wasm, content = set(), 0, False, False, False
    for info in infos:
        raw = info.filename
        mode = (info.external_attr >> 16) & 0o170000
        if (not raw or len(raw) > 1024 or '\\' in raw or ':' in raw or raw.startswith('/')
                or any(p in ("..", ".") for p in raw.split('/')) or raw in names or mode == 0o120000):
            return "Archive contains an unsafe or duplicate path"
        names.add(raw)
        total += info.file_size
        if total > 64 << 20: return "Expanded archive exceeds 512 MiB"
        manifest |= raw == "horizon-mod.toml"
        wasm |= raw == "mod.wasm"
        content |= (not info.is_dir()) and (raw.startswith("assets/") or raw.startswith("data/"))
    if not (manifest and (wasm or content)):
        return "Archive is not a Mods package (required files must be at the ZIP root)"
    return None

def main():
    files = sorted((ROOT/"releases").glob("*.hmod"))
    bad = 0
    for f in files:
        err = validate_mods(f)
        print(f"{'OK  ' if err is None else 'FAIL'} {f.name}" + ("" if err is None else f": Import failed: {err}"))
        bad += err is not None
    print(f"android-import validation: {len(files)-bad}/{len(files)} accepted")
    return 1 if bad else 0

if __name__ == "__main__": sys.exit(main())
