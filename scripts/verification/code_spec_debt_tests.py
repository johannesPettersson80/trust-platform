"""Tests for frozen debt partition and delete-only enforcement."""

from __future__ import annotations

import unittest
import tomllib
from pathlib import Path

from scripts.verification.code_spec_common import CodeFact
from scripts.verification.code_spec_debt import (
    build_debt_baseline,
    validate_debt_partition,
    validate_delete_only_history,
    render_debt_baseline_toml,
)


ROOT = Path(__file__).resolve().parents[2]


def fact(code_id: str, body: str = "sha256:" + "2" * 64) -> CodeFact:
    return CodeFact(
        code_id=code_id,
        source_kind="rust",
        path="crates/example/src/lib.rs",
        qualified_symbol=f"example::{code_id}",
        item_kind="rust_function",
        signature="fn run()",
        signature_digest="sha256:" + "1" * 64,
        body_digest=body,
        owner="example",
        area="runtime_safety",
        line=1,
    )


class CodeSpecDebtTests(unittest.TestCase):
    def test_committed_zero_debt_baseline_enables_full_enforcement(self) -> None:
        document = tomllib.loads(
            (ROOT / "verification/code-spec-debt-baseline.toml").read_text()
        )

        self.assertEqual([], document["entries"])
        self.assertEqual("full", document["enforcement_mode"])

    def test_baseline_partitions_only_unmapped_live_facts(self) -> None:
        mapped = fact("CODE_" + "A" * 64)
        debt = fact("CODE_" + "B" * 64)
        document = build_debt_baseline(
            {mapped.code_id: mapped, debt.code_id: debt},
            {mapped.code_id: {"code_id": mapped.code_id}},
            created_from_commit="c" * 40,
            captured_on="2026-07-21",
        )

        self.assertEqual([debt.code_id], [entry["code_id"] for entry in document["entries"]])
        self.assertEqual("changed", document["enforcement_mode"])
        self.assertEqual([], validate_debt_partition(
            document,
            live_facts={mapped.code_id: mapped, debt.code_id: debt},
            mappings={mapped.code_id: {"code_id": mapped.code_id}},
        ))
        self.assertEqual(document, tomllib.loads(render_debt_baseline_toml(document)))

    def test_partition_rejects_new_changed_overlapping_and_invented_facts(self) -> None:
        mapped = fact("CODE_" + "A" * 64)
        debt = fact("CODE_" + "B" * 64)
        baseline = build_debt_baseline(
            {mapped.code_id: mapped, debt.code_id: debt},
            {mapped.code_id: {"code_id": mapped.code_id}},
            created_from_commit="c" * 40,
            captured_on="2026-07-21",
        )
        baseline["entries"][0]["body_digest"] = "sha256:" + "9" * 64
        baseline["entries"].append({**baseline["entries"][0], "code_id": "CODE_" + "C" * 64})

        failures = validate_debt_partition(
            baseline,
            live_facts={mapped.code_id: mapped, debt.code_id: debt},
            mappings={mapped.code_id: {"code_id": mapped.code_id}},
        )

        self.assertTrue(any("stale_spec_review" in failure for failure in failures), failures)
        self.assertTrue(any("invented" in failure for failure in failures), failures)

    def test_frozen_history_is_exactly_delete_only(self) -> None:
        first = fact("CODE_" + "A" * 64)
        second = fact("CODE_" + "B" * 64)
        frozen = build_debt_baseline(
            {first.code_id: first, second.code_id: second},
            {},
            created_from_commit="c" * 40,
            captured_on="2026-07-21",
        )
        current = {**frozen, "entries": frozen["entries"][:1]}
        self.assertEqual([], validate_delete_only_history(current, frozen))

        edited = {**current, "entries": [{**current["entries"][0], "area": "hmi_ui"}]}
        self.assertTrue(validate_delete_only_history(edited, frozen))
        appended = {**frozen, "entries": [*frozen["entries"], {**frozen["entries"][0], "code_id": "CODE_" + "C" * 64}]}
        self.assertTrue(validate_delete_only_history(appended, frozen))

    def test_empty_baseline_round_trips_with_required_entries_array(self) -> None:
        document = build_debt_baseline(
            {},
            {},
            created_from_commit="c" * 40,
            captured_on="2026-07-21",
        )

        rendered = render_debt_baseline_toml(document)

        self.assertIn("entries = []", rendered)
        self.assertEqual(document, tomllib.loads(rendered))


if __name__ == "__main__":
    unittest.main()
