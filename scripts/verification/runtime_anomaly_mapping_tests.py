"""Tests for explicit runtime-anomaly test mapping analysis."""

from __future__ import annotations

import copy
import unittest

from scripts.verification.runtime_anomaly_mapping import (
    analyze_runtime_anomaly_mapping,
)
from scripts.verification.test_catalog_common import make_fact


class RuntimeAnomalyMappingTests(unittest.TestCase):
    def test_derives_runnable_partial_unmapped_and_gap_rows(self) -> None:
        facts = [
            fixture_fact("panic_test", ignore_state="not_ignored"),
            fixture_fact("timeout_probe", ignore_state="not_ignored"),
            fixture_fact("disconnect_test", ignore_state="ignored"),
        ]
        taxonomy = fixture_taxonomy(
            mappings=[
                fixture_mapping("MAP_PANIC", "panic", facts[0], "direct"),
                fixture_mapping("MAP_TIMEOUT", "timeout", facts[1], "partial"),
                fixture_mapping("MAP_DISCONNECT", "disconnect", facts[2], "direct"),
            ]
        )

        ignored_record = fixture_ignored_record(facts[2], "IGNORED_DISCONNECT")
        analysis = analyze_runtime_anomaly_mapping(
            taxonomy=taxonomy,
            facts=facts,
            ignored_tests={ignored_record["id"]: ignored_record},
            scanner_denominator=len(facts),
        )

        classes = {row["class_id"]: row for row in analysis["classes"]}
        self.assertEqual(classes["panic"]["state"], "mapped_runnable")
        self.assertEqual(
            classes["timeout"]["state"],
            "mapped_non_runnable_or_partial",
        )
        self.assertEqual(
            classes["disconnect"]["state"],
            "mapped_non_runnable_or_partial",
        )
        self.assertEqual(classes["queue_full"]["state"], "unmapped")
        self.assertEqual(
            [row["class_id"] for row in analysis["gap_rows"]],
            ["timeout", "disconnect", "queue_full"],
        )

        mappings = {row["mapping_id"]: row for row in analysis["mappings"]}
        self.assertTrue(mappings["MAP_PANIC"]["effectively_runnable"])
        self.assertFalse(mappings["MAP_TIMEOUT"]["effectively_runnable"])
        self.assertFalse(mappings["MAP_DISCONNECT"]["effectively_runnable"])
        self.assertEqual(mappings["MAP_DISCONNECT"]["ignore_state"], "ignored")
        self.assertEqual(
            mappings["MAP_DISCONNECT"]["ignored_registry_id"],
            "IGNORED_DISCONNECT",
        )
        self.assertIsNone(mappings["MAP_PANIC"]["ignored_registry_id"])

        self.assertEqual(analysis["summary"]["scanner_denominator"], 3)
        self.assertEqual(
            analysis["summary"]["by_primary_suite"],
            {"hardware_lab": 1, "nightly": 1, "pr": 1, "release": 1},
        )
        self.assertEqual(
            analysis["summary"]["by_association_kind"],
            {"context_only": 0, "direct": 2, "partial": 1, "protective_red": 0},
        )
        self.assertEqual(
            analysis["summary"]["by_state"],
            {
                "mapped_non_runnable_or_partial": 2,
                "mapped_runnable": 1,
                "unmapped": 1,
            },
        )
        self.assertEqual(
            analysis["boundaries"],
            {
                "association_only": True,
                "creates_invariant_coverage": False,
                "creates_proof": False,
                "executes_faults": False,
            },
        )

    def test_only_direct_not_ignored_mappings_are_effectively_runnable(self) -> None:
        cases = (
            ("direct", "not_ignored", True),
            ("partial", "not_ignored", False),
            ("protective_red", "not_ignored", False),
            ("context_only", "not_ignored", False),
            ("direct", "ignored", False),
            ("direct", "conditional", False),
        )
        for association_kind, ignore_state, expected in cases:
            with self.subTest(
                association_kind=association_kind,
                ignore_state=ignore_state,
            ):
                fact = fixture_fact("mapped_test", ignore_state=ignore_state)
                ignored = (
                    []
                    if ignore_state == "not_ignored"
                    else [fixture_ignored_record(fact, "IGNORED_MAPPED")]
                )
                taxonomy = fixture_taxonomy(
                    mappings=[
                        fixture_mapping(
                            "MAP_ONE",
                            "panic",
                            fact,
                            association_kind,
                        )
                    ]
                )

                analysis = analyze_runtime_anomaly_mapping(
                    taxonomy=taxonomy,
                    facts=[fact],
                    ignored_tests=ignored,
                    scanner_denominator=1,
                )

                self.assertEqual(
                    analysis["mappings"][0]["effectively_runnable"],
                    expected,
                )
                expected_state = (
                    "mapped_runnable"
                    if expected
                    else "mapped_non_runnable_or_partial"
                )
                self.assertEqual(analysis["classes"][0]["state"], expected_state)

    def test_names_paths_and_reference_candidates_never_create_mappings(self) -> None:
        fact = fixture_fact("panic", ignore_state="not_ignored")
        fact = make_fact(
            source_kind=fact.source_kind,
            name="panic",
            path="crates/trust-runtime/tests/panic.rs",
            line=fact.line,
            package=fact.package,
            command_hint=fact.command_hint,
            command_hint_authority=fact.command_hint_authority,
            discovery_confidence=fact.discovery_confidence,
            reference_candidates=("panic", "RISK_RUNTIME_PANIC"),
        )

        analysis = analyze_runtime_anomaly_mapping(
            taxonomy=fixture_taxonomy(mappings=[]),
            facts=[fact],
            ignored_tests=[],
            scanner_denominator=1,
        )

        self.assertEqual(analysis["mappings"], [])
        self.assertEqual(analysis["classes"][0]["state"], "unmapped")
        self.assertEqual(len(analysis["gap_rows"]), len(analysis["classes"]))

    def test_mapping_requires_one_exact_scanner_identity(self) -> None:
        fact = fixture_fact("panic_test")
        baseline = fixture_taxonomy(
            mappings=[fixture_mapping("MAP_ONE", "panic", fact, "direct")]
        )

        cases: list[tuple[str, list, dict, str]] = [
            ("missing", [], baseline, "does not resolve a scanner fact"),
            (
                "duplicate",
                [fact, copy.deepcopy(fact)],
                baseline,
                "resolves to 2 scanner facts",
            ),
        ]
        for field, scanner_value in (
            ("path", "crates/trust-runtime/tests/renamed.rs"),
            ("name", "renamed_test"),
            ("discovery_source_kind", "rust_unit_test"),
        ):
            changed = copy.deepcopy(baseline)
            changed["mappings"][0][field] = scanner_value
            cases.append((field, [fact], changed, f"{field} does not match scanner fact"))

        for label, facts, taxonomy, signal in cases:
            with self.subTest(label=label):
                with self.assertRaisesRegex(ValueError, signal):
                    analyze_runtime_anomaly_mapping(
                        taxonomy=taxonomy,
                        facts=facts,
                        ignored_tests=[],
                        scanner_denominator=len(facts),
                    )

    def test_ignored_mapping_requires_one_exact_registry_record(self) -> None:
        fact = fixture_fact("disconnect_test", ignore_state="ignored")
        taxonomy = fixture_taxonomy(
            mappings=[fixture_mapping("MAP_ONE", "disconnect", fact, "direct")]
        )
        baseline = fixture_ignored_record(fact, "IGNORED_ONE")
        cases: list[tuple[str, list[dict], str]] = [
            ("missing", [], "requires one ignored-test registry record"),
            (
                "duplicate",
                [baseline, {**baseline, "id": "IGNORED_TWO"}],
                "resolves to 2 ignored-test registry records",
            ),
        ]
        for field, value in (
            ("path", "crates/trust-runtime/tests/elsewhere.rs"),
            ("name", "elsewhere"),
            ("discovery_source_kind", "rust_unit_test"),
            ("ignore_state", "conditional"),
        ):
            changed = copy.deepcopy(baseline)
            changed[field] = value
            cases.append(
                (field, [changed], f"ignored-test {field} does not match scanner fact")
            )

        for label, ignored_tests, signal in cases:
            with self.subTest(label=label):
                with self.assertRaisesRegex(ValueError, signal):
                    analyze_runtime_anomaly_mapping(
                        taxonomy=taxonomy,
                        facts=[fact],
                        ignored_tests=ignored_tests,
                        scanner_denominator=1,
                    )

    def test_not_ignored_mapping_rejects_stale_ignored_registry_record(self) -> None:
        fact = fixture_fact("panic_test", ignore_state="not_ignored")
        taxonomy = fixture_taxonomy(
            mappings=[fixture_mapping("MAP_ONE", "panic", fact, "direct")]
        )
        stale_record = fixture_ignored_record(fact, "IGNORED_STALE")
        stale_record["ignore_state"] = "ignored"

        with self.assertRaisesRegex(
            ValueError,
            "not_ignored scanner fact still has an ignored-test registry record",
        ):
            analyze_runtime_anomaly_mapping(
                taxonomy=taxonomy,
                facts=[fact],
                ignored_tests=[stale_record],
                scanner_denominator=1,
            )

    def test_scanner_denominator_is_caller_bound_and_must_match_facts(self) -> None:
        fact = fixture_fact("panic_test")

        with self.assertRaisesRegex(ValueError, "scanner denominator 3021 does not match 1 facts"):
            analyze_runtime_anomaly_mapping(
                taxonomy=fixture_taxonomy(mappings=[]),
                facts=[fact],
                ignored_tests=[],
                scanner_denominator=3021,
            )


