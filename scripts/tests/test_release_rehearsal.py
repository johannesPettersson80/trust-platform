from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[2]
RELEASE = ROOT / ".github" / "workflows" / "release.yml"
TAG_REF = "startsWith(github.ref, 'refs/tags/v')"


def jobs(workflow: str) -> dict[str, str]:
    """Each job's text by its id; CI's Python has no YAML parser, and the file keeps its indentation."""
    body = workflow.split("\njobs:\n", 1)[1]
    parts = re.split(r"(?m)^  ([a-z0-9_-]+):\n", body)
    return dict(zip(parts[1::2], parts[2::2]))


def steps(job: str) -> dict[str, str]:
    """Each step's text by its name, its first line indented as the others."""
    found = {}
    for step in re.split(r"(?m)^      - ", job)[1:]:
        text = "        " + step
        name = re.search(r"(?m)^        name: (.+)$", text)
        found[name.group(1) if name else text.split("\n", 1)[0].strip()] = text
    return found


def field(step: str, key: str) -> str:
    match = re.search(rf"(?m)^        {key}: (.+)$", step)
    return match.group(1) if match else ""


class ReleaseRehearsalTests(unittest.TestCase):
    """The Release workflow rehearses a candidate before its tag (release skill)."""

    def setUp(self) -> None:
        self.text = RELEASE.read_text()
        self.jobs = jobs(self.text)

    def test_a_candidate_can_rehearse_the_release_and_a_tag_releases(self) -> None:
        triggers = self.text.split("\non:\n", 1)[1].split("\n\n", 1)[0]
        self.assertRegex(triggers, r"(?m)^  push:\n    tags:\n      - 'v\*'$")
        self.assertRegex(triggers, r"(?m)^  workflow_dispatch:$")

    def test_only_a_tag_publishes(self) -> None:
        # Catches a rehearsal that publishes to the Marketplace or creates a GitHub release: each
        # step that publishes runs for a tag ref only.
        publishing = [
            (job_id, name, step)
            for job_id, job in self.jobs.items()
            for name, step in steps(job).items()
            if "vsce publish" in step or "uses: softprops/action-gh-release" in step
        ]
        self.assertEqual(
            [name for _, name, _ in publishing], ["Publish to VS Code Marketplace", "Create GitHub Release"]
        )
        for job_id, name, step in publishing:
            self.assertIn(TAG_REF, field(step, "if"), f"{job_id}: {name}")

    def test_the_tag_check_runs_for_a_tag_only(self) -> None:
        # A rehearsal's commit is not on main yet; the tag check would refuse it.
        check = steps(self.jobs["release-preflight"])["Validate release tag"]
        self.assertEqual(field(check, "if"), "${{ " + TAG_REF + " }}")

    def test_the_release_is_named_by_the_preflights_tag(self) -> None:
        # Catches a job that names the release by the pushed ref, which for a rehearsal is a
        # branch: only the preflight reads the ref's name, and the Publish job takes its tag.
        for job_id, job in self.jobs.items():
            if job_id != "release-preflight":
                self.assertNotIn("github.ref_name", job, job_id)
                self.assertNotIn("GITHUB_REF_NAME", job, job_id)
        self.assertIn("    outputs:\n      tag: ${{ steps.tag.outputs.tag }}\n", self.jobs["release-preflight"])
        self.assertIn("      RELEASE_TAG: ${{ needs.release-preflight.outputs.tag }}\n", self.jobs["publish"])

    def test_a_rehearsal_keeps_its_release_text_and_evidence(self) -> None:
        keep = steps(self.jobs["publish"])["Keep the rehearsal's release files"]
        self.assertEqual(field(keep, "if"), "${{ !" + TAG_REF + " }}")
        self.assertIn("          path: |\n            release-notes.md\n            release-evidence/\n", keep)


if __name__ == "__main__":
    unittest.main()
