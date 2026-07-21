"""Focused tests for the Phase 17 symbol-specific risk acknowledgement ledger."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from scripts.verification import code_spec_risk
from scripts.verification.code_spec_common import CodeFact, sha256_digest, stable_code_id
from scripts.verification.test_catalog_json_schema import validate_json_schema_instance


ROOT = Path(__file__).resolve().parents[2]


def risky_fact(symbol: str = "boundary") -> CodeFact:
    signature = f"fn {symbol}()"
    return CodeFact(
        code_id=stable_code_id(
            source_kind="rust",
            path="crates/example/src/lib.rs",
            qualified_symbol=f"example::{symbol}",
            item_kind="rust_function",
            signature=signature,
        ),
        source_kind="rust",
        path="crates/example/src/lib.rs",
        qualified_symbol=f"example::{symbol}",
        item_kind="rust_function",
        signature=signature,
        signature_digest=sha256_digest(signature.encode()),
        body_digest=sha256_digest(f"{signature} {{ now(); }}".encode()),
        owner="trust-runtime",
        area="runtime_safety",
        visibility="private",
        risk_signals=("host_time",),
        line=1,
    )


def mapping(fact: CodeFact) -> dict[str, object]:
    return {
        "code_id": fact.code_id,
        "owner_ack": code_spec_risk.risk_ack_id(fact.code_id),
        "owner_ack_expires_on": "2099-01-01",
    }


def acknowledgement(fact: CodeFact) -> dict[str, object]:
    return {
        "schema_version": 1,
        "id": code_spec_risk.risk_ack_id(fact.code_id),
        "code_id": fact.code_id,
        "owner": fact.owner,
        "area": fact.area,
        "body_digest": fact.body_digest,
        "risk_signals": list(fact.risk_signals),
        "review_ref": "docs/review.md#exact-symbol",
        "reviewed_by": fact.owner,
        "reviewed_on": "2026-07-21",
        "expires_on": "2099-01-01",
        "rationale": "The exact risk-bearing boundary is mapped to an explicit contract or gap.",
    }


class RiskAcknowledgementLedgerTests(unittest.TestCase):
    def test_closed_schema_accepts_exact_record_and_rejects_free_form_identity(self) -> None:
        fact = risky_fact()
        schema = json.loads(
            (
                ROOT
                / "verification/schemas/code-spec-risk-acknowledgements.schema.json"
            ).read_text(encoding="utf-8")
        )
        valid = {"schema_version": 1, "acknowledgements": [acknowledgement(fact)]}
        self.assertEqual([], validate_json_schema_instance(valid, schema))

        invalid = {"schema_version": 1, "acknowledgements": [acknowledgement(fact)]}
        invalid["acknowledgements"][0]["id"] = "reviewed in prose"
        failures = validate_json_schema_instance(invalid, schema)
        self.assertTrue(any("pattern" in item for item in failures), failures)

    def test_document_requires_exact_one_to_one_live_acknowledgements(self) -> None:
        fact = risky_fact()
        document = {"schema_version": 1, "acknowledgements": [acknowledgement(fact)]}
        validator = getattr(
            code_spec_risk,
            "validate_risk_ack_document",
            lambda *_args, **_kwargs: ["risk acknowledgement document validator is missing"],
        )

        failures = validator(
            document,
            live_facts={fact.code_id: fact},
            mappings={fact.code_id: mapping(fact)},
        )

        self.assertEqual([], failures)

    def test_document_rejects_duplicate_and_invented_acknowledgements(self) -> None:
        fact = risky_fact()
        invented = risky_fact("invented")
        duplicate = acknowledgement(fact)
        document = {
            "schema_version": 1,
            "acknowledgements": [duplicate, dict(duplicate), acknowledgement(invented)],
        }
        validator = getattr(
            code_spec_risk,
            "validate_risk_ack_document",
            lambda *_args, **_kwargs: [],
        )

        failures = validator(
            document,
            live_facts={fact.code_id: fact},
            mappings={fact.code_id: mapping(fact)},
        )

        self.assertTrue(any("duplicate risk acknowledgement" in item for item in failures), failures)
        self.assertTrue(any("invented or stale code identity" in item for item in failures), failures)


if __name__ == "__main__":
    unittest.main()
