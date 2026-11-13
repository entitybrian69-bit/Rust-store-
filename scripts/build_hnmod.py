#!/usr/bin/env python3
"""Build and validate Horizon native mods: the .hnmod and the combined .hmod.

Zero dependencies: Python 3.10+ and the standard library only.

Two packages come out of one source folder, the same mod:

  * `fresh_moves-1.0.0.hnmod` — code only: `horizon-native.json` and
    `mixins/fresh_moves/`. What a registry native mod and Android's build
    service take.
  * `fresh_moves-1.0.0.hmod` — the combined mod: the same native bundle plus
    `horizon-mod.toml`. Horizon installs it as one `.hmod` under a version's
    native mods and snapshots its content with the exact native build
    (`launcher/src/native_mods/source.rs`, `install_content`).

The rules enforced here are the ones in Horizon's own readers and spec:

  * docs/api/mod-files.md          the zip's shape, paths, limits and entries
  * crates/horizon-mods/src/index/native.rs
                                   the native manifest's fields, what a bundle
                                   may carry under mixins/<id>/, and how a
                                   combined `.hmod`'s content half is read
  * crates/horizon-mods/src/manifest.rs
                                   `Package::read`: the content manifest, and
                                   `registries` when there is no `mod.wasm`
  * native-hooks.json              every name a hook may ask for

Usage:

    python3 scripts/build_hnmod.py                        # build + validate both
    python3 scripts/build_hnmod.py --check-only            # validate only
    python3 scripts/build_hnmod.py --check-only path       # validate one archive

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
CONTENT = SOURCE / "content"
RELEASES = ROOT / "releases"
CATALOG = ROOT / "scripts" / "native-hooks-catalog.json"

MANIFEST = "horizon-native.json"
MIXIN_MANIFEST = "horizon-mixin.toml"
# The content half of a combined `.hmod`: a `.hmod`'s own manifest, read by
# `horizon_mods::Package::read`. A bare `.hmod` without a native manifest is a
# content mod and not this script's business.
CONTENT_MANIFEST = "horizon-mod.toml"
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


def is_content(name: str) -> bool:
    """Whether a combined `.hmod`'s path is content rather than native source:
    `horizon_mods::index::native::content`. Content sits at the archive's top
    level and never inside `mixins/<id>/`."""
    return name == CONTENT_MANIFEST or name.startswith(("assets/", "data/"))


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


def check_entries(manifest: dict, names: list[str], files: dict[str, bytes], combined: bool) -> None:
    """The archive's paths, as `Bundle::read` checks them."""
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
        if combined and is_content(name):
            # A combined `.hmod`: `horizon_mods::index::native::content`.
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


def check_content(manifest: dict, names: list[str], files: dict[str, bytes]) -> dict:
    """The content half of a combined `.hmod`, as `Package::read` and
    `Content::read` check it."""
    if CONTENT_MANIFEST not in names:
        fail(f"a combined .hmod needs {CONTENT_MANIFEST}")
    if "mod.wasm" in names:
        fail("mod.wasm is gone: a .hmod carries native code or content, not WebAssembly")
    try:
        content = tomllib.loads(files[CONTENT_MANIFEST].decode("utf-8"))
    except (tomllib.TOMLDecodeError, UnicodeDecodeError) as e:
        fail(f"{CONTENT_MANIFEST} is not TOML: {e}")
    for field in ("id", "name", "version", "api"):
        if not isinstance(content.get(field), str) or not content[field]:
            fail(f"{CONTENT_MANIFEST} is missing {field!r}")
    if content["id"] != manifest["id"]:
        fail(
            f"{CONTENT_MANIFEST} is mod {content['id']!r}, the native manifest "
            f"{manifest['id']!r}: one archive is one mod"
        )
    if content["version"] != manifest["version"]:
        fail(
            f"{CONTENT_MANIFEST} is version {content['version']!r}, the native manifest "
            f"{manifest['version']!r}"
        )
    capabilities = content.get("capabilities", [])
    if not isinstance(capabilities, list) or "registries" not in capabilities:
        fail(
            f"{CONTENT_MANIFEST} needs capabilities = [\"registries\"]: a mod with no "
            "mod.wasm may only be a content mod"
        )
    namespaces = [content["id"], *content.get("namespaces", [])]
    for name in names:
        if not is_content(name) or name == CONTENT_MANIFEST:
            continue
        # `content::check_path`: assets/<namespace>/<file> or data/<namespace>/<file>,
        # every part lowercase letters, digits, `_`, `.` or `-`.
        parts = name.split("/")
        if parts[0] not in ("assets", "data") or len(parts) < 3:
            fail(f"{name}: content is assets/<namespace>/<file> or data/<namespace>/<file>")
        for part in parts:
            if not part or part in (".", "..") or any(
                not (c.islower() or c.isdigit() or c in "_.-") for c in part
            ):
                fail(f"{name}: content paths take a-z, 0-9, _, . and - only")
        # A mod adds files in its own namespaces, its listed ones, or vanilla
        # and convention tags and atlases.
        if parts[1] not in namespaces and not (
            parts[0] == "data" and parts[1] in ("minecraft", "c") and "/tags/" in name
        ):
            fail(f"{name}: a mod adds content in its own namespaces ({', '.join(namespaces)})")
    return content


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


