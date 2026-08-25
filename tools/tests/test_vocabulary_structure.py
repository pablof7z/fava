"""Deterministic compiler-structure binding for vocabulary approvals."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
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


if __name__ == "__main__":
    unittest.main()
