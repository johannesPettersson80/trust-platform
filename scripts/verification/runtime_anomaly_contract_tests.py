"""Tests for the reviewed Phase 8 runtime-anomaly taxonomy contract."""

from __future__ import annotations

import copy
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.metadata_validator.core import Validator
from scripts.verification.runtime_anomaly_contract import (
    ALLOCATION_REQUIRED_TEXT,
    CLASS_IDS,
    validate_runtime_anomaly_contract,
    validate_runtime_anomaly_schema_contract,
)
from scripts.verification.test_catalog_json_schema import validate_json_schema_instance


def fixture_schema() -> dict:
    path = ROOT / "verification/schemas/runtime-anomaly-taxonomy.schema.json"
    return json.loads(path.read_text())


def fixture_taxonomy() -> dict:
    classes = []
    for class_id in CLASS_IDS:
        classes.append(
            {
                "id": class_id,
                "title": class_id.replace("_", " ").title(),
                "stimulus": f"Inject the reviewed {class_id} stimulus.",
                "primary_suite": "pr",
                "conditional_suites": [],
                "injection_boundary": "ordinary_input",
                "rationale": "Association records inventory an existing stimulus only.",
            }
        )
    return {
        "schema_version": 1,
        "id": "RUNTIME_ANOMALY_TAXONOMY_V1",
        "title": "Runtime anomaly taxonomy v1",
        "area": "runtime_safety",
        "mapping_basis": "explicit_reviewed_discovery_id_only",
        "proof_posture": "association_only",
        "fault_interface_status": "not_implemented",
        "production_hook_policy": "design_review_required",
        "last_reviewed": "2026-07-11",
        "spec_gap_reviews": {
            "scan_cycle_allocation_policy": {
                "outcome": "written_contract_present",
                "source_ref": "SPEC_RUNTIME_ENGINE_001",
                "source_path": "docs/specs/11-runtime-engine.md",
                "required_text": list(ALLOCATION_REQUIRED_TEXT),
                "rationale": (
                    "The runtime specification forbids allocation in the execution hot path."
                ),
            },
            "restart_timebase": {
                "outcome": "existing_open_gap",
                "spec_gap_ref": "SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001",
                "rationale": "Restart and timer time-base semantics remain explicitly open.",
            },
        },
        "classes": classes,
        "mappings": [
            {
                "id": "ANOM_MAP_PANIC_001",
                "class_id": "panic",
                "discovery_id": "DISC_30C382889325B64C5854",
                "discovery_source_kind": "rust_integration_test",
                "path": "crates/trust-runtime/tests/runtime_safety_fail_closed.rs",
                "name": "panic_in_io_driver_faults_resource_visibly",
                "association_kind": "direct",
                "injection_mechanism": "test_harness",
                "assertion_summary": "The existing test observes a visible resource fault.",
                "limitations": [
                    "The association does not establish behavior outside this scenario."
                ],
                "last_reviewed": "2026-07-11",
            },
            {
                "id": "ANOM_MAP_CLOCK_001",
                "class_id": "monotonic_wall_clock_divergence",
                "discovery_id": "DISC_2418E946F38C19B5F5A8",
                "discovery_source_kind": "rust_unit_test",
                "path": "crates/trust-runtime/src/scheduler/tests.rs",
                "name": "scaled_clock_now_is_monotonic",
                "association_kind": "partial",
                "injection_mechanism": "ordinary_input",
                "assertion_summary": "The existing test observes monotonic scaled time.",
                "limitations": ["It does not inject wall-clock divergence."],
                "last_reviewed": "2026-07-11",
            },
        ],
    }


def fixture_spec_sources() -> dict[str, dict]:
    return {
        "SPEC_RUNTIME_ENGINE_001": {
            "id": "SPEC_RUNTIME_ENGINE_001",
            "area": "runtime_safety",
            "path": "docs/specs/11-runtime-engine.md",
            "source_status": "active",
            "authority": "normative_product",
            "oracle_eligible": True,
        }
    }


def resolved_restart_review() -> dict[str, object]:
    return {
        "outcome": "resolved_source",
        "source_ref": "SPEC_RUNTIME_ENGINE_001",
        "source_path": "docs/specs/11-runtime-engine.md",
        "superseded_gap_id": "SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001",
        "rationale": "The reviewed product source now owns restart and time-base semantics.",
    }


