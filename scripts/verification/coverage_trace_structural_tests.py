"""Focused tests for structural/ignored denominator trace routing."""

from __future__ import annotations

import unittest

from scripts.verification.coverage_trace_structural import (
    build_structural_trace_document,
    structural_route_reason,
)


def _fact(
    discovery_id: str,
    source_kind: str,
    *,
    ignored: bool = False,
) -> dict[str, object]:
    return {
        "discovery_id": discovery_id,
        "discovery_source_kind": source_kind,
        "path": f"tests/{discovery_id}",
        "name": discovery_id,
        "command_hint": "source-derived locator",
        "ignore_state": "ignored" if ignored else "not_ignored",
    }


class CoverageTraceStructuralTests(unittest.TestCase):
    def test_structural_and_declared_ignored_facts_are_skipped_without_coverage(self) -> None:
        gate_id = "DISC_" + "1" * 20
        ignored_id = "DISC_" + "2" * 20
        facts = [
            _fact(gate_id, "gate_script"),
            _fact(ignored_id, "rust_unit_test", ignored=True),
        ]

        document = build_structural_trace_document(
            facts=facts,
            scanner_input_digest="sha256:" + "3" * 64,
            code_map={},
            source_commit="a" * 40,
            platform_name="linux-x86_64",
            mode="focused_shard",
        )

        self.assertEqual([gate_id, ignored_id], document["scope"]["requested_discovery_ids"])
        self.assertTrue(
            all(record["trace_status"] == "skipped" for record in document["tests"])
        )
        self.assertTrue(
            all(not record["covered_code_ids"] for record in document["tests"])
        )

    def test_executable_nonignored_fact_cannot_be_silently_routed_away(self) -> None:
        fact = _fact("DISC_" + "1" * 20, "vscode_test")

        self.assertIsNone(structural_route_reason(fact))
        with self.assertRaisesRegex(ValueError, "executable"):
            build_structural_trace_document(
                facts=[fact],
                scanner_input_digest="sha256:" + "3" * 64,
                code_map={},
                source_commit="a" * 40,
                platform_name="linux-x86_64",
                mode="focused_shard",
            )


if __name__ == "__main__":
    unittest.main()
