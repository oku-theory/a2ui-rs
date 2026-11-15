#!/usr/bin/env python3
"""Builds fuzz/Cargo.lock from Cargo.lock, so the fuzz workspace cannot drift from it.

Usage: fuzz_lock.py assemble|verify|pin [--root DIR]. fuzz/Cargo.lock is generated, never
committed (plan 2b, Decision 45): `assemble` writes it from Cargo.lock's packages, which win for
every crate they hold a semver-compatible release of, then the pins of fuzz/fuzz-only.lock, the
reviewed releases of what fuzz/ alone depends on. Cargo then drops what is unused and resolves
anything still missing; `verify` exits 1 naming each release neither file holds, such as a new
fuzz-only dependency, and `pin` rewrites fuzz/fuzz-only.lock from the resolved fuzz/Cargo.lock for
a person to review. Runs offline; needs Python 3.11 or later (tomllib).
"""

import sys

if sys.version_info < (3, 11):
    sys.exit("fuzz_lock.py needs Python 3.11 or later (tomllib)")

import argparse  # noqa: E402
import pathlib  # noqa: E402
import re  # noqa: E402
import tomllib  # noqa: E402

PINS_HEADER = (
    "# The releases fuzz/ alone depends on, which `just fuzz-lock` writes and a person reviews.\n"
    "# fuzz/Cargo.lock is Cargo.lock's packages, then these (plan 2b, Decision 45).\n"
    "version = 4\n"
)


class LockError(Exception):
    """A lock this script cannot read, or a fuzz/Cargo.lock that is not resolved yet."""


def blocks(text):
    """The lock's header (all before the first package) and its `[[package]]` blocks, raw."""
    first = re.search(r"^\[\[package\]\]$", text, re.M)
    if first is None:
        return text, []
    start = first.start()
    found = re.findall(r"\[\[package\]\]\n.*?(?=\n\[|\Z)", text[start:], re.S)
    rest = re.sub(r"\[\[package\]\]\n.*?(?=\n\[|\Z)", "", text[start:], flags=re.S)
    if rest.strip():
        raise LockError(f"a lock holds something other than packages: {rest.strip()[:60]!r}")
    return text[:start], [block.strip("\n") + "\n" for block in found]


def fields(block):
    """The parsed `[[package]]` table of one raw block."""
    try:
        package = tomllib.loads(block)["package"][0]
        package["name"], package["version"]
    except (tomllib.TOMLDecodeError, KeyError, IndexError) as error:
        raise LockError(f"unreadable package block: {block[:60]!r}") from error
    return package


def key(block):
    """The (name, version, source) of one block; a path crate's source is None."""
    package = fields(block)
    return package["name"], package["version"], package.get("source")


def semver_line(name, version):
    """The crate and the releases Cargo deems compatible with `version` under a caret.

    The leftmost non-zero part of major.minor.patch decides; a pre-release matches only itself.
    """
    if "-" in version.split("+")[0]:
        return name, version
    major, minor, patch = version.split("+")[0].split(".")[:3]
    if major != "0":
        return name, major
    return name, "0." + minor if minor != "0" else "0.0." + patch


def registry(text):
    """The (name, version, source) of every package from a registry, path crates left out."""
    return {key(block) for block in blocks(text)[1] if key(block)[2] is not None}


def qualify(block, ambiguous, versions):
    """`block` with each bare dependency named in `ambiguous` given its version in `versions`."""
    def entry(match):
        name = match.group(2)
        if name in ambiguous and name in versions:
            return f'{match.group(1)}"{name} {versions[name]}",'
        return match.group(0)

    return re.sub(r'^( +)"([^" ]+)",$', entry, block, flags=re.M)


def assemble(root_text, pins_text):
    """Cargo.lock's packages, then the pins whose semver line Cargo.lock lacks.

    A bare dependency entry ("name") names the one release its own file holds; when the other
    file adds a second release of that name, the entry gets its version, as Cargo would write it.
    """
    header, root = blocks(root_text)
    held = {semver_line(*key(block)[:2]) for block in root}
    extra = [block for block in blocks(pins_text)[1] if semver_line(*key(block)[:2]) not in held]
    names = [key(block)[0] for block in root + extra]
    ambiguous = {name for name in names if names.count(name) > 1}

    def versions(group):
        return {key(block)[0]: key(block)[1] for block in group}

    root = [qualify(block, ambiguous, versions(root)) for block in root]
    extra = [qualify(block, ambiguous, versions(blocks(pins_text)[1])) for block in extra]
    return header + "\n".join(root + extra)


