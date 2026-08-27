"""Deterministic compiler-structure binding for vocabulary approvals."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "vocabulary_structure.py"
if str(SCRIPT.parent) not in sys.path:
    sys.path.insert(0, str(SCRIPT.parent))
SPEC = importlib.util.spec_from_file_location("vocabulary_structure", SCRIPT)
assert SPEC and SPEC.loader
structure = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(structure)


class PublicRecordsTest(unittest.TestCase):
    def test_keeps_paths_fields_variants_methods_and_signatures(self) -> None:
        output = """\
pub struct fava_probe::Query
pub fava_probe::Query::limit: usize
impl fava_probe::Query
pub fn fava_probe::Query::with_limit(self, usize) -> Self
pub enum fava_probe::QueryError
pub fava_probe::QueryError::TooLarge
pub fava_probe::QueryError::TooLarge::maximum: usize
"""
        self.assertEqual(
            structure.public_records(output, "fava_probe"),
            [
                {
                    "declaration": "pub struct fava_probe::Query",
                    "path": "fava_probe::Query",
                },
                {
                    "declaration": "pub fava_probe::Query::limit: usize",
                    "path": "fava_probe::Query::limit",
                },
                {
                    "declaration": "pub fn fava_probe::Query::with_limit(self, usize) -> Self",
                    "path": "fava_probe::Query::with_limit",
                    "implementation": "impl fava_probe::Query",
                },
                {
                    "declaration": "pub enum fava_probe::QueryError",
                    "path": "fava_probe::QueryError",
                },
                {
                    "declaration": "pub fava_probe::QueryError::TooLarge",
                    "path": "fava_probe::QueryError::TooLarge",
                },
                {
                    "declaration": "pub fava_probe::QueryError::TooLarge::maximum: usize",
                    "path": "fava_probe::QueryError::TooLarge::maximum",
                },
            ],
        )

    def test_rustdoc_projects_behavior_onto_the_external_impl_identity(self) -> None:
        rustdoc = {
            "index": {
                "10": {
                    "name": "from",
                    "docs": "Maps every closed state selector to its exact wire kind.",
                    "inner": {"function": {}},
                },
                "11": {
                    "name": None,
                    "inner": {
                        "impl": {
                            "blanket_impl": None,
                            "for": {
                                "resolved_path": {
                                    "id": 2,
                                    "path": "Kind",
                                    "args": None,
                                }
                            },
                            "items": [10],
                            "trait": {
                                "id": 3,
                                "path": "From",
                                "args": {
                                    "angle_bracketed": {
                                        "args": [
                                            {
                                                "type": {
                                                    "resolved_path": {
                                                        "id": 1,
                                                        "path": "StateKind",
                                                        "args": None,
                                                    }
                                                }
                                            }
                                        ]
                                    }
                                },
                            },
                        }
                    },
                },
            },
            "paths": {
                "1": {
                    "crate_id": 0,
                    "kind": "enum",
                    "path": ["fava_probe", "query", "StateKind"],
                },
                "2": {
                    "crate_id": 9,
                    "kind": "enum",
                    "path": ["nostr", "Kind"],
                },
                "3": {
                    "crate_id": 2,
                    "kind": "trait",
                    "path": ["core", "convert", "From"],
                },
            },
        }

        descriptions = structure.rustdoc_descriptions(
            rustdoc,
            [{"path": "fava_probe::StateKind", "target": "1"}],
        )

        self.assertEqual(
            descriptions[
                "<nostr::Kind as core::convert::From<fava_probe::StateKind>>::from"
            ],
            "Maps every closed state selector to its exact wire kind.",
        )

    def test_trait_method_path_carries_exact_implementation(self) -> None:
        output = """\
impl fava_contract::Source for fava_probe::Query
pub fn fava_probe::Query::open(&self, fava_contract::Request) -> fava_contract::Result
"""
        self.assertEqual(
            structure.public_records(output, "fava_probe"),
            [
                {
                    "declaration": (
                        "pub fn fava_probe::Query::open(&self, "
                        "fava_contract::Request) -> fava_contract::Result"
                    ),
                    "implementation": (
                        "impl fava_contract::Source for fava_probe::Query"
                    ),
                    "path": (
                        "<fava_probe::Query as fava_contract::Source>::open"
                    ),
                }
            ],
        )

    def test_external_trait_impl_is_bound_through_workspace_trait(self) -> None:
        output = """\
