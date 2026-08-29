"""Focused tests for Phase 14 governance."""

from __future__ import annotations

import copy
import re
import unittest
from datetime import date
from pathlib import Path

from scripts.verification.governance import (
    load_governance,
    load_retirements,
    validate_changed_files,
    validate_current_governance,
    validate_governance_document,
)
from scripts.verification.metadata_validator.core import Validator


class GovernanceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.validator = Validator()
        cls.validator.load_records()
        cls.document = load_governance(Path.cwd())
        cls.retirements = load_retirements(Path.cwd())

    def validate(self, document=None, retirements=None) -> list[str]:
        return validate_governance_document(
            document or self.document,
            invariants=self.validator.invariants,
            suites=self.validator.suites,
            matrix=self.validator.matrix,
            retirements=retirements or self.retirements,
            evidence=self.validator.evidence,
        )

    def test_live_governance_document_is_structurally_valid(self) -> None:
        self.assertEqual(self.validate(), [])

    def test_owner_alias_suite_and_template_drift_fail_closed(self) -> None:
        tampered = copy.deepcopy(self.document)
        tampered["area_owner_rules"][0]["owners"] = []
        self.assertIn("owner", "\n".join(self.validate(tampered)))

        tampered = copy.deepcopy(self.document)
        tampered["suite_composition"]["proof_inheritance"] = True
        self.assertIn("proof_inheritance", "\n".join(self.validate(tampered)))

        tampered = copy.deepcopy(self.document)
        bytecode = next(row for row in tampered["coverage_templates"] if row["area"] == "bytecode_vm")
        bytecode["required_dimensions"].remove("resource_limit")
        self.assertIn("does not match matrix", "\n".join(self.validate(tampered)))

    def test_product_changes_require_direct_specification_and_native_test_companions(self) -> None:
        direct_policy = copy.deepcopy(self.document)
        direct_policy["change_policy"]["product_change_requires"] = [
            "written_specification",
            "native_executable_test",
        ]
        direct_policy["change_policy"]["public_claim_change_requires"] = [
            "written_specification",
            "native_executable_test",
        ]
        product = ["crates/trust-runtime/src/runtime/cycle.rs"]
        self.assertEqual(
            validate_changed_files(direct_policy, product),
            [
                "behavior-changing production paths require a changed native executable test",
                "behavior-changing production paths require a changed written specification",
            ],
        )

        self.assertEqual(
            validate_changed_files(
                direct_policy,
                [*product, "docs/specs/11-runtime-engine.md"],
            ),
            ["behavior-changing production paths require a changed native executable test"],
        )
        self.assertEqual(
            validate_changed_files(
                direct_policy,
                [*product, "crates/trust-runtime/tests/runtime_cycle.rs"],
            ),
            ["behavior-changing production paths require a changed written specification"],
        )
        self.assertEqual(
            validate_changed_files(
                direct_policy,
                [
                    *product,
                    "docs/specs/11-runtime-engine.md",
                    "crates/trust-runtime/tests/runtime_cycle.rs",
                ],
            ),
            [],
        )

        public = ["docs/public/reference/conformance.md"]
        self.assertEqual(validate_changed_files(direct_policy, public), [])

        self.assertEqual(
            direct_policy["change_policy"]["product_change_requires"],
            ["written_specification", "native_executable_test"],
        )
        self.assertEqual(
            direct_policy["change_policy"]["public_claim_change_requires"],
            ["written_specification", "native_executable_test"],
        )

    def test_direct_change_contract_covers_every_executable_code_area(self) -> None:
        production_paths = [
            ".github/workflows/ci.yml",
            "Cargo.toml",
            "crates/trust-dev/src/main.rs",
            "editors/vscode/src/extension.tsx",
            "justfile",
            "scripts/release.ps1",
            "xtask/src/main.rs",
        ]

        for product_path in production_paths:
            with self.subTest(product_path=product_path):
                self.assertEqual(
                    validate_changed_files(self.document, [product_path]),
                    [
                        "behavior-changing production paths require a changed native executable test",
                        "behavior-changing production paths require a changed written specification",
                    ],
                )

    def test_mqtt_change_requires_the_mqtt_owning_specification(self) -> None:
        changed = [
            "crates/trust-runtime/src/io/mqtt/config.rs",
            "crates/trust-runtime/src/io/mqtt/tests/tag_mapping.rs",
            "docs/specs/11-runtime-engine.md",
        ]

        self.assertEqual(
            validate_changed_files(self.document, changed),
            [
                "MQTT production paths require the owning specification "
                "docs/specs/32-mqtt-io.md"
            ],
        )

        changed.append("docs/specs/32-mqtt-io.md")
        self.assertEqual(validate_changed_files(self.document, changed), [])

    def test_specification_index_is_complete_unique_and_ordered(self) -> None:
        specs = Path("docs/specs")
        expected = sorted(
            (
                path.name
                for path in specs.glob("[0-9][0-9]-*.md")
            ),
            key=lambda name: int(name.split("-", 1)[0]),
        ) + ["sfc-profile.md"]
        readme = (specs / "README.md").read_text(encoding="utf-8")
        indexed = re.findall(r"^\| \[([^]]+\.md)\]\([^)]*\) \|", readme, re.MULTILINE)

        self.assertEqual(indexed, expected)
        self.assertEqual(len(indexed), len(set(indexed)))

    def test_stale_metadata_unknown_grace_and_cadence_fail_closed(self) -> None:
        records = {"OLD": {"last_reviewed": "2026-01-01"}}
        failures = validate_current_governance(
            self.document,
            today=date(2026, 7, 21),
            record_groups=(("test", records),),
            ignored_tests={},
        )
        self.assertIn("is stale", "\n".join(failures))

        ignored = {
            "IGNORED": {
                "ignore_class": "unknown",
                "last_reviewed": "2026-06-01",
            }
        }
        failures = validate_current_governance(
            self.document,
            today=date(2026, 7, 21),
            record_groups=(),
            ignored_tests=ignored,
        )
        self.assertIn("unknown grace period", "\n".join(failures))

        failures = validate_current_governance(
            self.document,
            today=date(2026, 7, 21),
            record_groups=(),
            ignored_tests={},
            invariants={
                "INV": {
                    "risk": "safety_critical",
                    "oracle": {"ref": "MISSING"},
                    "last_reviewed": "2026-06-01",
                }
            },
            spec_sources={},
        )
        self.assertIn("missing-oracle grace period", "\n".join(failures))

        failures = validate_current_governance(
            self.document,
            today=date(2026, 11, 1),
            record_groups=(),
            ignored_tests={},
        )
        self.assertIn("review cadence", "\n".join(failures))

    def test_retirement_is_append_only_and_evidence_bound(self) -> None:
        registry = {
            "retirements": [
                {
                    "kind": "invariant",
                    "id": "MISSING",
                    "owner": "verification",
                    "rationale": "obsolete",
                    "replacement": "none",
                    "retired_at": "2026-07-19",
                    "evidence_refs": ["MISSING_EVIDENCE"],
                }
            ]
        }
        joined = "\n".join(self.validate(retirements=registry))
        self.assertIn("does not retain its source record", joined)
        self.assertIn("unknown evidence", joined)

    def _record_groups(self):
        return (
            ("spec source", self.validator.spec_sources),
            ("spec gap", self.validator.spec_gaps),
            ("test", self.validator.tests),
            ("ignored test", self.validator.ignored_tests),
            ("risk", self.validator.risks),
            ("invariant", self.validator.invariants),
            ("suite", self.validator.suites),
        )


if __name__ == "__main__":
    unittest.main()
