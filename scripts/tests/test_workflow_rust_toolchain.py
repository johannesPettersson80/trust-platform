"""Every workflow job installs the Rust toolchain that `rust-toolchain.toml` names in one setup step, with
the components that file lists and the other components and targets the job uses, so that no later step
makes rustup download from static.rust-lang.org outside the setup's retry (specification 22, "Pull-request
checks and change detection"). Like the release workflow's tests, these read the workflows' text."""

from __future__ import annotations

import re
import tomllib
import unittest

from scripts.tests.test_release_workflow import ROOT, jobs, run_block, step_texts

WORKFLOWS = sorted((ROOT / ".github" / "workflows").glob("*.yml"))
SETUP = re.compile(r"(?m)^        uses: dtolnay/rust-toolchain@(\S+)$")
COMPONENTS = re.compile(r"(?m)^          components: (.+)$")
RUSTUP_DOWNLOAD = re.compile(r"\brustup\s+(?:target\s+add|component\s+add|toolchain\s+install)\b|--install-missing\b")


def toolchain_file() -> tuple[str, set[str]]:
    """The channel and the components that the repository's `rust-toolchain.toml` names."""
    toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))["toolchain"]
    return toolchain["channel"], set(toolchain.get("components", []))


def setups() -> list[tuple[str, str, str, str]]:
    """(workflow, job id, toolchain, step text) of every `dtolnay/rust-toolchain` step."""
    found = []
    for path in WORKFLOWS:
        for job, text in jobs(path.read_text(encoding="utf-8")).items():
            for step in step_texts(text):
                match = SETUP.search(step)
                if match:
                    found.append((path.name, job, match.group(1), step))
    return found


def installed_components(step: str) -> set[str]:
    listed = COMPONENTS.search(step)
    return {name for name in re.split(r"[,\s]+", listed.group(1)) if name} if listed else set()


class RustToolchainSetupTests(unittest.TestCase):
    def test_every_setup_installs_the_toolchain_files_release_with_its_components(self) -> None:
        # Catches a setup step that leaves out a component the toolchain file lists: the job's first
        # `cargo` or `rustc` then makes rustup fetch the channel again, outside the setup's retry. On
        # 14 November 2026 that second fetch failed on the runner's DNS and failed #504's darwin-x64
        # toolchain bundle; none of the 24 setup steps of the file's release installed all of its
        # components. Also catches a step pinned to another release than the file's channel.
        channel, components = toolchain_file()
        for workflow, job, toolchain, step in setups():
            if toolchain == "nightly":
                continue
            with self.subTest(workflow=workflow, job=job):
                self.assertEqual(toolchain, channel)
                self.assertEqual(components - installed_components(step), set())

    def test_the_release_and_ci_setups_are_read(self) -> None:
        # Catches a reader that finds no setup step, which would let the test above pass on nothing:
        # the release's toolchain bundle and build jobs, CI's Test and Format jobs each have one.
        found = {(workflow, job) for workflow, job, _, _ in setups()}
        for expected in (
            ("release.yml", "toolchain-bundle"),
            ("release.yml", "build"),
            ("ci.yml", "test"),
            ("ci.yml", "fmt"),
        ):
            self.assertIn(expected, found)

    def test_a_job_that_installs_nightly_names_the_toolchain_in_each_cargo_command(self) -> None:
        # Catches a plain `cargo` in a job whose setup installs nightly: the toolchain file then picks
        # its own release, which that setup did not install, and rustup downloads it in that step.
        nightly_jobs = {(workflow, job) for workflow, job, toolchain, _ in setups() if toolchain == "nightly"}
        self.assertTrue(nightly_jobs)
        for path in WORKFLOWS:
            for job, text in jobs(path.read_text(encoding="utf-8")).items():
                if (path.name, job) not in nightly_jobs:
                    continue
                for step in step_texts(text):
                    if "        run: |\n" in step:
                        script = run_block(step)
                    else:
                        found = re.search(r"(?m)^        run: (.+)$", step)
                        script = found.group(1) if found else ""
                    for command in re.findall(r"(?m)^\s*cargo\b.*$", script):
                        with self.subTest(workflow=path.name, job=job, command=command):
                            self.assertRegex(command, r"^\s*cargo \+\S+ ")

    def test_no_later_step_installs_a_toolchain_component_or_target(self) -> None:
        # Catches a `rustup target add`, `rustup component add` or `rustup toolchain install` after the setup,
        # and a gate script told to install what is missing (`--install-missing`): each downloads from
        # static.rust-lang.org outside the setup's retry. Before 14 November 2026 eleven steps did: seven added
        # the targets of compiled programs (one also llvm-tools), one the browser IDE's WebAssembly target, the
        # Clippy job's gate the Windows target, and the two fuzz jobs installed the release that builds cargo-fuzz.
        for path in WORKFLOWS:
            for job, text in jobs(path.read_text(encoding="utf-8")).items():
                for step in step_texts(text):
                    with self.subTest(workflow=path.name, job=job, step=step.splitlines()[0].strip()):
                        self.assertIsNone(RUSTUP_DOWNLOAD.search(step))


if __name__ == "__main__":
    unittest.main()