impl fava_probe::IntoQuery for &nostr::PublicKey
pub type &nostr::PublicKey::Output = fava_query::Query
pub fn &nostr::PublicKey::into_query(self) -> Self::Output
"""
        records = structure.public_records(output, "fava_probe")
        self.assertEqual(
            [record["path"] for record in records],
            [
                "<&nostr::PublicKey as fava_probe::IntoQuery>::Output",
                "<&nostr::PublicKey as fava_probe::IntoQuery>::into_query",
            ],
        )
        self.assertTrue(
            all(
                record["implementation"]
                == "impl fava_probe::IntoQuery for &nostr::PublicKey"
                for record in records
            )
        )

    def test_external_receiver_impl_uses_the_complete_trait_identity(self) -> None:
        output = """\
impl core::convert::From<fava_probe::StateKind> for nostr::Kind
pub fn nostr::Kind::from(fava_probe::StateKind) -> Self
pub fn fava_probe::free(fava_probe::StateKind)
pub struct fava_probe::Later
"""

        records = structure.public_records(output, "fava_probe")
        self.assertEqual(
            records,
            [
                {
                    "binding_roots": ["fava_probe::StateKind"],
                    "declaration": "pub fn nostr::Kind::from(fava_probe::StateKind) -> Self",
                    "implementation": (
                        "impl core::convert::From<fava_probe::StateKind> for nostr::Kind"
                    ),
                    "path": (
                        "<nostr::Kind as "
                        "core::convert::From<fava_probe::StateKind>>::from"
                    ),
                },
                {
                    "declaration": "pub fn fava_probe::free(fava_probe::StateKind)",
                    "path": "fava_probe::free",
                },
                {
                    "declaration": "pub struct fava_probe::Later",
                    "path": "fava_probe::Later",
                },
            ],
        )
        self.assertTrue(
            structure._record_matches_root(records[0], "fava_probe::StateKind")
        )

    def test_external_receiver_associated_items_keep_distinct_impl_identities(self) -> None:
        output = """\
