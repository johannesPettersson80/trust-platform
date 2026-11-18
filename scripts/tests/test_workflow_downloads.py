"""Every download in the workflows is tried again on a transient failure (G106, W47; specification 22, "Pull-request checks
and change detection" and "CI reliability"). GitHub's release server answered 504 to the actionlint download once and
failed a Format job that had no fault of its own, and GitHub answered 403 to a clone of the OpenOT reference once and
failed `main`'s Documentation job. Wrong behaviour caught: a `curl` in a workflow without `--retry`, and a clone of the
OpenOT reference that is tried once."""

import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / ".github" / "workflows"
CHECKOUT_OPENOT = ROOT / "scripts" / "checkout_openot_ref.sh"

# A `git` that fails its first `$STUB_FAILURES` clones as GitHub's 403 did, and otherwise clones a project with its
# manifest into the folder it is given; each clone is counted in `$STUB_STATE/clones`.
STUB_GIT = """#!/usr/bin/env bash
if [ "$1" = clone ]; then
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


def checkout_openot(folder: Path, failures: int) -> tuple[subprocess.CompletedProcess[str], int]:
    """The OpenOT checkout of `folder/open-ot-ref` with a `git` whose first `failures` clones fail and a `sleep` that
    returns at once; the run and the number of clones it made."""
    stubs = folder / "stubs"
    stubs.mkdir()
    for name, text in (("git", STUB_GIT), ("sleep", "#!/usr/bin/env bash\nexit 0\n")):
        (stubs / name).write_text(text, encoding="utf-8")
        (stubs / name).chmod(0o755)
    environment = {
        **os.environ,
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


if __name__ == "__main__":
    unittest.main()