def fixture_taxonomy(*, mappings: list[dict]) -> dict:
    classes = [
        fixture_class("panic", "pr"),
        fixture_class("timeout", "nightly"),
        fixture_class("disconnect", "hardware_lab"),
        fixture_class("queue_full", "release"),
    ]
    return {"classes": classes, "mappings": mappings}


def fixture_class(class_id: str, primary_suite: str) -> dict:
    return {
        "id": class_id,
        "title": class_id.replace("_", " ").title(),
        "stimulus": f"Inject {class_id} through a reviewed boundary.",
        "primary_suite": primary_suite,
        "conditional_suites": [],
        "injection_boundary": "test_harness",
        "rationale": "Fixture class.",
    }


def fixture_fact(name: str, *, ignore_state: str = "not_ignored"):
    return make_fact(
        source_kind="rust_integration_test",
        name=name,
        path=f"crates/trust-runtime/tests/{name}.rs",
        line=12,
        package="trust-runtime",
        command_hint=f"cargo test -p trust-runtime --test {name} {name} -- --exact",
        command_hint_authority="exact",
        discovery_confidence="exact_attribute",
        ignore_state=ignore_state,
        ignore_reason=None if ignore_state == "not_ignored" else ignore_state,
    )


def fixture_mapping(
    mapping_id: str,
    class_id: str,
    fact,
    association_kind: str,
) -> dict:
    return {
        "id": mapping_id,
        "class_id": class_id,
        "discovery_id": fact.stable_id,
        "discovery_source_kind": fact.source_kind,
        "path": fact.path,
        "name": fact.name,
        "association_kind": association_kind,
        "injection_mechanism": "test_harness",
        "assertion_summary": "Fixture association only.",
        "limitations": ["Does not establish proof."],
        "last_reviewed": "2026-07-11",
    }


def fixture_ignored_record(fact, record_id: str) -> dict:
    return {
        "id": record_id,
        "discovery_id": fact.stable_id,
        "discovery_source_kind": fact.source_kind,
        "path": fact.path,
        "name": fact.name,
        "ignore_state": fact.ignore_state,
    }


if __name__ == "__main__":
    unittest.main()
