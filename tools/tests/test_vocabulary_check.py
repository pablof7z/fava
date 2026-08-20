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

    def run_check(
        self,
        *,
        source: str,
        symbols: list[str],
        specification: str = "",
        specification_symbols: list[str] | None = None,
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
