import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import yaml


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "docs-captures.yml"
PAGES = ROOT / ".github" / "workflows" / "demo-pages.yml"
PATHS = ROOT / ".github" / "paths" / "docs-captures.txt"


class DocsCapturesWorkflowContractTests(unittest.TestCase):
    def test_captures_run_read_only_and_nothing_commits_them(self) -> None:
        workflow = yaml.load(WORKFLOW.read_text(), Loader=yaml.BaseLoader)

        triggers = workflow["on"]
        # Specification 22: no trigger path filter (GitHub's compares only 300 files); the changes job
        # compares the whole change, with one list of paths for pull requests and pushes to main.
        self.assertEqual(triggers["pull_request"], "")
        self.assertEqual(triggers["push"], {"branches": ["main"]})
        self.assertEqual(workflow["permissions"], {"contents": "read"})
        paths = PATHS.read_text().splitlines()
        self.assertIn("scripts/tests/test_capture_lifecycle.py", paths)
        self.assertIn(
            "crates/trust-runtime/src/web/ui/**",
            paths,
            "runtime Web UI changes must execute the rendered capture suite",
        )
        changes = workflow["jobs"]["changes"]
        self.assertIn(
            "--paths-file .github/paths/docs-captures.txt",
            " ".join(step.get("run", "") for step in changes["steps"]),
        )

        refresh = workflow["jobs"]["refresh"]
        self.assertEqual(refresh["needs"], "changes")
        self.assertEqual(refresh["if"], "needs.changes.outputs.relevant == 'true'")
        self.assertEqual(refresh["permissions"], {"contents": "read"})
        refresh_steps = [step.get("name") for step in refresh["steps"]]
        self.assertIn("Verify capture process lifecycle", refresh_steps)

        # Catches the old refresh pull request coming back: its branch had to be merged into each
        # release candidate by hand (specification 22, "Docs Captures"). No job may write.
        for name, job in workflow["jobs"].items():
            permissions = job.get("permissions", {})
            self.assertNotIn("write", permissions.values() if isinstance(permissions, dict) else [permissions], name)

    def test_the_artifact_holds_only_what_the_captures_rendered(self) -> None:
        # Catches an upload of every image below the images folder: laid over a newer commit, an
        # older artifact would put back older hand-made images.
        workflow = yaml.load(WORKFLOW.read_text(), Loader=yaml.BaseLoader)
        steps = workflow["jobs"]["refresh"]["steps"]
        names = [step.get("name") for step in steps]
        mark = names.index("Mark the start of the captures")
        collect = names.index("Collect the rendered screenshots")
        upload = names.index("Upload refreshed captures")
        for capture in ("Refresh browser captures", "Refresh terminal captures", "Refresh code-server captures"):
            self.assertLess(mark, names.index(capture))
            self.assertLess(names.index(capture), collect)
        self.assertLess(collect, upload)
        self.assertIn('touch "${RUNNER_TEMP}/captures-start"', steps[mark]["run"])
        self.assertIn("scripts/docs_captures_artifact.py collect", steps[collect]["run"])
        self.assertIn('--since "${RUNNER_TEMP}/captures-start"', steps[collect]["run"])
        self.assertIn('--out "${RUNNER_TEMP}/docs-captures"', steps[collect]["run"])
        self.assertEqual(
            steps[upload]["with"],
            {
                "name": "docs-captures",
                "retention-days": "30",
                "if-no-files-found": "error",
                "path": "${{ runner.temp }}/docs-captures/",
            },
        )

    def test_the_site_is_published_with_the_newest_screenshots_of_main(self) -> None:
        # Catches a deploy that never takes the rendered screenshots, takes them after the site is
        # built, or publishes after a failed or a pull request's capture run.
        pages = yaml.load(PAGES.read_text(), Loader=yaml.BaseLoader)
        self.assertEqual(
            pages["on"]["workflow_run"],
            {"workflows": ["Docs Captures"], "types": ["completed"], "branches": ["main"]},
        )
        changes = pages["jobs"]["changes"]
        self.assertIn("github.event.workflow_run.conclusion == 'success'", changes["if"])
        self.assertIn('fromJSON(\'["push", "schedule", "workflow_dispatch"]\')', changes["if"])
        self.assertEqual(changes["permissions"], {"contents": "read", "actions": "read"})
        self.assertEqual(
            changes["outputs"]["relevant"],
            "${{ steps.captures.outputs.relevant || steps.changes.outputs.relevant }}",
        )
        captures = next(step for step in changes["steps"] if step.get("id") == "captures")
        self.assertEqual(captures["if"], "github.event_name == 'workflow_run'")
        self.assertIn('select(.name == "docs-captures"', captures["run"])

        build = pages["jobs"]["build"]
        self.assertEqual(build["if"], "needs.changes.outputs.relevant == 'true'")
        names = [step.get("name") for step in build["steps"]]
        take = names.index("Take the newest screenshots of main")
        self.assertLess(take, names.index("Generate docs media"))
        self.assertLess(take, names.index("Build docs site"))
        self.assertIn(
            'scripts/docs_captures_artifact.py take --repo "${GITHUB_REPOSITORY}" --dest docs/public/assets/images',
            build["steps"][take]["run"],
        )

    def test_a_pull_request_builds_and_checks_the_site_and_publishes_nothing(self) -> None:
        # Specification 22, "Docs Captures": a pull request that changes the site runs the deploy's own
        # checks and publishes nothing. Catches the site checks running only after the merge (the retired
        # release guard ran them before each push; Codex's review of #153), a pull request's run that
        # deploys, and one that cancels a deploy of main.
        pages = yaml.load(PAGES.read_text(), Loader=yaml.BaseLoader)
        self.assertEqual(pages["on"]["pull_request"], "")
        self.assertIn("github.event.pull_request.number", pages["concurrency"]["group"])
        self.assertIn("|| 'docs-pages' }}", pages["concurrency"]["group"])

        compare = next(step for step in pages["jobs"]["changes"]["steps"] if step.get("id") == "changes")
        self.assertEqual(compare["env"]["BASE"], "${{ github.event.pull_request.base.sha || github.event.before }}")
        self.assertEqual(compare["env"]["HEAD_SHA"], "${{ github.event.pull_request.head.sha || github.sha }}")

        steps = pages["jobs"]["build"]["steps"]
        upload = next(step for step in steps if step.get("uses", "").startswith("actions/upload-pages-artifact"))
        self.assertEqual(upload["if"], "github.event_name != 'pull_request'")
        for name in ("Take the newest screenshots of main", "Generate docs media", "Check docs links", "Build docs site"):
            step = next(step for step in steps if step.get("name") == name)
            self.assertNotIn("if", step, name)
        self.assertEqual(pages["jobs"]["deploy"]["if"], "github.event_name != 'pull_request'")
        self.assertEqual(pages["jobs"]["conclusion"]["name"], "Docs Pages Conclusion")

    def test_only_the_deploy_writes_to_pages_or_requests_an_oidc_token(self) -> None:
        # A pull request's run executes the pull request's own scripts, so its token must not publish the site or request an
        # OIDC token (Codex's review of #153). Catches write access granted to the whole workflow or to a job that a pull
        # request runs.
        pages = yaml.load(PAGES.read_text(), Loader=yaml.BaseLoader)
        self.assertEqual(pages["permissions"], {"contents": "read", "actions": "read"})
        for job_id, job in pages["jobs"].items():
            granted = job.get("permissions", {})
            writes = {name for name, level in granted.items() if level == "write"}
            if job_id == "deploy":
                self.assertEqual(writes, {"pages", "id-token"})
                self.assertIn("github.event_name != 'pull_request'", job["if"])
            else:
                self.assertEqual(writes, set(), job_id)
        deploy_steps = [step.get("uses", "") for step in pages["jobs"]["deploy"]["steps"]]
        self.assertLess(
            deploy_steps.index("actions/configure-pages@v6"), deploy_steps.index("actions/deploy-pages@v5")
        )

    def test_every_check_of_the_site_build_runs_when_it_or_what_it_reads_changes(self) -> None:
        # The Docs Pages build is the only run of its checks, so a pull request that changes a checker or what it reads
        # must run it. Catches a checker missing from the path list, and the inputs of the database-example checker
        # (Codex's review of #153: examples/openot_database/sqlite/runtime.toml or the runner script merged unchecked).
        import sys

        sys.path.insert(0, str(ROOT / "scripts"))
        import ci_changes

        patterns = ci_changes.read_patterns((ROOT / ".github" / "paths" / "docs-pages.txt").read_text())
        regexes = [ci_changes.pattern_regex(pattern) for pattern in patterns]
        listed = lambda path: any(regex.match(path) for regex in regexes)
        build = yaml.load(PAGES.read_text(), Loader=yaml.BaseLoader)["jobs"]["build"]
        scripts = sorted({
            word for step in build["steps"] for word in step.get("run", "").split()
            if word.startswith("scripts/") and word.endswith(".py")
        })
        self.assertGreaterEqual(len(scripts), 8, scripts)
        for script in scripts:
            self.assertTrue(listed(script), f"{script} runs in the site build but its change does not run it")
        for read in (
            "examples/openot_database/sqlite/runtime.toml",
            "examples/openot_database/workload/Main.st",
            "examples/openot_multi_program/runtime.toml",
            "scripts/openot_database_runner_prepare.sh",
            "scripts/openot_real_database_gate.sh",
            "crates/trust-runtime/src/host/openot_persistence/mod.rs",
        ):
            self.assertTrue(listed(read), f"check_openot_database_examples.py reads {read}, whose change does not run it")

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
