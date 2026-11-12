"""In a job that checks a pull request or a release, an upload of evidence never fails a job whose checks passed, and a
failed upload is tried once more; an upload that a later job or workflow reads still fails its job (specification 22,
"CI reliability and release-gate artifact contract").

A runner's DNS failure (`ENOTFOUND`) failed a macOS Test job after all its tests had passed (G107). On 11 November 2026
GitHub's artifact service failed the VS Code Extension job's log upload after all its tests had passed, which skipped
the job's gate marker and so failed the Release Gate Report too (G146). Wrong behaviour caught: an evidence upload
without `continue-on-error` or without a second attempt; a second attempt that runs after a successful first, fails the
job, or uploads other files; and an upload that a later job needs made unable to fail its job."""

from __future__ import annotations

import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / ".github" / "workflows"

# The artifacts that a later job or workflow reads, or that must exist, by the name their upload gives them: a failed
# upload of one fails its job (specification 22).
NEEDED = (
    "gate-*",  # the markers that the Release Gate Report requires
    "firmware-${{ matrix.chip }}",  # the image that Chip Bench flashes
    "vsix-${{ matrix.platform }}",  # the packages and archives that the release files take
    "runtime-${{ matrix.platform }}",
    "trust-lsp-${{ matrix.platform }}",
    "toolchain-${{ matrix.platform }}",  # the bundle that each build and the release files take
    "release-conformance",  # the release files' conformance status
    "release-rehearsal-${{ needs.plan.outputs.tag }}",  # the rehearsal's release text and evidence (specification 24)
    "${{ steps.bundle.outputs.name }}",  # the release bundle that a promotion publishes
    "docs-captures",  # the screenshots that Docs Pages lays over the site's images
    "openot-real-databases-${{ inputs.ref || github.sha }}",  # the real-database matrix's evidence, which must exist
)


def load(path: Path) -> dict:
    return yaml.load(path.read_text(encoding="utf-8"), Loader=yaml.BaseLoader)


def needed(artifact: str) -> bool:
    return any(
        artifact.startswith(entry[:-1]) if entry.endswith("*") else artifact == entry for entry in NEEDED
    )


def is_upload(step: dict) -> bool:
    return str(step.get("uses", "")).startswith("actions/upload-artifact@")


def checking_jobs() -> dict[str, list[dict]]:
    """The steps of every job that checks a pull request or a release, by `<workflow>#<job>`: the jobs of each workflow
    that a pull request starts and of each workflow that one of those calls, except a job that only a schedule or a
    manual run starts."""
    workflows = {path.name: load(path) for path in sorted(WORKFLOWS.glob("*.yml"))}
    names = [name for name, workflow in workflows.items() if "pull_request" in workflow.get("on", {})]
    for name in list(names):
        for job in workflows[name].get("jobs", {}).values():
            called = str(job.get("uses", ""))
            if called.startswith("./.github/workflows/"):
                names.append(called.rsplit("/", 1)[1])
    jobs = {}
    for name in names:
        for job_id, job in workflows[name].get("jobs", {}).items():
            condition = str(job.get("if", ""))
            if "'schedule'" in condition and "'pull_request'" not in condition:
                continue
            jobs[f"{name}#{job_id}"] = job.get("steps", [])
    return jobs


def upload_problems(steps: list[dict]) -> list[str]:
    """How one job's uploads break the rule, empty when they keep it."""
    problems = []
    for index, step in enumerate(steps):
        if not is_upload(step):
            continue
        settings = step.get("with", {})
        artifact = settings.get("name", "")
        label = step.get("name") or artifact
        fails_job = step.get("continue-on-error") != "true"
        if needed(artifact):
            if not fails_job:
                problems.append(f"{label}: a later job needs '{artifact}', so a failed upload must fail the job")
            continue
        if settings.get("overwrite") == "true":
            continue  # a second attempt, checked with its first
        if fails_job:
            problems.append(f"{label}: a failed upload of evidence fails the job")
        if "always()" not in str(step.get("if", "")):
            problems.append(f"{label}: the evidence is not uploaded after a failed check")
        retries = [
            later for later in steps[index + 1 :] if is_upload(later) and later.get("with", {}).get("name") == artifact
        ]
        if "id" not in step or len(retries) != 1:
            problems.append(f"{label}: a failed upload is not tried once more")
            continue
        retry = retries[0]
        retry_settings = dict(retry.get("with", {}))
        condition = str(retry.get("if", ""))
        if "always()" not in condition or f"steps.{step['id']}.outcome == 'failure'" not in condition:
            problems.append(f"{label}: the second attempt does not run exactly when the first failed")
        if retry.get("continue-on-error") != "true":
            problems.append(f"{label}: a failed second attempt fails the job")
        if retry_settings.pop("overwrite", None) != "true" or retry_settings != settings:
            problems.append(f"{label}: the second attempt does not upload the same files over the first")
    return problems


