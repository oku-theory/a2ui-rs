"""Tests for fuzz_targets: the fuzz targets' limits and corpus seeds."""

import contextlib
import io
import json
import pathlib
import re
import tempfile
import tomllib
import unittest

from fuzz_targets import TARGETS, at_caps, main, paths, seeds, templates, walk_strings

REPO = pathlib.Path(__file__).resolve().parent.parent


def nodes(value):
    """The runtime's component node count: 1, plus each container's member count."""
    if isinstance(value, dict):
        return len(value) + sum(nodes(item) - 1 for item in value.values()) + 1
    if isinstance(value, list):
        return len(value) + sum(nodes(item) - 1 for item in value) + 1
    return 1


def calls(value):
    """The objects holding an `@call` key, nested ones included."""
    if isinstance(value, dict):
        return ("@call" in value) + sum(calls(item) for item in value.values())
    if isinstance(value, list):
        return sum(calls(item) for item in value)
    return 0


class TableTest(unittest.TestCase):
    def test_table_matches_fuzz_cargo_bins(self):
        manifest = tomllib.loads((REPO / "fuzz" / "Cargo.toml").read_text())
        self.assertEqual(sorted(TARGETS), sorted(bin["name"] for bin in manifest["bin"]))

    def test_table_matches_workflow_matrix(self):
        workflow = (REPO / ".github" / "workflows" / "fuzz.yml").read_text()
        block = re.search(r"^ +target:\n((?: +- \w+\n)+)", workflow, re.M)
        self.assertIsNotNone(block)
        self.assertEqual(sorted(TARGETS), sorted(re.findall(r"- (\w+)", block.group(1))))

    def test_main_limits_prints_max_len_and_timeout(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(main(["limits", "pointer_parse"]), 0)
        self.assertEqual(out.getvalue().split(), ["4096", "1"])

    def test_main_unknown_target_fails(self):
        with contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(main(["limits", "nope"]), 2)


class WalkStringsTest(unittest.TestCase):
    def test_walk_strings_keys_and_values_in_order(self):
        document = {"a": ["x", {"b": "y"}], "c": 1}
        self.assertEqual(list(walk_strings(document)), ["a", "x", "b", "y", "c"])


class TemplatesTest(unittest.TestCase):
    def test_templates_only_strings_with_interpolation(self):
        document = {"text": "Hi ${/user/name}", "plain": "no", "esc": "\\${x}"}
        self.assertEqual(templates([document]), ["Hi ${/user/name}", "\\${x}"])

    def test_templates_deduplicated(self):
        self.assertEqual(templates([["${a}", "${a}"]]), ["${a}"])


class PathsTest(unittest.TestCase):
    def test_paths_binding_values_and_absolute_strings(self):
        document = {"value": {"@path": "items/0"}, "other": "/user/name", "word": "text"}
        self.assertEqual(paths([document]), ["/user/name", "items/0"])

    def test_paths_nested_binding_found(self):
        self.assertEqual(paths([[{"a": {"@path": "x/y"}}]]), ["x/y"])

    def test_paths_non_string_binding_ignored(self):
        self.assertEqual(paths([{"@path": 3}]), [])


class AtCapsTest(unittest.TestCase):
    shapes = [json.loads(seed) for seed in at_caps()]

    def test_at_caps_nodes_straddle_the_cap(self):
        self.assertEqual([nodes(shape) for shape in self.shapes[:2]], [65536, 65537])

    def test_at_caps_calls_straddle_the_cap(self):
        self.assertEqual([calls(shape) for shape in self.shapes[2:4]], [1024, 1025])

    def test_at_caps_fit_validate_component_max_len(self):
        self.assertTrue(all(len(seed) <= TARGETS["validate_component"][0] for seed in at_caps()))


class SeedsTest(unittest.TestCase):
    """Seeds from the vendored files, read from the repository root."""

    def setUp(self):
        self.cwd = pathlib.Path.cwd()
        import os

        os.chdir(REPO)
        self.addCleanup(os.chdir, self.cwd)

    def test_seeds_every_target_has_seeds(self):
        for target in TARGETS:
            with self.subTest(target=target):
                self.assertTrue(seeds(target))

    def test_seeds_parse_then_validate_component_first(self):
        component, catalog = seeds("parse_then_validate")[0].split(b"\0")
        self.assertIn("component", json.loads(component))
        self.assertIn("catalogId", json.loads(catalog))

    def test_seeds_catalog_parse_override_before_catalog(self):
        parts = seeds("catalog_parse")[-1].split(b"\0")
        self.assertEqual(len(parts), 3)
        self.assertIn("catalogId", json.loads(parts[2]))

    def test_main_seed_writes_hash_named_files(self):
        with tempfile.TemporaryDirectory() as directory:
            corpus = pathlib.Path(directory) / "corpus"
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(main(["seed", "pointer_parse", str(corpus)]), 0)
            self.assertTrue(all(path.name.startswith("seed-") for path in corpus.iterdir()))

    def test_seeds_missing_vendor_fails(self):
        import os

        with tempfile.TemporaryDirectory() as directory:
            os.chdir(directory)
            with self.assertRaises(SystemExit):
                seeds("catalog_parse")


if __name__ == "__main__":
    unittest.main()
