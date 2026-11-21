#!/usr/bin/env python3
"""The fuzz targets' table: each one's libFuzzer limits and corpus seeds.

Usage: fuzz_targets.py limits <target> prints `max_len timeout` for `just fuzz` and
`just fuzz-minimize`; fuzz_targets.py seed <target> <corpus-dir> writes the target's seeds from
vendor/a2ui, each named by its hash, so seeding twice adds nothing and libFuzzer's own entries
stay. The tests check the table against fuzz/Cargo.toml's bins and fuzz.yml's matrix.

Inputs with several parts join them with NUL, as fuzz/src/lib.rs's `fields` splits them, small
parts first so mutations reach them: catalog_parse takes [catalog_id, [protocol_version,]] then
the catalog; validate_component a component, then a default catalog id; parse_then_validate a
component, [a default catalog id,] then the catalog. template_parse gets each string holding
`${`, pointer_parse each `@path` value and each string starting with `/`. message_parse takes
one message as JSON text: each message of the basic catalog's examples and one per action.
regex_match takes a pattern, then a value: the protocol's regex examples and patterns a
backtracking engine takes exponential time on.
"""

import glob
import hashlib
import json
import pathlib
import sys

# Name: (max_len, timeout in seconds). Lengths stay near what each entry point is meant for:
# MAX_CATALOG_BYTES is 4 MiB, but 1 MiB catalogs already reach the node caps; 10 000 template
# characters; a component has no text cap of its own (Plan 3 caps messages). No input took a
# second on a laptop (plan 2b, Decision 46): a slower one is a finding.
TARGETS = {
    "catalog_parse": (1048576, 2),
    "validate_component": (262144, 1),
    "parse_then_validate": (1048576, 2),
    "template_parse": (16384, 1),
    "pointer_parse": (4096, 1),
    # Past the 1 MiB default message cap, so the cap itself is reached.
    "message_parse": (1100000, 2),
    # A 256-code-point pattern and a value past the 4096-code-point cap, in 4-byte code points.
    "regex_match": (20480, 1),
}
BASIC = "vendor/a2ui/catalogs/basic/v1/catalog.json"
# An id no catalog has, so a nested call's catalog is not found.
UNKNOWN_ID = b"https://example.com/catalogs/unknown/v1/catalog.json"
# Bounds seeds that come from every string of every document.
MAX_STRING_SEEDS = 2000
# The runtime's caps a component meets first (crates/a2ui-runtime/src/limits.rs).
MAX_COMPONENT_NODES = 65536
MAX_COMPONENT_CALLS = 1024
MAX_JSON_DEPTH = 64


def files(pattern):
    """The sorted files matching `pattern`; fails when none does, rather than seed nothing."""
    found = sorted(glob.glob(pattern))
    if not found:
        raise SystemExit(f"fuzz_targets.py: no file matches {pattern} (run from the repo root)")
    return found


def walk_strings(node):
    """Every string in a JSON value, object keys included, in document order."""
    if isinstance(node, str):
        yield node
    elif isinstance(node, list):
        for item in node:
            yield from walk_strings(item)
    elif isinstance(node, dict):
        for name, value in node.items():
            yield name
            yield from walk_strings(value)


def bindings(node):
    """Every `@path` string value in a JSON value."""
    if isinstance(node, list):
        for item in node:
            yield from bindings(item)
    elif isinstance(node, dict):
        for name, value in node.items():
            if name == "@path" and isinstance(value, str):
                yield value
            yield from bindings(value)


def templates(documents):
    """The distinct strings holding `${`, in first-seen order."""
    found = (text for document in documents for text in walk_strings(document) if "${" in text)
    return list(dict.fromkeys(found))[:MAX_STRING_SEEDS]


def paths(documents):
    """The distinct `@path` values and strings starting with `/`, sorted."""
    found = {path for document in documents for path in bindings(document)}
    found |= {
        text for document in documents for text in walk_strings(document) if text.startswith("/")
    }
    return sorted(found)[:MAX_STRING_SEEDS]


def at_caps():
    """Components just within and just past each component cap, as canonical JSON bytes.

    A component counts 1 node, plus one per member of each object and array; its four members
    here leave `MAX_COMPONENT_NODES - 5` for the `extra` array.
    """
    def component(extra):
        return {"id": "t", "component": "Text", "text": "x", "extra": extra}

    def nested(depth):
        value = []
        for _ in range(depth):
            value = [value]
        return value

    room = MAX_COMPONENT_NODES - 5
    call = {"@call": "now"}
    shapes = [component([0] * room), component([0] * (room + 1))]
    counts = (MAX_COMPONENT_CALLS, MAX_COMPONENT_CALLS + 1)
    shapes += [component([call] * count) for count in counts]
    shapes += [component(nested(depth)) for depth in range(MAX_JSON_DEPTH - 3, MAX_JSON_DEPTH + 1)]
    return [json.dumps(shape, separators=(",", ":")).encode() for shape in shapes]


def parsed(name):
    """The JSON document in file `name`, or None for the conformance cases' invalid ones."""
    try:
        return json.loads(pathlib.Path(name).read_bytes())
    except (json.JSONDecodeError, UnicodeDecodeError):
        return None


def components():
    """Each component instance of the basic catalog's examples, as canonical JSON bytes."""
    found = []
    for name in files("vendor/a2ui/catalogs/basic/v1/examples/*.json"):
        for message in json.loads(pathlib.Path(name).read_text()).get("messages", []):
            for component in (message.get("updateComponents") or {}).get("components", []):
                found.append(json.dumps(component, sort_keys=True).encode())
    return found


