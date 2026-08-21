import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
FEATURE = ROOT / "features" / "semantic-writes.feature"
SCENARIO = re.compile(r"^  Scenario: (?P<name>.+)$")
MAPPING = re.compile(
    r"^  # fava:rust=(?P<package>[a-z0-9-]+)/(?P<target>[a-z0-9_]+)#"
    r"(?P<test>[a-z][a-z0-9_]*)$"
)
STEP = re.compile(r"^    (?:Given|When|Then|And) (.+)$")


def parse_feature(text: str):
    scenarios = []
    pending_mapping = None
    current = None
    malformed_mappings = []
    for line in text.splitlines():
        if "fava:rust=" in line:
            match = MAPPING.fullmatch(line)
            if match is None:
                malformed_mappings.append(line)
            else:
                pending_mapping = match.groupdict()
            continue
        scenario = SCENARIO.fullmatch(line)
        if scenario is not None:
            current = {
                "name": scenario.group("name"),
                "mapping": pending_mapping,
                "steps": [],
            }
            scenarios.append(current)
            pending_mapping = None
            continue
        step = STEP.fullmatch(line)
        if step is not None and current is not None:
            current["steps"].append(step.group(1))
    return scenarios, malformed_mappings, pending_mapping


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


if __name__ == "__main__":
    unittest.main()
