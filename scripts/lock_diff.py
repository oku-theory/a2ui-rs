#!/usr/bin/env python3
"""Summarizes what a Cargo.lock change adds, removes and updates, for its reviewer.

Usage: lock_diff.py <base-ref> [<head-ref>]. Compares Cargo.lock at <base-ref> with <head-ref>, or
with the working tree, and prints a Markdown table: each crate added, removed or updated, with the
crates.io publish date of each new version, flagged when under a week old or yanked (plan 2b,
Decision 30). Workspace members, which have no source, are left out; a release from any source but
crates.io gets no date, only a request to read it by hand. It reads crates.io's API, one request a
second, retrying a transient error (429, 5xx, timeout, network) once; a release still undated is
marked FAILED in its cell, and so is every release after one that stays undated after a transient
error, unasked; the script exits 1 after printing the whole table. It fails at once
on a name, version or source it could not print safely, rather than print a table that hides a
change. Needs Python 3.11 or later (tomllib).
"""

import sys

if sys.version_info < (3, 11):
    sys.exit("lock_diff.py needs Python 3.11 or later (tomllib)")

import datetime  # noqa: E402
import http.client  # noqa: E402
import json  # noqa: E402
import re  # noqa: E402
import subprocess  # noqa: E402
import time  # noqa: E402
import tomllib  # noqa: E402
import urllib.error  # noqa: E402
import urllib.parse  # noqa: E402
import urllib.request  # noqa: E402
from collections.abc import Callable  # noqa: E402
from typing import NamedTuple  # noqa: E402

API = "https://crates.io/api/v1/crates"
# crates.io asks API clients for a User-Agent naming them; this one names no person or project.
USER_AGENT = "cargo-lock-diff"
CRATES_IO = (
    "registry+https://github.com/rust-lang/crates.io-index",
    "sparse+https://index.crates.io/",
)
# What a Markdown table cell may hold: no `|`, backtick, `<` or whitespace.
NAME = re.compile(r"[A-Za-z0-9_-]{1,64}")
VERSION = re.compile(r"[0-9A-Za-z.+-]{1,64}")
SOURCE = re.compile(r"[A-Za-z0-9+:/._?=&#%-]{1,256}")
YOUNG = datetime.timedelta(days=7)
# Seconds before the one retry of a transient crates.io error (429, 5xx, timeout, network).
RETRY_WAIT = 5
KINDS = ("added", "removed", "updated")


class Release(NamedTuple):
    """One version of a crate and where it comes from; `source` is empty for crates.io."""

    version: str
    source: str = ""

    def label(self) -> str:
        """The version, followed by its source unless that is crates.io."""
        return f"{self.version} from {self.source}" if self.source else self.version


class Change(NamedTuple):
    """One crate whose releases differ between the two locks, with all of them on each side."""

    kind: str
    name: str
    old: tuple[Release, ...]
    new: tuple[Release, ...]


class Published(NamedTuple):
    """What crates.io says of one release."""

    date: datetime.datetime
    yanked: bool


class Unavailable(NamedTuple):
    """A release crates.io did not date, with a short reason this script wrote, safe in a cell.

    `transient` says asking again later may succeed: a 429, a 5xx, a timeout or a network error.
    """

    reason: str
    transient: bool = False


def order(release: Release) -> tuple:
    """A sort key for releases: numeric parts as numbers, so 0.9.0 comes before 0.10.0."""
    core, _, build = release.version.partition("+")
    core, _, pre = core.partition("-")
    parts = tuple(
        (0, int(part), "") if part.isdigit() else (1, 0, part) for part in core.split(".")
    )
    return parts, pre == "", pre, build, release.source


def checked(pattern: re.Pattern[str], what: str, value: str) -> str:
    """`value` when `pattern` matches all of it; anything else stops the script."""
    if not pattern.fullmatch(value):
        raise ValueError(f"unexpected {what} in Cargo.lock: {value!r}")
    return value


