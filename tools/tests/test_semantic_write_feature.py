import json
import re
import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
FEATURE = ROOT / "features" / "semantic-writes.feature"
SCENARIO = re.compile(r"^  Scenario: (?P<name>.+)$")
MAPPING = re.compile(
    r"^  # fava:rust=(?P<package>[a-z0-9-]+)/(?P<target>[a-z0-9_]+)#"
    r"(?P<test>[a-z][a-z0-9_]*(?:::[a-z][a-z0-9_]*)*)$"
)
STEP = re.compile(r"^    (?:Given|When|Then|And) (.+)$")


def parse_feature(text: str):
    scenarios = []
    pending_mapping = None
    pending_mapping_line = None
    mapping_conflict = False
    current = None
    malformed_mappings = []
    for line in text.splitlines():
        if "fava:rust=" in line:
            match = MAPPING.fullmatch(line)
            if match is None:
                malformed_mappings.append(line)
            else:
                if pending_mapping is not None:
                    malformed_mappings.extend([pending_mapping_line, line])
                    pending_mapping = None
                    pending_mapping_line = None
                    mapping_conflict = True
                elif mapping_conflict:
                    malformed_mappings.append(line)
                else:
                    pending_mapping = match.groupdict()
                    pending_mapping_line = line
            continue
        scenario = SCENARIO.fullmatch(line)
        if scenario is not None:
            if mapping_conflict:
                pending_mapping = None
                pending_mapping_line = None
                mapping_conflict = False
                current = None
                continue
            current = {
                "name": scenario.group("name"),
                "mapping": pending_mapping,
                "steps": [],
            }
            scenarios.append(current)
            pending_mapping = None
            pending_mapping_line = None
            continue
        step = STEP.fullmatch(line)
        if step is not None and current is not None:
            current["steps"].append(step.group(1))
    return scenarios, malformed_mappings, pending_mapping


def validate_mapping_target(target, listed_lines, expected_test):
    if target is None or "test" not in target.get("kind", []):
        raise ValueError("mapping target does not resolve to a Cargo test target")
    listed_tests = [
        line.removesuffix(": test")
        for line in listed_lines
        if line.endswith(": test")
    ]
    if not listed_tests:
        raise ValueError("mapping target listed zero tests")
    if listed_tests.count(expected_test) != 1:
        raise ValueError("mapped test must occur exactly once in Cargo --list output")
    return expected_test


