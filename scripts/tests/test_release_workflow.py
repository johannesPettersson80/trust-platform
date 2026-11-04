"""The Release workflow and main's version guard (specification 24, "Release rehearsal on pull
requests", "Release after main CI" and "Main-push version alignment"). CI's Python has no YAML
parser, so the tests read the workflows' text, which keeps its indentation."""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import check_release_bundle as bundle  # noqa: E402
import check_version_release_evidence as verifier  # noqa: E402
import release_plan  # noqa: E402

RELEASE = ROOT / ".github" / "workflows" / "release.yml"
RELEASE_FILES = ROOT / ".github" / "actions" / "release-files" / "action.yml"
CI = ROOT / ".github" / "workflows" / "ci.yml"
RELEASE_ONLY = "${{ needs.plan.outputs.mode == 'release' }}"
# The steps that make a release public, in the order they run.
PUBLISHING = ["Tag the release", "Create GitHub Release", "Publish to VS Code Marketplace", "Verify the release"]


def jobs(workflow: str) -> dict[str, str]:
    """Each job's text by its id."""
    body = workflow.split("\njobs:\n", 1)[1]
    parts = re.split(r"(?m)^  ([a-z0-9_-]+):\n", body)
    return dict(zip(parts[1::2], parts[2::2]))


def step_texts(job: str) -> list[str]:
    """Each step's text in order, its first line indented as the others."""
    return ["        " + step for step in re.split(r"(?m)^      - ", job)[1:]]


def steps(job: str) -> dict[str, str]:
    """Each named step's text by its name."""
    found = {}
    for text in step_texts(job):
        name = re.search(r"(?m)^        name: (.+)$", text)
        if name:
            found[name.group(1)] = text
    return found


def run_block(step: str) -> str:
    """A step's `run: |` script, dedented as the runner sees it."""
    body = step.split("        run: |\n", 1)[1]
    return "\n".join(line[10:] for line in body.splitlines()) + "\n"


def field(text: str, key: str, indent: int = 8) -> str:
    match = re.search(rf"(?m)^{' ' * indent}{key}: (.+)$", text)
    return match.group(1) if match else ""


def needs(job: str) -> set[str]:
    value = field(job, "needs", 4)
    return {name.strip() for name in value.strip("[]").split(",") if name.strip()}


def condition(job: str) -> str | None:
    """A job's `if:` expression, also when it is folded over several lines; None without one."""
    match = re.search(r"(?m)^    if: (.*)$", job)
    if not match:
        return None
    if match.group(1) not in (">-", ">"):
        return match.group(1)
    lines = []
    for line in job[match.end() + 1:].splitlines():
        if not line.startswith("      "):
            break
        lines.append(line.strip())
    return " ".join(lines)


def action_steps(text: str) -> dict[str, str]:
    """Each named step of a composite action by its name."""
    found = {}
    for step in re.split(r"(?m)^    - ", text.split("\n  steps:\n", 1)[1])[1:]:
        name = re.match(r"name: (.+)", step)
        if name:
            found[name.group(1)] = step
    return found


STATUS_FUNCTIONS = re.compile(r"\b(?:success|failure|cancelled|always)\(\)")
TOKENS = re.compile(r"\s*(?:(?P<string>'(?:[^']|'')*')|(?P<op>==|!=|&&|\|\||[!()])|(?P<name>[A-Za-z_][A-Za-z0-9_.-]*))")


def evaluate(expression: str, context: dict[str, str], functions: dict[str, bool]) -> object:
    """The part of GitHub's expression language that the Release workflow's job conditions use: string literals,
    context paths, the status functions, `!`, `==`, `!=`, `&&`, `||` and parentheses, with GitHub's precedence. `==` on
    strings ignores case, `&&` and `||` give one of their operands, and a missing path or the empty string is false."""
    body = expression.strip()
    if body.startswith("${{") and body.endswith("}}"):
        body = body[3:-2]
    tokens, position = [], 0
    while body[position:].strip():
        match = TOKENS.match(body, position)
        if not match:
            raise ValueError(f"unsupported expression at {body[position:]!r}")
        tokens.append(match)
        position = match.end()
    index = 0

    def peek() -> str:
        return tokens[index].group(0).strip() if index < len(tokens) else ""

    def take(expected: str | None = None) -> re.Match:
        nonlocal index
        token = tokens[index]
        if expected is not None and token.group(0).strip() != expected:
            raise ValueError(f"expected {expected!r} in {body!r}")
        index += 1
        return token

    def primary() -> object:
        token = take()
        if token.group(0).strip() == "(":
            value = either()
            take(")")
            return value
        if token.group("string") is not None:
            return token.group("string")[1:-1].replace("''", "'")
        name = token.group("name")
        if name is None:
            raise ValueError(f"unexpected {token.group(0)!r} in {body!r}")
        if peek() == "(":
            take("(")
            take(")")
            return functions[name]
        return context.get(name, "")

    def unary() -> object:
        if peek() == "!":
            take()
            return not unary()
        return primary()

    def compare() -> object:
        left = unary()
        while peek() in ("==", "!="):
            operator = take().group(0).strip()
            right = unary()
            same = left.lower() == right.lower() if isinstance(left, str) and isinstance(right, str) else left == right
            left = same if operator == "==" else not same
        return left

    def both() -> object:
        left = compare()
        while peek() == "&&":
            take()
            right = compare()
            left = right if left else left
        return left

    def either() -> object:
        left = both()
        while peek() == "||":
            take()
            right = both()
            left = left if left else right
        return left

    value = either()
    if index != len(tokens):
        raise ValueError(f"unparsed rest of {body!r}")
    return value


