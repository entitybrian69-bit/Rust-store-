#!/usr/bin/env python3
"""Tests for the Volt Lapse packager.

    python3 -m unittest discover -s src-native/voltlapse/tests -v
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "tools"))

import build  # noqa: E402


class BuildTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.out = Path(self.tmp.name)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_both_packages_build_and_verify(self):
        reports = build.build(self.out)
        self.assertEqual([Path(r["path"]).suffix for r in reports], [".hnmod", ".hmod"])
        hnmod, hmod = reports
        self.assertTrue(hnmod["native"])
        self.assertFalse(hnmod["content"])
        self.assertTrue(hmod["native"])
        self.assertTrue(hmod["content"])
        self.assertEqual(hnmod["hooks"], 410)
        self.assertEqual(hnmod["version"], "1.0.0")
        self.assertGreater(hnmod["files"], 40)
        self.assertGreater(hmod["files"], hnmod["files"])

    def test_the_build_is_reproducible(self):
        first = build.build(self.out)
        second = build.build(self.out)
        self.assertEqual([r["sha256"] for r in first], [r["sha256"] for r in second])
        self.assertEqual([r["bytes"] for r in first], [r["bytes"] for r in second])

    def test_every_native_entry_is_allowed_by_the_spec(self):
        build.build(self.out)
        with zipfile.ZipFile(self.out / "voltlapse.hnmod") as archive:
            names = archive.namelist()
        self.assertIn("horizon-native.json", names)
        for name in names:
            if name == "horizon-native.json":
                continue
            self.assertTrue(name.startswith("mixins/voltlapse/"), name)
            rest = name[len("mixins/voltlapse/"):]
            if rest in ("LICENSE", "NOTICE", "README.md", ".gitignore"):
                continue
            self.assertIn(Path(rest).suffix, (".rs", ".toml"), name)
            self.assertNotIn("..", name)
        self.assertFalse([n for n in names if n.startswith("mixins/voltlapse/engine/") and n.endswith(".lock")])

    def test_the_content_half_ships_the_languages_and_the_icon(self):
        build.build(self.out)
        with zipfile.ZipFile(self.out / "voltlapse.hmod") as archive:
            names = set(archive.namelist())
            manifest = archive.read("horizon-mod.toml").decode()
        self.assertIn("horizon-mod.toml", names)
        for locale in ("en_us", "de_de", "ne_np"):
            path = f"assets/voltlapse/lang/{locale}.json"
            self.assertIn(path, names)
        self.assertIn("assets/voltlapse/icon.png", names)
        self.assertIn('id = "voltlapse"', manifest)

    def test_the_language_files_hold_the_engine_keys(self):
        en = json.loads((build.BUNDLE_ROOT / "lang" / "en_us.json").read_text())
        self.assertIn("ui.title", en)
        self.assertIn("toast.saved", en)
        for locale in ("de_de", "ne_np"):
            table = json.loads((build.BUNDLE_ROOT / "lang" / f"{locale}.json").read_text())
            self.assertEqual(set(table), set(en), locale)

    def test_a_missing_helper_is_refused(self):
        entries = [e for e in build.native_entries() if not e.path.endswith("hook_tick.rs")]
        with self.assertRaises(build.BuildError) as caught:
            build.check_native_manifest(entries)
        self.assertIn("hook_tick.rs", str(caught.exception))

    def test_a_disallowed_native_file_is_refused(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            mixin = root / "mixins" / "voltlapse"
            mixin.mkdir(parents=True)
            (root / "horizon-native.json").write_text((build.BUNDLE_ROOT / "horizon-native.json").read_text())
            (mixin / "horizon-mixin.toml").write_text((build.BUNDLE_ROOT / "mixins/voltlapse/horizon-mixin.toml").read_text())
            (mixin / "notes.png").write_bytes(b"\x89PNG")
            with self.assertRaises(build.BuildError) as caught:
                build.native_entries(root)
            self.assertIn("notes.png", str(caught.exception))

    def test_a_manifest_id_mismatch_is_refused(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            mixin = root / "mixins" / "voltlapse"
            mixin.mkdir(parents=True)
            manifest = json.loads((build.BUNDLE_ROOT / "horizon-native.json").read_text())
            manifest["id"] = "other"
            (root / "horizon-native.json").write_text(json.dumps(manifest))
            (mixin / "horizon-mixin.toml").write_text('[mod]\nid = "voltlapse"\nversion = "1.0.0"\n')
            with self.assertRaises(build.BuildError):
                build.check_native_manifest(build.native_entries(root))

    def test_a_tampered_archive_is_refused(self):
        build.build(self.out)
        path = self.out / "voltlapse.hnmod"
        tampered = self.out / "tampered.hnmod"
        with zipfile.ZipFile(path) as source, zipfile.ZipFile(tampered, "w") as target:
            for info in source.infolist():
                data = source.read(info)
                if info.filename == "horizon-native.json":
                    manifest = json.loads(data)
                    manifest["schema"] = 2
                    data = json.dumps(manifest).encode()
                target.writestr(info, data)
        with self.assertRaises(build.BuildError):
            build.verify(tampered, expect_native=True, expect_content=False)

    def test_check_mode_fails_without_packages(self):
        self.assertEqual(build.main(["--check", "--out", str(self.out)]), 1)
        build.build(self.out)
        self.assertEqual(build.main(["--check", "--out", str(self.out)]), 0)


if __name__ == "__main__":
    unittest.main()
