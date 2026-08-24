"""Focused tests for fail-closed oracle and behavior references."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.metadata_validator.oracle_refs import (
    ERROR_MODEL_TAG,
    validate_error_code_ref,
    validate_oracle_ref,
    validate_partition_contract,
)


class OracleReferenceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.path = Path("verification/test-catalog.toml")

    def test_oracle_ref_rejects_unknown_and_ineligible_authority(self) -> None:
        sources = {
            "SPEC_INACTIVE_001": self._source(source_status="stale"),
            "SPEC_PROVENANCE_001": self._source(oracle_eligible=False),
            "SPEC_UNREVIEWED_001": self._source(authority="public_claim"),
        }

        self.assertIn(
            "references unknown spec source 'SPEC_UNKNOWN_001'",
            self._oracle_failures("SPEC_UNKNOWN_001#contract", sources),
        )
        self.assertIn(
            "references non-active spec source 'SPEC_INACTIVE_001'",
            self._oracle_failures("SPEC_INACTIVE_001#contract", sources),
        )
        self.assertIn(
            "references provenance-only spec source 'SPEC_PROVENANCE_001'",
            self._oracle_failures("SPEC_PROVENANCE_001#contract", sources),
        )
        self.assertIn(
            "cannot use authority 'public_claim'",
            self._oracle_failures("SPEC_UNREVIEWED_001#contract", sources),
        )

    def test_error_code_requires_active_error_model_authority(self) -> None:
        failures: list[str] = []

        validate_error_code_ref(
            fail=lambda _path, message: failures.append(message),
            path=self.path,
            owner_id="INVARIANT_EXAMPLE_001",
            behavior={"error_code": "E_EXAMPLE"},
            spec_sources={
                "SPEC_STALE_ERROR_001": self._source(
                    source_status="stale", covers=[ERROR_MODEL_TAG]
                )
            },
        )

        self.assertIn(
            f"behavior error_code requires an active {ERROR_MODEL_TAG} spec source",
            "\n".join(failures),
        )

    def test_partition_rejects_unsupported_shape(self) -> None:
        failures = self._partition_failures(
            {"partition": {"min": 0, "equals": "ZERO"}}
        )

        self.assertIn(
            "behavior partition has unsupported key set ['equals', 'min']",
            failures,
        )

    def test_equals_partition_rejects_nonopaque_label(self) -> None:
        failures = self._partition_failures(
            {
                "partition": {"equals": "mixed-case"},
                "case_family": "happy_path",
            }
        )

        self.assertIn(
            "partition.equals must be an opaque UPPER_CASE_LABEL",
            failures,
        )

    def _oracle_failures(
        self, oracle_ref: str, spec_sources: dict[str, dict[str, object]]
    ) -> str:
        failures: list[str] = []
        validate_oracle_ref(
            fail=lambda _path, message: failures.append(message),
            path=self.path,
            owner_id="TEST_EXAMPLE_001",
            oracle_ref=oracle_ref,
            spec_sources=spec_sources,
        )
        return "\n".join(failures)

    def _partition_failures(self, behavior: dict[str, object]) -> str:
        failures: list[str] = []
        validate_partition_contract(
            fail=lambda _path, message: failures.append(message),
            path=self.path,
            owner_id="INVARIANT_EXAMPLE_001",
            behavior=behavior,
        )
        return "\n".join(failures)

    @staticmethod
    def _source(**overrides: object) -> dict[str, object]:
        source: dict[str, object] = {
            "source_status": "active",
            "oracle_eligible": True,
            "authority": "reviewed_decision",
            "covers": [],
        }
        source.update(overrides)
        return source


if __name__ == "__main__":
    unittest.main()