def fixture_spec_gaps() -> dict[str, dict]:
    return {
        "SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001": {
            "id": "SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001",
            "area": "compiler_iec",
            "status": "spec_gap",
            "resolution_status": "open",
        }
    }


class RuntimeAnomalyContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        (self.root / "verification/schemas").mkdir(parents=True)
        (self.root / "docs/specs").mkdir(parents=True)
        (self.root / "crates/trust-runtime/tests").mkdir(parents=True)
        (self.root / "crates/trust-runtime/src/scheduler").mkdir(parents=True)
        (self.root / "verification/schemas/runtime-anomaly-taxonomy.schema.json").write_text(
            json.dumps(fixture_schema(), indent=2) + "\n"
        )
        (self.root / "docs/specs/11-runtime-engine.md").write_text(
            "dynamic allocation in hot path is absent.\nNo heap allocation during execution.\n"
        )
        (self.root / "crates/trust-runtime/tests/runtime_safety_fail_closed.rs").write_text(
            "// fixture\n"
        )
        (self.root / "crates/trust-runtime/src/scheduler/tests.rs").write_text("// fixture\n")
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        subprocess.run(["git", "-C", str(self.root), "add", "."], check=True)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def validate(self, taxonomy: dict) -> list[str]:
        return validate_runtime_anomaly_contract(
            self.root,
            taxonomy,
            spec_sources=fixture_spec_sources(),
            spec_gaps=fixture_spec_gaps(),
        )

    def test_known_good_contract_validates(self) -> None:
        self.assertEqual([], self.validate(fixture_taxonomy()))

    def test_classes_are_exactly_the_board_order_and_closed(self) -> None:
        missing = fixture_taxonomy()
        missing["classes"].pop()
        reordered = fixture_taxonomy()
        reordered["classes"][0], reordered["classes"][1] = (
            reordered["classes"][1],
            reordered["classes"][0],
        )
        extra = fixture_taxonomy()
        extra["classes"][0]["proof_state"] = "validated"

        self.assertTrue(any("exact board order" in item for item in self.validate(missing)))
        self.assertTrue(any("exact board order" in item for item in self.validate(reordered)))
        self.assertTrue(any("class panic fields drift" in item for item in self.validate(extra)))

    def test_suite_and_injection_enums_and_conditional_uniqueness_fail_closed(self) -> None:
        bad_suite = fixture_taxonomy()
        bad_suite["classes"][0]["primary_suite"] = "veryquick"
        duplicate = fixture_taxonomy()
        duplicate["classes"][0]["conditional_suites"] = ["nightly", "nightly"]
        primary_repeated = fixture_taxonomy()
        primary_repeated["classes"][0]["conditional_suites"] = ["pr"]
        production = fixture_taxonomy()
        production["mappings"][0]["injection_mechanism"] = "production_hook"

        self.assertTrue(any("unknown primary_suite" in item for item in self.validate(bad_suite)))
        self.assertTrue(
            any("conditional_suites must be unique" in item for item in self.validate(duplicate))
        )
        self.assertTrue(
            any(
                "must exclude primary_suite" in item
                for item in self.validate(primary_repeated)
            )
        )
        self.assertTrue(
            any("unknown injection_mechanism" in item for item in self.validate(production))
        )

    def test_mapping_ids_discovery_ids_and_class_refs_are_unique_and_known(self) -> None:
        duplicate_mapping = fixture_taxonomy()
        duplicate_mapping["mappings"][1]["id"] = duplicate_mapping["mappings"][0]["id"]
        duplicate_discovery = fixture_taxonomy()
        duplicate_discovery["mappings"][1]["discovery_id"] = duplicate_discovery[
            "mappings"
        ][0]["discovery_id"]
        unknown = fixture_taxonomy()
        unknown["mappings"][0]["class_id"] = "invented"

        self.assertTrue(
            any("duplicate mapping IDs" in item for item in self.validate(duplicate_mapping))
        )
        self.assertTrue(
            any(
                "duplicate discovery IDs" in item
                for item in self.validate(duplicate_discovery)
            )
        )
        self.assertTrue(any("unknown class_id" in item for item in self.validate(unknown)))

    def test_schema_invalid_mapping_class_id_returns_failures(self) -> None:
        for value in ([], {}):
            with self.subTest(value=value):
                taxonomy = fixture_taxonomy()
                taxonomy["mappings"][0]["class_id"] = value

                failures = self.validate(taxonomy)

                self.assertTrue(any("class_id" in failure for failure in failures), failures)

    def test_mapping_paths_are_safe_tracked_non_symlinked_and_match_source_kind(self) -> None:
        unsafe = fixture_taxonomy()
        unsafe["mappings"][0]["path"] = "../escape.rs"
        wrong_kind = fixture_taxonomy()
        wrong_kind["mappings"][0]["discovery_source_kind"] = "rust_unit_test"
        untracked_path = self.root / "crates/trust-runtime/tests/untracked.rs"
        untracked_path.write_text("// untracked\n")
        untracked = fixture_taxonomy()
        untracked["mappings"][0]["path"] = "crates/trust-runtime/tests/untracked.rs"
        symlink_path = self.root / "crates/trust-runtime/tests/link.rs"
        symlink_path.symlink_to(
            self.root / "crates/trust-runtime/tests/runtime_safety_fail_closed.rs"
        )
        symlinked = fixture_taxonomy()
        symlinked["mappings"][0]["path"] = "crates/trust-runtime/tests/link.rs"

        self.assertTrue(
            any("normalized workspace-relative" in item for item in self.validate(unsafe))
        )
        self.assertTrue(
            any("does not match rust_unit_test" in item for item in self.validate(wrong_kind))
        )
        self.assertTrue(any("tracked durable file" in item for item in self.validate(untracked)))
        self.assertTrue(any("symlink" in item for item in self.validate(symlinked)))

        cross_crate_path = "crates/other-runtime/tests/runtime_safety_fail_closed.rs"
        cross_crate_file = self.root / cross_crate_path
        cross_crate_file.parent.mkdir(parents=True)
        cross_crate_file.write_text("// cross-crate fixture\n")
        subprocess.run(
            ["git", "-C", str(self.root), "add", cross_crate_path],
            check=True,
        )
        cross_crate = fixture_taxonomy()
        cross_crate["mappings"][0]["path"] = cross_crate_path
        invalid_id = fixture_taxonomy()
        invalid_id["mappings"][0]["id"] = "MAP_PANIC"

        self.assertTrue(
            any("must stay under crates/trust-runtime" in item for item in self.validate(cross_crate))
        )
        self.assertTrue(any("invalid mapping ID" in item for item in self.validate(invalid_id)))

    def test_spec_reviews_bind_active_source_open_gap_and_required_prose(self) -> None:
        taxonomy = fixture_taxonomy()
        missing_phrase = fixture_taxonomy()
        missing_phrase["spec_gap_reviews"]["scan_cycle_allocation_policy"]["required_text"] = [
            "No heap allocation during execution."
        ]
        bad_gap = fixture_spec_gaps()
        bad_gap["SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001"]["resolution_status"] = "resolved"
        bad_source = fixture_spec_sources()
        bad_source["SPEC_RUNTIME_ENGINE_001"]["source_status"] = "superseded"

        self.assertEqual([], self.validate(taxonomy))
        self.assertTrue(
            any("required_text must equal" in item for item in self.validate(missing_phrase))
        )
        self.assertTrue(
            any(
                "active oracle-eligible runtime source" in item
                for item in validate_runtime_anomaly_contract(
                    self.root,
                    taxonomy,
                    spec_sources=bad_source,
                    spec_gaps=fixture_spec_gaps(),
                )
            )
        )
        self.assertTrue(
            any(
                "existing_open_gap requires an open gap" in item
                for item in validate_runtime_anomaly_contract(
                    self.root,
                    taxonomy,
                    spec_sources=fixture_spec_sources(),
                    spec_gaps=bad_gap,
                )
            )
        )

    def test_resolved_restart_review_binds_active_non_claim_source(self) -> None:
        taxonomy = fixture_taxonomy()
        taxonomy["spec_gap_reviews"]["restart_timebase"] = resolved_restart_review()

        self.assertEqual([], self.validate(taxonomy))

        for field, value, signal in (
            ("source_status", "superseded", "active oracle-eligible non-public-claim"),
            ("oracle_eligible", False, "active oracle-eligible non-public-claim"),
            ("authority", "public_claim", "active oracle-eligible non-public-claim"),
        ):
            with self.subTest(field=field):
                sources = fixture_spec_sources()
                sources["SPEC_RUNTIME_ENGINE_001"][field] = value
                failures = validate_runtime_anomaly_contract(
                    self.root,
                    taxonomy,
                    spec_sources=sources,
                    spec_gaps=fixture_spec_gaps(),
                )
                self.assertTrue(any(signal in item for item in failures), failures)

    def test_restart_review_variants_reject_hybrids_and_invented_gap_ids(self) -> None:
        existing_hybrid = fixture_taxonomy()
        existing_hybrid["spec_gap_reviews"]["restart_timebase"]["source_ref"] = (
            "SPEC_RUNTIME_ENGINE_001"
        )
        resolved_hybrid = fixture_taxonomy()
        resolved_hybrid["spec_gap_reviews"]["restart_timebase"] = {
            **resolved_restart_review(),
            "spec_gap_ref": "SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001",
        }
        invented = fixture_taxonomy()
        invented["spec_gap_reviews"]["restart_timebase"] = {
            **resolved_restart_review(),
            "superseded_gap_id": "SPEC_GAP_INVENTED_001",
        }

        for taxonomy in (existing_hybrid, resolved_hybrid):
            with self.subTest(outcome=taxonomy["spec_gap_reviews"]["restart_timebase"]):
                failures = self.validate(taxonomy)
                self.assertTrue(
                    any("restart_timebase review fields drift" in item for item in failures),
                    failures,
                )
        self.assertTrue(
            any("superseded_gap_id" in item for item in self.validate(invented)),
            self.validate(invented),
        )

    def test_existing_restart_review_rejects_premature_closed_gap(self) -> None:
        gaps = fixture_spec_gaps()
        gaps["SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001"]["resolution_status"] = "closed"

        failures = validate_runtime_anomaly_contract(
            self.root,
            fixture_taxonomy(),
            spec_sources=fixture_spec_sources(),
            spec_gaps=gaps,
        )

        self.assertTrue(
            any("existing_open_gap requires an open gap" in item for item in failures),
            failures,
        )

    def test_resolved_restart_review_accepts_open_or_source_bound_closed_gap(self) -> None:
        taxonomy = fixture_taxonomy()
        taxonomy["spec_gap_reviews"]["restart_timebase"] = resolved_restart_review()
        closed = fixture_spec_gaps()
        closed_gap = closed["SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001"]
        closed_gap["resolution_status"] = "closed"
        closed_gap["resolution_source_ref"] = "SPEC_RUNTIME_ENGINE_001"

        self.assertEqual([], self.validate(taxonomy))
        self.assertEqual(
            [],
            validate_runtime_anomaly_contract(
                self.root,
                taxonomy,
                spec_sources=fixture_spec_sources(),
                spec_gaps=closed,
            ),
        )

    def test_resolved_restart_review_rejects_unbound_closed_gap(self) -> None:
        taxonomy = fixture_taxonomy()
        taxonomy["spec_gap_reviews"]["restart_timebase"] = resolved_restart_review()

        for resolution_ref in (None, "SPEC_OTHER_SOURCE_001"):
            with self.subTest(resolution_ref=resolution_ref):
                gaps = fixture_spec_gaps()
                gap = gaps["SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001"]
                gap["resolution_status"] = "closed"
                if resolution_ref is not None:
                    gap["resolution_source_ref"] = resolution_ref
                failures = validate_runtime_anomaly_contract(
                    self.root,
                    taxonomy,
                    spec_sources=fixture_spec_sources(),
                    spec_gaps=gaps,
                )
                self.assertTrue(
                    any(
                        "closed superseded gap must bind the resolved_source source_ref"
                        in item
                        for item in failures
                    ),
                    failures,
                )

    def test_resolved_restart_review_cannot_claim_test_coverage_or_proof(self) -> None:
        taxonomy = fixture_taxonomy()
        taxonomy["spec_gap_reviews"]["restart_timebase"] = {
            **resolved_restart_review(),
            "rationale": "This source proves complete test coverage.",
        }

        failures = self.validate(taxonomy)

        self.assertTrue(
            any("forbidden proof/coverage language" in item for item in failures),
            failures,
        )

    def test_free_text_cannot_claim_proof_or_coverage(self) -> None:
        proof = fixture_taxonomy()
        proof["mappings"][0]["assertion_summary"] = "This proves the invariant."
        coverage = fixture_taxonomy()
        coverage["classes"][0]["rationale"] = "Coverage is complete."

        self.assertTrue(
            any("forbidden proof/coverage language" in item for item in self.validate(proof))
        )
        self.assertTrue(
            any(
                "forbidden proof/coverage language" in item
                for item in self.validate(coverage)
            )
        )

    def test_schema_contract_pins_closed_fields_consts_and_enums(self) -> None:
        schema = fixture_schema()
        open_mapping = copy.deepcopy(schema)
        open_mapping["$defs"]["mapping"]["additionalProperties"] = True
        missing_root = copy.deepcopy(schema)
        missing_root["required"].remove("production_hook_policy")
        widened_mechanism = copy.deepcopy(schema)
        widened_mechanism["$defs"]["mapping"]["properties"]["injection_mechanism"][
            "enum"
        ].append("production_hook")
        reordered_classes = copy.deepcopy(schema)
        reordered_classes["$defs"]["class"]["properties"]["id"]["enum"].reverse()
        weak_mapping_id = copy.deepcopy(schema)
        weak_mapping_id["$defs"]["mapping"]["properties"]["id"]["pattern"] = ".*"
        weak_mapping_path = copy.deepcopy(schema)
        weak_mapping_path["$defs"]["mapping"]["properties"]["path"]["pattern"] = ".*"
        weak_required_text = copy.deepcopy(schema)
        weak_required_text["$defs"]["allocation_review"]["properties"][
            "required_text"
        ] = {
            "type": "array",
            "minItems": len(ALLOCATION_REQUIRED_TEXT),
            "maxItems": len(ALLOCATION_REQUIRED_TEXT),
            "uniqueItems": True,
            "items": {"type": "string", "minLength": 1},
        }
        widened_restart_union = copy.deepcopy(schema)
        widened_restart_union["$defs"]["restart_timebase_review"]["oneOf"].append(
            {"type": "object"}
        )
        restart_ref_sibling = copy.deepcopy(schema)
        restart_ref_sibling["$defs"]["restart_timebase_review"]["oneOf"][0][
            "type"
        ] = "string"
        open_resolved_restart = copy.deepcopy(schema)
        open_resolved_restart["$defs"]["restart_resolved_source_v1"][
            "additionalProperties"
        ] = True
        weak_open_rationale = copy.deepcopy(schema)
        weak_open_rationale["$defs"]["restart_existing_open_gap_v1"]["properties"][
            "rationale"
        ] = {"type": "string"}
        weak_resolved_source = copy.deepcopy(schema)
        weak_resolved_source["$defs"]["restart_resolved_source_v1"]["properties"][
            "source_ref"
        ] = {"pattern": "^SPEC_[A-Z0-9_]+$"}

        self.assertEqual([], validate_runtime_anomaly_schema_contract(schema))
        self.assertTrue(
            any(
                "mapping schema must be a closed object" in item
                for item in validate_runtime_anomaly_schema_contract(open_mapping)
            )
        )
        self.assertTrue(
            any(
                "root required fields drift" in item
                for item in validate_runtime_anomaly_schema_contract(missing_root)
            )
        )
        self.assertTrue(
            any(
                "injection_mechanism enum drifts" in item
                for item in validate_runtime_anomaly_schema_contract(widened_mechanism)
            )
        )
        self.assertTrue(
            any(
                "class ID enum drifts" in item
                for item in validate_runtime_anomaly_schema_contract(reordered_classes)
            )
        )
        self.assertTrue(
            any(
                "mapping ID pattern drifts" in item
                for item in validate_runtime_anomaly_schema_contract(weak_mapping_id)
            )
        )
        self.assertTrue(
            any(
                "mapping path pattern drifts" in item
                for item in validate_runtime_anomaly_schema_contract(weak_mapping_path)
            )
        )
        self.assertTrue(
            any(
                "allocation required_text schema drifts" in item
                for item in validate_runtime_anomaly_schema_contract(weak_required_text)
            )
        )
        self.assertTrue(
            any(
                "restart review union drifts" in item
                for item in validate_runtime_anomaly_schema_contract(
                    widened_restart_union
                )
            )
        )
        self.assertTrue(
            any(
                "restart review union drifts" in item
                for item in validate_runtime_anomaly_schema_contract(
                    restart_ref_sibling
                )
            )
        )
        self.assertTrue(
            any(
                "resolved_source schema must be a closed object" in item
                for item in validate_runtime_anomaly_schema_contract(
                    open_resolved_restart
                )
            )
        )
        self.assertTrue(
            any(
                "existing restart rationale schema drifts" in item
                for item in validate_runtime_anomaly_schema_contract(
                    weak_open_rationale
                )
            )
        )
        self.assertTrue(
            any(
                "resolved restart source_ref schema drifts" in item
                for item in validate_runtime_anomaly_schema_contract(
                    weak_resolved_source
                )
            )
        )

    def test_schema_evaluator_requires_exactly_one_restart_variant(self) -> None:
        schema = fixture_schema()
        existing = fixture_taxonomy()
        resolved = fixture_taxonomy()
        resolved["spec_gap_reviews"]["restart_timebase"] = resolved_restart_review()
        ambiguous_schema = copy.deepcopy(schema)
        ambiguous_schema["$defs"]["restart_timebase_review"]["oneOf"][1] = {
            "$ref": "#/$defs/restart_existing_open_gap_v1"
        }
        ref_sibling_schema = copy.deepcopy(schema)
        ref_sibling_schema["$defs"]["restart_timebase_review"]["oneOf"][0][
            "type"
        ] = "string"

        self.assertEqual([], validate_json_schema_instance(existing, schema))
        self.assertEqual([], validate_json_schema_instance(resolved, schema))
        self.assertTrue(
            any(
                "exactly one schema branch" in failure
                for failure in validate_json_schema_instance(existing, ambiguous_schema)
            )
        )
        self.assertTrue(validate_json_schema_instance(existing, ref_sibling_schema))

    def test_full_metadata_validator_wires_runtime_anomaly_contract(self) -> None:
        validator = Validator()
        validator.load_records()
        validator.runtime_anomaly_taxonomy["proof_posture"] = "proof"

        validator.validate()

        self.assertTrue(
            any(
                failure.path.as_posix() == "verification/runtime-anomaly-taxonomy.toml"
                and "proof_posture must equal 'association_only'" in failure.message
                for failure in validator.failures
            ),
            [failure.message for failure in validator.failures],
        )

    def test_full_metadata_validator_returns_failures_for_invalid_mapping_class_id(
        self,
    ) -> None:
        for value in ([], {}):
            with self.subTest(value=value):
                validator = Validator()
                validator.load_records()
                validator.runtime_anomaly_taxonomy["mappings"][0]["class_id"] = value

                validator.validate()

                self.assertTrue(
                    any(
                        failure.path.as_posix()
                        == "verification/runtime-anomaly-taxonomy.toml"
                        and "class_id" in failure.message
                        for failure in validator.failures
                    ),
                    [failure.message for failure in validator.failures],
                )

    def test_full_metadata_validator_rejects_restart_variant_hybrid(self) -> None:
        validator = Validator()
        validator.load_records()
        validator.runtime_anomaly_taxonomy["spec_gap_reviews"]["restart_timebase"] = {
            **resolved_restart_review(),
            "spec_gap_ref": "SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001",
        }

        validator.validate()

        self.assertTrue(
            any(
                failure.path.as_posix() == "verification/runtime-anomaly-taxonomy.toml"
                and "restart_timebase review fields drift" in failure.message
                for failure in validator.failures
            ),
            [failure.message for failure in validator.failures],
        )

    def test_full_metadata_validator_rejects_untrusted_or_premature_restart_state(
        self,
    ) -> None:
        scenarios = (
            ("inactive", "active oracle-eligible non-public-claim"),
            ("public", "active oracle-eligible non-public-claim"),
            ("invented_gap", "superseded_gap_id"),
            (
                "closed_gap_source_mismatch",
                "closed superseded gap must bind the resolved_source source_ref",
            ),
        )
        for scenario, signal in scenarios:
            with self.subTest(scenario=scenario):
                validator = Validator()
                validator.load_records()
                validator.runtime_anomaly_taxonomy["spec_gap_reviews"][
                    "restart_timebase"
                ] = resolved_restart_review()
                if scenario == "inactive":
                    validator.spec_sources["SPEC_RUNTIME_ENGINE_001"][
                        "source_status"
                    ] = "superseded"
                elif scenario == "public":
                    validator.spec_sources["SPEC_RUNTIME_ENGINE_001"][
                        "authority"
                    ] = "public_claim"
                elif scenario == "invented_gap":
                    validator.runtime_anomaly_taxonomy["spec_gap_reviews"][
                        "restart_timebase"
                    ]["superseded_gap_id"] = "SPEC_GAP_INVENTED_001"

                validator.validate()

                self.assertTrue(
                    any(
                        failure.path.as_posix()
                        == "verification/runtime-anomaly-taxonomy.toml"
                        and signal in failure.message
                        for failure in validator.failures
                    ),
                    [failure.message for failure in validator.failures],
                )


if __name__ == "__main__":
    unittest.main()
