#!/usr/bin/env python3
"""Build and verify Volt Lapse's Horizon packages.

    python3 src-native/voltlapse/tools/build.py            # build both packages
    python3 src-native/voltlapse/tools/build.py --check     # verify what is there
    python3 src-native/voltlapse/tools/build.py --out DIR   # write somewhere else

Two artefacts, per Horizon's `docs/api/mod-files.md`:

* ``voltlapse.hnmod`` — native code only: ``horizon-native.json`` + ``mixins/voltlapse/``.
* ``voltlapse.hmod``  — the same native code plus the content half
  (``horizon-mod.toml``, ``assets/voltlapse/…``), which is what an Android
  launcher installs as one file.

Both are written with the standard library only, deterministically: sorted
paths, stored entries, a fixed timestamp and fixed Unix modes, so rebuilding
unchanged sources reproduces the same bytes and SHA-256.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import struct
import sys
import tomllib
import zipfile
from dataclasses import dataclass
from pathlib import Path

BUNDLE_ROOT = Path(__file__).resolve().parent.parent
REPO_ROOT = BUNDLE_ROOT.parent.parent
NATIVE_MANIFEST = "horizon-native.json"
MIXINS_DIR = "mixins"
MOD_ID = "voltlapse"

MOD_ID_RE = re.compile(r"^[a-z0-9_]{1,64}$")
CONTENT_PART_RE = re.compile(r"^[a-z0-9_.-]+$")
SHA1_RE = re.compile(r"^[0-9a-f]{40}$")

# Limits from the spec.
MAX_NATIVE_FILES = 512
MAX_NATIVE_BYTES = 64 * 1024 * 1024
MAX_CONTENT_FILES = 65536
MAX_CONTENT_FILE_BYTES = 16 * 1024 * 1024
MAX_CONTENT_BYTES = 512 * 1024 * 1024

STAMP = (2026, 1, 1, 0, 0, 0)
NATIVE_SUFFIXES = (".rs", ".toml")
NATIVE_DOC_PREFIXES = ("README", "LICENSE", "LICENCE", "COPYING", "NOTICE")


class BuildError(Exception):
    """A source tree that the spec would refuse, with the reason."""


@dataclass(frozen=True)
class Entry:
    path: str
    data: bytes

    @property
    def sha256(self) -> str:
        return hashlib.sha256(self.data).hexdigest()


def _read(path: Path) -> bytes:
    try:
        return path.read_bytes()
    except OSError as error:  # pragma: no cover - reported, not raised
        raise BuildError(f"cannot read {path}: {error}") from error


def _is_native_doc(name: str) -> bool:
    return name.startswith(NATIVE_DOC_PREFIXES) or name == ".gitignore"


def _check_path(path: str) -> None:
    if not path or path.startswith("/") or "\\" in path or ":" in path:
        raise BuildError(f"bad path {path!r}")
    parts = path.split("/")
    if any(part in ("", ".", "..") for part in parts):
        raise BuildError(f"bad path {path!r}")


def native_entries(root: Path = BUNDLE_ROOT) -> list[Entry]:
    """Every file the native half ships, in zip order."""
    entries: list[Entry] = []
    manifest_path = root / NATIVE_MANIFEST
    if not manifest_path.is_file():
        raise BuildError(f"{NATIVE_MANIFEST} is missing")
    entries.append(Entry(NATIVE_MANIFEST, _read(manifest_path)))

    mod_dir = root / MIXINS_DIR / MOD_ID
    if not (mod_dir / "horizon-mixin.toml").is_file():
        raise BuildError(f"{MIXINS_DIR}/{MOD_ID}/horizon-mixin.toml is missing")

    for path in sorted(mod_dir.rglob("*")):
        if not path.is_file():
            continue
        relative = path.relative_to(mod_dir).as_posix()
        _check_path(relative)
        top = relative.split("/")[0]
        inside_data = "data" in relative.split("/")[:-1]
        if path.suffix in NATIVE_SUFFIXES:
            pass
        elif inside_data:
            pass
        elif _is_native_doc(path.name) and relative.count("/") <= 1:
            pass
        else:
            raise BuildError(
                f"{MIXINS_DIR}/{MOD_ID}/{relative} is not allowed inside a native bundle "
                "(only .rs/.toml, data/**, and README/LICENSE/NOTICE at the top)"
            )
        if inside_data and top == "data":
            # data/ is staged to native-data/, so its own name is not part of the path.
            pass
        entries.append(Entry(f"{MIXINS_DIR}/{MOD_ID}/{relative}", _read(path)))

    files = len(entries)
    size = sum(len(entry.data) for entry in entries)
    if files > MAX_NATIVE_FILES:
        raise BuildError(f"{files} native files, the limit is {MAX_NATIVE_FILES}")
    if size > MAX_NATIVE_BYTES:
        raise BuildError(f"{size} native bytes, the limit is {MAX_NATIVE_BYTES}")
    return entries


def content_entries(root: Path = BUNDLE_ROOT) -> list[Entry]:
    """The content half's files, with the manifest at the archive root."""
    content_dir = root / "content"
    manifest_path = content_dir / "horizon-mod.toml"
    if not manifest_path.is_file():
        raise BuildError("content/horizon-mod.toml is missing")
    entries = [Entry("horizon-mod.toml", _read(manifest_path))]

    files = 1
    total = len(entries[0].data)
    for top in ("assets", "data"):
        directory = content_dir / top
        if not directory.is_dir():
            continue
        for path in sorted(directory.rglob("*")):
            if not path.is_file():
                continue
            relative = path.relative_to(content_dir).as_posix()
            _check_path(relative)
            parts = relative.split("/")
            if len(parts) < 3:
                raise BuildError(f"content path {relative} needs a namespace and a file")
            for part in parts[1:-1]:
                if not CONTENT_PART_RE.match(part):
                    raise BuildError(f"content path {relative} has a bad part {part!r}")
            data = _read(path)
            if len(data) > MAX_CONTENT_FILE_BYTES:
                raise BuildError(f"{relative} is larger than 16 MiB")
            files += 1
            total += len(data)
            entries.append(Entry(relative, data))

    if files > MAX_CONTENT_FILES:
        raise BuildError(f"{files} content files, the limit is {MAX_CONTENT_FILES}")
    if total > MAX_CONTENT_BYTES:
        raise BuildError(f"{total} content bytes, the limit is {MAX_CONTENT_BYTES}")
    return entries


