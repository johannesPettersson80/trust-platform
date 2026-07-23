"""Focused tests for disjoint trace shard merging."""

from __future__ import annotations

import unittest

from scripts.verification.coverage_trace_merge import merge_trace_documents


def _fact(discovery_id: str, source_kind: str) -> dict[str, object]:
    return {
        "discovery_id": discovery_id,
        "discovery_source_kind": source_kind,
        "path": f"tests/{discovery_id}.test",
        "name": discovery_id,
    }


def _trace(
    discovery_id: str,
    source_kind: str,
    *,
    inventory_name: str,
) -> dict[str, object]:
    fact = _fact(discovery_id, source_kind)
    attribution = (
        "istanbul_source_coverage"
        if source_kind == "vscode_test"
        else "llvm_source_coverage"
    )
    return {
        "schema_version": 1,
        "generator": "test-coverage-trace",
        "generator_version": "1",
        "source_commit": "a" * 40,
        "platform": "linux-x86_64",
        "scope": {
            "mode": "focused_shard",
            "requested_discovery_ids": [discovery_id],
            "source_kinds": [source_kind],
        },
        "input_digest": "sha256:" + discovery_id[-1].lower() * 64,
        "code_inventory_digests": {
            inventory_name: "sha256:" + "b" * 64,
        },
        "tests": [
            {
                **fact,
                "command": "focused command",
                "command_authority": "verified_synthesized",
                "trace_status": "observed",
                "attribution_kind": attribution,
                "attempt_id": discovery_id,
                "exit_code": 0,
                "duration_ms": 1,
                "profile_digest": "sha256:" + "c" * 64,
                "covered_code_ids": [],
                "limitations": [],
            }
        ],
    }


class CoverageTraceMergeTests(unittest.TestCase):
    def test_disjoint_shards_merge_to_complete_final_campaign(self) -> None:
        rust_id = "DISC_" + "1" * 20
        vscode_id = "DISC_" + "2" * 20
        live = {
            rust_id: _fact(rust_id, "rust_unit_test"),
            vscode_id: _fact(vscode_id, "vscode_test"),
        }

        merged = merge_trace_documents(
            [
                _trace(rust_id, "rust_unit_test", inventory_name="rust_product"),
                _trace(
                    vscode_id,
                    "vscode_test",
                    inventory_name="typescript_product",
                ),
            ],
            live_facts=live,
            mode="full_campaign",
            require_complete_denominator=True,
        )

        self.assertEqual([rust_id, vscode_id], merged["scope"]["requested_discovery_ids"])
        self.assertEqual(
            {"rust_product", "typescript_product"},
            set(merged["code_inventory_digests"]),
        )

    def test_overlap_and_incomplete_denominator_fail_closed(self) -> None:
        discovery_id = "DISC_" + "1" * 20
        trace = _trace(
            discovery_id,
            "rust_unit_test",
            inventory_name="rust_product",
        )
        live = {discovery_id: _fact(discovery_id, "rust_unit_test")}

        with self.assertRaisesRegex(ValueError, "overlap"):
            merge_trace_documents(
                [trace, trace],
                live_facts=live,
                mode="focused_shard",
                require_complete_denominator=False,
            )
        with self.assertRaisesRegex(ValueError, "complete live denominator"):
            merge_trace_documents(
                [trace],
                live_facts={
                    **live,
                    "DISC_" + "2" * 20: _fact(
                        "DISC_" + "2" * 20,
                        "rust_unit_test",
                    ),
                },
                mode="full_campaign",
                require_complete_denominator=True,
            )


if __name__ == "__main__":
    unittest.main()