def check_bundle(
    manifest: dict,
    mixin: dict,
    names: list[str],
    files: dict[str, bytes],
    catalog: dict,
    combined: bool = False,
) -> dict:
    check_entries(manifest, names, files, combined)
    content = check_content(manifest, names, files) if combined else {}
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

    # The content half must be the source folder's copy too, byte for byte.
    for name, data in files.items():
        if combined and is_content(name):
            disk = CONTENT / name
            if disk.is_file() and disk.read_bytes() != data:
                fail(f"{name}: the archive's copy differs from {disk}")
    return content


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

def write_archive(target: Path, entries: dict[str, bytes]) -> None:
    """A deterministic zip: fixed timestamps, sorted names, deflate."""
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name in sorted(entries):
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            archive.writestr(info, entries[name])


def native_entries(source: Path) -> dict[str, bytes]:
    """`horizon-native.json` and `mixins/<id>/`, as they sit in an archive."""
    return {
        path.relative_to(source).as_posix(): path.read_bytes()
        for path in source_files(source)
        if not path.is_relative_to(CONTENT)
    }


def content_entries() -> dict[str, bytes]:
    """`horizon-mod.toml` and the assets and data beside it, at the archive's
    top level (the content half of a combined `.hmod`)."""
    return {
        path.relative_to(CONTENT).as_posix(): path.read_bytes()
        for path in source_files(CONTENT)
    }


def build(source: Path, content: Path, out_dir: Path) -> list[Path]:
    """Writes both packages of the one mod: the code-only `.hnmod` and the
    combined `.hmod` (the same native bundle plus its content manifest)."""
    manifest = check_manifest(read_text(source / MANIFEST))
    out_dir.mkdir(parents=True, exist_ok=True)
    native = native_entries(source)
    named = f"{manifest['id']}-{manifest['version']}"

    hnmod = out_dir / f"{named}.hnmod"
    write_archive(hnmod, native)

    hmod = out_dir / f"{named}.hmod"
    combined = dict(native)
    combined.update(content_entries() if content.is_dir() else {})
    write_archive(hmod, combined)
    return [hnmod, hmod]


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
    parser.add_argument("archive", nargs="?", help="a built .hnmod or combined .hmod to check")
    parser.add_argument("--check-only", action="store_true", help="check without building")
    parser.add_argument("--source", type=Path, default=SOURCE, help="the unpacked bundle")
    parser.add_argument("--content", type=Path, default=CONTENT, help="the content half of the .hmod")
    parser.add_argument("--out", type=Path, default=RELEASES, help="where the packages go")
    args = parser.parse_args()

    catalog = load_catalog()

    if args.archive:
        targets = [Path(args.archive)]
    elif args.check_only:
        # Every native package in releases: the .hnmod files, and the .hmod
        # files that carry a native manifest. A plain content .hmod is another
        # script's business and is left to build_hmods.py.
        targets = []
        for path in sorted(args.out.glob("*.hnmod")) + sorted(args.out.glob("*.hmod")):
            with zipfile.ZipFile(path) as archive:
                if MANIFEST in archive.namelist():
                    targets.append(path)
        if not targets:
            print(f"no native package in {args.out}; build one first", file=sys.stderr)
            return 1
    else:
        built = build(args.source, args.content, args.out)
        for path in built:
            print(f"built {path}")
        targets = built

    failures = 0
    for path in targets:
        try:
            manifest, mixin, names, files = read_bundle(path)
            combined = CONTENT_MANIFEST in files
            content = check_bundle(manifest, mixin, names, files, catalog, combined)
            injections = mixin.get("injection", [])
            modules = mixin.get("module", [])
            kind = "combined .hmod" if combined else ".hnmod"
            extra = f", content {CONTENT_MANIFEST}" if combined else ""
            print(f"ok  {path.name}: {kind}, {manifest['id']} {manifest['version']}, "
                  f"{len(names)} entries, {len(injections)} hooks, {len(modules)} modules{extra}")
            if combined and not content.get("description"):
                print(f"    note: {CONTENT_MANIFEST} has no description")
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