def unpinned(lock_text, root_text, pins_text):
    """The registry releases in the resolved lock that the assembled lock did not hold.

    That is any release Cargo resolved anew: a fuzz-only crate no pin covers, or a crate
    Cargo.lock holds that fuzz/ needs at another release, which `cargo audit` never saw.
    """
    known = registry(assemble(root_text, pins_text))
    return sorted(registry(lock_text) - known, key=lambda release: release[:2])


def pins(lock_text):
    """The registry packages the fuzz crate's non-path dependencies reach, as a pins file."""
    found = blocks(lock_text)[1]
    tables = [fields(block) for block in found]

    def resolve(entry):
        """The index of the package a `dependencies` entry ("name" or "name version") names."""
        name, *rest = entry.split(" ")
        matches = [
            index
            for index, table in enumerate(tables)
            if table["name"] == name and (not rest or table["version"] == rest[0])
        ]
        if not matches:
            raise LockError(f"fuzz/Cargo.lock names no package for dependency '{entry}'")
        return matches[0]

    fuzz = [table for table in tables if table["name"] == "a2ui-fuzz"]
    if not fuzz:
        raise LockError("fuzz/Cargo.lock holds no a2ui-fuzz: run just fuzz-lock, not pin alone")
    queue = [resolve(entry) for entry in fuzz[0].get("dependencies", [])]
    queue = [index for index in queue if "source" in tables[index]]
    reached = set()
    while queue:
        index = queue.pop()
        if index not in reached:
            reached.add(index)
            queue += [resolve(entry) for entry in tables[index].get("dependencies", [])]
    kept = sorted(reached, key=lambda index: (tables[index]["name"], tables[index]["version"]))
    return PINS_HEADER + "".join("\n" + found[index] for index in kept)


def main(argv):
    """Runs one command; returns the exit status."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("command", choices=["assemble", "verify", "pin"])
    parser.add_argument("--root", default=".", type=pathlib.Path)
    args = parser.parse_args(argv)
    root_lock = args.root / "Cargo.lock"
    pins_lock = args.root / "fuzz" / "fuzz-only.lock"
    fuzz_lock = args.root / "fuzz" / "Cargo.lock"
    try:
        if args.command == "assemble":
            fuzz_lock.write_text(assemble(root_lock.read_text(), pins_lock.read_text()))
        elif args.command == "verify":
            root, pinned = root_lock.read_text(), pins_lock.read_text()
            missing = unpinned(fuzz_lock.read_text(), root, pinned)
            if missing:
                held = {key(block)[0]: key(block)[1] for block in blocks(root)[1]}
                print("fuzz/ resolved releases Cargo.lock and fuzz/fuzz-only.lock do not give it:",
                      file=sys.stderr)
                for name, version, _ in missing:
                    note = f" (Cargo.lock holds {held[name]})" if name in held else ""
                    print(f"  {name} {version}{note}", file=sys.stderr)
                print("a fuzz-only crate: run just fuzz-lock, then review and commit the pins;",
                      file=sys.stderr)
                print("one Cargo.lock holds: fuzz/ needs a release it lacks, update Cargo.lock",
                      file=sys.stderr)
                return 1
        else:
            root = registry(root_lock.read_text())
            old = registry(pins_lock.read_text())
            text = pins(fuzz_lock.read_text())
            pins_lock.write_text(text)
            new = registry(text)
            print("fuzz/fuzz-only.lock adds, beyond Cargo.lock (review each new release):")
            for name, version, _ in sorted(new - old - root):
                print(f"  {name} {version}")
            print("and drops:")
            for name, version, _ in sorted(old - new):
                print(f"  {name} {version}")
    except (LockError, OSError) as error:
        print(f"fuzz_lock.py {args.command}: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