def cargo_mapping_evidence(mapping):
    package_name = mapping["package"]
    manifest = ROOT / "Cargo.toml"
    lockfile = manifest.parent / "Cargo.lock"
    if not lockfile.is_file():
        raise FileNotFoundError(f"mapping lockfile does not exist: {lockfile}")
    metadata = subprocess.run(
        [
            "cargo",
            "metadata",
            "--locked",
            "--manifest-path",
            str(manifest),
            "--no-deps",
            "--format-version",
            "1",
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    packages = [
        package
        for package in json.loads(metadata.stdout)["packages"]
        if package["name"] == package_name
    ]
    if len(packages) != 1:
        raise ValueError("mapping package must resolve exactly once")
    targets = [
        target
        for target in packages[0]["targets"]
        if target["name"] == mapping["target"]
    ]
    if len(targets) != 1:
        raise ValueError("mapping target must resolve exactly once")
    listed = subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "--manifest-path",
            str(manifest),
            "-p",
            package_name,
            "--test",
            mapping["target"],
            "--",
            "--list",
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return validate_mapping_target(
        targets[0], listed.stdout.splitlines(), mapping["test"]
    )


class SemanticWriteFeatureMappingTests(unittest.TestCase):
    def setUp(self):
        self.text = FEATURE.read_text(encoding="utf-8")
        self.scenarios, self.malformed, self.trailing = parse_feature(self.text)

    def test_scenario_names_are_unique(self):
        names = [scenario["name"] for scenario in self.scenarios]
        self.assertEqual(len(names), len(set(names)))

    def test_every_scenario_has_one_well_formed_rust_mapping(self):
        self.assertEqual(self.malformed, [])
        self.assertIsNone(self.trailing)
        self.assertTrue(self.scenarios)
        for scenario in self.scenarios:
            with self.subTest(scenario=scenario["name"]):
                self.assertIsNotNone(scenario["mapping"])

    def test_steps_are_observable_and_not_placeholders(self):
        placeholder = re.compile(r"\b(?:todo|tbd|placeholder|implement later)\b", re.I)
        for scenario in self.scenarios:
            with self.subTest(scenario=scenario["name"]):
                self.assertGreaterEqual(len(scenario["steps"]), 3)
                self.assertFalse(any(placeholder.search(step) for step in scenario["steps"]))

    def test_current_source_and_retired_mappings_are_exact(self):
        mappings = {
            scenario["name"]: scenario["mapping"] for scenario in self.scenarios
        }
        self.assertEqual(
            mappings["source-v2 rematerialization"]["test"],
            "newer_source_rematerializes_once_and_preserves_unrelated_fields",
        )
        self.assertEqual(
            mappings["retired completion"]["test"],
            "interleavings::retired_completion_is_attributable_and_inert",
        )

    def test_module_qualified_names_parse_and_malformed_names_refuse(self):
        valid = (
            "  # fava:rust=fava/semantic_write_publication#"
            "interleavings::retired_completion_is_attributable_and_inert\n"
            "  Scenario: retired completion\n"
            "    Given a current write\n"
            "    When retired work completes\n"
            "    Then it is inert\n"
        )
        scenarios, malformed, trailing = parse_feature(valid)
        self.assertEqual(malformed, [])
        self.assertIsNone(trailing)
        self.assertEqual(
            scenarios[0]["mapping"]["test"],
            "interleavings::retired_completion_is_attributable_and_inert",
        )
        for broken in ("::leading", "trailing::", "double::::component"):
            text = valid.replace(
                "interleavings::retired_completion_is_attributable_and_inert", broken
            )
            _, malformed, _ = parse_feature(text)
            self.assertEqual(len(malformed), 1)

    def test_duplicate_pending_mapping_comments_refuse(self):
        duplicate = (
            "  # fava:rust=fava/semantic_write_contract#first_value_receives_no_prior_and_exact_timestamp\n"
            "  # fava:rust=fava/semantic_write_store#memory_first_edit_has_no_prior\n"
            "  Scenario: ambiguous mapping\n"
            "    Given one scenario\n"
            "    When two mappings precede it\n"
            "    Then parsing refuses\n"
        )
        scenarios, malformed, trailing = parse_feature(duplicate)
        self.assertEqual(scenarios, [])
        self.assertEqual(len(malformed), 2)
        self.assertIsNone(trailing)

    def test_target_and_list_validation_fail_closed(self):
        target = {"name": "semantic", "kind": ["test"]}
        self.assertEqual(
            validate_mapping_target(target, ["module::case: test"], "module::case"),
            "module::case",
        )
        for target_value, listed, name in (
            (None, ["case: test"], "case"),
            ({"name": "semantic", "kind": ["bin"]}, ["case: test"], "case"),
            (target, [], "case"),
            (target, ["0 tests"], "case"),
            (target, ["case: test", "case: test"], "case"),
        ):
            with self.subTest(target=target_value, listed=listed, name=name):
                with self.assertRaises(ValueError):
                    validate_mapping_target(target_value, listed, name)

    def test_every_mapping_resolves_to_one_real_cargo_test(self):
        for scenario in self.scenarios:
            with self.subTest(scenario=scenario["name"]):
                self.assertEqual(
                    cargo_mapping_evidence(scenario["mapping"]),
                    scenario["mapping"]["test"],
                )


if __name__ == "__main__":
    unittest.main()
