"""Tests for lock_diff: the summary of what a Cargo.lock change adds, removes and updates."""

import contextlib
import datetime
import functools
import http.client
import io
import json
import unittest
import urllib.error
from unittest import mock

from lock_diff import (
    Change,
    Published,
    Release,
    Unavailable,
    collect,
    diff,
    lookup,
    main,
    packages,
    render,
)

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


def http_error(status):
    """The error urllib raises for an HTTP `status` answer."""
    return urllib.error.HTTPError("https://crates.io/x", status, "x", {}, io.BytesIO())


def parameterized(*cases):
    """Runs the decorated test once per case, each as its own subTest."""

    def decorate(test):
        @functools.wraps(test)
        def run(self):
            for case in cases:
                with self.subTest(case=case):
                    test(self, case)

        return run

    return decorate


parameterized_errors = parameterized(
    http_error(429),
    http_error(503),
    TimeoutError(),
    urllib.error.URLError("refused"),
    ConnectionResetError(),
    http.client.RemoteDisconnected(),
    http.client.IncompleteRead(b""),
)
parameterized_statuses = parameterized(400, 403, 404, 410)
parameterized_bad_answers = parameterized(
    json.JSONDecodeError("x", "<html>", 0), KeyError("version"), TypeError()
)


class Fetcher:
    """A stand-in for publish_info: raises each queued error in turn, then answers `answer`."""

    def __init__(self, *errors, answer=None):
        self.errors = list(errors)
        self.answer = answer
        self.calls = 0

    def __call__(self, name, version):
        self.calls += 1
        if self.errors:
            raise self.errors.pop(0)
        return self.answer


class LookupTest(unittest.TestCase):
    def setUp(self):
        self.waits = []

    def find(self, fetch):
        """lookup of serde 1.0.0 through `fetch`, its waits recorded in self.waits."""
        return lookup("serde", "1.0.0", fetch, self.waits.append)

    def test_lookup_answer_is_returned(self):
        self.assertEqual(self.find(Fetcher(answer=ago(3))), ago(3))

    def test_lookup_answer_asks_once(self):
        fetch = Fetcher(answer=ago(3))
        self.find(fetch)
        self.assertEqual((fetch.calls, self.waits), (1, []))

    @parameterized_errors
    def test_lookup_transient_error_then_answer_returns_answer(self, error):
        self.assertEqual(self.find(Fetcher(error, answer=ago(3))), ago(3))

    def test_lookup_transient_error_waits_five_seconds_before_retry(self):
        self.find(Fetcher(http_error(429), answer=ago(3)))
        self.assertEqual(self.waits, [5])

    def test_lookup_second_transient_error_is_unavailable(self):
        fetch = Fetcher(http_error(503), http_error(502))
        self.assertEqual(self.find(fetch), Unavailable("HTTP 502", transient=True))

    def test_lookup_second_transient_error_stops_after_two_requests(self):
        fetch = Fetcher(http_error(429), http_error(429), answer=ago(3))
        self.find(fetch)
        self.assertEqual(fetch.calls, 2)

    def test_lookup_status_500_is_transient(self):
        fetch = Fetcher(http_error(500), answer=ago(3))
        self.assertEqual(self.find(fetch), ago(3))

    def test_lookup_timeout_twice_is_unavailable_timeout(self):
        fetch = Fetcher(TimeoutError(), TimeoutError())
        self.assertEqual(self.find(fetch), Unavailable("timeout", transient=True))

    def test_lookup_connect_timeout_twice_is_unavailable_timeout(self):
        error = urllib.error.URLError(TimeoutError())
        self.assertEqual(
            self.find(Fetcher(error, error)), Unavailable("timeout", transient=True)
        )

    def test_lookup_network_error_twice_is_unavailable_network_error(self):
        error = urllib.error.URLError("refused")
        self.assertEqual(
            self.find(Fetcher(error, error)), Unavailable("network error", transient=True)
        )

    def test_lookup_reset_while_reading_twice_is_unavailable_network_error(self):
        fetch = Fetcher(ConnectionResetError(), http.client.IncompleteRead(b""))
        self.assertEqual(self.find(fetch), Unavailable("network error", transient=True))

    @parameterized_statuses
    def test_lookup_client_error_is_not_retried(self, status):
        fetch = Fetcher(http_error(status), answer=ago(3))
        self.assertEqual(
            (self.find(fetch), fetch.calls, self.waits), (Unavailable(f"HTTP {status}"), 1, [])
        )

    @parameterized_bad_answers
    def test_lookup_bad_answer_is_not_retried(self, error):
        fetch = Fetcher(error, answer=ago(3))
        self.assertEqual((self.find(fetch), fetch.calls), (Unavailable("bad answer"), 1))


