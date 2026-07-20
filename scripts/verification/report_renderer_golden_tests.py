"""Independent golden test for the generated catalog Markdown renderer."""

from __future__ import annotations

import hashlib
import unittest
from pathlib import Path

from scripts.verification.test_catalog_models import (
    GeneratedTestCatalog,
    InferredTestFact,
    ReportProvenance,
)
from scripts.verification.test_catalog_validation import validate_report_payload


ROOT = Path(__file__).resolve().parents[2]
GOLDEN = ROOT / "verification/selftests/test-catalog-renderer.golden.md"


class ReportRendererGoldenTests(unittest.TestCase):
    def test_generated_catalog_markdown_matches_independent_golden(self) -> None:
        report = fixture_report()
        digest = "sha256:" + hashlib.sha256(report.to_json().encode()).hexdigest()

        rendered = report.to_markdown(json_digest=digest)

        self.assertEqual(rendered, GOLDEN.read_text())

    def test_semantic_payload_tamper_is_rejected(self) -> None:
        payload = fixture_report().to_dict()
        payload["summary"]["records"] = 99

        failures = validate_report_payload(payload)

        self.assertTrue(any("summary.records" in failure for failure in failures), failures)


def fixture_report() -> GeneratedTestCatalog:
    fact = InferredTestFact(
        stable_id="DISC_0123456789ABCDEF0123",
        native_id="fixture::known_good",
        source_kind="rust_unit_test",
        name="known_good",
        path="crates/fixture/src/lib.rs",
        line=7,
        package="fixture",
        command_hint="cargo test -p fixture --lib known_good",
        command_hint_authority="exact",
        discovery_confidence="exact_attribute",
        ignore_state="not_ignored",
        ignore_reason=None,
        reference_candidates=("VERIF-P6A-007",),
    )
    return GeneratedTestCatalog(
        provenance=ReportProvenance(
            command=("python3", "scripts/generate_existing_test_catalog.py"),
            commit="0123456789abcdef0123456789abcdef01234567",
            timestamp="2026-07-11T00:00:00+02:00",
            platform="fixture-platform",
            input_paths=("crates/fixture/src/lib.rs",),
            output_json="target/gate-artifacts/verification/fixture.json",
            output_markdown="target/gate-artifacts/verification/fixture.md",
        ),
        input_digest="sha256:" + "1" * 64,
        inferred_facts=(fact,),
        diagnostics=(),
        limitations=("Fixture limitation.",),
    )


if __name__ == "__main__":
    unittest.main()