def packages(text: str) -> dict[str, set[Release]]:
    """The crates of a Cargo.lock that have a source, each name with its releases."""
    found: dict[str, set[Release]] = {}
    for package in tomllib.loads(text).get("package", []):
        if "source" not in package:
            continue
        name = checked(NAME, "crate name", package["name"])
        version = checked(VERSION, "version", package["version"])
        source = package["source"]
        if source not in CRATES_IO:
            source = checked(SOURCE, "source", source)
        found.setdefault(name, set()).add(Release(version, "" if source in CRATES_IO else source))
    return found


def diff(base: dict[str, set[Release]], head: dict[str, set[Release]]) -> list[Change]:
    """The crates added, removed or updated from `base` to `head`, added first, then by name."""
    changes = []
    for name in base.keys() | head.keys():
        old, new = base.get(name, set()), head.get(name, set())
        if old == new:
            continue
        kind = "added" if not old else "removed" if not new else "updated"
        old_releases, new_releases = tuple(sorted(old, key=order)), tuple(sorted(new, key=order))
        changes.append(Change(kind, name, old_releases, new_releases))
    return sorted(changes, key=lambda change: (KINDS.index(change.kind), change.name))


def fresh(change: Change) -> list[Release]:
    """The releases `change` brings in: in the new lock, not the old one."""
    return [release for release in change.new if release not in change.old]


def undated(published: dict[tuple[str, str], Published | Unavailable]) -> int:
    """How many releases in `published` crates.io did not date."""
    return sum(isinstance(entry, Unavailable) for entry in published.values())


def render(
    changes: list[Change],
    published: dict[tuple[str, str], Published | Unavailable],
    now: datetime.datetime,
) -> str:
    """The Markdown summary of `changes`, each new release with its publish date and age.

    A crate with two releases or more shows them all, so a duplicate a change adds stands out; a
    release crates.io did not date is marked FAILED and counted in the header.
    """
    lines = ["## Cargo.lock changes", ""]
    if not changes:
        lines.append("No crate added, removed or updated.")
        return "\n".join(lines) + "\n"
    counts = ", ".join(
        f"{sum(change.kind == kind for change in changes)} {kind}" for kind in KINDS
    )
    lines += [
        f"{counts}. Read each added crate and each release listed beside an older one;",
        "if a release is under a week old, close this and take next week's.",
    ]
    failed = undated(published)
    if failed:
        noun, pronoun = ("release", "it") if failed == 1 else ("releases", "them")
        lines.append(
            f"{failed} {noun} could not be dated: read {pronoun} by hand; this check fails."
        )
    lines += [
        "",
        "| change | crate | from | to | published |",
        "|---|---|---|---|---|",
    ]
    for change in changes:
        notes = []
        for release in fresh(change):
            if release.source:
                notes.append(f"{release.version}: not on crates.io, read it by hand")
                continue
            entry = published[(change.name, release.version)]
            if isinstance(entry, Unavailable):
                notes.append(f"{release.version}: FAILED ({entry.reason}), read it on crates.io")
                continue
            days = (now - entry.date).days
            flags = (", under a week" if now - entry.date < YOUNG else "") + (
                ", yanked" if entry.yanked else ""
            )
            plural = "" if days == 1 else "s"
            notes.append(f"{release.version}: {entry.date:%Y-%m-%d}, {days} day{plural}{flags}")
        cells = [
            change.kind,
            f"`{change.name}`",
            ", ".join(release.label() for release in change.old),
            ", ".join(release.label() for release in change.new),
            "; ".join(notes),
        ]
        lines.append(f"| {' | '.join(cells)} |")
    return "\n".join(lines) + "\n"