def check_native_manifest(entries: list[Entry]) -> dict:
    """Validate `horizon-native.json` and the mixin manifest against each other."""
    by_path = {entry.path: entry for entry in entries}
    manifest = json.loads(by_path[NATIVE_MANIFEST].data)
    allowed = {"schema", "id", "version", "game", "horizon", "mixins", "hooks"}
    unknown = set(manifest) - allowed
    if unknown:
        raise BuildError(f"horizon-native.json has unknown fields: {sorted(unknown)}")
    if manifest.get("schema") != 1:
        raise BuildError("horizon-native.json schema must be 1")
    if not MOD_ID_RE.match(str(manifest.get("id", ""))):
        raise BuildError("horizon-native.json id is not [a-z0-9_]{1,64}")
    if manifest["id"] != MOD_ID:
        raise BuildError(f"horizon-native.json id {manifest['id']!r} is not {MOD_ID!r}")
    for field in ("horizon", "mixins"):
        if not SHA1_RE.match(str(manifest.get(field, ""))):
            raise BuildError(f"horizon-native.json {field} is not a 40-character commit")
    for field in ("version", "game"):
        if not str(manifest.get(field, "")):
            raise BuildError(f"horizon-native.json {field} is empty")
    hooks = manifest.get("hooks", 1)
    if not isinstance(hooks, int) or hooks < 1:
        raise BuildError("horizon-native.json hooks must be an integer of at least 1")

    mixin = tomllib.loads(by_path[f"{MIXINS_DIR}/{MOD_ID}/horizon-mixin.toml"].data.decode("utf-8"))
    if mixin.get("mod", {}).get("id") != MOD_ID:
        raise BuildError("horizon-mixin.toml [mod] id does not match the manifest")
    if mixin["mod"].get("version") != manifest["version"]:
        raise BuildError("horizon-mixin.toml version does not match horizon-native.json")
    compatibility = mixin.get("compatibility", {})
    if compatibility.get("horizon_revision") != manifest["horizon"]:
        raise BuildError("[compatibility] horizon_revision does not match horizon-native.json")

    helpers = set()
    for injection in mixin.get("injection", []):
        if "hook" not in injection and "target" not in injection:
            raise BuildError(f"injection {injection.get('id')} has neither hook nor target")
        if "hook" in injection and "hook_version" not in injection:
            raise BuildError(f"injection {injection.get('id')} names a hook without hook_version")
        helper = injection.get("helper")
        if helper:
            helpers.add(helper)
            if f"{MIXINS_DIR}/{MOD_ID}/{helper}" not in by_path:
                raise BuildError(f"injection {injection.get('id')} names a missing helper {helper}")
    for module in mixin.get("module", []):
        source = module.get("source")
        if source and f"{MIXINS_DIR}/{MOD_ID}/{source}" not in by_path:
            raise BuildError(f"module {module.get('id')} names a missing source {source}")
    for crate in mixin.get("crate", []):
        crate_dir = crate.get("path", "")
        prefix = f"{MIXINS_DIR}/{MOD_ID}/{crate_dir}/"
        if not any(path.startswith(prefix) for path in by_path):
            raise BuildError(f"crate {crate.get('id')} has no files under {crate_dir}/")
    return manifest


