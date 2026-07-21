"""Focused tests for the Phase 17 tooling denominator."""

from __future__ import annotations

import unittest

from scripts.verification.code_spec_common import CodeFact, sha256_digest, stable_code_id
from scripts.verification.code_spec_contract import validate_map_document
from scripts.verification.code_spec_gate import blocking_failures
from scripts.verification.code_spec_tooling import extract_python_source, extract_shell_source


class CodeSpecToolingTests(unittest.TestCase):
    def test_python_ast_extracts_qualified_functions_methods_and_entrypoint(self) -> None:
        source = b'''"""fixture"""\n\ndef run(value: str) -> int:\n    return len(value)\n\nclass Gate:\n    def check(self) -> bool:\n        return True\n\nif __name__ == "__main__":\n    raise SystemExit(run("x"))\n'''

        facts = extract_python_source("scripts::gate", source)

        self.assertEqual(3, len(facts))
        self.assertEqual(
            {
                ("scripts::gate::run", "python_function"),
                ("scripts::gate::Gate::check", "python_method"),
                ("scripts::gate::<entrypoint>", "python_entrypoint"),
            },
            {(fact["qualified_symbol"], fact["item_kind"]) for fact in facts},
        )
        self.assertTrue(all(0 <= fact["body_start"] < fact["body_end"] <= len(source) for fact in facts))

    def test_shell_entrypoint_owns_the_exact_file_bytes(self) -> None:
        source = b"#!/usr/bin/env bash\nset -euo pipefail\nprintf '%s\\n' ok\n"

        facts = extract_shell_source("scripts::gate", source)

        self.assertEqual(1, len(facts))
        self.assertEqual("shell_entrypoint", facts[0]["item_kind"])
        self.assertEqual(0, facts[0]["body_start"])
        self.assertEqual(len(source), facts[0]["body_end"])

    def test_python_risk_policy_uses_ast_calls_assignments_and_exception_shapes(self) -> None:
        source = b'''import os\nimport subprocess\nimport time\n\ndef boundary():\n    state.ready = True\n    now = time.time()\n    token = os.environ["TOKEN"]\n    subprocess.run(["check"])\n    authorize_role("operator")\n    persist_and_restart()\n    try:\n        request_status()\n    except Exception:\n        pass\n'''

        fact = next(
            fact
            for fact in extract_python_source("scripts::gate", source)
            if fact["qualified_symbol"].endswith("::boundary")
        )

        for expected in (
            "host_time",
            "filesystem_or_environment",
            "subprocess_or_thread",
            "state_mutation",
            "authorization",
            "protocol_response_or_status",
            "broad_error_suppression",
            "persistence_retain_reload_restart",
        ):
            self.assertIn(expected, fact["risk_signals"])

    def test_changed_mode_rejects_a_removed_production_catcher(self) -> None:
        original_source = b'''def enforce(findings):
    if findings:
        raise SystemExit(1)
    return 0
'''
        catcher_removed = b'''def enforce(findings):
    return 0
'''

        def scanned_fact(source: bytes) -> CodeFact:
            raw = extract_python_source("scripts::gate", source)[0]
            signature = str(raw["signature"])
            code_id = stable_code_id(
                source_kind="python",
                path="scripts/gate.py",
                qualified_symbol=str(raw["qualified_symbol"]),
                item_kind=str(raw["item_kind"]),
                signature=signature,
            )
            return CodeFact(
                code_id=code_id,
                source_kind="python",
                path="scripts/gate.py",
                qualified_symbol=str(raw["qualified_symbol"]),
                item_kind=str(raw["item_kind"]),
                signature=signature,
                signature_digest=sha256_digest(signature.encode()),
                body_digest=sha256_digest(
                    source[int(raw["body_start"]):int(raw["body_end"])]
                ),
                owner="verification",
                area="verification",
                visibility=str(raw["visibility"]),
                risk_signals=tuple(raw["risk_signals"]),
            )

        reviewed = scanned_fact(original_source)
        changed = scanned_fact(catcher_removed)
        self.assertEqual(reviewed.code_id, changed.code_id)
        mapping = {
            "schema_version": 1,
            "code_id": reviewed.code_id,
            "source_kind": reviewed.source_kind,
            "path": reviewed.path,
            "qualified_symbol": reviewed.qualified_symbol,
            "item_kind": reviewed.item_kind,
            "signature_digest": reviewed.signature_digest,
            "reviewed_body_digest": reviewed.body_digest,
            "owner": reviewed.owner,
            "area": reviewed.area,
            "visibility": reviewed.visibility,
            "risk_signals": [],
            "disposition": "non_behavioral",
            "non_behavioral_reason": "mechanical_adapter",
            "rationale": "Fixture mapping for the changed-mode catcher mutation.",
            "last_reviewed": "2026-07-21",
        }
        findings = validate_map_document(
            {"schema_version": 1, "mappings": [mapping]},
            live_facts={changed.code_id: changed},
        )

        self.assertEqual(
            [f"{changed.code_id}: stale_spec_review body digest changed"],
            blocking_failures("changed", findings, frozenset()),
        )


if __name__ == "__main__":
    unittest.main()
