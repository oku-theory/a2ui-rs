"""Tests for lock_diff: the summary of what a Cargo.lock change adds, removes and updates."""

import datetime
import unittest

from lock_diff import Change, Published, Release, diff, packages, render

NOW = datetime.datetime(2026, 11, 14, tzinfo=datetime.timezone.utc)
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
SPARSE = "sparse+https://index.crates.io/"
GIT = "git+https://github.com/example/fork?rev=abc#0123456789abcdef"


def lock(*entries):
    """A Cargo.lock holding `entries`, each (name, version) or (name, version, source)."""
    blocks = ["version = 4\n"]
    for name, version, *source in entries:
        block = f'[[package]]\nname = "{name}"\nversion = "{version}"\n'
        if source:
            block += f'source = "{source[0]}"\n'
        blocks.append(block)
    return "\n".join(blocks)


def r(*versions):
    """crates.io releases of `versions`, as a set."""
    return {Release(version) for version in versions}


def ago(days, yanked=False):
    """What crates.io would say of a release published `days` days before NOW."""
    return Published(NOW - datetime.timedelta(days=days), yanked)


class PackagesTest(unittest.TestCase):
    def test_packages_workspace_member_is_left_out(self):
        text = lock(("ours", "0.1.0"), ("serde", "1.0.0", REGISTRY))
        self.assertEqual(packages(text), {"serde": r("1.0.0")})

    def test_packages_two_versions_of_one_crate_are_both_kept(self):
        text = lock(("syn", "1.0.0", REGISTRY), ("syn", "2.0.0", REGISTRY))
        self.assertEqual(packages(text), {"syn": r("1.0.0", "2.0.0")})

    def test_packages_sparse_crates_io_source_is_crates_io(self):
        self.assertEqual(packages(lock(("serde", "1.0.0", SPARSE))), {"serde": r("1.0.0")})

    def test_packages_git_source_is_kept_with_its_source(self):
        found = packages(lock(("fork", "1.0.0", GIT)))
        self.assertEqual(found, {"fork": {Release("1.0.0", GIT)}})

    def test_packages_name_with_markdown_fails(self):
        with self.assertRaises(ValueError):
            packages(lock(("a|b", "1.0.0", REGISTRY)))

    def test_packages_version_with_a_space_fails(self):
        with self.assertRaises(ValueError):
            packages(lock(("serde", "1.0.0 x", REGISTRY)))

    def test_packages_source_with_a_backtick_fails(self):
        with self.assertRaises(ValueError):
            packages(lock(("fork", "1.0.0", "git+https://x/`y`")))


class DiffTest(unittest.TestCase):
    def test_diff_new_crate_is_added(self):
        changes = diff({}, {"new": r("1.0.0")})
        self.assertEqual(changes, [Change("added", "new", (), (Release("1.0.0"),))])

    def test_diff_dropped_crate_is_removed(self):
        changes = diff({"old": r("1.0.0")}, {})
        self.assertEqual(changes, [Change("removed", "old", (Release("1.0.0"),), ())])

    def test_diff_new_version_of_a_kept_crate_is_updated(self):
        changes = diff({"serde": r("1.0.0")}, {"serde": r("1.0.1")})
        expected = Change("updated", "serde", (Release("1.0.0"),), (Release("1.0.1"),))
        self.assertEqual(changes, [expected])

    def test_diff_versions_sort_by_number(self):
        changes = diff({}, {"toml": r("0.10.0", "0.9.0")})
        self.assertEqual(changes[0].new, (Release("0.9.0"), Release("0.10.0")))

    def test_diff_prerelease_sorts_before_its_release(self):
        changes = diff({}, {"syn": r("2.0.0", "2.0.0-rc.1")})
        self.assertEqual(changes[0].new, (Release("2.0.0-rc.1"), Release("2.0.0")))

    def test_diff_unchanged_crate_is_left_out(self):
        self.assertEqual(diff({"serde": r("1.0.0")}, {"serde": r("1.0.0")}), [])

    def test_diff_same_version_from_another_source_is_updated(self):
        changes = diff({"fork": r("1.0.0")}, {"fork": {Release("1.0.0", GIT)}})
        self.assertEqual([change.kind for change in changes], ["updated"])

    def test_diff_changes_are_sorted_added_first_then_by_name(self):
        changes = diff({"b": r("1"), "z": r("1")}, {"b": r("2"), "a": r("1")})
        self.assertEqual(
            [(change.kind, change.name) for change in changes],
            [("added", "a"), ("removed", "z"), ("updated", "b")],
        )


