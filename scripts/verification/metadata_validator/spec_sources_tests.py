"""Focused tests for the closed specification-source metadata contract."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path
from unittest import mock

from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.metadata_validator.core import Validator
from scripts.verification.metadata_validator.spec_sources import (
    EXTERNAL_REFERENCE_FIELDS,
    SPEC_SOURCE_FIELDS,
    TRACKED_FILE_FIELDS,
    validate_spec_source_records,
)


class SpecSourceMetadataTests(unittest.TestCase):
    def setUp(self) -> None:
        validator = Validator()
        validator.load_records()
        self.records = validator.spec_sources

    def test_committed_spec_sources_use_closed_v2_contract(self) -> None:
        failures: list[tuple[Path, str]] = []

        validate_spec_source_records(
            root=ROOT,
            records=self.records,
            fail=lambda path, message: failures.append((path, message)),
        )

        self.assertEqual([], failures)
        self.assertTrue(self.records)
        self.assertTrue(
            all(record["schema_version"] == 2 for record in self.records.values())
        )
        self.assertTrue(
            all(set(record) - {"_path"} <= SPEC_SOURCE_FIELDS for record in self.records.values())
        )

    def test_tracked_source_requires_one_tracked_nonsymlink_file(self) -> None:
        record = copy.deepcopy(self.records["SPEC_BYTECODE_FORMAT_001"])
        record.pop("path")
        failures = self._validate_one(record)
        self.assertIn("tracked_file requires path", failures)

        record = copy.deepcopy(self.records["SPEC_BYTECODE_FORMAT_001"])
        record["external_ref"] = "invented external locator"
        failures = self._validate_one(record)
        self.assertIn("tracked_file forbids external-reference fields", failures)

    def test_external_source_is_nonredistributable_and_not_oracle_eligible(self) -> None:
        record = copy.deepcopy(self.records["SPEC_IEC_61131_3_ED3_EXTERNAL_001"])
        self.assertEqual("external_reference", record["locator_kind"])
        self.assertTrue(EXTERNAL_REFERENCE_FIELDS <= set(record))
        self.assertNotIn("path", record)

        for field, value, signal in (
            ("oracle_eligible", True, "must set oracle_eligible = false"),
            ("redistributable", True, "must set redistributable = false"),
            ("absence_blocks_proof", False, "must set absence_blocks_proof = true"),
        ):
            with self.subTest(field=field):
                tampered = copy.deepcopy(record)
                tampered[field] = value
                self.assertIn(signal, self._validate_one(tampered))

    def test_external_source_requires_nonempty_reference_and_retrieval_text(self) -> None:
        record = copy.deepcopy(self.records["SPEC_IEC_61131_3_ED3_EXTERNAL_001"])

        for field in ("external_ref", "retrieval_expectation"):
            with self.subTest(field=field):
                tampered = copy.deepcopy(record)
                tampered[field] = ""
                self.assertIn(
                    f"{field} must be a non-empty string",
                    self._validate_one(tampered),
                )

    def test_external_local_path_is_expected_ignored_and_never_bound_as_input(self) -> None:
        record = copy.deepcopy(self.records["SPEC_IEC_61131_3_ED3_EXTERNAL_001"])
        self.assertEqual(
            "docs/internal/standards/iec61131-3.txt",
            record["expected_local_path"],
        )
        self.assertFalse((ROOT / record["expected_local_path"]).is_symlink())

        with mock.patch(
            "scripts.verification.metadata_validator.spec_sources._is_ignored_untracked_path",
            return_value=False,
        ):
            self.assertIn("must be gitignored and untracked", self._validate_one(record))

        record["expected_local_path"] = "docs/internal/standards//iec61131-3.txt"
        self.assertIn(
            "expected_local_path must be normalized and workspace-relative",
            self._validate_one(record),
        )

    def test_optional_workflow_fields_keep_closed_schema_types(self) -> None:
        record = copy.deepcopy(self.records["SPEC_BYTECODE_FORMAT_001"])
        record["actor"] = ["not", "a", "string"]
        record["preconditions"] = "not an array"

        failures = self._validate_one(record)
        self.assertIn("actor must be a non-empty string", failures)
        self.assertIn("preconditions must be a string array", failures)

    def test_closed_contract_rejects_unknown_fields_and_locator_mixing(self) -> None:
        tracked = copy.deepcopy(self.records["SPEC_BYTECODE_FORMAT_001"])
        tracked["invented_classification"] = "normative"
        self.assertIn("unexpected fields: invented_classification", self._validate_one(tracked))

        external = copy.deepcopy(self.records["SPEC_IEC_61131_3_ED3_EXTERNAL_001"])
        external["path"] = "docs/specs/01-lexical.md"
        self.assertIn("external_reference forbids path", self._validate_one(external))

    def test_public_claim_obligations_are_preserved(self) -> None:
        claim = copy.deepcopy(self.records["PUBLIC_CLAIM_RUNTIME_WIRE_001"])
        claim.pop("claim_text")
        self.assertIn(
            "public claim PUBLIC_CLAIM_RUNTIME_WIRE_001 missing claim_text",
            self._validate_one(claim),
        )

        claim = copy.deepcopy(self.records["PUBLIC_CLAIM_RUNTIME_WIRE_001"])
        claim["oracle_eligible"] = True
        self.assertIn(
            "public claim PUBLIC_CLAIM_RUNTIME_WIRE_001 must set oracle_eligible = false",
            self._validate_one(claim),
        )

    def test_public_claim_surface_ref_is_safe_public_and_fragment_bound(self) -> None:
        claim = copy.deepcopy(self.records["PUBLIC_CLAIM_SOURCE_BUILD_RUNTIME_001"])
        self.assertNotIn("surface_ref", self._validate_one(claim))

        readme_claim = copy.deepcopy(self.records["PUBLIC_CLAIM_RUNTIME_WIRE_001"])
        self.assertNotIn("surface_ref", self._validate_one(readme_claim))

        tampered = copy.deepcopy(claim)
        tampered["surface_ref"] = "../../etc/passwd#x"
        self.assertIn(
            "surface_ref path must be normalized and workspace-relative",
            self._validate_one(tampered),
        )

        tampered = copy.deepcopy(readme_claim)
        tampered["surface_ref"] = "README.md#stale-heading"
        self.assertIn(
            "surface_ref fragment does not identify a heading",
            self._validate_one(tampered),
        )

        tampered = copy.deepcopy(readme_claim)
        tampered["surface_ref"] = "docs/specs/12-bytecode.md#bytecode-format"
        self.assertIn(
            "surface_ref path is not a reviewed public surface",
            self._validate_one(tampered),
        )

    def test_public_claim_surface_ref_rejects_untracked_and_symlink_paths(self) -> None:
        claim = copy.deepcopy(self.records["PUBLIC_CLAIM_RUNTIME_WIRE_001"])
        with mock.patch(
            "scripts.verification.metadata_validator.spec_sources._is_tracked",
            return_value=False,
        ):
            self.assertIn(
                "surface_ref path is not tracked",
                self._validate_one(claim),
            )

        with mock.patch(
            "scripts.verification.metadata_validator.spec_sources._has_symlink_component",
            return_value=True,
        ):
            self.assertIn(
                "surface_ref path contains a symlink component",
                self._validate_one(claim),
            )

    def test_schema_is_closed_and_matches_python_field_contract(self) -> None:
        schema = json.loads((ROOT / "verification/schemas/spec-source.schema.json").read_text())

        self.assertEqual(2, schema["properties"]["schema_version"]["const"])
        self.assertFalse(schema["additionalProperties"])
        self.assertEqual(SPEC_SOURCE_FIELDS, set(schema["properties"]))
        self.assertEqual(TRACKED_FILE_FIELDS, set(schema["oneOf"][0]["required"]))
        self.assertEqual(EXTERNAL_REFERENCE_FIELDS, set(schema["oneOf"][1]["required"]))

    def _validate_one(self, record: dict[str, object]) -> str:
        failures: list[str] = []
        validate_spec_source_records(
            root=ROOT,
            records={str(record.get("id", "FIXTURE")): record},
            fail=lambda _path, message: failures.append(message),
        )
        return "\n".join(failures)


if __name__ == "__main__":
    unittest.main()
