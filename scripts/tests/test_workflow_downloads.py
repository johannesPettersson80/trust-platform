"""Every download in the workflows is tried again on a transient failure (G106, W47; specification 22, "Pull-request checks
and change detection" and "CI reliability"). GitHub's release server answered 504 to the actionlint download once and
failed a Format job that had no fault of its own, and GitHub answered 403 to a clone of the OpenOT reference once and
failed `main`'s Documentation job. Wrong behaviour caught: a `curl` in a workflow without `--retry`, and a clone of the
OpenOT reference that is tried once. GitHub refused three anonymous clones in a row on 22 November 2026 (G281), so on
a runner the clone carries the job's token: caught too, a checkout step without the token and a token that reaches the
log."""

import base64
import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / ".github" / "workflows"
CHECKOUT_OPENOT = ROOT / "scripts" / "checkout_openot_ref.sh"

# A `git` that fails its first `$STUB_FAILURES` clones as GitHub's 403 did, and otherwise clones a project with its
# manifest into the folder it is given; each clone is counted in `$STUB_STATE/clones`.
STUB_GIT = """#!/usr/bin/env bash
if [ "$1" = clone ]; then
  env | grep '^GIT_CONFIG_' | sort > "$STUB_STATE/git-config"
  count=$(( $(cat "$STUB_STATE/clones" 2>/dev/null || echo 0) + 1 ))
  echo "$count" > "$STUB_STATE/clones"
  if [ "$count" -le "$STUB_FAILURES" ]; then
    echo "fatal: unable to access the repository: The requested URL returned error: 403" >&2
    exit 128
  fi
  folder="${@: -1}"
  mkdir -p "$folder"
  : > "$folder/Cargo.toml"
fi
exit 0
"""


def curl_lines_without_retry() -> list[str]:
    """Each line of a workflow that runs `curl` without `--retry`, as `file:line: text`."""
    offenders = []
    for workflow in sorted(WORKFLOWS.glob("*.yml")):
        for number, line in enumerate(workflow.read_text(encoding="utf-8").splitlines(), start=1):
            text = line.strip()
            if text.startswith("#") or not re.search(r"(^|[\s;&|(])curl\s", text):
                continue
            if "--retry" not in text:
                offenders.append(f"{workflow.name}:{number}: {text}")
    return offenders


def checkout_openot(
    folder: Path, failures: int, token: str | None = None
) -> tuple[subprocess.CompletedProcess[str], int]:
    """The OpenOT checkout of `folder/open-ot-ref` with a `git` whose first `failures` clones fail and a `sleep` that
    returns at once, with `token` as the job's token or none; the run and the number of clones it made."""
    stubs = folder / "stubs"
    stubs.mkdir()
    for name, text in (("git", STUB_GIT), ("sleep", "#!/usr/bin/env bash\nexit 0\n")):
        (stubs / name).write_text(text, encoding="utf-8")
        (stubs / name).chmod(0o755)
    inherited = {
        name: value
        for name, value in os.environ.items()
        if name != "GITHUB_TOKEN" and not name.startswith("GIT_CONFIG_")
    }
    environment = {
        **inherited,
        **({"GITHUB_TOKEN": token} if token else {}),
        "PATH": f"{stubs}{os.pathsep}{os.environ['PATH']}",
        "STUB_STATE": str(folder),
        "STUB_FAILURES": str(failures),
        "OPEN_OT_REF_DIR": str(folder / "open-ot-ref"),
    }
    result = subprocess.run(
        ["bash", str(CHECKOUT_OPENOT)], env=environment, capture_output=True, text=True, check=False
    )
    clones = int((folder / "clones").read_text(encoding="utf-8")) if (folder / "clones").exists() else 0
    return result, clones


class WorkflowDownloadsTest(unittest.TestCase):
    def test_every_curl_in_the_workflows_retries_a_transient_failure(self) -> None:
        self.assertEqual(curl_lines_without_retry(), [])

    def test_the_check_finds_a_curl_without_retry(self) -> None:
        # The rule's own red: a download written as main wrote the actionlint download before G106.
        line = 'curl -fsSL -o "${RUNNER_TEMP}/${archive}" \\'
        self.assertIsNotNone(re.search(r"(^|[\s;&|(])curl\s", line))
        self.assertNotIn("--retry", line)

    def test_the_openot_checkout_clones_again_after_a_transient_failure(self) -> None:
        # W47: the first clone fails as GitHub's 403 did on main's run 44229834419, the second passes, and the
        # reference is checked out. Catches a checkout that fails the job on one bad answer.
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            result, clones = checkout_openot(folder, failures=1)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(clones, 2)
            self.assertTrue((folder / "open-ot-ref" / "Cargo.toml").is_file())
            self.assertIn("trying again in 15 seconds", result.stderr)

    def test_the_openot_checkout_fails_after_its_third_clone_fails(self) -> None:
        # Specification 22: a lasting failure stays visible. Three failed clones fail the step, with no reference
        # left behind. Catches a checkout that tries forever or passes without the reference.
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            result, clones = checkout_openot(folder, failures=3)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(clones, 3)
            self.assertFalse((folder / "open-ot-ref").exists())


    def test_the_openot_checkout_authenticates_with_the_jobs_token(self) -> None:
        # G281: with the job's token the clone sends it as git's authorization header for github.com, through git's
        # environment configuration, and the token reaches neither the output nor the log. Catches an anonymous clone
        # on a runner, which GitHub refused three times in a row, and a token printed or put on a command line.
        token = "ghs_ExampleJobToken0123456789"
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            result, clones = checkout_openot(folder, failures=0, token=token)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(clones, 1)
            expected = base64.b64encode(f"x-access-token:{token}".encode()).decode()
            self.assertEqual(
                (folder / "git-config").read_text(encoding="utf-8").splitlines(),
                [
                    "GIT_CONFIG_COUNT=1",
                    "GIT_CONFIG_KEY_0=http.https://github.com/.extraheader",
                    f"GIT_CONFIG_VALUE_0=AUTHORIZATION: basic {expected}",
                ],
            )
            self.assertNotIn(token, result.stdout + result.stderr)
            self.assertNotIn(expected, result.stdout + result.stderr)

    def test_the_openot_checkout_without_a_token_clones_anonymously(self) -> None:
        # Outside a runner there is no job token: the clone sets no git configuration. Catches a header with an
        # empty token, which GitHub refuses.
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            result, clones = checkout_openot(folder, failures=0)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(clones, 1)
            self.assertEqual((folder / "git-config").read_text(encoding="utf-8"), "")

    def test_every_workflow_step_that_runs_the_openot_checkout_gives_it_the_jobs_token(self) -> None:
        # G281: each step that runs scripts/checkout_openot_ref.sh sets GITHUB_TOKEN to the job's token. Catches a
        # step, new or old, whose clone stays anonymous.
        steps, missing = 0, []
        for workflow in sorted(WORKFLOWS.glob("*.yml")):
            document = yaml.safe_load(workflow.read_text(encoding="utf-8"))
            for job_name, job in (document.get("jobs") or {}).items():
                for step in job.get("steps") or []:
                    if "checkout_openot_ref.sh" not in str(step.get("run", "")):
                        continue
                    steps += 1
                    if (step.get("env") or {}).get("GITHUB_TOKEN") != "${{ github.token }}":
                        missing.append(f"{workflow.name}: {job_name}: {step.get('name', step.get('run'))}")
        self.assertGreater(steps, 0)
        self.assertEqual(missing, [])


if __name__ == "__main__":
    unittest.main()
