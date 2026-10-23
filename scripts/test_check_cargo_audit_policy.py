#!/usr/bin/env python3
import copy
import unittest

import check_cargo_audit_policy as policy


def allowed_entry() -> dict[str, object]:
    return {
        "name": "spin",
        "version": "0.9.8",
        "source": "registry+https://github.com/rust-lang/crates.io-index",
        "checksum": "6980e8d7511241f8acf4aebddbb1ff938df5eebe98691418c4468d0b72a96a67",
        "owner": "runtime/communications",
        "rationale": "flume 0.11.1 pins this exact release",
        "review_date": "2026-07-15",
        "removal_condition": "remove when no locked dependency uses flume 0.11.1",
    }


def audit_report() -> dict[str, object]:
    entry = allowed_entry()
    return {
        "vulnerabilities": {"found": False, "count": 0, "list": []},
        "warnings": {
            "yanked": [
                {
                    "kind": "yanked",
                    "package": {
                        key: entry[key]
                        for key in ("name", "version", "source", "checksum")
                    },
                }
            ]
        },
    }


class CargoAuditPolicyTests(unittest.TestCase):
    def test_exact_yanked_exception_passes(self) -> None:
        errors = policy.validate_report(audit_report(), [allowed_entry()])
        self.assertEqual(errors, [])

    def test_unexpected_yanked_package_fails(self) -> None:
        report = audit_report()
        report["warnings"]["yanked"][0]["package"]["version"] = "0.9.9"
        errors = policy.validate_report(report, [allowed_entry()])
        self.assertTrue(any("unexpected yanked package" in error for error in errors))

    def test_stale_exception_fails(self) -> None:
        report = audit_report()
        report["warnings"] = {}
        errors = policy.validate_report(report, [allowed_entry()])
        self.assertTrue(any("stale yanked exception" in error for error in errors))

    def test_other_warning_class_fails(self) -> None:
        report = audit_report()
        report["warnings"]["unmaintained"] = [
            {"kind": "unmaintained", "package": {"name": "old", "version": "1.0.0"}}
        ]
        errors = policy.validate_report(report, [allowed_entry()])
        self.assertTrue(any("unsupported cargo-audit warning" in error for error in errors))

    def test_unsound_warning_is_informational(self) -> None:
        report = audit_report()
        report["warnings"]["unsound"] = [{"kind": "unsound", "id": "RUSTSEC-0000-0000"}]
        errors = policy.validate_report(report, [allowed_entry()])
        self.assertEqual(errors, [])

    def test_vulnerability_fails(self) -> None:
        report = audit_report()
        report["vulnerabilities"] = {"found": True, "count": 1, "list": [{}]}
        errors = policy.validate_report(report, [allowed_entry()])
        self.assertTrue(any("vulnerabilities" in error for error in errors))

    def test_incomplete_allowlist_metadata_fails(self) -> None:
        entry = copy.deepcopy(allowed_entry())
        entry["removal_condition"] = ""
        with self.assertRaisesRegex(ValueError, "removal_condition"):
            policy.validate_allowlist({"schema_version": 1, "yanked": [entry]})


if __name__ == "__main__":
    unittest.main()
