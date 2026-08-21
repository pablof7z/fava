import json
import re
import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
FEATURE = ROOT / "features" / "publication-door.feature"
TEST_SOURCE = ROOT / "crates" / "fava" / "tests" / "publication_door.rs"
SCENARIO = re.compile(r"^  Scenario: (?P<name>.+)$")
MAPPING = re.compile(
    r"^  # fava:rust=(?P<package>[a-z0-9-]+)/(?P<target>[a-z0-9_]+)#"
    r"(?P<test>[a-z][a-z0-9_]*)$"
)
STEP = re.compile(r"^    (?:Given|When|Then|And) (.+)$")
RUST_TEST = re.compile(r"^async fn (?P<name>[a-z][a-z0-9_]*)\(\)", re.MULTILINE)


def parse_feature(text: str):
    scenarios = []
    pending = None
    malformed = []
    current = None
    for line in text.splitlines():
        if "fava:rust=" in line:
            match = MAPPING.fullmatch(line)
            if match is None or pending is not None:
                malformed.append(line)
                pending = None
            else:
                pending = match.groupdict()
            continue
        scenario = SCENARIO.fullmatch(line)
        if scenario is not None:
            current = {
                "name": scenario.group("name"),
                "mapping": pending,
                "steps": [],
            }
            scenarios.append(current)
            pending = None
            continue
        step = STEP.fullmatch(line)
        if step is not None and current is not None:
            current["steps"].append(step.group(1))
    return scenarios, malformed, pending


class PublicationDoorFeatureTests(unittest.TestCase):
    def setUp(self):
        self.scenarios, self.malformed, self.trailing = parse_feature(
            FEATURE.read_text(encoding="utf-8")
        )

    def test_scenarios_are_unique_mapped_and_observable(self):
        names = [scenario["name"] for scenario in self.scenarios]
        self.assertEqual(len(names), len(set(names)))
        self.assertEqual(self.malformed, [])
        self.assertIsNone(self.trailing)
        self.assertTrue(self.scenarios)
        for scenario in self.scenarios:
            with self.subTest(scenario=scenario["name"]):
                self.assertIsNotNone(scenario["mapping"])
                self.assertGreaterEqual(len(scenario["steps"]), 3)

    def test_every_mapping_resolves_to_the_real_cargo_target_and_test(self):
        metadata = subprocess.run(
            ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
        package = next(
            package
            for package in json.loads(metadata.stdout)["packages"]
            if package["name"] == "fava"
        )
        targets = {target["name"]: target for target in package["targets"]}
        rust_tests = set(RUST_TEST.findall(TEST_SOURCE.read_text(encoding="utf-8")))
        for scenario in self.scenarios:
            mapping = scenario["mapping"]
            with self.subTest(scenario=scenario["name"]):
                self.assertEqual(mapping["package"], "fava")
                self.assertIn(mapping["target"], targets)
                self.assertIn("test", targets[mapping["target"]]["kind"])
                self.assertIn(mapping["test"], rust_tests)

    def test_parser_refuses_malformed_or_ambiguous_mappings(self):
        malformed = FEATURE.read_text(encoding="utf-8").replace(
            "fava/publication_door#publish_payload_forms",
            "fava/publication-door#PublishPayloadForms",
            1,
        )
        _, errors, _ = parse_feature(malformed)
        self.assertEqual(len(errors), 1)

        duplicate = (
            "  # fava:rust=fava/publication_door#one\n"
            "  # fava:rust=fava/publication_door#two\n"
            "  Scenario: ambiguous\n"
        )
        _, errors, _ = parse_feature(duplicate)
        self.assertEqual(len(errors), 1)


if __name__ == "__main__":
    unittest.main()