def simulate(workflow_jobs: dict[str, str], *, event: str, outputs: dict[str, str], ended: dict[str, str] | None = None,
             cancelled: bool = False) -> dict[str, str]:
    """Each job's result when the plan writes `outputs` for `event`: a job runs when its condition holds, with GitHub's
    implicit `success()` (every job it needs passed) unless the condition calls a status function itself. A job that
    runs passes, unless `ended` gives its result."""
    results: dict[str, str] = {}
    waiting = dict(workflow_jobs)
    while waiting:
        ready = [job_id for job_id, job in waiting.items() if needs(job) <= set(results)]
        if not ready:
            raise ValueError(f"the jobs {sorted(waiting)} need each other")
        for job_id in ready:
            job = waiting.pop(job_id)
            needed = {name: results[name] for name in needs(job)}
            context = {"github.event_name": event, **{f"needs.{name}.result": result for name, result in needed.items()}}
            if "plan" in needed:
                context.update({f"needs.plan.outputs.{key}": value for key, value in outputs.items()})
            functions = {"success": all(result == "success" for result in needed.values()), "always": True,
                         "cancelled": cancelled, "failure": any(result == "failure" for result in needed.values())}
            expression = condition(job)
            if expression is None:
                runs = functions["success"]
            elif STATUS_FUNCTIONS.search(expression):
                runs = bool(evaluate(expression, context, functions))
            else:
                runs = functions["success"] and bool(evaluate(expression, context, functions))
            results[job_id] = (ended or {}).get(job_id, "success") if runs else "skipped"
    return results


def concluded(results: dict[str, str]) -> bool:
    """The Release Conclusion's verdict: every job passed or the plan skipped it."""
    return all(result in ("success", "skipped") for job_id, result in results.items() if job_id != "conclusion")


BUILDS = {"toolchain-bundle", "build", "runtime-vm-validation", "openot-real-databases", "acceptance"}
PROMOTED = {"mode": "release", "sha": "new", "tag": "v0.31.0", "rehearsal_run": "41707764199",
            "rehearsal_artifact": "13281794816"}


