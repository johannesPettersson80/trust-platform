"""Tests for explicit VS Code extension-test registration auditing."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.verification.test_catalog_vscode_registration import audit_vscode_test_registration


class TestCatalogVscodeRegistrationTests(unittest.TestCase):
    def test_accepts_direct_literal_registrations_including_nested_paths(self) -> None:
        with vscode_root(
            files=("alpha.test.ts", "nested/beta.test.ts"),
            registrations=('require("./alpha.test");', 'require("./nested/beta.test");'),
        ) as root:
            audit = audit_vscode_test_registration(root)

        self.assertTrue(audit.is_clean)
        self.assertEqual(len(audit.test_files), 2)
        self.assertEqual(len(audit.registered_files), 2)

    def test_reports_orphan_missing_and_duplicate_targets(self) -> None:
        with vscode_root(
            files=("alpha.test.ts", "orphan.test.ts"),
            registrations=(
                'require("./alpha.test");',
                'require("./alpha.test");',
                'require("./missing.test");',
            ),
        ) as root:
            audit = audit_vscode_test_registration(root)

        kinds = {item.kind for item in audit.diagnostics}
        self.assertIn("unregistered_test_file", kinds)
        self.assertIn("duplicate_registration", kinds)
        self.assertIn("missing_registration_target", kinds)

    def test_rejects_dynamic_conditional_and_escaping_loaders(self) -> None:
        with vscode_root(
            files=("alpha.test.ts",),
            registrations=(
                "require(variable);",
                'if (enabled) require("./alpha.test");',
                'require("../outside.test");',
            ),
        ) as root:
            audit = audit_vscode_test_registration(root)

        kinds = [item.kind for item in audit.diagnostics]
        self.assertIn("unsupported_registration", kinds)
        self.assertIn("unsafe_registration_path", kinds)
        self.assertIn("unregistered_test_file", kinds)

    def test_ignores_comment_and_string_fakes(self) -> None:
        with vscode_root(
            files=("alpha.test.ts",),
            registrations=(
                '// require("./fake.test");',
                'const fake = \'require("./fake.test")\';',
                'require("./alpha.test");',
            ),
        ) as root:
            audit = audit_vscode_test_registration(root)

        self.assertTrue(audit.is_clean)

    def test_rejects_case_mismatch_and_missing_execution_boundaries(self) -> None:
        with vscode_root(
            files=("alpha.test.ts",),
            registrations=('require("./Alpha.test");',),
        ) as root:
            audit = audit_vscode_test_registration(root)
            index = root / "editors/vscode/src/test/suite/index.ts"
            index.write_text(index.read_text().replace("pre-require", "wrong-boundary"))
            missing_boundary = audit_vscode_test_registration(root)

        self.assertIn("missing_registration_target", {item.kind for item in audit.diagnostics})
        self.assertIn("unregistered_test_file", {item.kind for item in audit.diagnostics})
        self.assertIn("registration_boundaries", {item.kind for item in missing_boundary.diagnostics})

    def test_symlink_target_outside_suite_fails_closed(self) -> None:
        with vscode_root(
            files=(),
            registrations=('require("./escape.test");',),
        ) as root, tempfile.TemporaryDirectory() as outside_temp:
            outside = Path(outside_temp) / "outside.test.ts"
            outside.write_text('test("outside", () => {});\n')
            link = root / "editors/vscode/src/test/suite/escape.test.ts"
            link.symlink_to(outside)
            audit = audit_vscode_test_registration(root)

        kinds = {item.kind for item in audit.diagnostics}
        self.assertIn("missing_registration_target", kinds)
        self.assertIn("unregistered_test_file", kinds)

    def test_rejects_discovered_fact_outside_suite_registration_surface(self) -> None:
        with vscode_root(
            files=("alpha.test.ts",),
            registrations=('require("./alpha.test");',),
        ) as root:
            outside = root / "editors/vscode/src/test/outside.test.ts"
            outside.write_text('test("outside", () => {});\n')

            audit = audit_vscode_test_registration(root)

        self.assertFalse(audit.is_clean)
        self.assertEqual(
            audit.unregistered_fact_files,
            ("editors/vscode/src/test/outside.test.ts",),
        )
        self.assertIn("unregistered_vscode_fact_file", {item.kind for item in audit.diagnostics})

    def test_rejects_discovered_javascript_fact_missing_from_ts_inventory(self) -> None:
        with vscode_root(
            files=("alpha.test.ts",),
            registrations=('require("./alpha.test");',),
        ) as root:
            javascript = root / "editors/vscode/src/test/suite/legacy.test.js"
            javascript.write_text('test("legacy", () => {});\n')

            audit = audit_vscode_test_registration(root)

        self.assertFalse(audit.is_clean)
        self.assertEqual(
            audit.unregistered_fact_files,
            ("editors/vscode/src/test/suite/legacy.test.js",),
        )
        self.assertIn("unregistered_vscode_fact_file", {item.kind for item in audit.diagnostics})

    def test_live_repository_has_no_unregistered_vscode_facts(self) -> None:
        root = Path(__file__).resolve().parents[2]
        audit = audit_vscode_test_registration(root)

        self.assertTrue(audit.is_clean)
        self.assertEqual(len(audit.test_files), 39)
        self.assertEqual(len(audit.entries), 39)
        self.assertEqual(audit.fact_count, 461)
        self.assertEqual(audit.unregistered_fact_files, ())


class vscode_root:
    def __init__(self, *, files: tuple[str, ...], registrations: tuple[str, ...]) -> None:
        self._temp = tempfile.TemporaryDirectory()
        self.root = Path(self._temp.name)
        self.files = files
        self.registrations = registrations

    def __enter__(self) -> Path:
        suite = self.root / "editors/vscode/src/test/suite"
        suite.mkdir(parents=True)
        (self.root / "editors/vscode/package.json").write_text('{"name":"fixture-vscode"}\n')
        for relative in self.files:
            path = suite / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('test("fixture", () => {});\n')
        registration_lines = "\n".join(f"  {line}" for line in self.registrations)
        (suite / "index.ts").write_text(
            'import Mocha from "mocha";\n'
            "export function run(): Promise<void> {\n"
            '  const mocha = new Mocha({ ui: "tdd" });\n'
            '  mocha.suite.emit("pre-require", global, "nofile", mocha);\n'
            f"{registration_lines}\n"
            "  return new Promise((resolve) => {\n"
            "    mocha.run(() => resolve());\n"
            "  });\n"
            "}\n"
        )
        return self.root

    def __exit__(self, exc_type, exc, tb) -> None:
        self._temp.cleanup()


if __name__ == "__main__":
    unittest.main()