class CollectTest(unittest.TestCase):
    def setUp(self):
        self.asked = []
        self.pauses = []

    def collect(self, answers, wanted):
        """collect `wanted`, answering each request from `answers` in turn."""
        queue = list(answers)

        def find(name, version):
            self.asked.append(name)
            return queue.pop(0)

        return collect(wanted, find, self.pauses.append)

    def test_collect_dates_each_release(self):
        found = self.collect([ago(3), ago(4)], [("a", "1.0.0"), ("b", "1.0.0")])
        self.assertEqual(found, {("a", "1.0.0"): ago(3), ("b", "1.0.0"): ago(4)})

    def test_collect_pauses_a_second_between_requests(self):
        self.collect([ago(3), ago(4)], [("a", "1.0.0"), ("b", "1.0.0")])
        self.assertEqual(self.pauses, [1])

    def test_collect_goes_on_after_a_lasting_error(self):
        self.collect([Unavailable("HTTP 404"), ago(4)], [("a", "1.0.0"), ("b", "1.0.0")])
        self.assertEqual(self.asked, ["a", "b"])

    def test_collect_stops_asking_after_a_transient_failure(self):
        down = Unavailable("timeout", transient=True)
        self.collect([down], [("a", "1.0.0"), ("b", "1.0.0"), ("c", "1.0.0")])
        self.assertEqual(self.asked, ["a"])

    def test_collect_marks_releases_left_after_a_transient_failure(self):
        down = Unavailable("timeout", transient=True)
        found = self.collect([down], [("a", "1.0.0"), ("b", "1.0.0")])
        self.assertEqual(found[("b", "1.0.0")], Unavailable("not tried, crates.io unreachable"))


class MainTest(unittest.TestCase):
    def run_main(self, published):
        """main's output and exit status when it adds `new`, `collect` giving `published`."""
        locks = {"base": lock(), "head": lock(("new", "1.0.0", REGISTRY))}
        out = io.StringIO()
        with (
            mock.patch("lock_diff.lock_at", side_effect=locks.get),
            mock.patch("lock_diff.collect", return_value=published),
            mock.patch("lock_diff.datetime") as clock,
            contextlib.redirect_stdout(out),
        ):
            clock.datetime.now.return_value = NOW
            clock.timezone = datetime.timezone
            try:
                main(["base", "head"])
            except SystemExit as stop:
                return out.getvalue(), stop.code
        return out.getvalue(), None

    def test_main_all_dated_exits_zero(self):
        self.assertIsNone(self.run_main({("new", "1.0.0"): ago(10)})[1])

    def test_main_undated_release_exits_non_zero(self):
        code = self.run_main({("new", "1.0.0"): Unavailable("timeout", transient=True)})[1]
        self.assertTrue(code)

    def test_main_undated_release_prints_the_table_first(self):
        out = self.run_main({("new", "1.0.0"): Unavailable("timeout", transient=True)})[0]
        self.assertIn("| added | `new` |  | 1.0.0 | 1.0.0: FAILED (timeout)", out)


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

    def test_render_unavailable_release_is_marked_failed(self):
        changes = diff({}, {"new": r("1.0.0")})
        self.assertIn(
            "| added | `new` |  | 1.0.0 | 1.0.0: FAILED (HTTP 503), read it on crates.io |",
            render(changes, {("new", "1.0.0"): Unavailable("HTTP 503")}, NOW),
        )

    def test_render_unavailable_release_keeps_the_others_dated(self):
        changes = diff({}, {"a": r("1.0.0"), "b": r("1.0.0")})
        summary = render(
            changes, {("a", "1.0.0"): Unavailable("timeout"), ("b", "1.0.0"): ago(10)}, NOW
        )
        self.assertIn("| added | `b` |  | 1.0.0 | 1.0.0: 2026-11-04, 10 days |", summary)

    def test_render_unavailable_release_is_counted_in_the_header(self):
        changes = diff({}, {"new": r("1.0.0")})
        self.assertIn(
            "1 release could not be dated: read it by hand; this check fails.",
            render(changes, {("new", "1.0.0"): Unavailable("timeout")}, NOW),
        )

    def test_render_unavailable_releases_are_counted_in_the_plural(self):
        changes = diff({}, {"a": r("1.0.0"), "b": r("1.0.0")})
        failed = {("a", "1.0.0"): Unavailable("timeout"), ("b", "1.0.0"): Unavailable("HTTP 404")}
        self.assertIn(
            "2 releases could not be dated: read them by hand; this check fails.",
            render(changes, failed, NOW),
        )


if __name__ == "__main__":
    unittest.main()
