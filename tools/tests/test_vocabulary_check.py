"""Behavior tests for the architectural vocabulary gate."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import textwrap
import unittest
import json
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "check_vocabulary.py"
sys.path.insert(0, str(SCRIPT.parent))
import check_vocabulary as checker  # noqa: E402


class VocabularyCheckTest(unittest.TestCase):
    def test_complete_public_symbols_use_compiler_snapshot_root_identities(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "docs" / "internals" / "vocabulary-structure.json"
            path.parent.mkdir(parents=True)
            paths = [
                "sample",
                "sample::Thing",
                "sample::free_operation",
                "sample::Thing::method",
            ]
            path.write_text(
                json.dumps(
                    {
                        "terms": [
                            {
                                "structure": {
                                    "public_api": [
                                        {"path": value, "declaration": value}
                                        for value in paths
                                    ]
                                }
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )

            symbols, problems = checker.complete_public_symbols(
                root, frozenset({"sample"})
            )

        self.assertEqual(problems, [])
        self.assertEqual(
            symbols, {"sample", "sample::Thing", "sample::free_operation"}
        )

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

    def test_rejects_unapproved_nominal_variants_at_every_visibility(self) -> None:
        declarations = (
            "struct OpenedRelay;",
            "pub(crate) struct OpenedRelay;",
            "pub(super) struct OpenedRelay;",
            "pub(in crate) struct OpenedRelay;",
        )

        for declaration in declarations:
            with self.subTest(declaration=declaration):
                result = self.run_check(
                    source=f"pub struct RelayUrl;\n{declaration}\n",
                    symbols=["sample::RelayUrl"],
                    term_name="RelayUrl",
                )

                self.assertNotEqual(result.returncode, 0)
                self.assertIn("sample::OpenedRelay", result.stderr)
                self.assertIn("existing noun: Relay", result.stderr)

    def test_restricted_visibility_is_not_a_public_symbol(self) -> None:
        """`pub(crate)` is internal vocabulary, not the crate's public surface."""
        result = self.run_check(
            source="pub struct Query;\npub(crate) struct QueryLane;\n",
            symbols=["sample::Query"],
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unapproved nominal vocabulary variant: sample::QueryLane", result.stderr)
        self.assertNotIn("undocumented public architectural symbol: sample::QueryLane", result.stderr)

    def test_accepts_a_nominal_variant_approved_as_its_own_term(self) -> None:
        result = self.run_check(
            source="pub struct RelayUrl;\npub(super) struct OpenedRelay;\n",
            symbols=["sample::RelayUrl"],
            term_name="RelayUrl",
            approved_terms=["OpenedRelay"],
        )

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_an_unapproved_wrapper_around_registered_vocabulary(self) -> None:
        result = self.run_check(
            source="pub struct RelayUrl;\nstruct RelayWrapper;\n",
            symbols=["sample::RelayUrl"],
            term_name="RelayUrl",
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("sample::RelayWrapper", result.stderr)
        self.assertIn("existing noun: Relay", result.stderr)

    def test_rejects_a_single_word_internal_declaration(self) -> None:
        """A one-word name is still vocabulary; `len(words(name)) < 2` hid six."""
        result = self.run_check(
            source="pub struct Query;\nstruct Change;\n",
            symbols=["sample::Query"],
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "unapproved nominal vocabulary variant: sample::Change", result.stderr
        )

    def test_rejects_a_single_word_homonym_of_a_term_owned_elsewhere(self) -> None:
        """`Group` is approved for its owner only; the same spelling elsewhere is a
        second unrelated concept."""
        result = self.run_check(
            source="pub struct Query;\nstruct Group;\n",
            symbols=["sample::Query"],
            approved_terms=["Group"],
            approved_term_owner="other-crate",
            extra_packages={"crates/other-crate": ("other-crate", "")},
            registry_crates=["sample", "other-crate"],
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "unapproved nominal vocabulary variant: sample::Group", result.stderr
        )
        self.assertIn("existing noun: Group", result.stderr)

    def test_rejects_a_declaration_embedding_no_registered_noun(self) -> None:
        """The "must embed a registered noun" filter hid five more violations,
        including two of the nine unapproved lifecycle owners."""
        result = self.run_check(
            source="pub struct Query;\nstruct KnownLists;\n",
            symbols=["sample::Query"],
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "unapproved nominal vocabulary variant: sample::KnownLists", result.stderr
        )

    def test_rejects_every_nominal_keyword(self) -> None:
        declarations = (
            "struct HiddenOwner;",
            "enum HiddenOwner {}",
            "trait HiddenOwner {}",
            "type HiddenOwner = u8;",
            "union HiddenOwner { first: u8 }",
        )

        for declaration in declarations:
            with self.subTest(declaration=declaration):
                result = self.run_check(
                    source=f"pub struct Query;\n{declaration}\n",
                    symbols=["sample::Query"],
                )

                self.assertNotEqual(result.returncode, 0)
                self.assertIn("sample::HiddenOwner", result.stderr)

    def test_associated_items_inside_impl_blocks_are_not_declarations(self) -> None:
        """`fava_nip02::IntoIter` was nine false positives: an associated type
        inside an `impl` block declares no new noun."""
        source = textwrap.dedent(
            """\
            pub struct Query;

            impl IntoIterator for Query {
                type Item = u8;
                type IntoIter = std::vec::IntoIter<u8>;

                fn into_iter(self) -> Self::IntoIter {
                    Vec::new().into_iter()
                }
            }
            """
        )
        result = self.run_check(source=source, symbols=["sample::Query"])

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_associated_items_inside_trait_definitions_are_not_declarations(
        self,
    ) -> None:
        source = textwrap.dedent(
            """\
            pub struct Query;

            trait Projection {
                type Outcome;
            }
            """
        )
        result = self.run_check(source=source, symbols=["sample::Query"])

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("sample::Projection", result.stderr)
        self.assertNotIn("sample::Outcome", result.stderr)

    def test_comments_and_string_literals_are_not_declarations(self) -> None:
        source = textwrap.dedent(
            '''\
            pub struct Query;

            // pub struct CommentGhost;
            /* pub struct BlockGhost; */

            fn describe() -> &'static str {
                let _lifetime: &'static str = "pub struct StringGhost;";
                r#"pub struct RawGhost;"#
            }
            '''
        )
        result = self.run_check(source=source, symbols=["sample::Query"])

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_derive_attributes_do_not_hide_a_declaration(self) -> None:
        source = textwrap.dedent(
            """\
            pub struct Query;

            #[derive(Debug, Clone)]
            #[allow(dead_code)]
            struct HiddenOwner;
            """
        )
        result = self.run_check(source=source, symbols=["sample::Query"])

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("sample::HiddenOwner", result.stderr)

    def test_scans_every_workspace_package_not_only_the_crates_directory(self) -> None:
        """Three packages and twenty-one public declarations sat outside the old
        walk root entirely."""
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            extra_packages={"apps/tool": ("tool", "pub struct ToolOutcome;\n")},
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("undocumented architectural crate: tool", result.stderr)
        self.assertIn(
            "undocumented public architectural symbol: tool::ToolOutcome", result.stderr
        )

    def test_ignores_runnable_consumer_example_packages(self) -> None:
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            extra_packages={
                "examples/consumer": ("consumer", "pub struct ConsumerOutcome;\n")
            },
        )

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_scans_public_declarations_in_crate_tests(self) -> None:
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            tests_source="pub struct CountingSigner;\n",
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "undocumented public architectural symbol: sample::CountingSigner",
            result.stderr,
        )

    def test_private_helpers_outside_crates_are_not_fava_vocabulary(self) -> None:
        """A downstream package owns its private names; only what it publishes is
        architectural vocabulary."""
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            extra_packages={"apps/tool": ("tool", "struct LocalScratch;\n")},
            registry_crates=["sample", "tool"],
        )

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_planning_records_are_not_vocabulary_authority(self) -> None:
        """`.planning/**` is a record of plans, reviews, and audits. Harvesting it
        let prose invent crates and symbols and flip this gate in either
        direction."""
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            planning=(
                "The review mentions fava-canary and fava-owned-deadline.\n"
                "pub struct SubscriptionPlanDiff;\n"
            ),
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("fava-canary", result.stderr)
        self.assertNotIn("SubscriptionPlanDiff", result.stderr)

    def test_planning_records_cannot_satisfy_a_registered_specification_entry(
        self,
    ) -> None:
        """The reverse direction: a plan mentioning a term must not stand in for a
        specification that never named it."""
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification_symbols=["Session"],
            planning="pub struct Session;\n",
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("registered specified symbol does not exist: Session", result.stderr)

    def test_reports_a_specified_crate_with_no_implementation(self) -> None:
        """`fava-runtime` was registered, unimplemented, and silent for six
        milestones."""
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification="Routing lives in fava-runtime.\n",
            specification_crates=["fava-runtime"],
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "specified architectural crate is not implemented: fava-runtime",
            result.stderr,
        )

    def test_accepts_a_specified_crate_that_exists(self) -> None:
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification="Routing lives in fava-runtime.\n",
            specification_crates=["fava-runtime"],
            extra_packages={"crates/fava-runtime": ("fava-runtime", "")},
            registry_crates=["sample", "fava-runtime"],
        )

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_reports_a_specified_symbol_with_no_implementation(self) -> None:
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification="pub struct Session;\n",
            specification_symbols=["Session"],
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "specified architectural symbol is not implemented: Session", result.stderr
        )

    def test_rejects_a_registered_symbol_that_does_not_exist(self) -> None:
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query", "sample::Vanished"],
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "registered public symbol does not exist: sample::Vanished", result.stderr
        )

    def test_rejects_a_registered_crate_that_does_not_exist(self) -> None:
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            registry_crates=["sample", "ghost-crate"],
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "registered architectural crate does not exist: ghost-crate", result.stderr
        )

    def test_rejects_a_fava_term_without_a_nearest_nostr_concept(self) -> None:
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            omit_nearest_nostr=True,
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Query: Fava terms require nearest_nostr", result.stderr)

    def test_rejects_an_undocumented_specification_symbol(self) -> None:
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification="pub struct MysteryManager;\n",
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("MysteryManager", result.stderr)

    def test_accepts_a_registered_specification_symbol(self) -> None:
        # The spec_symbol must equal the term name (invariant); "Query" is valid
        # under the "Query" term.  This also verifies the acceptance path: when
        # the spec document declares a symbol that the registry has registered,
        # no "undocumented specified architectural symbol" diagnostic fires.
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query"],
            specification="pub struct Query;\n",
            specification_symbols=["Query"],
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
            extra_packages={"crates/fava-simple-groups": ("fava-simple-groups", "")},
            registry_crates=["sample", "fava-simple-groups"],
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
            extra_packages={"crates/fava-simple-groups": ("fava-simple-groups", "")},
            registry_crates=["sample", "fava-simple-groups"],
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

    def test_rejects_symbols_with_terminal_name_differing_from_term_name(self) -> None:
        """ShortfallReason under SubscriptionPlanner is the canonical counter-example."""
        result = self.run_check(
            source="pub struct SubscriptionPlanner;\npub struct ShortfallReason;\n",
            symbols=["sample::SubscriptionPlanner", "sample::ShortfallReason"],
            term_name="SubscriptionPlanner",
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("ShortfallReason", result.stderr)
        self.assertIn("SubscriptionPlanner", result.stderr)

    def test_rejects_spec_symbols_with_value_not_equal_to_term_name(self) -> None:
        """RouterSession in spec_symbols under Router hides a differently named concept."""
        result = self.run_check(
            source="pub struct Router;\n",
            symbols=["sample::Router"],
            term_name="Router",
            specification="pub struct Router;\npub struct RouterSession;\n",
            specification_symbols=["Router", "RouterSession"],
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("RouterSession", result.stderr)
        self.assertIn("Router", result.stderr)

    def test_accepts_multiple_module_paths_with_same_terminal_name(self) -> None:
        """fava_a::Query and fava_b::Query are both valid under a Query term."""
        result = self.run_check(
            source="pub struct Query;\n",
            symbols=["sample::Query", "sample_extra::Query"],
            extra_packages={"crates/sample-extra": ("sample-extra", "pub struct Query;\n")},
            registry_crates=["sample", "sample-extra"],
        )

        self.assertEqual(result.returncode, 0, result.stderr)

    def run_check(
        self,
        *,
        source: str,
        symbols: list[str],
        term_name: str = "Query",
        approved_terms: list[str] | None = None,
        approved_term_owner: str = "sample",
        registry_crates: list[str] | None = None,
        specification: str = "",
        specification_symbols: list[str] | None = None,
        specification_crates: list[str] | None = None,
        planning: str = "",
        tests_source: str = "",
        extra_packages: dict[str, tuple[str, str]] | None = None,
        omit_nearest_nostr: bool = False,
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
            if tests_source:
                (crate / "tests").mkdir(parents=True)
                (crate / "tests" / "support.rs").write_text(
                    tests_source, encoding="utf-8"
                )
            for location, (package, package_source) in (extra_packages or {}).items():
                package_root = root / location
                (package_root / "src").mkdir(parents=True)
                (package_root / "Cargo.toml").write_text(
                    f'[package]\nname = "{package}"\n', encoding="utf-8"
                )
                (package_root / "src" / "lib.rs").write_text(
                    package_source, encoding="utf-8"
                )
            (root / "docs" / "spec" / "ARCHITECTURE.md").write_text(
                specification, encoding="utf-8"
            )
            if planning:
                (root / ".planning").mkdir(parents=True)
                (root / ".planning" / "NOTES.md").write_text(planning, encoding="utf-8")
            rendered_symbols = ", ".join(f'"{symbol}"' for symbol in symbols)
            rendered_crates = ", ".join(
                f'"{name}"' for name in registry_crates or ["sample"]
            )
            rendered_specification_symbols = ", ".join(
                f'"{symbol}"' for symbol in specification_symbols or []
            )
            rendered_specification_crates = ", ".join(
                f'"{crate_name}"' for crate_name in specification_crates or []
            )
            rendered_approved_terms = "".join(
                textwrap.dedent(
                    f'''\

                    [[term]]
                    name = "{term}"
                    source = "fava"
                    meaning = "An explicitly approved architectural term."
                    owner = "{approved_term_owner}"
                    nearest_nostr = "{term_name}"
                    distinction = "The fixture explicitly approves this distinct concept."
                    symbols = []
                    crates = []
                    '''
                )
                for term in approved_terms or []
            )
            nearest_nostr = "" if omit_nearest_nostr else 'nearest_nostr = "Filter"\n'
            registry = textwrap.dedent(
                f'''\
                version = 1

                [[term]]
                name = "{term_name}"
                source = "fava"
                meaning = "A declarative request for events."
                owner = "sample"
                {nearest_nostr}\
                distinction = "A Query also carries local acquisition and presentation rules."
                symbols = [{rendered_symbols}]
                crates = [{rendered_crates}]
                spec_symbols = [{rendered_specification_symbols}]
                spec_crates = [{rendered_specification_crates}]
                '''
            ) + rendered_approved_terms
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
