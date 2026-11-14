import json
import os
from pathlib import Path, PurePosixPath
import posixpath
import subprocess
import tempfile
import unittest

import yaml


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "docs-captures.yml"
PAGES = ROOT / ".github" / "workflows" / "demo-pages.yml"
PATHS = ROOT / ".github" / "paths" / "docs-captures.txt"
CAPTURES = ROOT / "scripts" / "captures"
# The programs that the captures run in Linux containers, built as the release builds its Linux packages.
CROSS_BUILD = [
    "build",
    "--target",
    "x86_64-unknown-linux-gnu",
    "-p",
    "trust-lsp",
    "-p",
    "trust-runtime",
    "-p",
    "trust-debug",
    "-p",
    "trust-dev",
]
# The cross build's folder, as a container sees it in place of target/debug.
BUILT = f"{ROOT}/target/x86_64-unknown-linux-gnu/debug"
# The condition of the steps and jobs that publish the site: neither a pull request nor a merge group does.
PUBLISHING = "github.event_name != 'pull_request' && github.event_name != 'merge_group'"


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
        # Specification 22, "Docs Captures": a pull request or a merge group that changes the site runs the
        # deploy's own checks and publishes nothing. Catches the site checks running only after the merge (the retired
        # release guard ran them before each push; Codex's review of #153), a pull request's run that
        # deploys, and one that cancels a deploy of main.
        pages = yaml.load(PAGES.read_text(), Loader=yaml.BaseLoader)
        self.assertEqual(pages["on"]["pull_request"], "")
        self.assertEqual(pages["on"]["merge_group"], "")
        self.assertIn("github.event.pull_request.number", pages["concurrency"]["group"])
        self.assertIn("format('docs-pages-queue-{0}', github.run_id)", pages["concurrency"]["group"])
        self.assertIn("|| 'docs-pages' }}", pages["concurrency"]["group"])

        compare = next(step for step in pages["jobs"]["changes"]["steps"] if step.get("id") == "changes")
        self.assertEqual(
            compare["env"]["BASE"],
            "${{ github.event.pull_request.base.sha || github.event.merge_group.base_sha || github.event.before }}",
        )
        self.assertEqual(compare["env"]["HEAD_SHA"], "${{ github.event.pull_request.head.sha || github.sha }}")

        steps = pages["jobs"]["build"]["steps"]
        upload = next(step for step in steps if step.get("uses", "").startswith("actions/upload-pages-artifact"))
        self.assertEqual(upload["if"], PUBLISHING)
        for name in ("Take the newest screenshots of main", "Generate docs media", "Check docs links", "Build docs site"):
            step = next(step for step in steps if step.get("name") == name)
            self.assertNotIn("if", step, name)
        self.assertEqual(pages["jobs"]["deploy"]["if"], PUBLISHING)
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
                self.assertEqual(job["if"], PUBLISHING)
            else:
                self.assertEqual(writes, set(), job_id)
        deploy_steps = [step.get("uses", "") for step in pages["jobs"]["deploy"]["steps"]]
        self.assertLess(
            deploy_steps.index("actions/configure-pages@v6"), deploy_steps.index("actions/deploy-pages@v5")
        )

    def test_every_check_of_the_site_build_runs_when_it_or_what_it_reads_changes(self) -> None:
        # The Docs Pages build is the only run of its checks, so a pull request that changes a checker or what it reads
        # must run it. Catches a checker missing from the path list, and the inputs of the media step and of the
        # database-example checker (Codex's review of #153: docs/media/hero-runtime.png,
        # examples/openot_database/sqlite/runtime.toml or the runner script merged unchecked).
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
        import generate_public_docs_media

        for source, _ in generate_public_docs_media.COPY_MAP:
            copied = source.relative_to(ROOT).as_posix()
            self.assertTrue(listed(copied), f"generate_public_docs_media.py copies {copied}, whose change does not run it")
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
        tapes = terminal_tapes()
        self.assertTrue(tapes)

        with tempfile.TemporaryDirectory() as temp:

            def container_arguments(extra_env: dict[str, str]) -> list[list[str]]:
                result, runs, _ = run_terminal_captures(Path(temp), extra_env)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertTrue(tape_runs(runs), f"the runner ran no capture: {result.stderr}")
                # What follows `-w <working folder>`: the toolchain's arguments, the image, the
                # tape; sorted, because the shell orders its glob by its locale.
                return sorted(run[run.index("-w") + 2 :] for run in tape_runs(runs))

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

    def test_terminal_captures_run_programs_built_in_cross_that_start_in_the_image(self) -> None:
        # Catches the terminal captures running programs built on the runner in the vhs image, a
        # Debian stable system: they bound the runner's newer glibc and stopped at start, and on
        # 14 November 2026 every terminal capture of main showed only "version `GLIBC_2.43' not
        # found". The programs are built in cross for x86_64-unknown-linux-gnu, as the release builds
        # its Linux packages, each program the tapes run starts once in the image before the first
        # tape, and every tape sees that build as target/debug (specification 22, "Docs Captures").
        with tempfile.TemporaryDirectory() as temp:
            result, runs, cross = run_terminal_captures(Path(temp), {})
        self.assertEqual(result.returncode, 0, result.stderr)
        # The container's cargo runs without the repository's sccache wrapper, which its image lacks.
        self.assertEqual(cross, [["CROSS_CONTAINER_OPTS=--env RUSTC_WRAPPER=", *CROSS_BUILD]])
        starts = [run for run in runs if "--version" in run]
        self.assertEqual(
            starts,
            [
                [
                    "run",
                    "--rm",
                    "-v",
                    f"{BUILT}:/workspaces/trust-platform/target/debug:ro",
                    "--entrypoint",
                    f"/workspaces/trust-platform/target/debug/{program}",
                    "vhs-under-test",
                    "--version",
                ]
                for program in ("trust-runtime", "trust-dev")
            ],
        )
        self.assertEqual(runs[: len(starts)], starts)
        self.assertEqual(len(tape_runs(runs)), len(terminal_tapes()))
        for run in tape_runs(runs):
            self.assertIn(f"{BUILT}:/workspaces/trust-platform/target/debug", mounts(run))

    def test_terminal_captures_stop_when_a_program_does_not_start_in_the_image(self) -> None:
        # Catches captures that go on after a program failed to start in the image: each tape would
        # record the loader's error, and the site would show it in place of the command's output.
        with tempfile.TemporaryDirectory() as temp:
            result, runs, _ = run_terminal_captures(Path(temp), {"DOCKER_STUB_FAILS_VERSION": "1"})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("trust-runtime does not start in vhs-under-test", result.stderr)
        self.assertEqual(tape_runs(runs), [])

    def test_code_server_captures_run_the_language_server_built_in_cross_that_starts_in_the_image(self) -> None:
        # Catches the code-server captures running a language server built on the runner. The
        # captured workspace names target/debug/trust-lsp, which was the runner's own build, and a
        # trust-lsp built on Ubuntu 26.04 (glibc 2.43) stops in the code-server image (Debian 13,
        # glibc 2.41) with "version `GLIBC_2.43' not found" (measured 14 November 2026): every view
        # that needs it would be captured without it. The container sees the cross build as
        # target/debug, and the language server starts once in the image before code-server does
        # (specification 22, "Docs Captures").
        with tempfile.TemporaryDirectory() as temp:
            result, calls = run_stubbed(Path(temp), ["/bin/bash", str(CAPTURES / "vscode" / "start-code-server.sh")], {})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            [call for call in calls if call[0] == "cross"],
            [["cross", "CROSS_CONTAINER_OPTS=--env RUSTC_WRAPPER=", *CROSS_BUILD]],
        )
        start = [
            "docker",
            "run",
            "--rm",
            "-v",
            f"{BUILT}:/workspaces/trust-platform/target/debug:ro",
            "--entrypoint",
            "/workspaces/trust-platform/target/debug/trust-lsp",
            "code-server-under-test",
            "command",
            "--help",
        ]
        self.assertIn(start, calls)
        servers = code_server_runs(calls)
        self.assertEqual(len(servers), 1, calls)
        self.assertIn(f"{BUILT}:/workspaces/trust-platform/target/debug:ro", mounts(servers[0]))
        # The build, then the start check, then the extension's package and the server.
        names = [call[0] for call in calls]
        self.assertLess(names.index("cross"), calls.index(start))
        self.assertLess(calls.index(start), names.index("npx"))
        self.assertLess(calls.index(start), calls.index(servers[0]))
        # The language server that the opened workspace names is the one the container sees there.
        opened = PurePosixPath(servers[0][-1].split()[-1])
        workspace = json.loads((ROOT / opened.relative_to("/workspaces/trust-platform")).read_text())
        folder = opened.parent / workspace["folders"][0]["path"]
        named = workspace["settings"]["trust-lsp.server.path"].replace("${workspaceFolder}", str(folder))
        self.assertEqual(posixpath.normpath(named), "/workspaces/trust-platform/target/debug/trust-lsp")

    def test_code_server_captures_stop_when_the_language_server_does_not_start_in_the_image(self) -> None:
        # Catches captures that go on after the language server failed to start in the image: the
        # extension would run without it, and every view that needs it would be captured empty.
        with tempfile.TemporaryDirectory() as temp:
            result, calls = run_stubbed(
                Path(temp),
                ["/bin/bash", str(CAPTURES / "vscode" / "start-code-server.sh")],
                {"DOCKER_STUB_FAILS_HELP": "1"},
            )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("trust-lsp does not start in code-server-under-test", result.stderr)
        self.assertEqual(code_server_runs(calls), [])
        self.assertNotIn("npx", [call[0] for call in calls])

    def test_vscode_captures_build_the_language_server_in_cross_before_playwright_waits_for_it(self) -> None:
        # Catches the VS Code captures building the language server on the runner, a build that does
        # not start in the code-server image, or leaving a first cross build to start-code-server.sh,
        # which Playwright gives 240 seconds to answer (playwright.vscode.config.mjs).
        with tempfile.TemporaryDirectory() as temp:
            result, calls = run_stubbed(
                Path(temp), ["/bin/bash", str(CAPTURES / "run-playwright-captures.sh"), "vscode"], {}
            )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([call for call in calls if call[0] == "cargo"], [])
        captures = [call for call in calls if call[0] == "npm" and call[-1] == "capture:vscode"]
        self.assertEqual(len(captures), 1, calls)
        self.assertLess([call[0] for call in calls].index("cross"), calls.index(captures[0]))