def check_content_manifest(entries: list[Entry]) -> dict:
    by_path = {entry.path: entry for entry in entries}
    manifest = tomllib.loads(by_path["horizon-mod.toml"].data.decode("utf-8"))
    allowed = {"id", "name", "version", "description", "authors", "api", "side", "capabilities", "namespaces"}
    unknown = set(manifest) - allowed
    if unknown:
        raise BuildError(f"horizon-mod.toml has unknown keys: {sorted(unknown)}")
    if manifest.get("id") != MOD_ID:
        raise BuildError("horizon-mod.toml id does not match the native half")
    for field in ("name", "version", "description"):
        if not isinstance(manifest.get(field), str) or not manifest[field]:
            raise BuildError(f"horizon-mod.toml {field} is required")
    if not isinstance(manifest.get("authors"), list) or not manifest["authors"]:
        raise BuildError("horizon-mod.toml needs at least one author")
    if manifest.get("api") != "0.2":
        raise BuildError("horizon-mod.toml api must be 0.2")
    if manifest.get("side") not in ("client", "server", "both"):
        raise BuildError("horizon-mod.toml side must be client, server or both")
    for name in manifest.get("namespaces", []):
        if not MOD_ID_RE.match(name):
            raise BuildError(f"horizon-mod.toml namespace {name!r} is invalid")
    return manifest


