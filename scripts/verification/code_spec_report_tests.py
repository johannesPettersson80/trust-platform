"""Canonical rendering and tamper tests for Phase 17 reports."""

from __future__ import annotations

import hashlib
import inspect
import json
import unittest
from pathlib import Path

from scripts.verification.code_spec_common import CodeFact, sha256_digest, stable_code_id
from scripts.verification.code_spec_report import (
    CodeSpecDenominator,
    CodeSpecProvenance,
    build_report_payload,
    canonical_json,
    render_markdown,
    validate_report_bytes,
)
from scripts.verification.test_catalog_json_schema import validate_json_schema_instance


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "verification/schemas/code-spec-coverage-report.schema.json"


def fact(
    symbol: str,
    *,
    area: str = "runtime_safety",
    risks: tuple[str, ...] = (),
) -> CodeFact:
    signature = f"fn {symbol}()"
    path = "crates/example/src/lib.rs"
    code_id = stable_code_id(
        source_kind="rust",
        path=path,
        qualified_symbol=f"example::{symbol}",
        item_kind="rust_function",
        signature=signature,
    )
    return CodeFact(
        code_id=code_id,
        source_kind="rust",
        path=path,
        qualified_symbol=f"example::{symbol}",
        item_kind="rust_function",
        signature=signature,
        signature_digest=sha256_digest(signature.encode()),
        body_digest=sha256_digest(f"{signature} {{}}".encode()),
        owner="trust-runtime" if area != "unmapped" else "unmapped",
        area=area,
        area_candidates=(
            ("control_security", "editor_safety") if area == "unmapped" else (area,)
        ),
        risk_signals=risks,
        line=1,
    )


class CodeSpecReportTests(unittest.TestCase):
    def test_report_contract_accepts_authoritative_cargo_bindings(self) -> None:
        self.assertIn(
            "cargo_bindings",
            inspect.signature(build_report_payload).parameters,
            "report must receive the Rust scanner's authoritative Cargo joins",
        )

    def test_report_contract_accepts_exact_risk_acknowledgement_ledger(self) -> None:
        self.assertIn(
            "risk_acknowledgements",
            inspect.signature(build_report_payload).parameters,
            "report must distinguish map references from exact ledger records",
        )

    def report(self) -> dict[str, object]:
        mapped = fact("mapped", risks=("host_time",))
        missing = fact("missing", area="unmapped")
        mapping = {
            "code_id": mapped.code_id,
            "disposition": "non_behavioral",
            "reviewed_body_digest": sha256_digest(b"old body"),
            "owner": mapped.owner,
            "area": mapped.area,
            "owner_ack": "RISK_ACK_" + mapped.code_id.removeprefix("CODE_"),
        }
        return build_report_payload(
            facts={mapped.code_id: mapped, missing.code_id: missing},
            mappings={mapped.code_id: mapping},
            failures=(
                f"{mapped.code_id}: stale_spec_review body digest changed",
                f"{missing.code_id}: missing_spec",
                f"{missing.code_id}: unmapped",
            ),
            provenance=CodeSpecProvenance(
                commit="a" * 40,
                timestamp="2026-07-21T00:00:00Z",
                platform="fixture",
                input_paths=("verification/code-spec-map.toml",),
                output_json="target/code-spec.json",
                output_markdown="target/code-spec.md",
            ),
            input_digest=sha256_digest(b"inputs"),
            debt_baseline={
                "entries": [
                    {
                        "code_id": missing.code_id,
                        "signature_digest": missing.signature_digest,
                        "body_digest": missing.body_digest,
                    }
                ]
            },
            cargo_bindings={mapped.code_id: ("example:example:lib",)},
            denominator=CodeSpecDenominator(
                source_counts={"rust_product": 1},
                cargo_target_counts={"rust_product": 1},
            ),
            inventory_bindings={mapped.code_id: ("gate:GATE_EXAMPLE",)},
            risk_acknowledgements={
                mapping["owner_ack"]: {"code_id": mapped.code_id}
            },
        )

    def test_payload_has_raw_rows_counts_boundaries_and_digest_rebinds(self) -> None:
        payload = self.report()
        self.assertEqual(2, len(payload["facts"]))
        self.assertTrue(all("disposition_ref" in row for row in payload["facts"]))
        self.assertEqual(2, payload["summary"]["facts_total"])
        self.assertEqual(1, payload["summary"]["missing_spec"])
        self.assertEqual(1, payload["summary"]["historical_debt"])
        self.assertEqual(0, payload["summary"]["new_missing_spec"])
        self.assertEqual(1, payload["summary"]["unmapped"])
        self.assertEqual(1, payload["summary"].get("risk_acknowledgement_refs"))
        self.assertEqual(1, payload["summary"].get("risk_acknowledgement_records"))
        self.assertEqual(1, len(payload["digest_rebinds"]))
        self.assertEqual("none", payload["boundaries"]["proof_kind"])
        self.assertEqual(
            {"mode": "report", "blocking_total": 0, "blocking_findings": []},
            payload.get("enforcement"),
        )
        self.assertEqual(
            [{"name": "example", "count": 1}], payload["summary"]["by_crate"]
        )
        self.assertEqual(
            [{"name": "example:example:lib", "count": 1}],
            payload["summary"]["by_cargo_target"],
        )
        self.assertEqual(
            [{"name": "rust_product", "count": 1}],
            payload["denominator"]["source_counts"],
        )
        mapped_row = next(
            row for row in payload["facts"] if row["code_id"] == payload["digest_rebinds"][0]["code_id"]
        )
        self.assertEqual(["gate:GATE_EXAMPLE"], mapped_row["inventory_bindings"])
        self.assertEqual(
            "RISK_ACK_" + mapped_row["code_id"].removeprefix("CODE_"),
            mapped_row.get("risk_acknowledgement"),
        )
        self.assertEqual("exact_record", mapped_row.get("risk_acknowledgement_status"))
        unmapped_row = next(row for row in payload["facts"] if row["area"] == "unmapped")
        self.assertEqual(
            ["control_security", "editor_safety"],
            unmapped_row["area_candidates"],
        )

    def test_canonical_json_markdown_round_trip_and_tamper_rejection(self) -> None:
        payload = self.report()
        rendered = canonical_json(payload).encode()
        digest = hashlib.sha256(rendered).hexdigest()
        markdown = render_markdown(payload, json_digest=digest)
        self.assertEqual([], validate_report_bytes(rendered, markdown))

        noncanonical = (json.dumps(payload) + "\n").encode()
        failures = validate_report_bytes(noncanonical, markdown)
        self.assertTrue(any("canonical JSON" in item for item in failures), failures)
        failures = validate_report_bytes(rendered, markdown + "tamper\n")
        self.assertTrue(any("Markdown" in item for item in failures), failures)

    def test_report_schema_is_closed_and_accepts_the_canonical_payload(self) -> None:
        schema = json.loads(SCHEMA_PATH.read_text())
        self.assertEqual(False, schema.get("additionalProperties"))
        self.assertIn("facts", schema.get("required", []))
        self.assertEqual([], validate_json_schema_instance(self.report(), schema))


if __name__ == "__main__":
    unittest.main()