impl fava_probe::Project<fava_probe::Input> for external::Receiver
pub type external::Receiver::Output = fava_probe::Result
pub const external::Receiver::LIMIT: usize
pub fn external::Receiver::project(fava_probe::Input) -> Self::Output
impl fava_probe::Project<fava_probe::OtherInput> for external::Receiver
pub fn external::Receiver::project(fava_probe::OtherInput) -> Self::Output
"""

        records = structure.public_records(output, "fava_probe")

        self.assertEqual(
            [record["path"] for record in records],
            [
                (
                    "<external::Receiver as "
                    "fava_probe::Project<fava_probe::Input>>::Output"
                ),
                (
                    "<external::Receiver as "
                    "fava_probe::Project<fava_probe::Input>>::LIMIT"
                ),
                (
                    "<external::Receiver as "
                    "fava_probe::Project<fava_probe::Input>>::project"
                ),
                (
                    "<external::Receiver as "
                    "fava_probe::Project<fava_probe::OtherInput>>::project"
                ),
            ],
        )
        self.assertEqual(
            [record["binding_roots"] for record in records],
            [
                ["fava_probe::Input", "fava_probe::Project"],
                ["fava_probe::Input", "fava_probe::Project"],
                ["fava_probe::Input", "fava_probe::Project"],
                ["fava_probe::OtherInput", "fava_probe::Project"],
            ],
        )


class PublicApiCoverageTest(unittest.TestCase):
    def test_counts_semantic_identities_and_deduplicates_identical_records(self) -> None:
        record = {
            "declaration": "pub fn probe::Thing::run(&self)",
            "path": "probe::Thing::run",
        }
        packets = {
            "Thing": {
                "structure": {
                    "public_api": [record],
                }
            }
        }

        coverage = structure.public_api_binding_coverage([record, record], packets)

        self.assertEqual(coverage["public_items"], 1)
        self.assertEqual(coverage["bound_items"], 1)
        self.assertEqual(coverage["collisions"], [])

    def test_distinct_trait_impl_identities_do_not_collide(self) -> None:
        records = [
            {
                "declaration": "pub fn external::Receiver::project(fava_probe::Input)",
                "path": (
                    "<external::Receiver as "
                    "fava_probe::Project<fava_probe::Input>>::project"
                ),
            },
            {
                "declaration": "pub fn external::Receiver::project(fava_probe::OtherInput)",
                "path": (
                    "<external::Receiver as "
                    "fava_probe::Project<fava_probe::OtherInput>>::project"
                ),
            },
        ]
        packets = {
            "Project": {"structure": {"public_api": records}},
        }

        coverage = structure.public_api_binding_coverage(records, packets)

        self.assertEqual(coverage["public_items"], 2)
        self.assertEqual(coverage["bound_items"], 2)
        self.assertEqual(coverage["collisions"], [])

    def test_conflicting_records_for_one_semantic_identity_are_a_collision(self) -> None:
        records = [
            {"declaration": "pub fn probe::Thing::run(&self)", "path": "probe::Thing::run"},
            {
                "declaration": "pub fn probe::Thing::run(&mut self)",
                "path": "probe::Thing::run",
            },
        ]
        packets = {"Thing": {"structure": {"public_api": [records[0]]}}}

        coverage = structure.public_api_binding_coverage(records, packets)

        self.assertEqual(coverage["public_items"], 1)
        self.assertEqual(coverage["bound_items"], 0)
        self.assertEqual(len(coverage["collisions"]), 1)
        self.assertEqual(coverage["collisions"][0]["identity"], "probe::Thing::run")


class ReexportTest(unittest.TestCase):
    def test_walks_exact_public_reexport_paths(self) -> None:
        rustdoc = {
            "root": 1,
            "index": {
                "1": {
                    "name": "fava_probe",
                    "visibility": "public",
                    "inner": {"module": {"items": [2, 3]}},
                },
                "2": {
                    "name": None,
                    "visibility": "public",
                    "inner": {
                        "use": {
                            "source": "model::Query",
                            "name": "Query",
                            "id": 9,
                            "is_glob": False,
                        }
                    },
                },
                "3": {
                    "name": "api",
                    "visibility": "public",
                    "inner": {"module": {"items": [4]}},
                },
                "4": {
                    "name": None,
                    "visibility": "public",
                    "inner": {
                        "use": {
                            "source": "crate::model::Query",
                            "name": "Query",
                            "id": 9,
                            "is_glob": False,
                        }
                    },
                },
            },
        }
        self.assertEqual(
            structure.reexports(rustdoc),
            [
                {
                    "path": "fava_probe::Query",
                    "source": "model::Query",
                    "target": "9",
                },
                {
                    "path": "fava_probe::api::Query",
                    "source": "crate::model::Query",
                    "target": "9",
                },
            ],
        )


class PrivateStateTest(unittest.TestCase):
    def test_compiler_span_binds_exact_private_fields_and_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/fava-probe/src/lib.rs"
            source.parent.mkdir(parents=True)
            declaration = "struct QueryState {\n    generation: u64,\n}\n"
            source.write_text("// unrelated\n" + declaration, encoding="utf-8")
            package = type(
                "Package",
                (),
                {"crate_name": "fava_probe"},
            )()
            rustdoc = {
                "index": {
                    "7": {
                        "name": "QueryState",
                        "visibility": {
                            "restricted": {"parent": 1, "path": "::state"}
                        },
                        "span": {
                            "filename": "crates/fava-probe/src/lib.rs",
                            "begin": [2, 1],
                            "end": [4, 2],
                        },
                        "inner": {"struct": {}},
                    }
                },
                "paths": {
                    "7": {
                        "path": ["fava_probe", "state", "QueryState"]
                    }
                },
            }
            records = structure.private_state_records(
                root,
                package,
                rustdoc,
                {"name": "QueryState", "owner": "fava-probe"},
            )
        self.assertEqual(records[0]["declaration"], declaration.rstrip("\n"))
        self.assertEqual(records[0]["path"], "fava_probe::state::QueryState")
        self.assertEqual(records[0]["source"], "crates/fava-probe/src/lib.rs")
        self.assertEqual(records[0]["visibility"], {"restricted": "::state"})

    def test_wrong_owner_does_not_classify_private_homonym(self) -> None:
        package = type("Package", (), {"crate_name": "other"})()
        self.assertEqual(
            structure.private_state_records(
                Path("/unused"),
                package,
                {"index": {}, "paths": {}},
                {"name": "QueryState", "owner": "fava-probe"},
            ),
            [],
        )


class CanonicalStructureTest(unittest.TestCase):
    def test_deterministic_key_order_and_exact_declaration(self) -> None:
        first = {
            "reexports": [],
            "public_api": [
                {"path": "fava::Query::open", "declaration": "pub fn open()"}
            ],
            "private_architectural_state": [],
        }
        second = {
            "private_architectural_state": [],
            "public_api": [
                {"declaration": "pub fn open()", "path": "fava::Query::open"}
            ],
            "reexports": [],
        }
        self.assertEqual(
            structure.canonical_structure(first),
            structure.canonical_structure(second),
        )
        changed = json.loads(json.dumps(second))
        changed["public_api"][0]["declaration"] = "pub fn open(&self)"
        self.assertNotEqual(
            structure.canonical_structure(first),
            structure.canonical_structure(changed),
        )

    def test_explicit_empty_structure_is_not_absent(self) -> None:
        self.assertEqual(
            structure.canonical_structure(structure.EMPTY_STRUCTURE),
            '{"private_architectural_state":[],"public_api":[],"reexports":[]}',
        )

    def test_per_term_structure_has_an_explicit_refusal_bound(self) -> None:
        self.assertLess(structure.MAXIMUM_TERM_STRUCTURE_BYTES, 256 * 1024)


class HumanReviewInventoryTest(unittest.TestCase):
    def test_renders_every_bound_identity_with_description_and_signature(self) -> None:
        term = {
            "name": "Widget",
            "meaning": "One useful widget.",
            "lifecycle": "Constructed once and then immutable.",
        }
        compiled = {
            "private_architectural_state": [
                {
                    "declaration": "pub struct Widget {\n    value: usize,\n}",
                    "kind": "struct",
                    "path": "probe::widget::Widget",
                    "source": "crates/probe/src/lib.rs",
                    "visibility": "public",
                }
            ],
            "public_api": [
                {
                    "declaration": "pub struct probe::Widget",
                    "path": "probe::Widget",
                },
                {
                    "declaration": "pub fn probe::Widget::from_value(usize) -> Self",
                    "implementation": "impl probe::Widget",
                    "path": "probe::Widget::from_value",
                },
            ],
            "reexports": [{"path": "probe::Widget", "source": "widget::Widget"}],
        }
        catalog = {
            "probe::Widget": {
                "kind": "Struct",
                "purpose": "Stores one validated value.",
                "signature": "pub struct probe::Widget",
            },
            "probe::Widget::from_value": {
                "kind": "Method",
                "purpose": "Constructs a widget from one value or refuses overflow.",
                "signature": "pub fn probe::Widget::from_value(usize) -> Self",
            },
        }

        review, problems = structure.human_review_inventory(term, compiled, catalog)

        self.assertEqual(problems, [])
        self.assertEqual(
            [item["kind"] for item in review],
            ["Bound declaration", "Struct", "Constructor", "Public export"],
        )
        self.assertEqual(
            [item["path"] for item in review],
            [
                "probe::widget::Widget",
                "probe::Widget",
                "probe::Widget::from_value",
                "probe::Widget",
            ],
        )
        self.assertTrue(all(item["description"] for item in review))
        self.assertTrue(all(item["signature"] for item in review))

    def test_missing_human_description_is_visible_and_blocks_review(self) -> None:
        compiled = {
            "private_architectural_state": [],
            "public_api": [
                {
                    "declaration": "pub fn probe::undocumented()",
                    "path": "probe::undocumented",
                }
            ],
            "reexports": [],
        }
        review, problems = structure.human_review_inventory(
            {"name": "Undocumented", "meaning": "A probe."}, compiled, {}
        )
        self.assertEqual(len(review), 1)
        self.assertIn("Review blocked", review[0]["description"])
        self.assertEqual(
            problems,
            ["probe::undocumented: missing human interface description"],
        )

    def test_trait_method_description_covers_its_qualified_implementation(self) -> None:
        compiled = {
            "private_architectural_state": [],
            "public_api": [
                {
                    "binding_roots": ["probe::Compose"],
                    "declaration": "pub fn probe::Builder::compose(self) -> probe::Builder",
                    "path": "<probe::Builder as probe::Compose>::compose",
                }
            ],
            "reexports": [],
        }
        catalog = {
            "probe::Compose::compose": {
                "kind": "Method",
                "purpose": "Adds one selected context and returns the same builder.",
                "signature": "pub fn probe::Compose::compose(self) -> probe::Builder",
            }
        }

        review, problems = structure.human_review_inventory(
            {"name": "Compose", "meaning": "Builder composition."}, compiled, catalog
        )

        self.assertEqual(problems, [])
        self.assertEqual(
            review[0]["description"],
            "Adds one selected context and returns the same builder.",
        )

    def test_tautological_description_is_visible_and_blocks_review(self) -> None:
        compiled = {
            "private_architectural_state": [],
            "public_api": [
                {
                    "declaration": "pub fn probe::Widget::value(&self) -> usize",
                    "path": "probe::Widget::value",
                }
            ],
            "reexports": [],
        }
        for description in (
            "Provides the compiler-visible `value` method shown below.",
            "Compiler-visible method owned by `probe::Widget`.",
        ):
            with self.subTest(description=description):
                catalog = {
                    "probe::Widget::value": {
                        "kind": "Method",
                        "purpose": description,
                        "signature": "pub fn probe::Widget::value(&self) -> usize",
                    }
                }
                review, problems = structure.human_review_inventory(
                    {"name": "Widget", "meaning": "One widget."}, compiled, catalog
                )

                self.assertEqual(len(review), 1)
                self.assertEqual(
                    problems,
                    ["probe::Widget::value: tautological human interface description"],
                )

    def test_readme_description_changes_the_input_fingerprint(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "docs/internals").mkdir(parents=True)
            (root / "crates/probe/src").mkdir(parents=True)
            for relative in (
                "Cargo.lock",
                "Cargo.toml",
                "rust-toolchain.toml",
                "docs/internals/vocabulary.toml",
                "docs/internals/vocabulary-candidates.jsonl",
                "tools/crate_readme_api.py",
                "tools/vocabulary_approval.py",
                "tools/vocabulary_structure.py",
                "crates/probe/Cargo.toml",
                "crates/probe/src/lib.rs",
            ):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(relative, encoding="utf-8")
            readme = root / "crates/probe/README.md"
            readme.write_text("first description", encoding="utf-8")
            first = structure.input_fingerprint(root)
            readme.write_text("changed description", encoding="utf-8")
            self.assertNotEqual(first, structure.input_fingerprint(root))

    def test_all_thirty_five_simple_group_terms_have_complete_human_reviews(self) -> None:
        root = Path(__file__).parents[2]
        terms = tomllib.loads(
            (root / "docs/internals/vocabulary.toml").read_text(encoding="utf-8")
        )["term"]
        simple_group_terms = [
            term for term in terms if term.get("owner") == "fava-simple-groups"
        ]
        self.assertEqual(len(simple_group_terms), 35)

        snapshot, snapshot_problems = structure.read_snapshot(
            root / "docs/internals/vocabulary-structure.json"
        )
        self.assertEqual(snapshot_problems, [])
        for term in simple_group_terms:
            with self.subTest(term=term["name"]):
                packet = snapshot[term["name"]]
                self.assertEqual(packet["review_problems"], [])
                review = packet["interface"]
                visible_signatures = [item["signature"] for item in review]
                bound_signatures = [
                    item["declaration"]
                    for item in packet["structure"]["private_architectural_state"]
                ] + [
                    item["declaration"]
                    for item in packet["structure"]["public_api"]
                ]
                for signature in bound_signatures:
                    self.assertEqual(visible_signatures.count(signature), 1)
                self.assertTrue(
                    all("Review blocked" not in item["description"] for item in review)
                )

    def test_every_simple_groups_public_identity_has_one_owning_term(self) -> None:
        root = Path(__file__).parents[2]
        registry = tomllib.loads(
            (root / "docs/internals/vocabulary.toml").read_text(encoding="utf-8")
        )
        required = set(registry.get("complete_public_api_crates", []))
        self.assertIn("fava-simple-groups", required)
        packets, problems = structure.read_snapshot(
            root / "docs/internals/vocabulary-structure.json"
        )
        self.assertEqual(problems, [])
        package = structure.public_api.workspace_packages(root)["fava-simple-groups"]
        output, _, _ = structure._compiled_package(root, package)
        records = structure.public_records(output, package.crate_name)
        coverage = structure.public_api_binding_coverage(records, packets)
        self.assertEqual(coverage["unbound"], [])
        self.assertEqual(coverage["multiply_bound"], [])
        self.assertEqual(coverage["collisions"], [])
        self.assertEqual(coverage["public_items"], 121)
        self.assertEqual(coverage["bound_items"], 121)

    def test_simple_groups_free_functions_and_module_have_own_terms(self) -> None:
        root = Path(__file__).parents[2]
        terms = tomllib.loads(
            (root / "docs/internals/vocabulary.toml").read_text(encoding="utf-8")
        )["term"]
        symbols = {
            symbol: term["name"]
            for term in terms
            for symbol in term.get("symbols", [])
            if symbol == "fava_simple_groups"
            or symbol.startswith("fava_simple_groups::")
        }
        expected = {
            "fava_simple_groups": "fava_simple_groups",
            "fava_simple_groups::create_group": "create_group",
            "fava_simple_groups::delete_event": "delete_event",
            "fava_simple_groups::delete_group": "delete_group",
            "fava_simple_groups::edit_metadata": "edit_metadata",
            "fava_simple_groups::invite": "invite",
            "fava_simple_groups::join_request": "join_request",
            "fava_simple_groups::leave_group": "leave_group",
            "fava_simple_groups::put_user": "put_user",
            "fava_simple_groups::remove_saved_relay": "remove_saved_relay",
            "fava_simple_groups::remove_saved_simple_group": "remove_saved_simple_group",
            "fava_simple_groups::remove_user": "remove_user",
            "fava_simple_groups::rename_saved_simple_group": "rename_saved_simple_group",
            "fava_simple_groups::save_relay": "save_relay",
            "fava_simple_groups::save_simple_group": "save_simple_group",
            "fava_simple_groups::saved_group_list_materializer": "saved_group_list_materializer",
            "fava_simple_groups::saved_group_lists": "saved_group_lists",
        }
        for symbol, owner in expected.items():
            with self.subTest(symbol=symbol):
                self.assertEqual(symbols.get(symbol), owner)

    def test_state_event_kind_descriptions_bind_the_exact_numeric_mapping(self) -> None:
        root = Path(__file__).parents[2]
        packets, problems = structure.read_snapshot(
            root / "docs/internals/vocabulary-structure.json"
        )
        self.assertEqual(problems, [])
        descriptions = "\n".join(
            item["description"]
            for item in packets["SimpleGroupStateEventKind"]["interface"]
        )
        expected = {
            "Metadata": "39000",
            "Admins": "39001",
            "Members": "39002",
            "Roles": "39003",
            "LivekitParticipants": "39004",
            "Pins": "39005",
        }
        for variant, number in expected.items():
            with self.subTest(variant=variant):
                self.assertRegex(descriptions, rf"{variant}[^\n]*{number}|{number}[^\n]*{variant}")
        conversion = next(
            item
            for item in packets["SimpleGroupStateEventKind"]["interface"]
            if "Kind::from" in item["signature"]
        )
        self.assertEqual(
            conversion["path"],
            (
                "<nostr::event::kind::Kind as "
                "core::convert::From<fava_simple_groups::SimpleGroupStateEventKind>>::from"
            ),
        )
        conversion_record = next(
            item
            for item in packets["SimpleGroupStateEventKind"]["structure"]["public_api"]
            if item["declaration"] == conversion["signature"]
        )
        self.assertEqual(
            conversion_record["implementation"],
            (
                "impl core::convert::From<"
                "fava_simple_groups::SimpleGroupStateEventKind> "
                "for nostr::event::kind::Kind"
            ),
        )
        self.assertEqual(
            conversion_record["binding_roots"],
            ["fava_simple_groups::SimpleGroupStateEventKind"],
        )
        for variant, number in expected.items():
            with self.subTest(conversion=variant):
                self.assertIn(variant, conversion["description"])
                self.assertIn(number, conversion["description"])

    def test_no_compiler_bound_identity_is_hidden_from_any_human_packet(self) -> None:
        root = Path(__file__).parents[2]
        packets, problems = structure.read_snapshot(
            root / "docs/internals/vocabulary-structure.json"
        )
        self.assertEqual(problems, [])
        for name, packet in packets.items():
            with self.subTest(term=name):
                visible = [item["signature"] for item in packet["interface"]]
                bound = [
                    item["declaration"]
                    for item in packet["structure"]["private_architectural_state"]
                ] + [
                    item["declaration"]
                    for item in packet["structure"]["public_api"]
                ]
                for declaration in bound:
                    self.assertEqual(visible.count(declaration), 1)


if __name__ == "__main__":
    unittest.main()
