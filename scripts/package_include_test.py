"""Tests that each published crate packages from an `include` allowlist, never an `exclude`.

A denylist ships whatever else lies in the crate directory: trybuild's wip/, a stray local file
under `--allow-dirty`, a future benches/ (plan 2b, Decision 42).
"""

import pathlib
import tomllib
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent


def manifest(path):
    """The TOML file at `path`, parsed."""
    with path.open("rb") as file:
        return tomllib.load(file)


def published():
    """Each workspace member's `[package]` table, by directory, unless `publish = false`."""
    members = manifest(ROOT / "Cargo.toml")["workspace"]["members"]
    packages = {member: manifest(ROOT / member / "Cargo.toml")["package"] for member in members}
    return {member: package for member, package in packages.items() if package.get("publish", True)}


class PackageIncludeTest(unittest.TestCase):
    """Every published crate lists the files it ships."""

    def test_published_crates_are_the_two_libraries(self):
        self.assertEqual(
            sorted(published()),
            ["crates/a2ui-catalog-basic", "crates/a2ui-runtime"],
            "a published crate was added or removed: give it an `include` and update this list",
        )

    def test_published_crates_declare_include(self):
        for member, package in published().items():
            with self.subTest(member=member):
                self.assertIn("include", package)

    def test_published_crates_declare_no_exclude(self):
        for member, package in published().items():
            with self.subTest(member=member):
                self.assertNotIn("exclude", package)

    def test_published_crates_include_their_manifest_and_sources(self):
        for member, package in published().items():
            with self.subTest(member=member):
                self.assertTrue({"src/**", "Cargo.toml"} <= set(package.get("include", [])))


if __name__ == "__main__":
    unittest.main()
