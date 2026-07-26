from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.verification.frozen_st_batch import (
    FrozenStSelection,
    build_test_command,
    derive_frozen_selection,
    execute_batch,
    normalize_st_json,
)


def _raw_document(
    rows: list[dict[str, object]],
) -> dict[str, object]:
    passed = sum(row["status"] == "passed" for row in rows)
    failed = sum(row["status"] == "failed" for row in rows)
    errors = sum(row["status"] == "error" for row in rows)
    return {
        "version": 1,
        "project": "fixture",
        "summary": {
            "total": len(rows),
            "passed": passed,
            "failed": failed,
            "errors": errors,
            "duration_ms": sum(int(row["duration_ms"]) for row in rows),
        },
        "tests": rows,
    }


def _test_row(
    name: str,
    *,
    status: str = "passed",
    duration_ms: int = 1,
) -> dict[str, object]:
    return {
        "name": name,
        "kind": "TEST_PROGRAM",
        "status": status,
        "file": "fixture/src/tests.st",
        "line": 4,
        "source": f"PROGRAM {name}",
        "message": None if status == "passed" else f"{name} failed",
        "duration_ms": duration_ms,
    }


class FrozenStBatchTests(unittest.TestCase):
    def test_selection_is_exact_union_of_reviewed_and_mapped_rows(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "verification").mkdir()
            (root / "verification/test-catalog-denominator.toml").write_text(
                """
[[reviews]]
discovery_id = "DISC_A"
discovery_source_kind = "structured_text_test"
path = "fixture/src/tests.st"
name = "batch_a"
ignore_state = "not_ignored"
disposition = "reviewed_nonmapping"

[[reviews]]
discovery_id = "DISC_B"
discovery_source_kind = "structured_text_test"
path = "fixture/src/tests.st"
name = "batch_b"
ignore_state = "not_ignored"
disposition = "catalog_mapped"
"""
            )
            (root / "verification/test-catalog.toml").write_text(
                """
[[tests]]
id = "TEST_B"
discovery_id = "DISC_B"
discovery_source_kind = "structured_text_test"
path = "fixture/src/tests.st"
name = "batch_b"
status = "mapped"
"""
            )
            selection = derive_frozen_selection(
                root=root,
                path="fixture/src/tests.st",
                expected_reviewed_nonmapping=1,
                expected_catalog_mapped=1,
            )
            self.assertEqual(("batch_a", "batch_b"), selection.names)
            with self.assertRaisesRegex(ValueError, "expected 2"):
                derive_frozen_selection(
                    root=root,
                    path="fixture/src/tests.st",
                    expected_reviewed_nonmapping=2,
                    expected_catalog_mapped=1,
                )

    def test_normalizer_preserves_exact_names_durations_and_failures(
        self,
    ) -> None:
        raw = json.dumps(
            _raw_document(
                [
                    _test_row("batch_b", status="error", duration_ms=3),
                    _test_row("batch_a", duration_ms=2),
                ]
            )
        )
        self.assertEqual(
            {
                "tests": [
                    {"title": "batch_a", "duration": 2, "state": "passed"},
                    {"title": "batch_b", "duration": 3, "state": "error"},
                ],
                "failures": [
                    {
                        "title": "batch_b",
                        "state": "error",
                        "message": "batch_b failed",
                    }
                ],
                "pending": [],
                "stats": {
                    "tests": 2,
                    "passes": 1,
                    "failures": 1,
                    "pending": 0,
                },
            },
            normalize_st_json(
                raw, expected_names={"batch_a", "batch_b"}
            ),
        )
        with self.assertRaisesRegex(ValueError, "frozen selection"):
            normalize_st_json(raw, expected_names={"batch_a"})

    def test_command_is_shell_free_for_binary_or_cargo_prefix(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "trust-dev"
            binary.write_bytes(b"binary")
            expected_tail = [
                "test",
                "--project",
                "fixture",
                "--filter",
                "batch_",
                "--output",
                "json",
            ]
            self.assertEqual(
                [str(binary), *expected_tail],
                build_test_command(
                    project_path="fixture",
                    test_prefix="batch_",
                    trust_dev_binary=binary,
                ),
            )
            self.assertEqual(
                ["cargo", "run", "-p", "trust-dev", "--", *expected_tail],
                build_test_command(
                    project_path="fixture",
                    test_prefix="batch_",
                    cargo_command="cargo run -p trust-dev --",
                ),
            )
            with self.assertRaisesRegex(ValueError, "exactly one"):
                build_test_command(
                    project_path="fixture",
                    test_prefix="batch_",
                )

    def test_execute_batch_runs_once_and_binds_raw_provenance(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for relative, content in (
                (
                    "scripts/verification/frozen_st_batch.py",
                    "runner\n",
                ),
                ("scripts/run_frozen_st_batch.py", "shim\n"),
                ("fixture/src/tests.st", "PROGRAM batch_a\n"),
            ):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            invocation_log = root / "invocations.txt"
            fake = root / "fake-trust-dev"
            fake.write_text(
                "#!/usr/bin/env python3\n"
                "import json, pathlib, sys\n"
                f"pathlib.Path({str(invocation_log)!r}).write_text("
                "' '.join(sys.argv[1:]))\n"
                f"print(json.dumps({_raw_document([_test_row('batch_a')])!r}))\n"
            )
            fake.chmod(0o755)
            command = build_test_command(
                project_path="fixture",
                test_prefix="batch_",
                trust_dev_binary=fake,
            )
            with patch(
                "scripts.verification.frozen_st_batch._git_head",
                return_value="a" * 40,
            ):
                document, exit_code = execute_batch(
                    root=root,
                    selection=FrozenStSelection(
                        path="fixture/src/tests.st",
                        reviewed_nonmapping_count=1,
                        catalog_mapped_count=0,
                        names=("batch_a",),
                    ),
                    project_path="fixture",
                    test_prefix="batch_",
                    command=command,
                    raw_path=root / "evidence/raw.json",
                )
            self.assertEqual(0, exit_code)
            self.assertEqual(
                "test --project fixture --filter batch_ --output json",
                invocation_log.read_text(),
            )
            self.assertEqual("st_json", document["execution"]["kind"])
            self.assertEqual("a" * 40, document["execution"]["git_head"])
            self.assertEqual([], document["failures"])


if __name__ == "__main__":
    unittest.main()
