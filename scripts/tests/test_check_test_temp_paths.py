"""Each test's temporary path is its own (specification 22): the guard finds a `temp_dir()` call in test code and
nowhere else. Each expected finding follows from the source text the test writes."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import check_test_temp_paths as check  # noqa: E402

CALL = 'fn helper() -> PathBuf {\n    std::env::temp_dir().join("a")\n}\n'


class TestFiles(unittest.TestCase):
    """Which files are test code as a whole: the module tree decides, not the file's name."""

    def test_the_module_tree_decides_which_files_are_tests(self) -> None:
        """A gated module, a gated `#[path]` module, a child of a test file and an integration test are test files;
        a `hmi_test.rs`, and a `tests.rs` that a product module declares without a gate, are product modules. Catches
        a guard that takes a file's name or folder for proof and so flags product code, and one that misses a gated or
        a nested test file."""
        sources = {
            "crates/x/src/lib.rs": '#[cfg(test)]\nmod checks;\nmod hmi_test;\nmod product;\n'
                                   '#[cfg(all(test, unix))]\n#[path = "helpers/support.rs"]\nmod support;\n',
            "crates/x/src/checks.rs": "mod nested;\n",
            "crates/x/src/checks/nested.rs": "",
            "crates/x/src/hmi_test.rs": "",
            "crates/x/src/product.rs": "mod tests;\n",
            "crates/x/src/product/tests/mod.rs": "",
            "crates/x/src/helpers/support.rs": "",
            "crates/x/tests/cli.rs": "",
        }
        self.assertEqual(
            check.test_module_files(sources),
            {"crates/x/src/checks.rs", "crates/x/src/checks/nested.rs", "crates/x/src/helpers/support.rs",
             "crates/x/tests/cli.rs"},
        )

    def test_a_file_that_test_code_includes_is_test_code(self) -> None:
        """A file that a gated test module includes, and a file that such an included file or a test file includes in
        turn, are test files; a file that product code includes is not. Catches a guard that never reads the test files a
        test module pastes in with `include!` (trust-lsp's command tests, trust-runtime's realtime tests), and one that
        takes product code that `include!` splits up for tests."""
        sources = {
            "crates/x/src/lib.rs": '#[cfg(test)]\nmod tests {\n    include!("tests/part.rs");\n}\ninclude!("generated.rs");\n',
            "crates/x/src/tests/part.rs": 'include!("more.rs");\n',
            "crates/x/src/tests/more.rs": "",
            "crates/x/src/generated.rs": "",
            "crates/x/tests/cli.rs": 'include!("common/shared.rs");\n',
            "crates/x/tests/common/shared.rs": "",
        }
        self.assertEqual(
            check.test_module_files(sources),
            {"crates/x/src/tests/part.rs", "crates/x/src/tests/more.rs", "crates/x/tests/cli.rs",
             "crates/x/tests/common/shared.rs"},
        )

    def test_a_test_file_is_test_code_throughout_and_a_product_file_is_not(self) -> None:
        """Catches a guard that misses the calls of a whole test file, and one that flags a product file's call."""
        self.assertEqual(check.violations("crates/x/src/tests.rs", CALL, {"crates/x/src/tests.rs"}),
                         [(2, 'std::env::temp_dir().join("a")')])
        self.assertEqual(check.violations("crates/x/src/hmi_test.rs", CALL, set()), [])


class GatedItems(unittest.TestCase):
    """Which items of a product file are test code."""

    def test_only_the_gated_items_of_a_product_file_are_test_code(self) -> None:
        """A product path stays allowed, and a gated module's call is found though a raw string and a character
        hold braces. Catches a guard that flags product code, and one that a brace in a literal ends early."""
        text = (
            'pub fn socket_root() -> PathBuf {\n'
            '    std::env::temp_dir().join("trust-runtime")\n'
            '}\n'
            '\n'
            '#[cfg(test)]\n'
            'mod tests {\n'
            '    const ANSWER: &str = r#"x"}}}"#;\n'
            "    const BRACE: char = '}';\n"
            '    fn root() -> PathBuf {\n'
            '        env::temp_dir().join("x")\n'
            '    }\n'
            '}\n'
        )
        self.assertEqual(check.violations("crates/x/src/lib.rs", text, set()), [(10, 'env::temp_dir().join("x")')])

    def test_a_gated_function_is_test_code_and_a_gated_import_ends_at_its_semicolon(self) -> None:
        """Catches a gate read as covering the rest of the file."""
        text = (
            '#[cfg(test)]\n'
            'use std::env;\n'
            'pub fn product() -> PathBuf { std::env::temp_dir() }\n'
            '#[cfg(all(test, unix))]\n'
            'fn helper() -> PathBuf {\n'
            '    std::env::temp_dir()\n'
            '}\n'
        )
        self.assertEqual(check.violations("crates/x/src/lib.rs", text, set()), [(6, "std::env::temp_dir()")])


class Exceptions(unittest.TestCase):
    """What the guard leaves alone."""

    def test_the_marker_a_comment_and_the_helper_crate_are_allowed(self) -> None:
        """Catches a guard that refuses a marked call, one that reads comments, and one that refuses the helper."""
        text = (
            '// std::env::temp_dir() in a comment\n'
            'let name = "std::env::temp_dir()";\n'
            '// temp-root-ok: a product path\n'
            'assert!(path.starts_with(std::env::temp_dir()));\n'
            'let also = std::env::temp_dir(); // temp-root-ok: the socket budget\n'
        )
        self.assertEqual(check.violations("crates/x/tests/a.rs", text, {"crates/x/tests/a.rs"}), [])
        helper = "crates/trust-test-support/src/lib.rs"
        self.assertEqual(check.violations(helper, "std::env::temp_dir()\n", {helper}), [])

    def test_a_local_helper_named_temp_dir_is_not_the_system_call(self) -> None:
        """Catches a guard that flags a test's own function of that name."""
        text = 'fn temp_dir() -> PathBuf {\n    unique()\n}\nlet root = temp_dir();\n'
        self.assertEqual(check.violations("crates/x/tests/a.rs", text, {"crates/x/tests/a.rs"}), [])


class Command(unittest.TestCase):
    def test_the_command_fails_on_a_finding_and_passes_without(self) -> None:
        """Catches a guard whose exit status does not follow its findings."""
        with tempfile.TemporaryDirectory() as root:
            tests = Path(root, "crates/x/tests")
            tests.mkdir(parents=True)
            (tests / "a.rs").write_text('let p = std::env::temp_dir().join("a");\n', encoding="utf-8")
            self.assertEqual(check.main(["check", root]), 1)
            (tests / "a.rs").write_text('let p = trust_test_support::unique_temp_path("a");\n', encoding="utf-8")
            self.assertEqual(check.main(["check", root]), 0)


if __name__ == "__main__":
    unittest.main()
