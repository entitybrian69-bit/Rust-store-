#!/usr/bin/env python3
"""Build and validate a Horizon native mod (.hnmod) bundle.

Zero dependencies: Python 3.10+ and the standard library only.

The rules enforced here are the ones in Horizon's own readers and spec:

  * docs/api/mod-files.md          the zip's shape, paths, limits and entries
  * crates/horizon-mods/src/index/native.rs
                                   the manifest's fields and what a bundle may
                                   carry under mixins/<id>/
  * native-hooks.json              every name a hook may ask for

Usage:

    python3 scripts/build_hnmod.py                       # build + validate
    python3 scripts/build_hnmod.py --check-only           # validate only
    python3 scripts/build_hnmod.py --check-only path.hnmod

Exit status is 0 only when every check passes.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import tomllib
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "src-mods" / "fresh_moves"
RELEASES = ROOT / "releases"
CATALOG = ROOT / "scripts" / "native-hooks-catalog.json"

MANIFEST = "horizon-native.json"
MIXIN_MANIFEST = "horizon-mixin.toml"

# docs/api/mod-files.md, "Limits".
MAX_ENTRIES = 512
MAX_UNPACKED = 64 * 1024 * 1024
MAX_FILE = 64 * 1024 * 1024

# docs/api/mod-files.md: a path is relative, `/`-separated, and has no empty,
# `.` or `..` part.
PATH_PART = re.compile(r"^[A-Za-z0-9_.\-@+]+$")
ID = re.compile(r"^[a-z0-9_]{1,64}$")
REVISION = re.compile(r"^[0-9a-f]{40}$")
CONTENT_PATH = re.compile(r"^(assets|data)/[a-z0-9_.\-]+(/[a-z0-9_.\-]+)+$")

# crates/horizon-mods/src/index/native.rs, `allowed`.
ALLOWED_NAMES = re.compile(r"^(README|LICENSE|LICENCE|COPYING|NOTICE)[^/]*$")


class Problem(Exception):
    """A check that failed, with the reason a reader would give."""


def fail(message: str) -> None:
    raise Problem(message)


# ---------------------------------------------------------------------------
# The source folder
# ---------------------------------------------------------------------------

def source_files(folder: Path) -> list[Path]:
    if not folder.is_dir():
        fail(f"{folder} is not a folder")
    files = sorted(p for p in folder.rglob("*") if p.is_file())
    if not files:
        fail(f"{folder} holds no files")
    return files


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


# ---------------------------------------------------------------------------
# Checks shared by the source folder and a packed archive
# ---------------------------------------------------------------------------

def check_manifest(text: str) -> dict:
    try:
        manifest = json.loads(text)
    except json.JSONDecodeError as e:
        fail(f"{MANIFEST} is not JSON: {e}")
    if not isinstance(manifest, dict):
        fail(f"{MANIFEST} is not a JSON object")

    # `#[serde(deny_unknown_fields)]` on the manifest struct.
    known = {"schema", "id", "version", "game", "horizon", "mixins", "hooks"}
    unknown = set(manifest) - known
    if unknown:
        fail(f"{MANIFEST} has fields Horizon refuses: {sorted(unknown)}")

    for field in ("schema", "id", "version", "game", "horizon", "mixins"):
        if field not in manifest:
            fail(f"{MANIFEST} is missing {field!r}")
    if manifest["schema"] != 1:
        fail(f"{MANIFEST}: schema must be 1, not {manifest['schema']!r}")
    if not isinstance(manifest["id"], str) or not ID.match(manifest["id"]):
        fail(f"{MANIFEST}: id must match [a-z0-9_]{{1,64}}")
    for field in ("version", "game"):
        if not isinstance(manifest[field], str) or not manifest[field]:
            fail(f"{MANIFEST}: {field} must be a non-empty string")
    for field in ("horizon", "mixins"):
        value = manifest[field]
        if not isinstance(value, str) or not REVISION.match(value):
            fail(f"{MANIFEST}: {field} must be 40 lowercase hex characters")
    hooks = manifest.get("hooks")
    if hooks is not None:
        if not isinstance(hooks, int) or hooks < 1:
            fail(f"{MANIFEST}: hooks must be a positive integer when given")
    return manifest


def check_mixin_manifest(text: str, manifest: dict) -> dict:
    try:
        mixin = tomllib.loads(text)
    except tomllib.TOMLDecodeError as e:
        fail(f"{MIXIN_MANIFEST} is not TOML: {e}")

    mod = mixin.get("mod")
    if not isinstance(mod, dict):
        fail(f"{MIXIN_MANIFEST} has no [mod] table")
    if mod.get("id") != manifest["id"]:
        fail(f"{MIXIN_MANIFEST}: [mod] id is {mod.get('id')!r}, the manifest names {manifest['id']!r}")
    if mod.get("version") != manifest["version"]:
        fail(f"{MIXIN_MANIFEST}: [mod] version is {mod.get('version')!r}, the manifest names {manifest['version']!r}")

    compatibility = mixin.get("compatibility")
    if not isinstance(compatibility, dict):
        fail(f"{MIXIN_MANIFEST} has no [compatibility] table")
    if compatibility.get("horizon_revision") != manifest["horizon"]:
        fail(
            f"{MIXIN_MANIFEST}: [compatibility] horizon_revision is "
            f"{compatibility.get('horizon_revision')!r}, the manifest names {manifest['horizon']!r}"
        )
    return mixin


def check_entries(manifest: dict, names: list[str], files: dict[str, bytes]) -> None:
    """The archive's paths, as `Bundle::read_format` checks them."""
    if len(names) > MAX_ENTRIES:
        fail(f"the bundle has {len(names)} entries, the limit is {MAX_ENTRIES}")
    total = sum(len(v) for v in files.values())
    if total > MAX_UNPACKED:
        fail(f"the bundle unpacks to {total} bytes, the limit is {MAX_UNPACKED}")
    for name in names:
        if len(files.get(name, b"")) > MAX_FILE:
            fail(f"{name} is larger than {MAX_FILE} bytes")
        if name.startswith("/") or name.endswith("/"):
            fail(f"{name}: a path starts with neither / nor ends with one")
        if "\\" in name or ":" in name:
            fail(f"{name}: a path has no \\ and no :")
        for part in name.split("/"):
            if part in ("", ".", ".."):
                fail(f"{name}: a path has no empty, . or .. part")

    if MANIFEST not in names:
        fail(f"the bundle has no {MANIFEST}")
    prefix = f"mixins/{manifest['id']}/"
    for name in names:
        if name == MANIFEST:
            continue
        rest = name[len(prefix):] if name.startswith(prefix) else None
        if rest is None:
            fail(f"unexpected native source: {name}")
        if not allowed(rest):
            fail(f"unexpected native source: {name}")

    if f"{prefix}{MIXIN_MANIFEST}" not in names:
        fail(f"the bundle has no {prefix}{MIXIN_MANIFEST}")