class EvidenceUploadTest(unittest.TestCase):
    def test_every_evidence_upload_is_tried_twice_and_never_fails_its_job(self) -> None:
        for job, steps in checking_jobs().items():
            with self.subTest(job=job):
                self.assertEqual(upload_problems(steps), [])

    def test_the_rule_covers_every_job_that_checks_a_pull_request_or_a_release(self) -> None:
        # Catches the scope losing a workflow: the jobs whose upload failed them, a workflow that the Release workflow
        # calls, and the scheduled jobs that decide no merge or release.
        jobs = checking_jobs()
        for job in (
            "ci.yml#test",
            "ci.yml#vscode-extension",
            "ci.yml#release-gate-report",
            "salsa-hardening.yml#fuzz-smoke",
            "verification-gate.yml#verification-report",
            "release.yml#acceptance",
            "release.yml#rehearse",
            "openot-real-databases.yml#exact-product-matrix",
            "docs-captures.yml#refresh",
        ):
            self.assertIn(job, jobs)
        self.assertNotIn("salsa-hardening.yml#fuzz-extended-nightly", jobs)

    def test_the_check_finds_the_uploads_that_failed_jobs_whose_checks_passed(self) -> None:
        # The rule's own red: the VS Code Extension job's log upload as main had it before G146, and the test log's
        # before G107.
        vscode_before = yaml.load(
            """
- name: Upload VS Code integration test log
  if: ${{ always() }}
  uses: actions/upload-artifact@v7
  with:
    name: vscode-extension-test-log
    path: gate-artifacts/vscode-extension-test.log
- name: Mark gate success
  if: ${{ success() }}
  run: echo ok
""",
            Loader=yaml.BaseLoader,
        )
        self.assertEqual(
            upload_problems(vscode_before),
            [
                "Upload VS Code integration test log: a failed upload of evidence fails the job",
                "Upload VS Code integration test log: a failed upload is not tried once more",
            ],
        )

    def test_the_check_finds_a_second_attempt_that_differs_from_the_first(self) -> None:
        steps = yaml.load(
            """
- name: Upload the log
  id: log
  if: ${{ always() }}
  continue-on-error: true
  uses: actions/upload-artifact@v7
  with:
    name: the-log
    path: gate-artifacts/a.log
- name: Upload the log (second attempt)
  if: ${{ steps.log.outcome == 'failure' }}
  uses: actions/upload-artifact@v7
  with:
    name: the-log
    overwrite: true
    path: gate-artifacts/b.log
""",
            Loader=yaml.BaseLoader,
        )
        self.assertEqual(
            upload_problems(steps),
            [
                "Upload the log: the second attempt does not run exactly when the first failed",
                "Upload the log: a failed second attempt fails the job",
                "Upload the log: the second attempt does not upload the same files over the first",
            ],
        )

    def test_the_check_finds_a_needed_upload_that_cannot_fail_its_job(self) -> None:
        steps = yaml.load(
            """
- name: Upload gate marker
  continue-on-error: true
  uses: actions/upload-artifact@v7
  with:
    name: gate-clippy
    path: gate-artifacts/clippy.txt
""",
            Loader=yaml.BaseLoader,
        )
        self.assertEqual(
            upload_problems(steps),
            ["Upload gate marker: a later job needs 'gate-clippy', so a failed upload must fail the job"],
        )


if __name__ == "__main__":
    unittest.main()