def write_zip(path: Path, entries: list[Entry]) -> None:
    """A deterministic zip: sorted paths, stored entries, fixed metadata."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_STORED, allowZip64=True) as archive:
        for entry in sorted(entries, key=lambda item: item.path):
            info = zipfile.ZipInfo(entry.path, date_time=STAMP)
            info.compress_type = zipfile.ZIP_STORED
            info.external_attr = 0o100644 << 16
            info.create_system = 3
            archive.writestr(info, entry.data)


def read_zip(path: Path) -> list[Entry]:
    if not path.is_file():
        raise BuildError(f"{path} does not exist")
    entries: list[Entry] = []
    seen: set[str] = set()
    with zipfile.ZipFile(path) as archive:
        bad = archive.testzip()
        if bad is not None:
            raise BuildError(f"{path} has a corrupt entry: {bad}")
        for info in archive.infolist():
            if info.is_dir():
                continue
            if info.filename in seen:
                raise BuildError(f"{path} holds {info.filename} twice")
            seen.add(info.filename)
            _check_path(info.filename)
            if info.compress_type not in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED):
                raise BuildError(f"{path}: {info.filename} uses compression method {info.compress_type}")
            mode = (info.external_attr >> 16) & 0o170000
            if mode not in (0, 0o100000):
                raise BuildError(f"{path}: {info.filename} is not a regular file")
            entries.append(Entry(info.filename, archive.read(info)))
    return entries


def verify(path: Path, expect_native: bool, expect_content: bool) -> dict:
    """Check an archive against the spec, returning a report."""
    entries = read_zip(path)
    by_path = {entry.path: entry for entry in entries}
    top = {entry.path.split("/")[0] for entry in entries}
    allowed_top = {"horizon-native.json", "horizon-mod.toml", "mixins", "assets", "data"}
    unknown = top - allowed_top
    if unknown:
        raise BuildError(f"{path.name} has unknown top-level entries: {sorted(unknown)}")
    if expect_native and NATIVE_MANIFEST not in by_path:
        raise BuildError(f"{path.name} has no {NATIVE_MANIFEST}")
    if expect_content and "horizon-mod.toml" not in by_path:
        raise BuildError(f"{path.name} has no horizon-mod.toml")
    native = check_native_manifest(entries) if NATIVE_MANIFEST in by_path else None
    content = check_content_manifest(entries) if "horizon-mod.toml" in by_path else None
    if native and content and native["id"] != content["id"]:
        raise BuildError(f"{path.name}: the two manifests name different mods")

    # Every mixin file must be under mixins/<id>/, and nothing else may be there.
    for entry in entries:
        if entry.path.startswith(f"{MIXINS_DIR}/") and not entry.path.startswith(f"{MIXINS_DIR}/{MOD_ID}/"):
            raise BuildError(f"{path.name}: {entry.path} is outside {MIXINS_DIR}/{MOD_ID}/")
        if entry.path.startswith(f"{MIXINS_DIR}/") and entry.path.count("/") < 2:
            raise BuildError(f"{path.name}: {entry.path} is not a file inside a bundle")

    return {
        "path": str(path),
        "bytes": path.stat().st_size,
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "files": len(entries),
        "native": bool(native),
        "content": bool(content),
        "hooks": (native or {}).get("hooks"),
        "version": (native or content or {}).get("version"),
    }


def build(out_dir: Path) -> list[dict]:
    native = native_entries()
    check_native_manifest(native)
    content = content_entries()
    check_content_manifest(content)

    hnmod = out_dir / f"{MOD_ID}.hnmod"
    write_zip(hnmod, native)
    hmod = out_dir / f"{MOD_ID}.hmod"
    write_zip(hmod, native + content)

    return [
        verify(hnmod, expect_native=True, expect_content=False),
        verify(hmod, expect_native=True, expect_content=True),
    ]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build Volt Lapse for Horizon (PC and Android).")
    parser.add_argument("--out", default=str(REPO_ROOT / "releases"), help="output folder")
    parser.add_argument("--check", action="store_true", help="verify the built packages instead of building")
    parser.add_argument("--json", action="store_true", help="print the report as JSON")
    args = parser.parse_args(argv)
    out_dir = Path(args.out)

    try:
        if args.check:
            reports = [
                verify(out_dir / f"{MOD_ID}.hnmod", expect_native=True, expect_content=False),
                verify(out_dir / f"{MOD_ID}.hmod", expect_native=True, expect_content=True),
            ]
        else:
            reports = build(out_dir)
    except BuildError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    if args.json:
        print(json.dumps(reports, indent=2))
    else:
        for report in reports:
            print(
                f"[OK] {Path(report['path']).name:18s} v{report['version']} "
                f"{report['bytes']:7d} B {report['files']:4d} files "
                f"hooks:{report['hooks']} sha256:{report['sha256'][:16]}…"
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