def allowed(rest: str) -> bool:
    """`allowed` in crates/horizon-mods/src/index/native.rs."""
    if rest.endswith(".rs") or rest.endswith(".toml"):
        return True
    if rest.startswith("data/"):
        return True
    # A top-level folder's own data and licence files.
    first, _, tail = rest.partition("/")
    if first and tail:
        if tail.startswith("data/"):
            return True
        if ALLOWED_NAMES.match(tail.split("/")[0]):
            return True
    if "/" not in rest and ALLOWED_NAMES.match(rest):
        return True
    if rest == ".gitignore":
        return True
    return False


def check_injections(manifest: dict, mixin: dict, names: set[str], catalog: dict) -> None:
    prefix = f"mixins/{manifest['id']}/"
    if not isinstance(mixin.get("injection", []), list):
        fail(f"{MIXIN_MANIFEST}: [[injection]] entries must be an array of tables")

    modules = mixin.get("module", [])
    sources: set[str] = set()
    for module in modules:
        for field in ("id", "file", "name", "source"):
            if field not in module:
                fail(f"{MIXIN_MANIFEST}: a [[module]] has no {field}")
        source = module["source"]
        sources.add(source)
        if f"{prefix}{source}" not in names:
            fail(f"{MIXIN_MANIFEST}: module {module['name']!r} reads {source}, which the bundle lacks")
        if not str(module["file"]).endswith(".rs"):
            fail(f"{MIXIN_MANIFEST}: module {module['name']!r} is not placed in a .rs file")

    if not modules and not mixin.get("injection"):
        fail(f"{MIXIN_MANIFEST} declares neither a module nor an injection")

    for injection in mixin.get("injection", []):
        for field in ("id", "kind", "helper"):
            if field not in injection:
                fail(f"{MIXIN_MANIFEST}: an [[injection]] has no {field}")
        helper = injection["helper"]
        if f"{prefix}{helper}" not in names:
            fail(f"{MIXIN_MANIFEST}: injection {injection['id']!r} uses helper {helper}, which the bundle lacks")
        if not helper.endswith(".rs"):
            fail(f"{MIXIN_MANIFEST}: helper {helper} is not a .rs file")
        if "hook" in injection:
            if "target" in injection:
                fail(f"{MIXIN_MANIFEST}: injection {injection['id']!r} names both a hook and a target")
            name = injection["hook"]
            version = injection.get("hook_version")
            if name not in catalog:
                fail(f"{MIXIN_MANIFEST}: injection {injection['id']!r} names hook {name!r}, which native-hooks.json lacks")
            if version is not None and int(version) not in catalog[name]:
                fail(
                    f"{MIXIN_MANIFEST}: injection {injection['id']!r} needs hook {name!r} version {version}, "
                    f"the catalog has {sorted(catalog[name])}"
                )
        elif "target" not in injection:
            fail(f"{MIXIN_MANIFEST}: injection {injection['id']!r} names no hook and no target")
        if not isinstance(injection.get("bindings", []), list):
            fail(f"{MIXIN_MANIFEST}: injection {injection['id']!r} has bindings that are not a table array")
        for binding in injection.get("bindings", []):
            for field in ("name", "ty", "value"):
                if field not in binding:
                    fail(f"{MIXIN_MANIFEST}: injection {injection['id']!r} has a binding without {field}")


