from __future__ import annotations

import hashlib
import inspect
import json
import unittest

from scripts.verification.authority_route_mapping import (
    build_authority_route_mapping_inputs,
)


def _fact() -> dict[str, object]:
    return {
        "stable_id": "DISC_AAAAAAAAAAAAAAAAAAAA",
        "source_kind": "rust_unit_test",
        "path": "crates/example/src/tests.rs",
        "name": "locks_reviewed_behavior",
        "ignore_state": "not_ignored",
    }


def _route() -> dict[str, object]:
    return {
        "discovery_id": "DISC_AAAAAAAAAAAAAAAAAAAA",
        "path": "crates/example/src/tests.rs",
        "name": "locks_reviewed_behavior",
        "invariant_id": "INV_EXAMPLE_001",
        "behavior_partition": "REVIEWED_VALUE",
        "behavior_partitions": ["REVIEWED_VALUE"],
        "route_source": "manual-spec-review:rollout:1",
    }


def _invariant() -> dict[str, object]:
    return {
        "id": "INV_EXAMPLE_001",
        "status": "specified",
        "spec": {"status": "specified"},
        "oracle": {"ref": "SPEC_EXAMPLE_001#reviewed-value"},
        "behavior": [
            {
                "partition": {"equals": "REVIEWED_VALUE"},
                "case_family": "happy_path",
                "rationale": "The reviewed value is preserved.",
            }
        ],
    }


def _review() -> dict[str, object]:
    return {
        "discovery_id": "DISC_AAAAAAAAAAAAAAAAAAAA",
        "discovery_source_kind": "rust_unit_test",
        "path": "crates/example/src/tests.rs",
        "name": "locks_reviewed_behavior",
        "ignore_state": "not_ignored",
        "disposition": "reviewed_nonmapping",
        "reason_code": "registered_backlog",
        "last_reviewed": "2026-08-01",
    }


def _execution() -> dict[str, object]:
    return {
        "schema_version": 1,
        "execution": {
            "kind": "rust_nextest_jsonl",
            "batch_command": "cargo nextest run -p example -E test(=locks_reviewed_behavior)",
            "source_path": "crates/example/src/tests.rs",
        },
        "tests": [
            {
                "title": "locks_reviewed_behavior",
                "duration": 7,
                "state": "passed",
            }
        ],
        "failures": [],
        "pending": [],
    }


def _demand() -> dict[str, object]:
    return {
        "discovery_id": "DISC_AAAAAAAAAAAAAAAAAAAA",
        "candidate_tier": "C4",
        "covered_code_ids": ["CODE_" + "A" * 64],
        "spec_gap_refs": ["SPEC_GAP_EXAMPLE_001"],
        "unmapped_code_ids": [],
    }


def _focused_trace(
    *,
    discovery_id: str = "DISC_AAAAAAAAAAAAAAAAAAAA",
    covered_code_ids: list[str] | None = None,
) -> dict[str, object]:
    return {
        "schema_version": 1,
        "generator": "test-coverage-trace",
        "generator_version": "1",
        "source_commit": "a" * 40,
        "tests": [
            {
                "discovery_id": discovery_id,
                "discovery_source_kind": "rust_unit_test",
                "path": "crates/example/src/tests.rs",
                "name": "locks_reviewed_behavior",
                "command": (
                    "cargo nextest run -p example "
                    "-E test(=locks_reviewed_behavior)"
                ),
                "command_authority": "exact",
                "trace_status": "observed",
                "exit_code": 0,
                "duration_ms": 7,
                "covered_code_ids": covered_code_ids
                if covered_code_ids is not None
                else ["CODE_" + "A" * 64],
            }
        ],
    }