class ReleaseWorkflowTests(unittest.TestCase):
    def setUp(self) -> None:
        self.text = RELEASE.read_text(encoding="utf-8")
        self.jobs = jobs(self.text)

    def test_a_release_follows_main_ci_or_a_push_to_main_and_no_tag_push_starts_one(self) -> None:
        # Catches a release that waits for a hand-pushed tag: 0.25.1's took two days of tag and CI
        # rounds (27 to 29 October 2026). A push starts the Release only for main, where it promotes a
        # rehearsal of the same tree (specification 24, "Release after main CI"), and never for a tag.
        triggers = self.text.split("\non:\n", 1)[1].split("\n\npermissions:", 1)[0]
        self.assertIn("\n  pull_request:\n", "\n" + triggers)
        self.assertIn("\n  merge_group:\n", "\n" + triggers)
        self.assertIn(
            "  workflow_run:\n    workflows: [CI]\n    types: [completed]\n    branches: [main]\n", triggers
        )
        self.assertIn("  workflow_dispatch:", triggers)
        self.assertNotIn("tags:", triggers)
        self.assertRegex(triggers, r"(?m)^  push:\n    branches: \[main\]\n")

    def test_the_plan_alone_decides_what_runs(self) -> None:
        # Catches a job that runs when the plan skips, or that the plan's mode cannot stop.
        plan = self.jobs["plan"]
        self.assertIn("python3 scripts/release_plan.py", plan)
        self.assertEqual(field(self.jobs["release-preflight"], "if", 4), "${{ needs.plan.outputs.mode != 'skip' }}")
        for job_id, job in self.jobs.items():
            if job_id in {"plan", "release-preflight", "conclusion"}:
                continue
            self.assertIn("plan", needs(job), job_id)
            self.assertIn("release-preflight", needs(job), job_id)

    def test_the_plan_never_runs_a_forks_code(self) -> None:
        # Catches a plan that runs the code of the commit it plans for: after CI of a fork's pull
        # request that commit is the fork's, and the plan's token and outputs decide a release. A
        # fork's own pull request makes `github.sha` its merge commit, so the plan runs the base then
        # (Codex's review of #146). The checkout keeps no credentials, and only the run after main's
        # CI gets the token.
        checkouts = [text for text in step_texts(self.jobs["plan"]) if "uses: actions/checkout@" in text]
        self.assertEqual(len(checkouts), 1)
        self.assertIn(
            "ref: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name"
            " != github.repository && github.event.pull_request.base.sha || github.sha }}",
            checkouts[0],
        )
        self.assertIn("persist-credentials: false", checkouts[0])
        self.assertNotIn("head_sha", checkouts[0])
        self.assertNotIn("head.sha", checkouts[0])
        self.assertIn(
            "GITHUB_TOKEN: ${{ (github.event_name == 'workflow_run' || github.event_name == 'push') && github.token || '' }}",
            self.jobs["plan"],
        )

    def test_every_job_checks_out_the_plans_commit(self) -> None:
        # Catches a job of a release after CI that builds main's newest commit instead of the one
        # whose CI passed, or a rehearsal that builds the pull request's merge commit.
        for job_id, job in self.jobs.items():
            if job_id in {"plan", "conclusion"}:
                continue
            checkouts = [text for text in step_texts(job) if "uses: actions/checkout@" in text]
            if job_id == "openot-real-databases":
                self.assertEqual(checkouts, [])
                self.assertIn("    with:\n      ref: ${{ needs.plan.outputs.sha }}\n", job)
                continue
            self.assertTrue(checkouts, job_id)
            for text in checkouts:
                self.assertIn("ref: ${{ needs.plan.outputs.sha }}", text, job_id)

    def test_only_a_release_tags_and_publishes_and_in_this_order(self) -> None:
        # Catches a rehearsal that publishes, and a Marketplace upload before the release exists.
        publish = steps(self.jobs["publish"])
        order = list(publish)
        self.assertEqual([name for name in order if name in PUBLISHING], PUBLISHING)
        for name in PUBLISHING:
            self.assertEqual(field(publish[name], "if"), RELEASE_ONLY, name)
        for job_id, job in self.jobs.items():
            if job_id == "publish":
                continue
            for needle in ("vsce publish", "action-gh-release", "git push", "git tag"):
                self.assertNotIn(needle, job, job_id)

    def test_the_planner_reads_the_verification_by_the_names_it_runs_under(self) -> None:
        # Catches a renamed job or step: the plan would then find no verified run and re-run every
        # release from the start, publishing a complete one again.
        publish = self.jobs["publish"]
        self.assertEqual(field(publish, "name", 4), release_plan.VERIFY_JOB)
        self.assertEqual(list(steps(publish))[-1], release_plan.VERIFY_STEP)

    def test_the_release_follows_every_build_and_test(self) -> None:
        # Catches a tag created while a platform's build or a test job still runs or has failed
        # (0.25.0's release jobs failed after its tag, 25 October 2026), and a rehearsal that keeps
        # files before every build and test passed.
        others = set(self.jobs) - {"rehearse", "publish", "conclusion"}
        self.assertEqual(needs(self.jobs["publish"]), others)
        self.assertEqual(needs(self.jobs["rehearse"]), others)

    def test_the_acceptance_journeys_run_on_the_linux_package_this_run_built(self) -> None:
        # Specification 22, "Acceptance journeys on the packaged extension": the journeys run on the package that the
        # build job made for linux-x64, under a virtual display, and their journals and screenshots are kept also when a
        # journey fails. Catches journeys run on another package or none, and a failure without its evidence.
        acceptance = self.jobs["acceptance"]
        self.assertEqual(needs(acceptance), {"plan", "release-preflight", "build"})
        named = steps(acceptance)
        self.assertIn("name: vsix-linux-x64", named["Take the linux-x64 package"])
        self.assertIn("npx tsc -p ./", run_block(named["Build the journeys"]))
        script = run_block(named["Run the acceptance journeys"])
        self.assertIn('xvfb-run -a -s "-screen 0 1920x1080x24"', script)
        self.assertIn('npm run acceptance -- --vsix "${vsix}" --out "${RUNNER_TEMP}/acceptance"', script)
        kept = named["Keep the journals and screenshots"]
        self.assertEqual(field(kept, "if"), "${{ always() }}")
        self.assertIn("path: ${{ runner.temp }}/acceptance", kept)

    def test_the_plan_chooses_the_platforms_that_are_built(self) -> None:
        # Specification 24: the toolchain-bundle and build jobs build the platforms the plan names, every one for a
        # rehearsal or a release and linux-x64 for a pull request's package. Catches a matrix that ignores the plan, and
        # a package check that also runs the runtime validation, which CI's tests already hold.
        plan = self.jobs["plan"]
        for name in ("toolchain_targets", "build_targets"):
            self.assertIn(f"{name}: ${{{{ steps.plan.outputs.{name} }}}}", plan)
        self.assertIn("include: ${{ fromJSON(needs.plan.outputs.toolchain_targets) }}", self.jobs["toolchain-bundle"])
        self.assertIn("include: ${{ fromJSON(needs.plan.outputs.build_targets) }}", self.jobs["build"])
        self.assertIn("--release-paths .github/paths/release.txt", plan)
        self.assertIn("--package-paths .github/paths/package.txt", plan)
        package = simulate(self.jobs, event="pull_request", outputs={"mode": "package", "rehearsal_run": ""})
        self.assertEqual({job for job, result in package.items() if result == "success"},
                         {"plan", "release-preflight", "toolchain-bundle", "build", "acceptance", "conclusion"})

    def test_every_file_the_build_copies_into_a_package_is_in_the_package_list(self) -> None:
        # Specification 24: a pull request that changes what a package holds builds it. Catches a repository file copied into
        # the runtime archives whose change builds no package (Codex's review of #157: docs/deploy/INSTALL.md and the
        # systemd unit).
        import ci_changes

        patterns = ci_changes.read_patterns((ROOT / ".github" / "paths" / "package.txt").read_text())
        copied = sorted(set(re.findall(r"(?m)^\s*(?:cp|Copy-Item) ([A-Za-z][^\s$]*) dist/", self.jobs["build"])))
        self.assertGreaterEqual(len(copied), 2, copied)
        for path in copied:
            self.assertTrue(any(ci_changes.pattern_regex(pattern).match(path) for pattern in patterns), path)

    def test_every_script_the_release_runs_is_in_the_release_list(self) -> None:
        # Specification 24: a pull request that changes a script the release runs is rehearsed. Catches a script added to
        # the release, or to its release-files action, whose changes would merge without a rehearsal.
        import ci_changes

        patterns = ci_changes.read_patterns((ROOT / ".github" / "paths" / "release.txt").read_text())
        run = sorted(set(re.findall(r"scripts/[A-Za-z0-9_./-]+\.(?:py|sh)", self.text + RELEASE_FILES.read_text())))
        self.assertGreaterEqual(len(run), 10, run)
        for script in run:
            self.assertTrue(any(ci_changes.pattern_regex(pattern).match(script) for pattern in patterns), script)

    def test_a_promotion_audits_the_vs_code_lockfile_again(self) -> None:
        # Specification 24, review finding of 4 November 2026: a promotion builds nothing, so the npm audit of its
        # rehearsal's build can be days old. Catches a promotion that publishes without auditing the lockfile on the
        # day it publishes; a release that builds audits it in its build job.
        preflight = steps(self.jobs["release-preflight"])
        promoted = "${{ needs.plan.outputs.rehearsal_run != '' }}"
        audit = preflight["Audit the VS Code dependency lockfile"]
        self.assertEqual(field(audit, "if"), promoted)
        self.assertEqual(field(audit, "working-directory"), "editors/vscode")
        self.assertEqual(field(audit, "run"), "python3 ../../scripts/check_npm_audit.py")
        node = preflight["Setup Node.js for the dependency audit"]
        self.assertEqual(field(node, "if"), promoted)
        self.assertLess(list(preflight).index("Setup Node.js for the dependency audit"),
                        list(preflight).index("Audit the VS Code dependency lockfile"))
        self.assertIn("python3 ../../scripts/check_npm_audit.py", steps(self.jobs["build"])["Audit VS Code dependency lockfile"])

    def test_a_missing_marketplace_token_fails_a_release_before_it_builds(self) -> None:
        require = steps(self.jobs["release-preflight"])["Require the Marketplace token"]
        self.assertEqual(field(require, "if"), RELEASE_ONLY)
        self.assertIn("exit 1", require)
        self.assertNotIn("Skip Marketplace publish", self.text)

    def test_the_tag_check_runs_for_a_release_only(self) -> None:
        # A rehearsal's commit is not on main yet; the tag check would refuse it.
        check = steps(self.jobs["release-preflight"])["Validate release tag"]
        self.assertEqual(field(check, "if"), RELEASE_ONLY)

    def test_the_release_is_named_by_the_plans_tag_and_commit(self) -> None:
        # Catches a release named by the event's ref or commit: after CI they are main's newest.
        for job_id, job in self.jobs.items():
            if job_id == "plan":
                continue
            for needle in ("github.ref_name", "GITHUB_REF_NAME", "github.sha", "GITHUB_SHA"):
                self.assertNotIn(needle, job, job_id)
        publish = self.jobs["publish"]
        self.assertIn("      RELEASE_TAG: ${{ needs.plan.outputs.tag }}\n", publish)
        self.assertIn("      RELEASE_SHA: ${{ needs.plan.outputs.sha }}\n", publish)
        files = RELEASE_FILES.read_text(encoding="utf-8")
        self.assertEqual(files.count('--commit "${RELEASE_SHA}"'), 2)
        for needle in ("github.sha", "GITHUB_SHA", "github.ref_name", "GITHUB_REF_NAME"):
            self.assertNotIn(needle, files)
        for job_id in ("rehearse", "publish"):
            write = steps(self.jobs[job_id])["Write the release files"]
            self.assertIn("uses: ./.github/actions/release-files", write, job_id)
            self.assertIn("sha: ${{ needs.plan.outputs.sha }}", write, job_id)
            self.assertIn("tag: ${{ needs.plan.outputs.tag }}", write, job_id)
        release = steps(publish)["Create GitHub Release"]
        self.assertIn("tag_name: ${{ env.RELEASE_TAG }}", release)
        self.assertIn("target_commitish: ${{ env.RELEASE_SHA }}", release)

    def test_a_rerun_continues_with_the_tag_on_the_same_commit(self) -> None:
        tag = steps(self.jobs["publish"])["Tag the release"]
        self.assertIn('if [ "${peeled}" != "${RELEASE_SHA}" ]', tag)
        self.assertIn('git tag -a "${RELEASE_TAG}"', tag)
        self.assertIn("already published", steps(self.jobs["publish"])["Publish to VS Code Marketplace"])

    def test_a_failed_marketplace_upload_fails_its_step(self) -> None:
        # Catches the step that ended green after vsce failed: `exit_code=$?` after an `if` whose condition failed
        # read 0, so the release waited an hour in its verification for a target never uploaded (Codex, PR #146).
        # The step runs here with a stand-in `vsce` that answers each case as the Marketplace tool does.
        script = run_block(steps(self.jobs["publish"])["Publish to VS Code Marketplace"])
        cases = [
            ("ERROR  The Personal Access Token verification has failed.", 1, 1),
            ("ERROR  trust-platform.trust-lsp@0.27.0 already exists.", 1, 0),
            ("DONE  Published trust-platform.trust-lsp v0.27.0.", 0, 0),
        ]
        for answer, vsce_status, step_status in cases:
            with tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                (root / "vsix-artifacts").mkdir()
                (root / "vsix-artifacts" / "trust-lsp-linux-x64.vsix").write_text("vsix")
                (root / "bin").mkdir()
                vsce = root / "bin" / "vsce"
                vsce.write_text(f"#!/bin/sh\necho '{answer}'\nexit {vsce_status}\n")
                vsce.chmod(0o755)
                result = subprocess.run(
                    ["bash", "-c", script],
                    cwd=root,
                    env={**os.environ, "PATH": f"{root / 'bin'}:{os.environ['PATH']}"},
                    capture_output=True,
                    text=True,
                    timeout=60,
                )
                self.assertEqual(result.returncode, step_status, f"{answer}: {result.stdout}{result.stderr}")

    def test_a_rehearsal_keeps_its_release_text_and_evidence(self) -> None:
        keep = steps(self.jobs["rehearse"])["Keep the rehearsal's release files"]
        self.assertIn("name: release-rehearsal-${{ needs.plan.outputs.tag }}", keep)
        self.assertIn("          path: |\n            release-notes.md\n            release-evidence/\n", keep)

    def test_a_rehearsal_keeps_every_release_file_under_its_version_and_tree(self) -> None:
        # Specification 24, "Release rehearsal on pull requests": the rehearsal keeps the files its release publishes as
        # `release-bundle-v<version>-<tree>` for 30 days, named after the tree of the commit it checked out. The naming
        # step runs here in a repository whose commit's tree is known. Catches a bundle that the plan cannot find (a name
        # other than release_plan.BUNDLE_NAME, or the tree of another commit), one that lacks a release folder the check
        # requires, and one that expires before the release pull request merges.
        named = steps(self.jobs["rehearse"])
        order = list(named)
        self.assertLess(order.index("Write the release files"), order.index("Keep the release bundle for its promotion"))
        naming = named["Name the release bundle after the rehearsed tree"]
        self.assertIn("RELEASE_SHA: ${{ needs.plan.outputs.sha }}", naming)
        self.assertIn("RELEASE_TAG: ${{ needs.plan.outputs.tag }}", naming)
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            git = lambda *args: subprocess.run(["git", *args], cwd=root, check=True, capture_output=True, text=True).stdout
            git("init", "-q")
            (root / "Cargo.toml").write_text("[workspace.package]\nversion = \"0.31.0\"\n")
            git("add", "Cargo.toml")
            git("-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "-m", "Release 0.31.0")
            commit, tree = git("rev-parse", "HEAD").strip(), git("rev-parse", "HEAD^{tree}").strip()
            output = root / "output"
            subprocess.run(["bash", "-c", run_block(naming)], cwd=root, check=True, capture_output=True, text=True,
                           env={**os.environ, "RELEASE_SHA": commit, "RELEASE_TAG": "v0.31.0", "GITHUB_OUTPUT": str(output)})
            self.assertEqual(output.read_text().strip(),
                             "name=" + release_plan.BUNDLE_NAME.format(tag="v0.31.0", tree=tree))
        keep = named["Keep the release bundle for its promotion"]
        self.assertIn("uses: actions/upload-artifact@", keep)
        self.assertIn("name: ${{ steps.bundle.outputs.name }}", keep)
        kept = keep.split("          path: |\n", 1)[1].split("\n          retention-days:", 1)[0].split()
        self.assertEqual(kept, [f"{folder}/" for folder in bundle.FOLDERS] + [str(bundle.TEXT)])
        self.assertGreaterEqual(int(field(keep, "retention-days", 10)), 14)
        self.assertEqual(field(keep, "compression-level", 10), "0")
        self.assertEqual(field(keep, "if-no-files-found", 10), "error")

    def test_a_promoted_release_checks_the_rehearsals_files_before_it_publishes(self) -> None:
        # Specification 24, "Release after main CI": the Publish job takes the one artifact the plan chose, from the
        # rehearsal run it names, checks every file against the rehearsal's SHA256SUMS and provenance, and only then
        # writes the release's provenance and checksums, tags and publishes. Catches a promotion that publishes files it
        # did not check, downloads another run's or artifact's files, or still takes this run's build artifacts.
        publish = steps(self.jobs["publish"])
        write = publish["Write the release files"]
        self.assertIn("rehearsal-run: ${{ needs.plan.outputs.rehearsal_run }}", write)
        self.assertIn("rehearsal-artifact: ${{ needs.plan.outputs.rehearsal_artifact }}", write)
        order = list(publish)
        self.assertLess(order.index("Write the release files"), order.index(PUBLISHING[0]))
        self.assertIn("rehearsal_run: ${{ steps.plan.outputs.rehearsal_run }}", self.jobs["plan"])
        self.assertIn("rehearsal_artifact: ${{ steps.plan.outputs.rehearsal_artifact }}", self.jobs["plan"])

        action = action_steps(RELEASE_FILES.read_text(encoding="utf-8"))
        order = list(action)
        promoted, built = "if: ${{ inputs.rehearsal-run != '' }}", "if: ${{ inputs.rehearsal-run == '' }}"
        for name in ("Download VSIX artifacts", "Download runtime artifacts", "Download trust-lsp artifacts",
                     "Download toolchain bundles", "Download release conformance artifact",
                     "Generate result-derived conformance status"):
            self.assertIn(built, action[name], name)
        take = action["Take the rehearsal's release files"]
        self.assertIn(promoted, take)
        self.assertIn("uses: actions/download-artifact@", take)
        for line in ("run-id: ${{ inputs.rehearsal-run }}", "artifact-ids: ${{ inputs.rehearsal-artifact }}",
                     "github-token: ${{ github.token }}", "path: ${{ runner.temp }}/rehearsal-bundle"):
            self.assertIn(line, take)
        self.assertNotIn("name:", take.split("with:", 1)[1], "the artifact the plan chose, by its id")
        check = action["Check the rehearsal's files against their checksums"]
        self.assertIn(promoted, check)
        self.assertIn('tree=$(git rev-parse "${RELEASE_SHA}^{tree}")', check)
        self.assertIn("python3 scripts/check_release_bundle.py", check)
        for argument in ('--bundle "${RUNNER_TEMP}/rehearsal-bundle"', '--tag "${RELEASE_TAG}"', '--tree "${tree}"',
                         '--run-id "${REHEARSAL_RUN}"', "--set-aside rehearsal-evidence"):
            self.assertIn(argument, check)
        self.assertEqual(
            order[order.index("Take the rehearsal's release files"):],
            ["Take the rehearsal's release files", "Check the rehearsal's files against their checksums",
             "Generate release artifact provenance", "Generate release checksums", "Generate release body from changelog"],
        )
        provenance = action["Generate release artifact provenance"]
        self.assertIn("REHEARSAL_PROVENANCE: ${{ inputs.rehearsal-run != '' && "
                      "'rehearsal-evidence/release-provenance.json' || '' }}", provenance)
        self.assertIn('--rehearsal-provenance "${REHEARSAL_PROVENANCE}"', provenance)
        self.assertIn('--tree "${tree}"', provenance)
        self.assertIn("cmp rehearsal-evidence/release-notes.md release-notes.md",
                      action["Generate release body from changelog"])

    def test_the_jobs_that_run_for_each_plan(self) -> None:
        # Specification 24. Each case runs the workflow's own job conditions for what the plan decides.
        results = lambda event, outputs, **kwargs: simulate(self.jobs, event=event, outputs=outputs, **kwargs)
        ran = lambda found: {job for job, result in found.items() if result != "skipped"}

        # A promoted release builds and tests nothing of its packages, runs the real-database matrix on main's commit and
        # publishes the rehearsal's files. Catches one that builds every package again (the hours the owner's decision of
        # 2 November 2026 saves), one that never publishes, and one that publishes without the matrix, which no pull
        # request's rehearsal runs (owner, 4 November 2026).
        promoted = results("push", PROMOTED)
        self.assertEqual(ran(promoted), {"plan", "release-preflight", "openot-real-databases", "publish", "conclusion"})
        self.assertTrue(concluded(promoted))
        failed_preflight = results("push", PROMOTED, ended={"release-preflight": "failure"})
        self.assertEqual(failed_preflight["publish"], "skipped", "a failed preflight stops a promotion")
        for end in ("failure", "cancelled"):
            stopped = results("push", PROMOTED, ended={"openot-real-databases": end})
            self.assertEqual(stopped["publish"], "skipped", f"a {end} matrix stops a promotion")
            self.assertFalse(concluded(stopped))
        # The release after main's CI of a commit whose tree a passed rehearsal built promotes it the same way, so that
        # both paths publish the same bytes. Catches one that builds and publishes other packages.
        self.assertEqual(ran(results("workflow_run", PROMOTED)), ran(promoted))

        # A release after main's CI builds and tests everything, the real-database matrix included, and publishes only
        # when all passed. Catches a tag after a failed or cancelled build (0.25.0, 25 October 2026).
        built = {**PROMOTED, "rehearsal_run": "", "rehearsal_artifact": ""}
        after_ci = results("workflow_run", built)
        self.assertEqual(ran(after_ci), {"plan", "release-preflight", "publish", "conclusion"} | BUILDS)
        for job in BUILDS:
            for end in ("failure", "cancelled"):
                stopped = results("workflow_run", built, ended={job: end})
                self.assertEqual(stopped["publish"], "skipped", f"{job} {end}")
                self.assertFalse(concluded(stopped))
        self.assertEqual(results("workflow_run", built, cancelled=True)["publish"], "skipped")

        # A rehearsal runs every build and test and keeps its files only when all passed; a pull request's rehearsal runs
        # no real-database matrix, which its promotion runs on main. Catches the rehearsal that kept its files after a
        # cancelled darwin-arm64 build (0.30.0's first attempt, 3 November 2026), one that keeps nothing because the
        # matrix did not run, and a manual rehearsal without the matrix.
        rehearsal = {"mode": "rehearsal", "sha": "head", "tag": "v0.31.0", "rehearsal_run": "", "rehearsal_artifact": ""}
        rehearsed = results("pull_request", rehearsal)
        self.assertEqual(ran(rehearsed), {"plan", "release-preflight", "rehearse", "conclusion"}
                         | (BUILDS - {"openot-real-databases"}))
        self.assertTrue(concluded(rehearsed))
        manual = results("workflow_dispatch", rehearsal)
        self.assertEqual(ran(manual), {"plan", "release-preflight", "rehearse", "conclusion"} | BUILDS)
        for job in BUILDS - {"openot-real-databases"}:
            for end in ("failure", "cancelled"):
                stopped = results("pull_request", rehearsal, ended={job: end})
                self.assertEqual(stopped["rehearse"], "skipped", f"{job} {end}")
        for end in ("failure", "cancelled"):
            stopped = results("workflow_dispatch", rehearsal, ended={"openot-real-databases": end})
            self.assertEqual(stopped["rehearse"], "skipped", f"the matrix {end}")

        # A run that the plan skips, as for a fork's pull request, a merge group or a push without a rehearsal of its
        # tree, runs nothing on any runner, the project's own included.
        for event in ("pull_request", "merge_group", "push", "workflow_run"):
            self.assertEqual(ran(results(event, {"mode": "skip", "rehearsal_run": ""})), {"plan", "conclusion"}, event)

    def test_one_job_at_a_time_publishes_from_main_and_checks_first_that_no_run_released(self) -> None:
        # Specification 24, "A release never happens twice". The runs after main's CI hold `release-main`; a push's run
        # plans in a group of its own, so that it never replaces a waiting release after CI, and its Publish job joins
        # `release-main`, a group the job of a run after CI must not repeat (GitHub cancels a job whose group is its
        # run's). The job then asks whether a Release run of its commit verified the release before it writes anything.
        # Catches a promotion that publishes while a release after main's CI of the same commit runs, and a re-run that
        # publishes a released version again.
        workflow_group = self.text.split("\nconcurrency:\n", 1)[1].split("\n\nenv:", 1)[0]
        self.assertIn("(github.event_name == 'workflow_run' && 'release-main'", workflow_group)
        self.assertIn("(github.event_name == 'push' && format('release-push-{0}', github.sha)", workflow_group)
        publish = self.jobs["publish"]
        self.assertIn(
            "    concurrency:\n"
            "      group: ${{ github.event_name == 'push' && 'release-main' || format('release-publish-{0}', github.run_id) }}\n"
            "      cancel-in-progress: false\n",
            publish,
        )
        names = list(steps(publish))
        self.assertEqual(names[0], "Check that no run has released this version")
        check = steps(publish)[names[0]]
        self.assertIn('python3 scripts/release_plan.py --repository "${GITHUB_REPOSITORY}" '
                      '--check-unreleased "${RELEASE_SHA}"', check)
        self.assertIn("GITHUB_TOKEN: ${{ github.token }}", check)
        checkout = [text for text in step_texts(publish) if "uses: actions/checkout@" in text][0]
        self.assertIn("fetch-depth: 0", checkout, "the check reads the commit's date")

    def test_a_run_publishes_nothing_over_other_packages_of_its_version(self) -> None:
        # Specification 24, "A release never happens twice": before the tag, Publish compares the packages already on a
        # GitHub release of the tag with its own. Catches a Publish job that tags or uploads first, and one that never
        # compares, so that a run continuing another one replaces published packages with other bytes (review finding
        # of 4 November 2026).
        named = steps(self.jobs["publish"])
        order = list(named)
        check = "Check that a published release holds the same packages"
        self.assertLess(order.index("Write the release files"), order.index(check))
        self.assertLess(order.index(check), order.index(PUBLISHING[0]))
        self.assertIn('python3 scripts/check_published_release.py --repository "${GITHUB_REPOSITORY}" '
                      '--tag "${RELEASE_TAG}"', named[check])
        self.assertIn("GITHUB_TOKEN: ${{ github.token }}", named[check])
        self.assertEqual(field(named[check], "if"), "", "it runs whenever Publish runs")

    def test_a_rehearsal_records_the_commit_its_workflow_ran_from(self) -> None:
        # Specification 24: the plan learns which commit's workflow a rehearsal ran from GitHub's record of the local
        # reusable workflow that the Release workflow calls, which GitHub resolves at that commit. Catches a Release
        # workflow that no longer calls one: no rehearsal would qualify, and every release would build again.
        called = re.findall(r"(?m)^    uses: (\./\.github/workflows/[A-Za-z0-9_.-]+\.ya?ml)$", self.text)
        self.assertTrue(called)
        for path in called:
            self.assertTrue((ROOT / path[2:]).is_file(), path)

    def test_a_run_after_ci_is_named_by_the_commit_it_releases(self) -> None:
        # The completeness check finds a run after main's CI by its name, since GitHub gives it main's newest commit as
        # its head. Catches a run-name that the check no longer recognises: it would publish a released version again.
        run_name = self.text.split("\nrun-name: >-\n", 1)[1].split("\n\non:", 1)[0]
        self.assertIn(
            f"format('{release_plan.AFTER_CI_RUN_NAME.format(sha='{0}')}', github.event.workflow_run.head_sha)", run_name
        )

    def test_a_rehearsal_holds_no_token_that_writes(self) -> None:
        # Catches a rehearsal whose candidate code, or what it installs, could read the Marketplace token
        # or write with the GitHub token (Codex's review of #146): it runs only for a rehearsal, reads, and
        # keeps no credential in its checkout. Only a release runs the Publish job.
        rehearse = self.jobs["rehearse"]
        self.assertIn("needs.plan.outputs.mode == 'rehearsal'", condition(rehearse) or "")
        self.assertIn("    permissions:\n      contents: read\n      actions: read\n", rehearse)
        self.assertNotIn("secrets.", rehearse)
        checkout = [text for text in step_texts(rehearse) if "uses: actions/checkout@" in text][0]
        self.assertIn("persist-credentials: false", checkout)
        for mode in ("rehearsal", "package", "skip"):
            for event in ("pull_request", "workflow_dispatch", "push", "workflow_run"):
                found = simulate(self.jobs, event=event, outputs={"mode": mode, "rehearsal_run": ""})
                self.assertEqual(found["publish"], "skipped", (mode, event))

    def test_only_the_marketplace_steps_get_the_marketplace_token(self) -> None:
        # Catches the token in a job's environment, where every step of the candidate reads it.
        self.assertEqual(self.text.count("secrets.VSCE_PAT"), 2)
        self.assertIn("secrets.VSCE_PAT", steps(self.jobs["release-preflight"])["Require the Marketplace token"])
        self.assertIn("secrets.VSCE_PAT", steps(self.jobs["publish"])["Publish to VS Code Marketplace"])
        self.assertNotIn("secrets.", RELEASE_FILES.read_text(encoding="utf-8"))

    def test_the_project_runner_never_runs_pull_request_code(self) -> None:
        # Specification 24: the real-database matrix runs on the project's own runner in a maintainer's manual rehearsal
        # and in every release, a promoted one included, on main's commit; a pull request or a merge group, whose code it
        # would run, never reaches that runner, nor does any run the plan skips, and the matrix is given no secret
        # (Codex's review of #146; owner, 4 November 2026). Catches the matrix in a pull request's rehearsal and a
        # promotion without it.
        runs = lambda event, mode, rehearsal_run="": simulate(
            self.jobs, event=event, outputs={"mode": mode, "rehearsal_run": rehearsal_run})["openot-real-databases"]
        self.assertEqual(runs("pull_request", "rehearsal"), "skipped")
        self.assertEqual(runs("merge_group", "rehearsal"), "skipped")
        self.assertEqual(runs("workflow_dispatch", "rehearsal"), "success")
        self.assertEqual(runs("workflow_run", "release"), "success")
        self.assertEqual(runs("push", "release", "41707764199"), "success", "a promotion runs it on main")
        self.assertEqual(runs("pull_request", "package"), "skipped")
        for event in ("pull_request", "merge_group", "push", "workflow_run"):
            self.assertEqual(runs(event, "skip"), "skipped", event)
        databases = self.jobs["openot-real-databases"]
        self.assertNotIn("secrets", databases)
        self.assertNotIn("secrets.", (ROOT / ".github" / "workflows" / "openot-real-databases.yml").read_text())

    def test_the_verification_outlives_its_marketplace_wait(self) -> None:
        # A job that GitHub stops first fails the release while the Marketplace still lists it.
        limit = int(field(self.jobs["publish"], "timeout-minutes", 4))
        self.assertGreaterEqual(limit, verifier.DEFAULT_MARKETPLACE_WAIT_SECONDS / 60 + 30)

    def test_one_conclusion_fails_on_any_failed_or_cancelled_job(self) -> None:
        # Catches a run that shows success while a job failed, and one that fails when the plan
        # skipped every job.
        conclusion = self.jobs["conclusion"]
        self.assertEqual(needs(conclusion), set(self.jobs) - {"conclusion"})
        self.assertEqual(field(conclusion, "if", 4), "${{ !cancelled() }}")
        self.assertIn('all(.[]; .result == "success" or .result == "skipped")', conclusion)

    def test_releases_run_one_at_a_time_and_a_push_replaces_a_rehearsal(self) -> None:
        concurrency = self.text.split("\nconcurrency:\n", 1)[1].split("\n\njobs:", 1)[0]
        self.assertIn("'release-main'", concurrency)
        self.assertIn("cancel-in-progress: ${{ github.event_name == 'pull_request' }}", concurrency)

    def test_the_release_workflow_does_not_wait_on_ci(self) -> None:
        # It starts after CI instead, and builds its own conformance result.
        self.assertNotIn("ci_run_id", self.text)
        self.assertNotIn("gh run download", self.text)
        self.assertIn("name: Runtime Validation", self.text)
        self.assertIn("name: release-conformance", self.text)


class MainPushAlignmentTests(unittest.TestCase):
    def test_the_ci_version_guard_checks_alignment_and_waits_for_nothing(self) -> None:
        # Catches the wait for a hand-tagged release, which failed main's CI after its 90 minutes
        # while 0.25.1's release still ran (29 October 2026).
        guard = jobs(CI.read_text(encoding="utf-8"))["version-release-guard"]
        self.assertIn("python3 scripts/check_release_version_alignment.py", guard)
        self.assertNotIn("check_version_release_evidence", guard)
        self.assertLessEqual(int(field(guard, "timeout-minutes", 4)), 15)



class MainPushConcurrencyTests(unittest.TestCase):
    def test_the_pushes_to_main_run_one_at_a_time(self) -> None:
        """Specification 22: CI, Render Diagrams and Docs Captures run the pushes to `main` one at a
        time, a newer push replacing a run that still waits, and cancel only a pull request's
        unfinished run. Catches a full run queued per merge ahead of the release pull request (six
        merges queued about 120 CI jobs on 1 November 2026), and a running check of `main` cancelled,
        whose commit the release would then never follow."""
        for name in ("ci.yml", "diagrams.yml", "docs-captures.yml"):
            text = (ROOT / ".github" / "workflows" / name).read_text(encoding="utf-8")
            block = text.split("\nconcurrency:\n", 1)[1].split("\n\n", 1)[0]
            self.assertIn("github.event_name == 'push' && github.ref", block, name)
            self.assertIn("cancel-in-progress: ${{ github.event_name == 'pull_request' }}", block, name)


if __name__ == "__main__":
    unittest.main()