def check_rust(name: str, text: str) -> None:
    """The rules horizon-mixins puts on injected source (README, template)."""
    # A module payload is inserted inside the game's own module: it may not be
    # a crate of its own.
    if "//!" in text:
        fail(f"{name}: a module payload cannot use inner doc comments (//!)")
    if "#![allow" in text or "#![" in text:
        fail(f"{name}: a module payload cannot use inner attributes (#![...])")
    # An external module (`mod name;`) would be a file the bundle has not got.
    # Inline modules (`mod name { ... }`) are allowed and used here.
    if re.search(r"^\s*(pub\s+)?mod\s+\w+\s*;", text, re.MULTILINE):
        fail(f"{name}: a module payload cannot declare an external mod file")
    if "include!" in text:
        fail(f"{name}: a module payload cannot use include!")
    for forbidden in ("todo!", "unimplemented!", "unreachable!", "panic!"):
        if forbidden in text:
            fail(f"{name}: {forbidden} has no place in a shipped mod")
    if re.search(r"\bunsafe\s*(fn|impl|trait|\{)", text):
        fail(f"{name}: the mod uses no unsafe code")
    if text.count("{") != text.count("}"):
        fail(f"{name}: braces do not balance ({text.count('{')} open, {text.count('}')} close)")
    if text.count("(") != text.count(")"):
        fail(f"{name}: parentheses do not balance")


def check_bundle(manifest: dict, mixin: dict, names: list[str], files: dict[str, bytes], catalog: dict) -> None:
    check_entries(manifest, names, files)
    prefix = f"mixins/{manifest['id']}/"
    check_injections(manifest, mixin, set(names), catalog)

    helper_fns: dict[str, str] = {}
    for name in names:
        if not name.endswith(".rs"):
            continue
        try:
            text = files[name].decode("utf-8")
        except UnicodeDecodeError:
            fail(f"{name} is not UTF-8")
        check_rust(name, text)
        short = name[len(prefix):] if name.startswith(prefix) else name
        # A helper is one function; a module is a whole file of items.
        if short in {i["helper"] for i in mixin.get("injection", [])}:
            if text.count("fn ") != 1:
                fail(f"{name}: a helper holds one function, this holds {text.count('fn ')}")
            if helper_fns.get(short):
                fail(f"{name}: two injections share the helper file")
            helper_fns[short] = text
        if short.endswith(".rs") and short not in {m["source"] for m in mixin.get("module", [])}:
            if text.count("fn ") != 1:
                fail(f"{name}: a helper holds one function, this holds {text.count('fn ')}")

    # The Rust files on disk must be the ones the bundle carries, byte for byte.
    for name, data in files.items():
        if name.startswith(prefix):
            disk = SOURCE / "mixins" / manifest["id"] / name[len(prefix):]
            if disk.is_file() and disk.read_bytes() != data:
                fail(f"{name}: the bundle's copy differs from {disk}")


