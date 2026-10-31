"""The VS Code lockfile passes `npm audit` under the owned-exception policy (specification 24, "Dependency
Exceptions"). Each expected finding follows from the audit report and the exceptions each test writes; the reports have
the shape `npm audit --json` answers."""

from __future__ import annotations

import sys
import tempfile
import unittest
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import check_npm_audit as check  # noqa: E402

ReleaseEvidenceError = check.ReleaseEvidenceError

TODAY = date(2026, 10, 31)
MINIMATCH = "GHSA-g3hv-75wg-m249"
GLOB = "GHSA-3g3m-2fjr-6pw4"


def advisory(name: str, ghsa: str) -> dict:
    return {"source": 1, "name": name, "url": f"https://github.com/advisories/{ghsa}", "severity": "moderate"}


def report(*entries: tuple[str, list]) -> dict:
    return {"vulnerabilities": {name: {"name": name, "via": via} for name, via in entries}, "metadata": {}}


def exception(ghsa: str, review: str = "2026-10-31", expires: str = "2026-12-30") -> str:
    return (f'    {{ id = "{ghsa}", reason = "owner=vscode/extension; rationale=not reached; review={review}; '
            f'removal=when upstream takes the fix; expires={expires}" }},\n')


def root_with(*rows: str) -> tuple[tempfile.TemporaryDirectory, Path]:
    holder = tempfile.TemporaryDirectory()
    root = Path(holder.name)
    (root / "editors" / "vscode").mkdir(parents=True)
    (root / "editors" / "vscode" / "audit-exceptions.toml").write_text("[advisories]\nignore = [\n" + "".join(rows) + "]\n")
    return holder, root


class NpmAuditPolicyTests(unittest.TestCase):
    def test_an_advisory_without_an_owned_exception_fails(self) -> None:
        # Catches a gate that lets a new advisory through because another one is excepted.
        holder, root = root_with(exception(MINIMATCH))
        audit = report(("minimatch", [advisory("minimatch", MINIMATCH)]), ("glob", [advisory("glob", GLOB)]))
        with holder:
            unowned, stale, owned = check.findings(audit, root, TODAY)
        self.assertEqual(unowned, [f"{GLOB} (glob) has no owned exception in editors/vscode/audit-exceptions.toml"])
        self.assertEqual((stale, owned), ([], 1))

    def test_every_reported_advisory_owned_passes(self) -> None:
        # Catches a check that counts the packages a report names through `via` as advisories of their own.
        holder, root = root_with(exception(MINIMATCH), exception(GLOB))
        audit = report(
            ("minimatch", [advisory("minimatch", MINIMATCH)]),
            ("glob", [advisory("glob", GLOB), "minimatch"]),
            ("vscode-languageclient", ["minimatch"]),
        )
        with holder:
            self.assertEqual(check.findings(audit, root, TODAY), ([], [], 2))

    def test_an_expired_exception_fails(self) -> None:
        # Catches an exception that outlives its review: the policy expires it.
        holder, root = root_with(exception(MINIMATCH, expires="2026-10-30", review="2026-10-01"))
        with holder, self.assertRaisesRegex(ReleaseEvidenceError, "expired on 2026-10-30"):
            check.findings(report(("minimatch", [advisory("minimatch", MINIMATCH)])), root, TODAY)

    def test_an_exception_longer_than_90_days_fails(self) -> None:
        # Catches an exception written to last past the 90-day limit of specification 24.
        holder, root = root_with(exception(MINIMATCH, expires="2027-02-01"))
        with holder, self.assertRaisesRegex(ReleaseEvidenceError, "exceeds 90 days"):
            check.findings(report(), root, TODAY)

    def test_an_exception_without_its_removal_condition_fails(self) -> None:
        # Catches an unowned exception: every field of deny.toml's policy is required.
        row = f'    {{ id = "{MINIMATCH}", reason = "owner=vscode; rationale=x; review=2026-10-31; expires=2026-12-30" }},\n'
        holder, root = root_with(row)
        with holder, self.assertRaisesRegex(ReleaseEvidenceError, "missing fields: removal"):
            check.findings(report(), root, TODAY)

    def test_an_exception_needs_a_github_advisory_id(self) -> None:
        # Catches an npm exception keyed by a RustSec id, which no npm report can ever name.
        holder, root = root_with(exception("RUSTSEC-2026-0001"))
        with holder, self.assertRaisesRegex(ReleaseEvidenceError, "invalid advisory id"):
            check.findings(report(), root, TODAY)

    def test_an_exception_npm_no_longer_reports_is_named(self) -> None:
        # Catches an exception list that keeps entries after the fix arrives.
        holder, root = root_with(exception(MINIMATCH), exception(GLOB))
        with holder:
            unowned, stale, _ = check.findings(report(("glob", [advisory("glob", GLOB)])), root, TODAY)
        self.assertEqual(unowned, [])
        self.assertEqual(stale, [f"{MINIMATCH}: npm audit no longer reports it; take its exception off "
                                 "editors/vscode/audit-exceptions.toml"])

    def test_an_advisory_without_a_github_id_fails(self) -> None:
        # Catches a report whose advisory cannot be matched to any exception and would otherwise be skipped.
        holder, root = root_with()
        audit = report(("left-pad", [{"source": 7, "name": "left-pad", "url": "https://example.com/7"}]))
        with holder, self.assertRaisesRegex(ReleaseEvidenceError, "without a GitHub id"):
            check.findings(audit, root, TODAY)

    def test_the_repository_lists_valid_exceptions(self) -> None:
        # Catches a malformed or expired exception in the repository's own file.
        check.validate_file(ROOT / check.EXCEPTIONS, today=TODAY, id_pattern=check.GHSA_ID)


if __name__ == "__main__":
    unittest.main()
