"""Tests for hand-owned test-catalog intent and subject classification."""

from __future__ import annotations

import unittest

from scripts.verification.test_catalog_intent import validate_catalog_intent


class TestCatalogIntentTests(unittest.TestCase):
    def test_accepts_sparse_generated_binding_for_inventoried_area(self) -> None:
        failures = validate_catalog_intent(
            tests={"TEST_INVALID_MAGIC": generated_record()},
            matrix=mapped_matrix(),
            invariants={"VM_SEAM_VALID_001": {"area": "bytecode_vm"}},
            spec_sources={"SPEC_BYTECODE_FORMAT_001": {"area": "bytecode_vm"}},
            spec_gaps={"SPEC_GAP_VM_ERROR_MODEL_001": {"area": "bytecode_vm"}},
        )

        self.assertEqual(failures, [])

    def test_rejects_missing_or_duplicate_generated_identity(self) -> None:
        missing = generated_record()
        missing.pop("name")
        duplicate = generated_record()
        duplicate["id"] = "TEST_DUPLICATE"
        failures = validate_catalog_intent(
            tests={missing["id"]: missing, duplicate["id"]: duplicate},
            matrix=mapped_matrix(),
            invariants={"VM_SEAM_VALID_001": {"area": "bytecode_vm"}},
            spec_sources={"SPEC_BYTECODE_FORMAT_001": {"area": "bytecode_vm"}},
            spec_gaps={"SPEC_GAP_VM_ERROR_MODEL_001": {"area": "bytecode_vm"}},
        )

        self.assertTrue(any("generated_test requires name" in item for item in failures))
        self.assertTrue(any("duplicate discovery_id" in item for item in failures))

    def test_rejects_uninventoried_or_cross_area_mapping(self) -> None:
        uninventoried = generated_record()
        uninventoried["area"] = "runtime_safety"
        cross_area = generated_record()
        cross_area["id"] = "TEST_CROSS_AREA"
        cross_area["oracle_ref"] = "SPEC_WRONG_AREA"
        failures = validate_catalog_intent(
            tests={uninventoried["id"]: uninventoried, cross_area["id"]: cross_area},
            matrix=mapped_matrix(),
            invariants={"VM_SEAM_VALID_001": {"area": "bytecode_vm"}},
            spec_sources={
                "SPEC_BYTECODE_FORMAT_001": {"area": "bytecode_vm"},
                "SPEC_WRONG_AREA": {"area": "runtime_safety"},
            },
            spec_gaps={"SPEC_GAP_VM_ERROR_MODEL_001": {"area": "bytecode_vm"}},
        )

        self.assertTrue(any("uninventoried area runtime_safety" in item for item in failures))
        self.assertTrue(any("oracle_ref SPEC_WRONG_AREA area" in item for item in failures))

    def test_special_subjects_are_closed_exceptions_not_generic_bypasses(self) -> None:
        case_table = common_record("TEST_CASE", "case_table_artifact")
        case_table.update(
            test_class="metadata_validation",
            path="verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml",
            case_file="verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml",
            case_file_digest="sha256:" + "0" * 64,
        )
        mutation = common_record("TEST_MUTATION", "mutation_shard_runner")
        mutation.update(
            test_class="mutation",
            path="scripts/bytecode_validator_mutation.py",
            mutation_shard_id="MUTATION_SHARD_BYTECODE_VALIDATOR_001",
            mutations=[{"id": "MUTANT"}],
        )

        self.assertEqual(
            validate_catalog_intent(
                tests={case_table["id"]: case_table, mutation["id"]: mutation},
                matrix=mapped_matrix(),
                invariants={"VM_SEAM_VALID_001": {"area": "bytecode_vm"}},
                spec_sources={},
                spec_gaps={"SPEC_GAP_VM_ERROR_MODEL_001": {"area": "bytecode_vm"}},
            ),
            [],
        )

        case_table["path"] = "crates/trust-runtime/tests/bytecode_container.rs"
        mutation["discovery_id"] = "DISC_" + "A" * 20
        failures = validate_catalog_intent(
            tests={case_table["id"]: case_table, mutation["id"]: mutation},
            matrix=mapped_matrix(),
            invariants={"VM_SEAM_VALID_001": {"area": "bytecode_vm"}},
            spec_sources={},
            spec_gaps={"SPEC_GAP_VM_ERROR_MODEL_001": {"area": "bytecode_vm"}},
        )
        self.assertTrue(any("case_table_artifact path must equal case_file" in item for item in failures))
        self.assertTrue(any("mutation_shard_runner forbids discovery_id" in item for item in failures))

    def test_rust_unit_exact_command_requires_fully_qualified_test_path(self) -> None:
        record = generated_record()
        record.update(
            discovery_source_kind="rust_unit_test",
            command="cargo test -p trust-lsp bare_test_name -- --exact",
        )
        failures = validate_catalog_intent(
            tests={record["id"]: record},
            matrix=mapped_matrix(),
            invariants={"VM_SEAM_VALID_001": {"area": "bytecode_vm"}},
            spec_sources={"SPEC_BYTECODE_FORMAT_001": {"area": "bytecode_vm"}},
            spec_gaps={"SPEC_GAP_VM_ERROR_MODEL_001": {"area": "bytecode_vm"}},
        )

        self.assertTrue(
            any("requires a fully qualified test path" in item for item in failures)
        )

        record["command"] = (
            "cargo test -p trust-lsp handlers::tests::module::bare_test_name -- --exact"
        )
        self.assertEqual(
            validate_catalog_intent(
                tests={record["id"]: record},
                matrix=mapped_matrix(),
                invariants={"VM_SEAM_VALID_001": {"area": "bytecode_vm"}},
                spec_sources={"SPEC_BYTECODE_FORMAT_001": {"area": "bytecode_vm"}},
                spec_gaps={"SPEC_GAP_VM_ERROR_MODEL_001": {"area": "bytecode_vm"}},
            ),
            [],
        )


def generated_record() -> dict:
    record = common_record("TEST_INVALID_MAGIC", "generated_test")
    record.update(
        discovery_id="DISC_88F921D24D3708CEF3E1",
        discovery_source_kind="rust_integration_test",
        path="crates/trust-runtime/tests/bytecode_container.rs",
        name="header_validation",
        malformed_input_class_ids=["bad_magic"],
        test_class="negative_malformed_input",
        oracle_ref="SPEC_BYTECODE_FORMAT_001",
    )
    return record


def common_record(record_id: str, subject_kind: str) -> dict:
    return {
        "schema_version": 2,
        "id": record_id,
        "subject_kind": subject_kind,
        "area": "bytecode_vm",
        "status": "mapped",
        "invariants": ["VM_SEAM_VALID_001"],
        "spec_gap_ref": "SPEC_GAP_VM_ERROR_MODEL_001",
        "expected_result": "A review-owned expected result.",
        "expected_failure_mode": "A review-owned failure symptom.",
        "evidence_destination": "target/gate-artifacts/verification/result.json",
        "suite_tiers": [],
    }


def mapped_matrix() -> dict:
    return {"areas": [{"id": "bytecode_vm", "status": "mapped"}]}


if __name__ == "__main__":
    unittest.main()