# ---------------------------------------------------------------------------
# The catalog of hook names, from the game's own native-hooks.json
# ---------------------------------------------------------------------------

def load_catalog() -> dict[str, set[int]]:
    if not CATALOG.is_file():
        fail(f"{CATALOG} is missing: it is the game's native-hooks.json, reduced to names and versions")
    raw = json.loads(CATALOG.read_text(encoding="utf-8"))
    hooks = raw["hooks"] if isinstance(raw, dict) else raw
    catalog: dict[str, set[int]] = {}
    for hook in hooks:
        catalog.setdefault(hook["name"], set()).add(int(hook["version"]))
    return catalog


# ---------------------------------------------------------------------------
# Packing
# ---------------------------------------------------------------------------

def build(source: Path, out_dir: Path) -> Path:
    manifest = check_manifest(read_text(source / MANIFEST))
    out_dir.mkdir(parents=True, exist_ok=True)
    target = out_dir / f"{manifest['id']}-{manifest['version']}.hnmod"
    files = source_files(source)
    # Deterministic: fixed timestamps, sorted names, deflate.
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in files:
            name = path.relative_to(source).as_posix()
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            archive.writestr(info, path.read_bytes())
    return target


def read_bundle(path: Path) -> tuple[dict, dict, list[str], dict[str, bytes]]:
    if not path.is_file():
        fail(f"{path} does not exist")
    try:
        archive = zipfile.ZipFile(path)
    except zipfile.BadZipFile as e:
        fail(f"{path} is not a zip archive: {e}")
    with archive:
        names: list[str] = []
        files: dict[str, bytes] = {}
        for info in archive.infolist():
            if info.is_dir():
                # The readers skip directory entries; so does this.
                continue
            if info.compress_type not in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED):
                fail(f"{info.filename}: only stored and deflate entries are read")
            names.append(info.filename)
            files[info.filename] = archive.read(info)
        if len(names) != len(set(names)):
            fail(f"{path}: a path appears twice")
        if MANIFEST not in files:
            fail(f"{path}: missing {MANIFEST}")
        manifest = check_manifest(files[MANIFEST].decode("utf-8"))
        prefix = f"mixins/{manifest['id']}/"
        mixin_name = f"{prefix}{MIXIN_MANIFEST}"
        if mixin_name not in files:
            fail(f"{path}: missing {mixin_name}")
        mixin = check_mixin_manifest(files[mixin_name].decode("utf-8"), manifest)
    return manifest, mixin, names, files


def report(path: Path) -> None:
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    print(f"\n  file    {path.relative_to(ROOT) if path.is_relative_to(ROOT) else path}")
    print(f"  size    {len(data)} bytes")
    print(f"  sha256  {digest}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("archive", nargs="?", help="a built .hnmod to check")
    parser.add_argument("--check-only", action="store_true", help="check without building")
    parser.add_argument("--source", type=Path, default=SOURCE, help="the unpacked bundle")
    parser.add_argument("--out", type=Path, default=RELEASES, help="where the .hnmod goes")
    args = parser.parse_args()

    catalog = load_catalog()
    target = Path(args.archive) if args.archive else None

    if args.check_only and target is None:
        built = sorted(args.out.glob("*.hnmod"))
        if not built:
            print(f"no .hnmod in {args.out}; build one first", file=sys.stderr)
            return 1
        targets = built
    elif target is not None:
        targets = [target]
    else:
        built = build(args.source, args.out)
        print(f"built {built}")
        targets = [built]

    failures = 0
    for path in targets:
        try:
            manifest, mixin, names, files = read_bundle(path)
            check_bundle(manifest, mixin, names, files, catalog)
            injections = mixin.get("injection", [])
            modules = mixin.get("module", [])
            print(f"ok  {path.name}: {manifest['id']} {manifest['version']}, "
                  f"{len(names)} entries, {len(injections)} hooks, {len(modules)} modules")
        except Problem as e:
            failures += 1
            print(f"FAIL {path.name}: {e}", file=sys.stderr)
            continue
        report(path)

    if failures:
        print(f"\n{failures} of {len(targets)} failed", file=sys.stderr)
        return 1
    print(f"\nall {len(targets)} passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