def example_messages():
    """Each message of the basic catalog's examples, as compact JSON bytes.

    A file holds a `{"messages": [...]}` wrapper, an array or one message; another shape fails.
    """
    found = []
    for name in files("vendor/a2ui/catalogs/basic/v1/examples/*.json"):
        document = json.loads(pathlib.Path(name).read_text())
        if isinstance(document, dict) and isinstance(document.get("messages"), list):
            messages = document["messages"]
        elif isinstance(document, list):
            messages = document
        elif isinstance(document, dict) and "version" in document:
            messages = [document]
        else:
            raise SystemExit(f"fuzz_targets.py: {name} holds no message, array or wrapper")
        found += [json.dumps(message, separators=(",", ":")).encode() for message in messages]
    return found


def action_messages():
    """One valid message per action, as compact JSON bytes."""
    call = {"@call": "now", "catalogId": "c"}
    bodies = [
        {"createSurface": {"surfaceId": "s", "catalogId": "c", "sendDataModel": True}},
        {"updateComponents": {"surfaceId": "s", "components": [
            {"id": "root", "component": "Text", "text": "hi"}]}},
        {"updateDataModel": {"surfaceId": "s", "path": "/a/0", "value": {"b": [1, None]}}},
        {"deleteSurface": {"surfaceId": "s"}},
        {"callRendererFunction": {"surfaceId": "s", "callFunction": call,
                                  "functionCallId": "f1"}},
        {"agentFunctionResponse": {"functionCallId": "f1", "value": 3}},
    ]
    return [json.dumps({"version": "1.0", **body}, separators=(",", ":")).encode()
            for body in bodies]


def regex_pairs():
    """Pattern and value pairs: the dialect's examples, then adversarial shapes at the caps."""
    long = "a" * 4095 + "!"
    pairs = [
        ("^[0-9]+$", "12345"), ("^abc$", "abc\n"), ("^\\d+$", "\u0661\u0662"),
        ("\\bcaf\\b", "caf\u00e9"), ("^.$", "\U0001F600"), ("^\\u{1F600}+$", "\U0001F600"),
        ("(?:ab+)?c", "abbc"), ("^[a-z]+@[a-z]+\\.com$", "ada@example.com"),
        ("^(?=.*\\d).{8,}$", "secret42"), ("^(?!a)[^\\s]{1,3}?$", "b c"),
        ("a{1,1000}b", "a" * 4096), ("(?:(?:a|a)?)*", long), ("^(?:(?:a|a)|b)*$", long),
        ("(?:x(?:a|a))*", "xa" * 2047 + "!!"), ("^a+(?:b?a*)$", long),
        ("a?" * 127 + "b$", "a" * 4097), ("(a{1,1000}){1,1000}", "a"),
    ]
    return [pattern.encode() + b"\0" + value.encode() for pattern, value in pairs]


def catalogs():
    """The vendored catalogs' texts: the published ones, then the conformance suite's."""
    names = files("vendor/a2ui/catalogs/*/v1/catalog.json")
    names += files("vendor/a2ui/conformance/test_data/catalogs/*.json")
    return [pathlib.Path(name).read_bytes() for name in names]


def seeds(target):
    """The seeds of `target`, as bytes."""
    if target == "catalog_parse":
        basic = pathlib.Path(files(BASIC)[0]).read_bytes()
        return catalogs() + [UNKNOWN_ID + b"\0" + basic, UNKNOWN_ID + b"\x001.0\0" + basic]
    if target == "validate_component":
        found = components()
        return found + [c + b"\0" + UNKNOWN_ID for c in found[:20]] + at_caps()
    if target == "parse_then_validate":
        found = components()
        basic = pathlib.Path(files(BASIC)[0]).read_bytes()
        pairs = [component + b"\0" + basic for component in found]
        pairs += [found[index % len(found)] + b"\0" + text for index, text in enumerate(catalogs())]
        return pairs + [found[0] + b"\0" + UNKNOWN_ID + b"\0" + basic]
    if target == "message_parse":
        return example_messages() + action_messages()
    if target == "regex_match":
        return regex_pairs()
    names = files("vendor/a2ui/catalogs/basic/v1/examples/*.json")
    names += files("vendor/a2ui/conformance/test_data/catalogs/*.json")
    names += files("vendor/a2ui/specification/v1_0/test/cases/*.json")
    documents = [document for name in names if (document := parsed(name)) is not None]
    if target == "template_parse":
        return [text.encode() for text in templates(documents)]
    if target == "pointer_parse":
        return [text.encode() for text in paths(documents)] + [b"", b"/", b"/a~1b/~0c/0"]
    raise SystemExit(f"fuzz_targets.py: unknown target '{target}': {', '.join(TARGETS)}")


def main(argv):
    """Runs `limits <target>` or `seed <target> <corpus-dir>`; returns the exit status."""
    if len(argv) < 2 or argv[0] not in ("limits", "seed") or argv[1] not in TARGETS:
        print("usage: fuzz_targets.py limits|seed <target> [corpus-dir]; targets:",
              ", ".join(TARGETS), file=sys.stderr)
        return 2
    command, target = argv[0], argv[1]
    if command == "limits":
        print(*TARGETS[target])
        return 0
    if len(argv) != 3:
        print("usage: fuzz_targets.py seed <target> <corpus-dir>", file=sys.stderr)
        return 2
    corpus = pathlib.Path(argv[2])
    corpus.mkdir(parents=True, exist_ok=True)
    found = seeds(target)
    for seed in found:
        (corpus / ("seed-" + hashlib.sha256(seed).hexdigest()[:16])).write_bytes(seed)
    print(f"{len(found)} seeds for {target}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
