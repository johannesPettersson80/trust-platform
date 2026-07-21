"""Focused tests for the Phase 17 tooling denominator."""

from __future__ import annotations

import unittest

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


if __name__ == "__main__":
    unittest.main()