class RenderTest(unittest.TestCase):
    def test_render_whole_summary(self):
        changes = diff(
            {"old": r("1.0.0"), "syn": r("1.0.0", "2.0.0")},
            {"new": r("1.0.0"), "syn": r("2.0.0")},
        )
        expected = "\n".join([
            "## Cargo.lock changes",
            "",
            "1 added, 1 removed, 1 updated. Read each added crate and each release listed"
            " beside an older one;",
            "if a release is under a week old, close this and take next week's.",
            "",
            "| change | crate | from | to | published |",
            "|---|---|---|---|---|",
            "| added | `new` |  | 1.0.0 | 1.0.0: 2026-11-04, 10 days |",
            "| removed | `old` | 1.0.0 |  |  |",
            "| updated | `syn` | 1.0.0, 2.0.0 | 2.0.0 |  |",
            "",
        ])
        self.assertEqual(render(changes, {("new", "1.0.0"): ago(10)}, NOW), expected)

    def test_render_no_change_says_so(self):
        self.assertIn("No crate added, removed or updated.", render([], {}, NOW))

    def test_render_kept_version_gets_no_date(self):
        changes = diff({"syn": r("1.0.0")}, {"syn": r("1.0.0", "2.0.0")})
        self.assertIn(
            "| updated | `syn` | 1.0.0 | 1.0.0, 2.0.0 | 2.0.0: 2026-10-15, 30 days |",
            render(changes, {("syn", "2.0.0"): ago(30)}, NOW),
        )

    def test_render_release_under_a_week_old_is_flagged(self):
        changes = diff({}, {"new": r("1.0.0")})
        self.assertIn(
            "| added | `new` |  | 1.0.0 | 1.0.0: 2026-11-12, 2 days, under a week |",
            render(changes, {("new", "1.0.0"): ago(2)}, NOW),
        )

    def test_render_release_a_week_old_is_not_flagged(self):
        changes = diff({"serde": r("1.0.0")}, {"serde": r("1.0.1")})
        self.assertIn(
            "| updated | `serde` | 1.0.0 | 1.0.1 | 1.0.1: 2026-11-07, 7 days |",
            render(changes, {("serde", "1.0.1"): ago(7)}, NOW),
        )

    def test_render_one_day_is_singular(self):
        changes = diff({}, {"new": r("1.0.0")})
        self.assertIn(
            "1.0.0: 2026-11-13, 1 day, under a week |",
            render(changes, {("new", "1.0.0"): ago(1)}, NOW),
        )

    def test_render_yanked_release_is_flagged(self):
        changes = diff({}, {"new": r("1.0.0")})
        self.assertIn(
            "1.0.0: 2026-10-15, 30 days, yanked |",
            render(changes, {("new", "1.0.0"): ago(30, yanked=True)}, NOW),
        )

    def test_render_release_off_crates_io_asks_to_read_it(self):
        changes = diff({}, {"fork": {Release("1.0.0", GIT)}})
        self.assertIn(
            f"| added | `fork` |  | 1.0.0 from {GIT} | 1.0.0: not on crates.io, read it by hand |",
            render(changes, {}, NOW),
        )


if __name__ == "__main__":
    unittest.main()
