import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import yaml


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "docs-captures.yml"


class DocsCapturesWorkflowContractTests(unittest.TestCase):
    def test_pull_requests_run_read_only_captures_without_refresh_pr_writes(self) -> None:
        workflow = yaml.load(WORKFLOW.read_text(), Loader=yaml.BaseLoader)

        triggers = workflow["on"]
        self.assertIn("pull_request", triggers)
        self.assertEqual(
            triggers["pull_request"]["paths"],
            triggers["push"]["paths"],
            "PR and main-push capture paths must stay aligned",
        )
        self.assertEqual(workflow["permissions"], {"contents": "read"})
        self.assertIn(
            "scripts/tests/test_capture_lifecycle.py",
            triggers["pull_request"]["paths"],
        )
        self.assertIn(
            "crates/trust-runtime/src/web/ui/**",
            triggers["pull_request"]["paths"],
            "runtime Web UI changes must execute the rendered capture suite",
        )

        refresh = workflow["jobs"]["refresh"]
        self.assertEqual(refresh["permissions"], {"contents": "read"})
        refresh_steps = [step.get("name") for step in refresh["steps"]]
        self.assertIn("Verify capture process lifecycle", refresh_steps)
        self.assertNotIn("Create capture refresh PR", refresh_steps)

        refresh_pr = workflow["jobs"]["create-refresh-pr"]
        self.assertEqual(refresh_pr["needs"], "refresh")
        self.assertEqual(refresh_pr["if"], "github.event_name != 'pull_request'")
        self.assertEqual(
            refresh_pr["permissions"],
            {"contents": "write", "pull-requests": "write"},
        )
        refresh_pr_steps = [step.get("name") for step in refresh_pr["steps"]]
        self.assertIn("Download refreshed captures", refresh_pr_steps)
        self.assertIn("Create capture refresh PR", refresh_pr_steps)

    def test_captures_build_programs_with_the_toolchain_the_job_builds(self) -> None:
        # The browser IDE and the terminal captures build Structured Text programs. Without a
        # toolchain a build downloads its version's, which does not exist before that version is
        # published: the browser IDE's build failed on 0.25.1's pull request (28 October 2026).
        workflow = yaml.load(WORKFLOW.read_text(), Loader=yaml.BaseLoader)
        steps = workflow["jobs"]["refresh"]["steps"]
        names = [step.get("name") for step in steps]
        toolchain = names.index("Toolchain for compiled programs (specification 44 §4.6)")
        self.assertLess(toolchain, names.index("Refresh browser captures"))
        self.assertLess(toolchain, names.index("Refresh terminal captures"))
        self.assertIn("scripts/native/build_toolchain_libraries.sh", steps[toolchain]["run"])
        self.assertIn('echo "TRUST_TOOLCHAIN=${folder}" >> "${GITHUB_ENV}"', steps[toolchain]["run"])

    def test_terminal_captures_hand_the_toolchain_to_the_container_only_when_one_is_named(self) -> None:
        # The terminal captures run in a container, which sees the toolchain only when it is handed
        # over. Without TRUST_TOOLCHAIN nothing is handed over, and the runner still runs every tape:
        # under `set -u`, Bash before 4.4 calls an empty array's expansion unbound, so on macOS,
        # whose /bin/bash is 3.2, the runner ended before its first capture, with exit status 0
        # (Codex's review of 0.25.1 and a macOS run, 29 October 2026). The runner runs here under
        # /bin/bash.
        runner = ROOT / "scripts" / "captures" / "run-terminal-captures.sh"
        tapes = sorted(
            tape.relative_to(ROOT).as_posix()
            for tape in (ROOT / "scripts" / "captures" / "terminal").glob("*.tape")
        )
        self.assertTrue(tapes)

        with tempfile.TemporaryDirectory() as temp:
            stubs = Path(temp) / "bin"
            stubs.mkdir()
            calls = Path(temp) / "docker-calls"
            (stubs / "cargo").write_text("#!/bin/sh\nexit 0\n")
            (stubs / "rustc").write_text("#!/bin/sh\necho /opt/rust-sysroot\n")
            (stubs / "docker").write_text(f'#!/bin/sh\nprintf "%s\\n" "$@" "" >> "{calls}"\n')
            for stub in stubs.iterdir():
                stub.chmod(0o755)
            env = {name: value for name, value in os.environ.items() if name != "TRUST_TOOLCHAIN"}
            env["PATH"] = f"{stubs}{os.pathsep}{env['PATH']}"
            env["TRUST_CAPTURE_VHS_IMAGE"] = "vhs-under-test"

            def container_arguments(extra_env: dict[str, str]) -> list[list[str]]:
                calls.unlink(missing_ok=True)
                result = subprocess.run(
                    ["/bin/bash", str(runner)],
                    env={**env, **extra_env},
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertTrue(calls.exists(), f"the runner ran no capture: {result.stderr}")
                runs =[run.split("\n") for run in calls.read_text().split("\n\n") if run]
                # What follows `-w <working folder>`: the toolchain's arguments, the image, the
                # tape; sorted, because the shell orders its glob by its locale.
                return sorted(run[run.index("-w") + 2 :] for run in runs)

            self.assertEqual(
                container_arguments({}),
                [["vhs-under-test", tape] for tape in tapes],
            )
            self.assertEqual(
                container_arguments({"TRUST_TOOLCHAIN": "/opt/trust-toolchain"}),
                [
                    [
                        "-e",
                        "TRUST_TOOLCHAIN=/opt/trust-toolchain",
                        "-v",
                        "/opt/trust-toolchain:/opt/trust-toolchain:ro",
                        "-v",
                        "/opt/rust-sysroot:/opt/rust-sysroot:ro",
                        "vhs-under-test",
                        tape,
                    ]
                    for tape in tapes
                ],
            )


if __name__ == "__main__":
    unittest.main()
