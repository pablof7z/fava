"""Behavior tests for the architectural vocabulary gate."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "check_vocabulary.py"


class VocabularyCheckTest(unittest.TestCase):
    def test_rejects_an_undocumented_public_symbol(self) -> None:
        result = self.run_check(
            source="pub struct Query;\npub struct ResolvedQuery;\n",
            symbols=["sample::Query"],
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("sample::ResolvedQuery", result.stderr)
        self.assertIn("existing noun: Query", result.stderr)

    def test_accepts_public_symbols_defined_by_the_registry(self) -> None:
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
        )

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_an_undocumented_specification_symbol(self) -> None:
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification="pub struct MysteryManager;\n",
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("MysteryManager", result.stderr)

    def test_accepts_a_registered_specification_symbol(self) -> None:
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification="pub struct ReplaceableEventEdit;\n",
            specification_symbols=["ReplaceableEventEdit"],
        )

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_ignores_temporary_evidence_directory_as_a_crate(self) -> None:
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification="evidence: /tmp/fava-tag-values-06-1/run-id\n",
        )

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_temporary_path_does_not_suppress_same_line_crate_reference(self) -> None:
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification=(
                "unregistered fava-hidden remains invalid; "
                "evidence: /tmp/fava-hidden/run-id\n"
            ),
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("undocumented specified architectural crate: fava-hidden", result.stderr)

    def test_phase_slug_metadata_is_not_a_crate(self) -> None:
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification="---\nslug: deliver-fava-example-capability-as-a-service\n---\n",
        )

        self.assertEqual(result.returncode, 0, result.stderr)

        phase_path = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification=(
                "source: /var/project/phases/12.3-deliver-"
                "fava-example-capability-as-a-service/CONTEXT.md\n"
            ),
        )
        self.assertEqual(phase_path.returncode, 0, phase_path.stderr)

    def test_linked_worktree_prefix_is_not_a_crate(self) -> None:
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification=(
                "worktree: /var/work/fava-worktree-agent-example-resume/"
                "crates/fava-simple-groups\n"
            ),
            specification_crates=["fava-simple-groups"],
        )

        self.assertEqual(result.returncode, 0, result.stderr)

        path_control = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification=(
                "worktree: /var/work/fava-worktree-agent-example-resume/"
                "crates/fava-unregistered/src/lib.rs\n"
            ),
        )
        self.assertNotEqual(path_control.returncode, 0)
        self.assertIn(
            "undocumented specified architectural crate: fava-unregistered",
            path_control.stderr,
        )

    def test_metadata_exclusion_does_not_hide_real_crate_reference(self) -> None:
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification=(
                "slug: deliver-fava-example-capability-as-a-service "
                "# uses fava-simple-groups and fava-unregistered\n"
            ),
            specification_crates=["fava-simple-groups"],
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "undocumented specified architectural crate: fava-unregistered",
            result.stderr,
        )

    def test_checker_diagnostic_is_not_a_crate_declaration(self) -> None:
        diagnostic = (
            "checker output: `undocumented specified architectural crate: "
            "fava-example-capability-as-a-service`"
        )
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification=f"{diagnostic}\n",
        )

        self.assertEqual(result.returncode, 0, result.stderr)

        control = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification=f"{diagnostic}; actual dependency fava-unregistered\n",
        )
        self.assertNotEqual(control.returncode, 0)
        self.assertIn(
            "undocumented specified architectural crate: fava-unregistered",
            control.stderr,
        )

    def run_check(
        self,
        *,
        source: str,
        symbols: list[str],
        specification: str = "",
        specification_symbols: list[str] | None = None,
        specification_crates: list[str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            crate = root / "crates" / "sample"
            (crate / "src").mkdir(parents=True)
            (root / "docs" / "internals").mkdir(parents=True)
            (root / "docs" / "spec").mkdir(parents=True)
            (crate / "Cargo.toml").write_text(
                '[package]\nname = "sample"\n', encoding="utf-8"
            )
            (crate / "src" / "lib.rs").write_text(source, encoding="utf-8")
            (root / "docs" / "spec" / "ARCHITECTURE.md").write_text(
                specification, encoding="utf-8"
            )
            rendered_symbols = ", ".join(f'"{symbol}"' for symbol in symbols)
            rendered_specification_symbols = ", ".join(
                f'"{symbol}"' for symbol in specification_symbols or []
            )
            rendered_specification_crates = ", ".join(
                f'"{crate}"' for crate in specification_crates or []
            )
            registry = textwrap.dedent(
                f'''\
                version = 1

                [[term]]
                name = "Query"
                source = "fava"
                meaning = "A declarative request for events."
                owner = "sample"
                nearest_nostr = "Filter"
                distinction = "A Query also carries local acquisition and presentation rules."
                symbols = [{rendered_symbols}]
                crates = ["sample"]
                spec_symbols = [{rendered_specification_symbols}]
                spec_crates = [{rendered_specification_crates}]
                '''
            )
            (root / "docs" / "internals" / "vocabulary.toml").write_text(
                registry, encoding="utf-8"
            )
            return subprocess.run(
                [sys.executable, str(SCRIPT), "--root", str(root)],
                check=False,
                capture_output=True,
                text=True,
            )


if __name__ == "__main__":
    unittest.main()
