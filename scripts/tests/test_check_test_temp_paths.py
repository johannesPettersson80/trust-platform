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


class TestCode(unittest.TestCase):
    """What counts as test code."""

    def test_a_test_file_is_test_code_throughout(self) -> None:
        """Catches a guard that misses the calls of a whole test file."""
        text = 'fn helper() -> PathBuf {\n    std::env::temp_dir().join("a")\n}\n'
        for relative in ["crates/x/tests/cli.rs", "crates/x/src/tests.rs", "crates/x/src/a_tests.rs",
                         "crates/x/src/b_test.rs"]:
            self.assertEqual(check.violations(relative, text), [(2, 'std::env::temp_dir().join("a")')], relative)

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
        self.assertEqual(check.violations("crates/x/src/lib.rs", text), [(10, 'env::temp_dir().join("x")')])

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
        self.assertEqual(check.violations("crates/x/src/lib.rs", text), [(6, "std::env::temp_dir()")])


class Exceptions(unittest.TestCase):
    """What the guard leaves alone."""

    def test_the_marker_a_comment_and_the_helper_crate_are_allowed(self) -> None:
        """Catches a guard that refuses a marked product path, one that reads comments, and one that refuses the
        helper itself."""
        text = (
            '// std::env::temp_dir() in a comment\n'
            'let name = "std::env::temp_dir()";\n'
            '// product temp path\n'
            'assert!(path.starts_with(std::env::temp_dir()));\n'
            'let also = std::env::temp_dir(); // product temp path\n'
        )
        self.assertEqual(check.violations("crates/x/tests/a.rs", text), [])
        self.assertEqual(check.violations("crates/trust-test-support/src/lib.rs", "std::env::temp_dir()\n"), [])

    def test_a_local_helper_named_temp_dir_is_not_the_system_call(self) -> None:
        """Catches a guard that flags a test's own function of that name."""
        text = 'fn temp_dir() -> PathBuf {\n    unique()\n}\nlet root = temp_dir();\n'
        self.assertEqual(check.violations("crates/x/tests/a.rs", text), [])


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
