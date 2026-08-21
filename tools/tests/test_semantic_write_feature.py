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


if __name__ == "__main__":
    unittest.main()
