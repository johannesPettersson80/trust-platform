"""Focused tests for the Phase 17 code-to-spec mapping contract."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from scripts.verification.code_spec_common import (
    CODE_ID_PATTERN,
    DISPOSITIONS,
    NON_BEHAVIORAL_REASONS,
    RISK_SIGNALS,
    CodeFact,
    sha256_digest,
    stable_code_id,
)
from scripts.verification.code_spec_contract import validate_map_document
from scripts.verification.test_catalog_json_schema import validate_json_schema_instance


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "verification/schemas/code-spec-map.schema.json"


def fact(*, body: bytes = b"fn run() {}", line: int = 4) -> CodeFact:
    signature = "fn run()"
    return CodeFact(
        code_id=stable_code_id(
            source_kind="rust",
            path="crates/example/src/lib.rs",
            qualified_symbol="example::run",
            item_kind="rust_function",
            signature=signature,
        ),
        source_kind="rust",
        path="crates/example/src/lib.rs",
        qualified_symbol="example::run",
        item_kind="rust_function",
        signature=signature,
        signature_digest=sha256_digest(signature.encode()),
        body_digest=sha256_digest(body),
        owner="trust-runtime",
        area="runtime_safety",
        line=line,
    )


class CodeSpecIdentityTests(unittest.TestCase):
    def test_line_movement_keeps_identity_but_body_change_only_changes_body_digest(self) -> None:
        original = fact(line=4)
        moved = fact(line=90)
        changed = fact(body=b"fn run() { changed(); }")

        self.assertRegex(original.code_id, CODE_ID_PATTERN)
        self.assertEqual(original.code_id, moved.code_id)
        self.assertEqual(original.code_id, changed.code_id)
        self.assertEqual(original.signature_digest, changed.signature_digest)
        self.assertNotEqual(original.body_digest, changed.body_digest)

    def test_identity_changes_for_path_symbol_kind_or_signature(self) -> None:
        base = dict(
            source_kind="rust",
            path="crates/example/src/lib.rs",
            qualified_symbol="example::run",
            item_kind="rust_function",
            signature="fn run()",
        )
        identity = stable_code_id(**base)
        for field, value in (
            ("source_kind", "typescript"),
            ("path", "crates/example/src/moved.rs"),
            ("qualified_symbol", "example::renamed"),
            ("item_kind", "rust_method"),
            ("signature", "fn run(value: u32)"),
        ):
            changed = dict(base)
            changed[field] = value
            self.assertNotEqual(identity, stable_code_id(**changed), field)


class CodeSpecSchemaTests(unittest.TestCase):
    def test_schema_is_closed_and_pins_contract_vocabularies(self) -> None:
        schema = json.loads(SCHEMA_PATH.read_text())
        self.assertEqual(False, schema["additionalProperties"])
        self.assertEqual({"schema_version", "mappings"}, set(schema["required"]))
        record = schema["$defs"]["mapping"]
        self.assertEqual(False, record["additionalProperties"])
        self.assertEqual(sorted(DISPOSITIONS), sorted(record["properties"]["disposition"]["enum"]))
        self.assertEqual(
            sorted(NON_BEHAVIORAL_REASONS),
            sorted(record["properties"]["non_behavioral_reason"]["enum"]),
        )
        self.assertEqual(
            sorted(RISK_SIGNALS),
            sorted(record["properties"]["risk_signals"]["items"]["enum"]),
        )
        self.assertEqual(
            "^RISK_ACK_[0-9A-F]{64}$",
            record["properties"]["owner_ack"].get("pattern"),
        )

    def test_schema_rejects_unknown_fields_and_non_anchored_code_ids(self) -> None:
        schema = json.loads(SCHEMA_PATH.read_text())
        code_fact = fact()
        record = {
            "schema_version": 1,
            "code_id": code_fact.code_id,
            "source_kind": code_fact.source_kind,
            "path": code_fact.path,
            "qualified_symbol": code_fact.qualified_symbol,
            "item_kind": code_fact.item_kind,
            "signature_digest": code_fact.signature_digest,
            "reviewed_body_digest": code_fact.body_digest,
            "owner": code_fact.owner,
            "area": code_fact.area,
            "visibility": code_fact.visibility,
            "disposition": "non_behavioral",
            "risk_signals": [],
            "non_behavioral_reason": "mechanical_adapter",
            "rationale": "Fixture only.",
            "last_reviewed": "2026-07-21",
        }
        self.assertEqual([], validate_json_schema_instance({"schema_version": 1, "mappings": [record]}, schema))
        bad = dict(record, code_id=code_fact.code_id + "TRAILING", surprise=True)
        failures = validate_json_schema_instance({"schema_version": 1, "mappings": [bad]}, schema)
        self.assertTrue(any("pattern" in failure for failure in failures), failures)
        self.assertTrue(any("additional property surprise" in failure for failure in failures), failures)


class CodeSpecContractTests(unittest.TestCase):
    def test_omission_is_reported_as_missing_spec(self) -> None:
        code_fact = fact()
        failures = validate_map_document(
            {"schema_version": 1, "mappings": []},
            live_facts={code_fact.code_id: code_fact},
        )
        self.assertEqual([f"{code_fact.code_id}: missing_spec"], failures)

    def test_body_digest_drift_is_reported_as_stale_spec_review(self) -> None:
        code_fact = fact()
        mapping = {
            "schema_version": 1,
            "code_id": code_fact.code_id,
            "source_kind": code_fact.source_kind,
            "path": code_fact.path,
            "qualified_symbol": code_fact.qualified_symbol,
            "item_kind": code_fact.item_kind,
            "signature_digest": code_fact.signature_digest,
            "reviewed_body_digest": sha256_digest(b"old body"),
            "owner": code_fact.owner,
            "area": code_fact.area,
            "visibility": code_fact.visibility,
            "disposition": "non_behavioral",
            "risk_signals": [],
            "non_behavioral_reason": "mechanical_adapter",
            "rationale": "Fixture only.",
            "last_reviewed": "2026-07-21",
        }
        failures = validate_map_document(
            {"schema_version": 1, "mappings": [mapping]},
            live_facts={code_fact.code_id: code_fact},
        )
        self.assertIn(f"{code_fact.code_id}: stale_spec_review body digest changed", failures)


if __name__ == "__main__":
    unittest.main()
