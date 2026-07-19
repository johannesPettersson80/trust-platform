"""Deleted-source provenance fixtures for ignored-test reports."""

from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from .ignored_test_models import (
    IgnoredTestFact,
    IgnoredTestInventoryReport,
    InventoryProvenance,
    write_reports,
)
from .ignored_test_report import LIMITATIONS, SURFACE_NOTES
from .ignored_test_validation import validate_report_files
from .test_catalog_common import input_digest, stable_discovery_id


class IgnoredTestDeletedSourceProvenanceTests(unittest.TestCase):
    def test_at_rest_report_rejects_modeled_source_deleted_since_claimed_commit(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = make_deleted_path_fixture(
                Path(directory),
                deleted_path="scripts/captures/vscode/deleted.spec.mjs",
            )
            with patch(
                "scripts.verification.ignored_test_live.build_live_inventory_state",
                return_value=fixture.state,
            ):
                failures = validate_report_files(
                    fixture.root,
                    fixture.json_path,
                    fixture.markdown_path,
                    fixture.schema_path,
                )

        self.assertTrue(
            any(
                "claimed source commit has modeled source paths absent from the current report closure"
                in failure
                and "scripts/captures/vscode/deleted.spec.mjs" in failure
                for failure in failures
            ),
            failures,
        )

    def test_at_rest_report_excludes_deleted_durable_evidence_from_modeled_sources(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = make_deleted_path_fixture(
                Path(directory),
                deleted_path=(
                    "docs/internal/testing/evidence/plc-verification-program/"
                    "2026-07-10/deleted-review-output.md"
                ),
            )
            with patch(
                "scripts.verification.ignored_test_live.build_live_inventory_state",
                return_value=fixture.state,
            ):
                failures = validate_report_files(
                    fixture.root,
                    fixture.json_path,
                    fixture.markdown_path,
                    fixture.schema_path,
                )

        self.assertEqual(failures, [])


class DeletedPathFixture:
    def __init__(
        self,
        *,
        root: Path,
        json_path: Path,
        markdown_path: Path,
        schema_path: Path,
        state: SimpleNamespace,
    ) -> None:
        self.root = root
        self.json_path = json_path
        self.markdown_path = markdown_path
        self.schema_path = schema_path
        self.state = state


def make_deleted_path_fixture(root: Path, *, deleted_path: str) -> DeletedPathFixture:
    _git(root, "init", "-q")
    _git(root, "config", "user.email", "fixture@example.invalid")
    _git(root, "config", "user.name", "Fixture")
    kept_relative = "scripts/captures/vscode/kept.spec.mjs"
    kept = root / kept_relative
    kept.parent.mkdir(parents=True, exist_ok=True)
    kept.write_text('test.skip("kept", async () => {});\n')
    deleted = root / deleted_path
    deleted.parent.mkdir(parents=True, exist_ok=True)
    deleted.write_text("historical tracked content\n")
    _git(root, "add", kept_relative, deleted_path)
    _git(root, "commit", "-qm", "source revision")
    source_commit = _git_output(root, "rev-parse", "HEAD")
    _git(root, "rm", "-q", deleted_path)
    _git(root, "commit", "-qm", "delete historical path")

    native_id = f"{kept_relative}#kept"
    fact = IgnoredTestFact(
        discovery_id=stable_discovery_id(
            source_kind="playwright_test",
            package="trust-doc-captures",
            native_id=native_id,
        ),
        native_id=native_id,
        discovery_source_kind="playwright_test",
        name="kept",
        path=kept_relative,
        line=1,
        package="trust-doc-captures",
        command_hint="cd scripts/captures && npx playwright test vscode/kept.spec.mjs",
        ignore_state="ignored",
        ignore_mechanism="playwright_literal_skip",
        ignore_reason="literal test.skip declaration",
        reference_candidates=(),
    )
    surfaces = tuple(
        {
            "surface": surface,
            "scanned_files": 1 if surface == "playwright" else 0,
            "records": 1 if surface == "playwright" else 0,
            "ignored": 1 if surface == "playwright" else 0,
            "conditional": 0,
            "coverage": (
                "limitation" if surface in {"shell", "conformance"} else "mechanical"
            ),
            "note": SURFACE_NOTES[surface],
        }
        for surface in sorted(SURFACE_NOTES)
    )
    json_relative = Path("target/gate-artifacts/verification/ignored-test-inventory.json")
    markdown_relative = Path("target/gate-artifacts/verification/ignored-test-inventory.md")
    report = IgnoredTestInventoryReport(
        provenance=InventoryProvenance(
            command=(
                "python3",
                "scripts/report_ignored_test_inventory.py",
                "--json-out",
                json_relative.as_posix(),
                "--markdown-out",
                markdown_relative.as_posix(),
                "--timestamp",
                "2026-07-10T12:00:00Z",
            ),
            commit=source_commit,
            timestamp="2026-07-10T12:00:00Z",
            platform="linux-aarch64",
            input_paths=(kept_relative,),
            output_json=json_relative.as_posix(),
            output_markdown=markdown_relative.as_posix(),
        ),
        input_digest=input_digest(root, [kept_relative]),
        records=(fact,),
        diagnostics=(),
        surface_summary=surfaces,
        limitations=LIMITATIONS,
    )
    json_path = root / json_relative
    markdown_path = root / markdown_relative
    write_reports(report, json_path=json_path, markdown_path=markdown_path)
    state = SimpleNamespace(
        analysis=SimpleNamespace(
            records=report.records,
            diagnostics=report.diagnostics,
            surface_summary=report.surface_summary,
        ),
        input_paths=(kept_relative,),
    )
    schema_path = (
        Path(__file__).resolve().parents[2]
        / "verification/schemas/ignored-test-inventory-report.schema.json"
    )
    return DeletedPathFixture(
        root=root,
        json_path=json_path,
        markdown_path=markdown_path,
        schema_path=schema_path,
        state=state,
    )


def _git(root: Path, *args: str) -> None:
    subprocess.run(["git", "-C", str(root), *args], check=True)


def _git_output(root: Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-C", str(root), *args],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


if __name__ == "__main__":
    unittest.main()
