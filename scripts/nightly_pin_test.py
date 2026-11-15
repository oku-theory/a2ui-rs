"""Tests that Renovate still finds the justfile's dated nightly and CI checks its bump.

Renovate fails silently on both: an edit its pattern no longer matches leaves the pin stale, and
a branch name ci.yml does not expect skips the `minimal-versions` job, which reads as green
(plan 2b, Decision 41).
"""

import json
import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
DEP_NAME = "rust-nightly"


def renovate():
    """renovate.json, parsed."""
    return json.loads((ROOT / "renovate.json").read_text(encoding="utf-8"))


def nightly_manager(config):
    """The custom manager that names its dependency `rust-nightly`."""
    found = [m for m in config["customManagers"] if m.get("depNameTemplate") == DEP_NAME]
    if len(found) != 1:
        raise AssertionError(f"expected one custom manager for {DEP_NAME}, found {len(found)}")
    return found[0]


def nightly_rule(config):
    """The package rule matching `rust-nightly`."""
    found = [r for r in config["packageRules"] if DEP_NAME in r.get("matchDepNames", [])]
    if len(found) != 1:
        raise AssertionError(f"expected one package rule for {DEP_NAME}, found {len(found)}")
    return found[0]


def python_pattern(pattern):
    """A Renovate (JavaScript) pattern in Python's syntax: `(?<name>` becomes `(?P<name>`."""
    return re.sub(r"\(\?<(?=[A-Za-z])", "(?P<", pattern)


class NightlyPinTest(unittest.TestCase):
    """The pin, Renovate's manager and rule, and ci.yml agree."""

    def test_manager_on_justfile_extracts_the_one_assignment(self):
        manager = nightly_manager(renovate())
        justfile = (ROOT / "justfile").read_text(encoding="utf-8")
        assigned = re.findall(r'^nightly := "([^"]*)"$', justfile, re.MULTILINE)
        matches = [
            m.groupdict()
            for pattern in manager["matchStrings"]
            for m in re.finditer(python_pattern(pattern), justfile)
        ]
        self.assertEqual(
            matches,
            [
                {
                    "datasource": "rust-version",
                    "versioning": "rust-release-channel",
                    "currentValue": assigned[0] if len(assigned) == 1 else None,
                }
            ],
        )

    def test_manager_file_pattern_matches_the_justfile(self):
        patterns = nightly_manager(renovate())["managerFilePatterns"]
        self.assertTrue(any(re.search(p.strip("/"), "justfile") for p in patterns))

    def test_ci_job_condition_names_the_rule_branch(self):
        config = renovate()
        rule = nightly_rule(config)
        # The branch name is the default prefix and the rule's topic only while nothing
        # overrides them.
        for key in ("branchPrefix", "additionalBranchPrefix", "branchName", "groupName"):
            self.assertNotIn(key, config)
            self.assertNotIn(key, rule)
        branch = f"renovate/{rule['branchTopic']}"
        ci = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        job = ci.split("\n  minimal-versions:\n", 1)[1].split("\n    steps:\n", 1)[0]
        self.assertIn(f"(github.head_ref || github.ref_name) == '{branch}'", job)


if __name__ == "__main__":
    unittest.main()
