from __future__ import annotations

import unittest

from scripts.verification.wasm_analysis_assignments import (
    JSON_RESULTS,
    MALFORMED_JSON,
    TEST_BINDINGS,
    _behavior_for_assertion,
    _disposition,
)


class WasmAnalysisAssignmentsTests(unittest.TestCase):
    def test_functional_batch_is_exactly_fifteen_tests(self) -> None:
        self.assertEqual(15, len(TEST_BINDINGS))
        names = {name for _path, name in TEST_BINDINGS}
        self.assertNotIn(
            "browser_analysis_latency_budget_against_native_is_within_spike_limits",
            names,
        )
        self.assertNotIn(
            "representative_corpus_memory_budget_gate",
            names,
        )

    def test_only_plain_expect_is_setup(self) -> None:
        self.assertEqual(
            "setup_or_precondition",
            _disposition("method:expect"),
        )
        for kind in (
            "method:expect_err",
            "macro:assert",
            "macro:assert_eq",
            "macro:assert_ne",
        ):
            self.assertEqual("oracle_bound", _disposition(kind))

    def test_malformed_json_assertions_have_narrow_behavior(self) -> None:
        self.assertEqual(
            MALFORMED_JSON,
            _behavior_for_assertion(
                name="wasm_json_adapter_contract_is_stable",
                default=JSON_RESULTS,
                kind="method:expect_err",
                expression="engine.apply_documents_json(...).expect_err(...)",
            ),
        )
        self.assertEqual(
            MALFORMED_JSON,
            _behavior_for_assertion(
                name="wasm_json_adapter_contract_is_stable",
                default=JSON_RESULTS,
                kind="macro:assert",
                expression='bad_json.contains("invalid documents json")',
            ),
        )
        self.assertEqual(
            JSON_RESULTS,
            _behavior_for_assertion(
                name="wasm_json_adapter_contract_is_stable",
                default=JSON_RESULTS,
                kind="macro:assert_eq",
                expression="status.document_count == 1",
            ),
        )

    def test_unknown_assertion_kind_fails_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            _disposition("method:unwrap_or_default")


if __name__ == "__main__":
    unittest.main()
