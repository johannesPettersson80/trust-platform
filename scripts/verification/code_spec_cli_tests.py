"""Focused orchestration tests for the Phase 17 code-spec CLI."""

from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

from scripts.verification.code_spec_cli import (
    collect_report_inputs,
    validate_frozen_debt_history,
)
from scripts.verification.code_spec_debt import (
    build_debt_baseline,
    render_debt_baseline_toml,
)
from scripts.verification.code_spec_debt_tests import fact


class CodeSpecCliTests(unittest.TestCase):
    def test_input_closure_contains_every_scanner_source_and_control_input(self) -> None:
        state = SimpleNamespace(
            rust=SimpleNamespace(source_paths=("crates/example/src/lib.rs",)),
            typescript=SimpleNamespace(source_paths=("editors/vscode/src/extension.ts",)),
            tooling=SimpleNamespace(source_paths=("scripts/check.py",)),
        )

        inputs = collect_report_inputs(Path("."), state)

        for expected in (
            "crates/example/src/lib.rs",
            "editors/vscode/src/extension.ts",
            "scripts/check.py",
            "verification/code-spec-map.toml",
            "verification/code-spec-private-inputs.toml",
            "verification/code-spec-risk-acknowledgements.toml",
            "verification/code-spec-debt-baseline.toml",
            "verification/matrix.toml",
            "scripts/verification/code_spec_live.py",
            "scripts/verification/code_spec_ts_scan.mjs",
        ):
            self.assertIn(expected, inputs)
        self.assertEqual(len(inputs), len(set(inputs)))

    def test_committed_baseline_history_is_delete_only(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.email", "test@example.com"], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.name", "Test"], check=True)
            (root / "scanner.py").write_text("# frozen scanner implementation\n")
            subprocess.run(["git", "-C", str(root), "add", "."], check=True)
            subprocess.run(["git", "-C", str(root), "commit", "-qm", "implementation"], check=True)
            implementation_commit = subprocess.run(
                ["git", "-C", str(root), "rev-parse", "HEAD"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            path = root / "verification/code-spec-debt-baseline.toml"
            path.parent.mkdir(parents=True)
            first = fact("CODE_" + "A" * 64)
            second = fact("CODE_" + "B" * 64)
            frozen = build_debt_baseline(
                {first.code_id: first, second.code_id: second},
                {},
                created_from_commit=implementation_commit,
                captured_on="2026-07-21",
            )
            path.write_text(render_debt_baseline_toml(frozen))
            subprocess.run(["git", "-C", str(root), "add", "."], check=True)
            subprocess.run(["git", "-C", str(root), "commit", "-qm", "freeze"], check=True)

            current = {**frozen, "entries": frozen["entries"][:1]}
            self.assertEqual([], validate_frozen_debt_history(root, current))

            appended = {**frozen, "entries": [*frozen["entries"], {**frozen["entries"][0], "code_id": "CODE_" + "C" * 64}]}
            failures = validate_frozen_debt_history(root, appended)
            self.assertTrue(any("append is forbidden" in failure for failure in failures), failures)

    def test_frozen_baseline_origin_must_match_its_introduction_parent(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.email", "test@example.com"], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.name", "Test"], check=True)
            (root / "scanner.py").write_text("# implementation\n")
            subprocess.run(["git", "-C", str(root), "add", "."], check=True)
            subprocess.run(["git", "-C", str(root), "commit", "-qm", "implementation"], check=True)
            path = root / "verification/code-spec-debt-baseline.toml"
            path.parent.mkdir(parents=True)
            code_fact = fact("CODE_" + "A" * 64)
            frozen = build_debt_baseline(
                {code_fact.code_id: code_fact},
                {},
                created_from_commit="c" * 40,
                captured_on="2026-07-21",
            )
            path.write_text(render_debt_baseline_toml(frozen))
            subprocess.run(["git", "-C", str(root), "add", "."], check=True)
            subprocess.run(["git", "-C", str(root), "commit", "-qm", "freeze"], check=True)

            failures = validate_frozen_debt_history(root, frozen)

        self.assertTrue(
            any("created_from_commit does not match" in failure for failure in failures),
            failures,
        )


if __name__ == "__main__":
    unittest.main()