def publish_info(name: str, version: str) -> Published:
    """When crates.io published `name` `version`, and whether it is yanked."""
    path = "/".join(urllib.parse.quote(part, safe="") for part in (name, version))
    request = urllib.request.Request(f"{API}/{path}", headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=30) as response:
        found = json.load(response)["version"]
    return Published(datetime.datetime.fromisoformat(found["created_at"]), bool(found["yanked"]))


def attempt(
    name: str, version: str, fetch: Callable[[str, str], Published]
) -> Published | Unavailable:
    """One request: its answer, or what failed.

    A 4xx other than 429 is not transient: asking again would get the same answer; nor is an
    answer this script cannot read. urllib wraps only the errors of sending the request in
    URLError: a connection reset or cut short while reading the answer comes as a bare OSError
    or HTTPException, so both count as network errors.
    """
    try:
        return fetch(name, version)
    except urllib.error.HTTPError as error:
        # The error holds the open response; close it, as `with urlopen` would have.
        error.close()
        return Unavailable(f"HTTP {error.code}", error.code == 429 or error.code >= 500)
    except urllib.error.URLError as error:
        timeout = isinstance(error.reason, TimeoutError)
        return Unavailable("timeout" if timeout else "network error", transient=True)
    except TimeoutError:
        return Unavailable("timeout", transient=True)
    except (OSError, http.client.HTTPException):
        return Unavailable("network error", transient=True)
    except (ValueError, KeyError, TypeError):
        return Unavailable("bad answer")


def lookup(
    name: str,
    version: str,
    fetch: Callable[[str, str], Published] = publish_info,
    wait: Callable[[float], None] = time.sleep,
) -> Published | Unavailable:
    """`fetch`'s answer, retried once after a transient error; else what failed."""
    found = attempt(name, version, fetch)
    if not (isinstance(found, Unavailable) and found.transient):
        return found
    wait(RETRY_WAIT)
    return attempt(name, version, fetch)


def collect(
    wanted: list[tuple[str, str]],
    find: Callable[[str, str], Published | Unavailable] = lookup,
    pause: Callable[[float], None] = time.sleep,
) -> dict[tuple[str, str], Published | Unavailable]:
    """What `find` says of each (name, version) in `wanted`, one request a second.

    Once a release stays undated after a transient error, crates.io is taken as down and the rest
    are marked without a request: a release costs up to 66 seconds then (two 30-second timeouts,
    the retry wait and the pause), so asking on would outlast the CI job, and lose the table.
    """
    published: dict[tuple[str, str], Published | Unavailable] = {}
    down = False
    for key in wanted:
        if down:
            published[key] = Unavailable("not tried, crates.io unreachable")
            continue
        if published:
            # crates.io's crawler policy: at most one request a second.
            pause(1)
        published[key] = find(*key)
        down = isinstance(published[key], Unavailable) and published[key].transient
    return published


def lock_at(ref: str | None) -> str:
    """Cargo.lock at `ref`, or in the working tree when `ref` is None."""
    if ref is None:
        with open("Cargo.lock", encoding="utf-8") as file:
            return file.read()
    return subprocess.run(
        ["git", "show", f"{ref}:Cargo.lock"], check=True, capture_output=True, text=True
    ).stdout


def main(args: list[str]) -> None:
    """Prints the summary of Cargo.lock from `args[0]` to `args[1]` or the working tree."""
    if len(args) not in (1, 2):
        sys.exit("usage: lock_diff.py <base-ref> [<head-ref>]")
    base = packages(lock_at(args[0]))
    head = packages(lock_at(args[1] if len(args) == 2 else None))
    changes = diff(base, head)
    published = collect(
        [
            (change.name, release.version)
            for change in changes
            for release in fresh(change)
            if not release.source
        ]
    )
    print(render(changes, published, datetime.datetime.now(datetime.timezone.utc)), end="")
    # The table is printed first, so one failed request costs one cell, not the summary.
    if undated(published):
        sys.exit("some releases could not be dated: their cells say FAILED")


if __name__ == "__main__":
    main(sys.argv[1:])