def terminal_tapes() -> list[str]:
    return sorted(
        tape.relative_to(ROOT).as_posix()
        for tape in (ROOT / "scripts" / "captures" / "terminal").glob("*.tape")
    )


def run_stubbed(
    temp: Path, command: list[str], extra_env: dict[str, str]
) -> tuple[subprocess.CompletedProcess[str], list[list[str]]]:
    """Runs `command` with `cross`, `docker`, `npm`, `npx`, `cargo` and `rustc` stubbed, the images
    named `vhs-under-test` and `code-server-under-test`: the result, and each stubbed call in order as
    [name, argument, ...], where a `cross` call names its CROSS_CONTAINER_OPTS first. The docker stub
    fails a call with `--version` when DOCKER_STUB_FAILS_VERSION is set, and one with
    `command --help` when DOCKER_STUB_FAILS_HELP is set; `rustc` prints /opt/rust-sysroot."""
    stubs = temp / "bin"
    stubs.mkdir(exist_ok=True)
    log = temp / "calls"
    log.unlink(missing_ok=True)
    for name in ("cross", "docker", "npm", "npx", "cargo"):
        options = ' "CROSS_CONTAINER_OPTS=$CROSS_CONTAINER_OPTS"' if name == "cross" else ""
        failures = (
            'case " $* " in *" --version "*) [ -n "$DOCKER_STUB_FAILS_VERSION" ] && exit 1 ;; esac\n'
            'case " $* " in *" command --help "*) [ -n "$DOCKER_STUB_FAILS_HELP" ] && exit 1 ;; esac\n'
            if name == "docker"
            else ""
        )
        (stubs / name).write_text(f'#!/bin/sh\nprintf "%s\\n" {name}{options} "$@" "" >> "{log}"\n{failures}exit 0\n')
    (stubs / "rustc").write_text("#!/bin/sh\necho /opt/rust-sysroot\n")
    for stub in stubs.iterdir():
        stub.chmod(0o755)
    env = {
        name: value
        for name, value in os.environ.items()
        if name not in ("TRUST_TOOLCHAIN", "CARGO_TARGET_DIR")
    }
    env["PATH"] = f"{stubs}{os.pathsep}{env['PATH']}"
    env["TRUST_CAPTURE_VHS_IMAGE"] = "vhs-under-test"
    env["TRUST_CAPTURE_CODESERVER_IMAGE"] = "code-server-under-test"
    result = subprocess.run(command, env={**env, **extra_env}, capture_output=True, text=True)
    calls = [call.split("\n") for call in log.read_text().split("\n\n") if call] if log.exists() else []
    return result, calls


def run_terminal_captures(
    temp: Path, extra_env: dict[str, str]
) -> tuple[subprocess.CompletedProcess[str], list[list[str]], list[list[str]]]:
    """Runs `run-terminal-captures.sh` under /bin/bash through `run_stubbed`: the result, the arguments
    of each `docker` call in order, and those of each `cross` call."""
    result, calls = run_stubbed(temp, ["/bin/bash", str(CAPTURES / "run-terminal-captures.sh")], extra_env)
    runs = [call[1:] for call in calls if call[0] == "docker"]
    cross = [call[1:] for call in calls if call[0] == "cross"]
    return result, runs, cross


def tape_runs(runs: list[list[str]]) -> list[list[str]]:
    """The `docker` calls that record a tape: those that run in the repository's folder."""
    return [run for run in runs if "-w" in run]


def code_server_runs(calls: list[list[str]]) -> list[list[str]]:
    """The `docker` calls that start the code-server container: those that name it."""
    return [call for call in calls if call[0] == "docker" and "--name" in call]


def mounts(run: list[str]) -> list[str]:
    """The volumes that a `docker` call mounts."""
    return [run[index + 1] for index, argument in enumerate(run) if argument == "-v"]


if __name__ == "__main__":
    unittest.main()
