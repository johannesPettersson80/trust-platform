"""Tests for durable proof output and source-revision acquisition."""

from __future__ import annotations

import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path

from scripts.verification.proof_output import (
    ProofOutputError,
    append_evidence_record,
    clean_head_revision,
)


FULL_SHA = "a" * 40


class ProofOutputTests(unittest.TestCase):
    def test_clean_head_revision_returns_full_commit(self) -> None:
        with git_fixture() as fx:
            self.assertEqual(clean_head_revision(fx.root), fx.head())
            self.assertEqual(len(clean_head_revision(fx.root)), 40)

    def test_clean_head_revision_rejects_tracked_and_untracked_changes(self) -> None:
        for mode in ("tracked", "untracked"):
            with self.subTest(mode=mode), git_fixture() as fx:
                path = fx.root / ("tracked.txt" if mode == "tracked" else "untracked.txt")
                path.write_text(mode)

                with self.assertRaisesRegex(ProofOutputError, "clean Git worktree"):
                    clean_head_revision(fx.root)

    def test_append_writes_one_record_to_canonical_tracked_index(self) -> None:
        with git_fixture() as fx:
            record = evidence_record()

            path = append_evidence_record(
                root=fx.root,
                evidence_index_path=fx.evidence_index,
                record=record,
            )

            self.assertEqual(path, fx.evidence_index)
            payload = tomllib.loads(path.read_text())
            self.assertEqual(payload["evidence"][-1], record)
            self.assertEqual(path.read_text().count("[[evidence]]"), 2)

    def test_append_round_trips_nested_executed_test_records(self) -> None:
        with git_fixture() as fx:
            record = evidence_record()
            record["executed_tests"] = [
                {
                    "test_id": "TEST_TIMER",
                    "command": "cargo test timer",
                    "exit_status": 0,
                    "per_case_summary": ["CASE_ONE:passed", "CASE_TWO:passed"],
                }
            ]

            append_evidence_record(
                root=fx.root,
                evidence_index_path=fx.evidence_index,
                record=record,
            )

            payload = tomllib.loads(fx.evidence_index.read_text())
            self.assertEqual(payload["evidence"][-1], record)

    def test_append_rejects_duplicate_id_without_changing_index(self) -> None:
        with git_fixture() as fx:
            original = fx.evidence_index.read_bytes()
            duplicate = evidence_record(record_id="EVID_EXISTING")

            with self.assertRaisesRegex(ProofOutputError, "already exists"):
                append_evidence_record(
                    root=fx.root,
                    evidence_index_path=fx.evidence_index,
                    record=duplicate,
                )

            self.assertEqual(fx.evidence_index.read_bytes(), original)

    def test_append_rejects_noncanonical_or_untracked_destination(self) -> None:
        with git_fixture() as fx:
            alternate = fx.root / "verification" / "alternate.toml"
            alternate.write_text("")

            with self.assertRaisesRegex(ProofOutputError, "canonical evidence index"):
                append_evidence_record(
                    root=fx.root,
                    evidence_index_path=alternate,
                    record=evidence_record(),
                )


def evidence_record(*, record_id: str = "EVID_NEW") -> dict[str, object]:
    return {
        "schema_version": 1,
        "id": record_id,
        "title": "Proof output fixture",
        "area": "verification",
        "owner": "verification",
        "status": "mapped",
        "kind": "committed_file",
        "path": "verification/evidence-index.toml",
        "command": "python3 fixture.py",
        "commit": FULL_SHA,
        "platform": "test",
        "date": "2026-07-12",
        "suite_id": "veryquick",
        "producer": "prove.py v1",
        "generated_report_version": "prove-red-v1",
        "linked_invariants": [],
        "linked_tests": [],
        "last_reviewed": "2026-07-12",
        "proof_kind": "red",
        "failure_kind": "assertion_failure",
    }


class git_fixture:
    def __enter__(self) -> "git_fixture":
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.evidence_index = self.root / "verification" / "evidence-index.toml"
        self.evidence_index.parent.mkdir(parents=True)
        self.evidence_index.write_text(
            "[[evidence]]\n"
            'schema_version = 1\n'
            'id = "EVID_EXISTING"\n'
            'proof_kind = "none"\n'
        )
        (self.root / "tracked.txt").write_text("tracked\n")
        self.git("init", "-q")
        self.git("config", "user.email", "verification@example.invalid")
        self.git("config", "user.name", "Verification Tests")
        self.git("add", ".")
        self.git("commit", "-qm", "fixture")
        return self

    def __exit__(self, *exc: object) -> None:
        self.temp.cleanup()

    def git(self, *args: str) -> str:
        result = subprocess.run(
            ["git", *args],
            cwd=self.root,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
        )
        return result.stdout.strip()

    def head(self) -> str:
        return self.git("rev-parse", "HEAD")


if __name__ == "__main__":
    unittest.main()