def _canonical_digest(document: object) -> str:
    encoded = json.dumps(
        document,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _authority_document(
    trace: dict[str, object],
    *,
    route: dict[str, object] | None = None,
) -> dict[str, object]:
    return {
        "schema_version": 1,
        "kind": "reviewed_test_invariant_authority_routes",
        "source_commit": trace["source_commit"],
        "trace_digest": _canonical_digest(trace),
        "routes": [route if route is not None else _route()],
    }


def _authenticated_build(
    *,
    trace: dict[str, object] | None = None,
    authority_document: dict[str, object] | None = None,
    authority_route_digest: str | None = None,
    demand: dict[str, object] | None = None,
) -> dict[str, object]:
    selected_trace = trace if trace is not None else _focused_trace()
    selected_authority = (
        authority_document
        if authority_document is not None
        else _authority_document(selected_trace)
    )
    kwargs: dict[str, object] = {
        "routes": selected_authority["routes"],
        "facts": [_fact()],
        "denominator_reviews": [_review()],
        "invariants": {"INV_EXAMPLE_001": _invariant()},
        "execution_documents": [_execution()],
    }
    parameters = inspect.signature(build_authority_route_mapping_inputs).parameters
    if "mapping_demands" in parameters:
        kwargs["mapping_demands"] = [demand if demand is not None else _demand()]
    if "authenticated_inputs" in parameters:
        kwargs["authenticated_inputs"] = {
            "authority_document": selected_authority,
            "authority_route_digest": authority_route_digest
            if authority_route_digest is not None
            else _canonical_digest(selected_authority),
            "focused_trace": selected_trace,
            "focused_trace_digest": _canonical_digest(selected_trace),
        }
    return build_authority_route_mapping_inputs(**kwargs)


class AuthorityRouteMappingTests(unittest.TestCase):
    def test_authenticated_c4_route_accepts_exact_reviewed_join(self) -> None:
        result = _authenticated_build()

        self.assertEqual(1, len(result["candidates"]))
        candidate = result["candidates"][0]
        self.assertEqual("DISC_AAAAAAAAAAAAAAAAAAAA", candidate["discovery_id"])
        self.assertEqual("C2", candidate["candidate_tier"])
        self.assertEqual(["INV_EXAMPLE_001"], candidate["candidate_invariant_ids"])
        self.assertEqual(
            ["INV_EXAMPLE_001#behavior:0"],
            candidate["exact_behavior_refs"],
        )
        self.assertEqual(["CODE_" + "A" * 64], candidate["covered_code_ids"])
        self.assertEqual(["CODE_" + "A" * 64], candidate["spec_gap_code_ids"])
        self.assertEqual(["SPEC_GAP_EXAMPLE_001"], candidate["spec_gap_refs"])
        self.assertEqual([], candidate["specified_code_ids"])
        self.assertEqual([], candidate["unmapped_code_ids"])
        self.assertRegex(candidate["authority_route_digest"], r"^sha256:[0-9a-f]{64}$")
        self.assertEqual("exact", result["trace_records"][0]["command_authority"])

    def test_tampered_reviewed_route_or_digest_fails_closed(self) -> None:
        trace = _focused_trace()
        authority = _authority_document(trace)
        digest = _canonical_digest(authority)
        authority["routes"][0]["behavior_partitions"] = ["TAMPERED"]

        with self.assertRaisesRegex(ValueError, "authority route digest mismatch"):
            _authenticated_build(
                trace=trace,
                authority_document=authority,
                authority_route_digest=digest,
            )

    def test_authenticated_route_rejects_erased_gap_state(self) -> None:
        demand = _demand()
        demand["spec_gap_refs"] = []

        with self.assertRaisesRegex(ValueError, "requires non-empty spec-gap state"):
            _authenticated_build(demand=demand)

    def test_route_for_wrong_discovery_is_rejected(self) -> None:
        trace = _focused_trace(discovery_id="DISC_BBBBBBBBBBBBBBBBBBBB")
        authority = _authority_document(trace, route=_route())

        with self.assertRaisesRegex(ValueError, "authority route discovery set mismatch"):
            _authenticated_build(trace=trace, authority_document=authority)

    def test_route_for_wrong_trace_is_rejected(self) -> None:
        trace = _focused_trace()
        authority = _authority_document(trace)
        trace["tests"][0]["covered_code_ids"] = ["CODE_" + "B" * 64]

        with self.assertRaisesRegex(ValueError, "trace digest mismatch"):
            _authenticated_build(
                trace=trace,
                authority_document=authority,
                authority_route_digest=_canonical_digest(authority),
            )

    def test_nullable_primary_resolves_all_route_behavior_partitions(self) -> None:
        route = _route()
        route["behavior_partition"] = None
        route["behavior_partitions"] = ["REVIEWED_VALUE", "SECONDARY"]
        invariant = _invariant()
        invariant["behavior"].append(
            {
                "partition": {"equals": "SECONDARY"},
                "case_family": "happy_path",
                "rationale": "The secondary reviewed behavior is preserved.",
            }
        )

        result = build_authority_route_mapping_inputs(
            routes=[route],
            facts=[_fact()],
            denominator_reviews=[_review()],
            invariants={"INV_EXAMPLE_001": invariant},
            execution_documents=[_execution()],
        )

        self.assertEqual(
            [
                "INV_EXAMPLE_001#behavior:0",
                "INV_EXAMPLE_001#behavior:1",
            ],
            result["candidates"][0]["exact_behavior_refs"],
        )

    def test_exact_route_becomes_candidate_and_observed_trace(self) -> None:
        result = build_authority_route_mapping_inputs(
            routes=[_route()],
            facts=[_fact()],
            denominator_reviews=[_review()],
            invariants={"INV_EXAMPLE_001": _invariant()},
            execution_documents=[_execution()],
        )

        self.assertEqual(
            {
                "candidate_invariant_ids": ["INV_EXAMPLE_001"],
                "candidate_tier": "C1",
                "covered_code_ids": [],
                "discovery_id": "DISC_AAAAAAAAAAAAAAAAAAAA",
                "exact_behavior_refs": ["INV_EXAMPLE_001#behavior:0"],
                "mutation_kill_ids": [],
                "spec_gap_code_ids": [],
                "spec_gap_refs": [],
                "specified_code_ids": [],
            },
            result["candidates"][0],
        )
        self.assertEqual(
            {
                "command": "cargo nextest run -p example -E test(=locks_reviewed_behavior)",
                "command_authority": "verified_synthesized",
                "discovery_id": "DISC_AAAAAAAAAAAAAAAAAAAA",
                "duration_ms": 7,
                "trace_status": "observed",
            },
            result["trace_records"][0],
        )
        self.assertEqual(
            {
                "candidates": 1,
                "execution_documents": 1,
                "routes": 1,
            },
            result["summary"],
        )

    def test_missing_execution_fails_closed(self) -> None:
        with self.assertRaisesRegex(
            ValueError,
            "DISC_AAAAAAAAAAAAAAAAAAAA lacks one passing focused execution",
        ):
            build_authority_route_mapping_inputs(
                routes=[_route()],
                facts=[_fact()],
                denominator_reviews=[_review()],
                invariants={"INV_EXAMPLE_001": _invariant()},
                execution_documents=[],
            )

    def test_non_equals_partition_predicates_resolve_by_canonical_label(self) -> None:
        route = _route()
        route["behavior_partition"] = "malformed_PRIVATE_ACCESS"
        route["behavior_partitions"] = [
            "malformed_PRIVATE_ACCESS",
            "above_COMPLEXITY_15",
        ]
        invariant = _invariant()
        invariant["behavior"] = [
            {
                "partition": {"malformed": "PRIVATE_ACCESS"},
                "case_family": "invalid_input",
                "rationale": "Private access outside its boundary is rejected.",
            },
            {
                "partition": {"above": "COMPLEXITY_15"},
                "case_family": "boundary",
                "rationale": "Complexity above the reviewed limit is reported.",
            },
        ]

        result = build_authority_route_mapping_inputs(
            routes=[route],
            facts=[_fact()],
            denominator_reviews=[_review()],
            invariants={"INV_EXAMPLE_001": invariant},
            execution_documents=[_execution()],
        )

        self.assertEqual(
            [
                "INV_EXAMPLE_001#behavior:0",
                "INV_EXAMPLE_001#behavior:1",
            ],
            result["candidates"][0]["exact_behavior_refs"],
        )

    def test_unknown_or_ambiguous_partition_fails_closed(self) -> None:
        invariant = _invariant()
        invariant["behavior"] = [
            *invariant["behavior"],
            invariant["behavior"][0],
        ]
        with self.assertRaisesRegex(
            ValueError,
            "REVIEWED_VALUE must resolve to exactly one behavior row",
        ):
            build_authority_route_mapping_inputs(
                routes=[_route()],
                facts=[_fact()],
                denominator_reviews=[_review()],
                invariants={"INV_EXAMPLE_001": invariant},
                execution_documents=[_execution()],
            )


if __name__ == "__main__":
    unittest.main()
