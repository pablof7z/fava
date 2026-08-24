"""Focused behavior tests for crate README public-API inventories."""

from __future__ import annotations

import importlib.util
import io
import sys
import tempfile
import textwrap
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch


SCRIPT = Path(__file__).parents[1] / "crate_readme_api.py"
SPEC = importlib.util.spec_from_file_location("crate_readme_api", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
api = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = api
SPEC.loader.exec_module(api)


class PublicApiParsingTest(unittest.TestCase):
    def test_extractor_uses_exact_tools_all_features_hidden_items_and_lock(self) -> None:
        package = api.Package(
            name="sample",
            crate_name="sample",
            directory=Path("/workspace/crates/sample"),
            manifest=Path("/workspace/crates/sample/Cargo.toml"),
            readme=Path("/workspace/crates/sample/README.md"),
        )

        extraction = api.extractor_command(package, Path("/tmp/api-target"))
        rendering = api.renderer_command(Path("/tmp/api-target/doc/sample.json"))

        self.assertEqual(extraction[1], f"+{api.RUSTDOC_TOOLCHAIN}")
        self.assertIn("--all-features", extraction)
        self.assertIn("--locked", extraction)
        self.assertIn("--document-hidden-items", extraction)
        self.assertEqual(rendering[1], f"+{api.RUSTDOC_TOOLCHAIN}")
        self.assertIn("--rustdoc-json", rendering)

    def test_includes_every_requested_export_kind(self) -> None:
        output = textwrap.dedent(
            """\
            pub mod sample
            pub mod sample::visible
            pub macro sample::make!
            pub proc macro sample::#[derive(Generated)]
            pub proc macro sample::#[attribute]
            pub proc macro sample::function_like!()
            pub enum sample::Choice
            pub sample::Choice::Named
            pub sample::Choice::Named::value: u8
            pub sample::Choice::Tuple(alloc::string::String, u8)
            pub sample::Choice::Unit
            pub struct sample::Named
            pub sample::Named::field: u8
            pub struct sample::Tuple(pub u8, _, pub alloc::vec::Vec<(u8, u8)>)
            pub union sample::Either
            pub sample::Either::number: u64
            pub trait sample::Contract
            pub type sample::Contract::Output
            pub const sample::Contract::LIMIT: usize
            pub fn sample::Contract::apply(&self)
            pub type sample::Alias = u8
            pub const sample::LIMIT: usize
            pub static sample::NAME: &str
            pub mut static sample::COUNT: usize
            pub fn sample::visible::free()
            pub struct sample::Reexported
            pub fn sample::reexported_function()
            """
        )

        items = set(api.parse_public_api(output, "sample"))

        expected = {
            api.ApiItem("sample", "Module"),
            api.ApiItem("sample::visible", "Module"),
            api.ApiItem("sample::make!", "Macro"),
            api.ApiItem("sample::#[derive(Generated)]", "Macro"),
            api.ApiItem("sample::#[attribute]", "Macro"),
            api.ApiItem("sample::function_like!", "Macro"),
            api.ApiItem("sample::Choice", "Enum"),
            api.ApiItem("sample::Choice::Named", "Enum variant"),
            api.ApiItem("sample::Choice::Named::value", "Public field"),
            api.ApiItem("sample::Choice::Tuple", "Enum variant"),
            api.ApiItem("sample::Choice::Tuple::0", "Public field"),
            api.ApiItem("sample::Choice::Tuple::1", "Public field"),
            api.ApiItem("sample::Choice::Unit", "Enum variant"),
            api.ApiItem("sample::Named", "Struct"),
            api.ApiItem("sample::Named::field", "Public field"),
            api.ApiItem("sample::Tuple", "Struct"),
            api.ApiItem("sample::Tuple::0", "Public field"),
            api.ApiItem("sample::Tuple::2", "Public field"),
            api.ApiItem("sample::Either", "Union"),
            api.ApiItem("sample::Either::number", "Public field"),
            api.ApiItem("sample::Contract", "Trait"),
            api.ApiItem("sample::Contract::Output", "Type alias"),
            api.ApiItem("sample::Contract::LIMIT", "Constant"),
            api.ApiItem("sample::Contract::apply", "Method"),
            api.ApiItem("sample::Alias", "Type alias"),
            api.ApiItem("sample::LIMIT", "Constant"),
            api.ApiItem("sample::NAME", "Static"),
            api.ApiItem("sample::COUNT", "Static"),
            api.ApiItem("sample::visible::free", "Function"),
            api.ApiItem("sample::Reexported", "Struct"),
            api.ApiItem("sample::reexported_function", "Function"),
        }
        self.assertEqual(items, expected)

    def test_ignores_impl_headers_and_non_public_output(self) -> None:
        output = textwrap.dedent(
            """\
            pub mod sample
            pub struct sample::Value
            impl sample::Value
            pub fn sample::Value::method(&self)
            warning: irrelevant build output
            """
        )

        self.assertEqual(
            api.parse_public_api(output, "sample"),
            [
                api.ApiItem("sample", "Module"),
                api.ApiItem("sample::Value", "Struct"),
                api.ApiItem("sample::Value::method", "Method"),
            ],
        )

    def test_distinguishes_same_named_methods_from_multiple_trait_impls(self) -> None:
        output = textwrap.dedent(
            """\
            pub mod sample
            pub struct sample::Value
            impl core::convert::From<u8> for sample::Value
            pub fn sample::Value::from(u8) -> Self
            impl core::convert::From<u16> for sample::Value
            pub fn sample::Value::from(u16) -> Self
            impl core::convert::From<u32> for sample::Value<u32>
            pub fn sample::Value::from(u32) -> Self
            impl<T> sample::Convert<T> for sample::Value<T> where T: Copy
            pub fn sample::Value::convert(T) -> Self
            impl sample::Value
            pub fn sample::Value::from_text(&str) -> Self
            """
        )

        self.assertEqual(
            api.parse_public_api(output, "sample"),
            [
                api.ApiItem("sample", "Module"),
                api.ApiItem("sample::Value", "Struct"),
                api.ApiItem(
                    "<sample::Value as core::convert::From<u16>>::from", "Method"
                ),
                api.ApiItem(
                    "<sample::Value as core::convert::From<u8>>::from", "Method"
                ),
                api.ApiItem("sample::Value::from_text", "Method"),
                api.ApiItem(
                    "<sample::Value<T> as sample::Convert<T>>::convert", "Method"
                ),
                api.ApiItem(
                    "<sample::Value<u32> as core::convert::From<u32>>::from",
                    "Method",
                ),
            ],
        )


class ReadmeManagementTest(unittest.TestCase):
    def test_adds_a_managed_table_without_changing_existing_prose(self) -> None:
        original = "# sample\n\nArbitrary prose.\n\n\n"
        items = [api.ApiItem("sample::Value", "Struct")]

        updated = api.expected_readme(original, items)

        self.assertTrue(updated.startswith(original))
        self.assertIn("| Struct | `sample::Value` |  |", updated)
        self.assertEqual(updated.count(api.BEGIN_MARKER), 1)
        self.assertEqual(updated.count(api.END_MARKER), 1)

    def test_preserves_descriptions_and_removes_stale_rows(self) -> None:
        original = textwrap.dedent(
            f"""\
            Before.
            {api.BEGIN_MARKER}
            | Kind | Item | Description |
            | --- | --- | --- |
            | Struct | `sample::Kept` | Hand-written meaning with \\| separator. |
            | Struct | `sample::Removed` | Must disappear. |
            {api.END_MARKER}
            After.
            """
        )
        items = [
            api.ApiItem("sample::Added", "Function"),
            api.ApiItem("sample::Kept", "Struct"),
        ]

        updated = api.expected_readme(original, items)

        self.assertIn("Before.\n", updated)
        self.assertTrue(updated.endswith("After.\n"))
        self.assertIn("Hand-written meaning with \\| separator.", updated)
        self.assertNotIn("sample::Removed", updated)
        self.assertIn("| Function | `sample::Added` |  |", updated)

    def test_rejects_duplicate_or_malformed_managed_rows(self) -> None:
        duplicate = textwrap.dedent(
            f"""\
            {api.BEGIN_MARKER}
            | Kind | Item | Description |
            | --- | --- | --- |
            | Struct | `sample::Value` | one |
            | Struct | `sample::Value` | two |
            {api.END_MARKER}
            """
        )

        with self.assertRaisesRegex(api.InventoryError, "duplicate"):
            api.expected_readme(duplicate, [])

    def test_check_rejects_missing_and_stale_inventories(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            readme = root / "README.md"
            package = api.Package(
                name="sample",
                crate_name="sample",
                directory=root,
                manifest=root / "Cargo.toml",
                readme=readme,
            )
            readme.write_text("# sample\n", encoding="utf-8")
            self.assertIn("missing", api.check_package(root, package) or "")

            current = [api.ApiItem("sample::Current", "Struct")]
            readme.write_text(
                api.expected_readme("# sample\n", current), encoding="utf-8"
            )
            with patch.object(api, "inventory_for", return_value=current):
                self.assertIsNone(api.check_package(root, package))

            changed = current + [api.ApiItem("sample::Added", "Function")]
            with patch.object(api, "inventory_for", return_value=changed):
                self.assertIn("stale", api.check_package(root, package) or "")

    def test_update_can_create_a_missing_crate_readme(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package = api.Package(
                name="sample",
                crate_name="sample",
                directory=root,
                manifest=root / "Cargo.toml",
                readme=root / "README.md",
            )
            items = [api.ApiItem("sample", "Module")]

            with patch.object(api, "inventory_for", return_value=items):
                with redirect_stdout(io.StringIO()):
                    api.update_package(root, package)

            created = package.readme.read_text(encoding="utf-8")
            self.assertTrue(created.startswith("# sample\n"))
            self.assertIn("| Module | `sample` |  |", created)


class ModifiedCratesTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.packages = {
            name: api.Package(
                name=name,
                crate_name=name.replace("-", "_"),
                directory=self.root / "crates" / name,
                manifest=self.root / "crates" / name / "Cargo.toml",
                readme=self.root / "crates" / name / "README.md",
            )
            for name in ("alpha", "beta")
        }

    def tearDown(self) -> None:
        self.temp.cleanup()

    def test_maps_any_file_under_a_crate_to_that_crate(self) -> None:
        changed = api.modified_packages(
            self.root,
            self.packages,
            [Path("crates/alpha/tests/public.rs"), Path("docs/guide.md")],
        )

        self.assertEqual([package.name for package in changed], ["alpha"])

    def test_root_manifest_change_checks_every_library_crate(self) -> None:
        changed = api.modified_packages(
            self.root, self.packages, [Path("Cargo.toml")]
        )

        self.assertEqual([package.name for package in changed], ["alpha", "beta"])


if __name__ == "__main__":
    unittest.main()
