"""The check of a rehearsal's kept files before a release promotes them (specification 24, "Release after main CI").

Each bundle is written as the Release workflow writes it: the packages of every platform, the conformance status, the
provenance from the real generator and SHA256SUMS from the release-files action's own script. The names are those of
0.30.0's release."""

from __future__ import annotations

import contextlib
import io
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import check_release_bundle as bundle  # noqa: E402
import generate_release_provenance as generator  # noqa: E402

ACTION = ROOT / ".github" / "actions" / "release-files" / "action.yml"
VERSION = "0.31.0"
TAG = f"v{VERSION}"
# 0.30.0's rehearsal: its run, the head it rehearsed and that head's tree, which its release commit shares.
RUN = "41707764199"
REHEARSED = "36ecae4797a0f81e64197fc6bc568cb4166f0d67"
TREE = "474f2f242b2039236ffb0116fbafc66bae0b811e"


def action_script(name: str) -> str:
    """The `run: |` script of the release-files action's step `name`, dedented as the runner sees it."""
    step = ACTION.read_text(encoding="utf-8").split(f"    - name: {name}\n", 1)[1].split("\n    - name: ", 1)[0]
    body = step.split("      run: |\n", 1)[1]
    return "\n".join(line[8:] for line in body.splitlines()) + "\n"


def package_names(platforms=generator.PLATFORMS) -> list[str]:
    names = []
    for platform in platforms:
        archive = "zip" if platform.startswith("win32") else "tar.gz"
        names += [
            f"vsix-artifacts/trust-lsp-{platform}-{VERSION}.vsix",
            f"runtime-artifacts/trust-runtime-{platform}.{archive}",
            f"trust-lsp-artifacts/trust-lsp-{platform}.{archive}",
            f"toolchain-artifacts/trust-toolchain-{platform}.tar.gz",
        ]
    return names


def write_records(root: Path, *, tag: str = TAG, tree: str = TREE, run: str = RUN) -> None:
    """The rehearsal's provenance and SHA256SUMS over the files the folders hold now."""
    record = generator.build_release_provenance(
        files=generator._collect_artifacts([root / folder for folder in bundle.FOLDERS]),
        tag=tag,
        commit=REHEARSED,
        tree=tree,
        workflow_run_id=run,
        workflow_run_url=f"https://github.com/johannesPettersson80/trust-platform/actions/runs/{run}",
        timestamp="2026-11-04T00:11:12+00:00",
    )
    (root / "release-evidence" / "release-provenance.json").write_text(
        json.dumps(record, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )
    subprocess.run(["bash", "-c", action_script("Generate release checksums")], cwd=root, check=True,
                   capture_output=True, text=True)


def write_bundle(root: Path, platforms=generator.PLATFORMS) -> None:
    for name in package_names(platforms):
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(f"{name}, built by rehearsal run {RUN}".encode())
    for folder in bundle.FOLDERS:
        (root / folder).mkdir(exist_ok=True)
    (root / "release-evidence" / "conformance-status.json").write_text('{"failed": 0}\n', encoding="utf-8")
    (root / "release-evidence" / "conformance-status.md").write_text("# Conformance\n", encoding="utf-8")
    write_records(root)
    (root / "release-notes.md").write_text(f"# {TAG}\n\n- Build once.\n", encoding="utf-8")


class ReleaseBundleTests(unittest.TestCase):
    def setUp(self) -> None:
        # The bundle as download-artifact extracts it into an empty folder, and a separate checkout that receives it.
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.root = Path(folder.name) / "rehearsal-bundle"
        self.root.mkdir()
        write_bundle(self.root)
        self.workspace = Path(folder.name) / "checkout"
        (self.workspace / "scripts").mkdir(parents=True)
        (self.workspace / "scripts" / "check_release_bundle.py").write_text("# the checker of the checkout\n")

    def promote(self) -> tuple[int, str]:
        errors = io.StringIO()
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(errors):
            status = bundle.main(["--bundle", str(self.root), "--tag", TAG, "--tree", TREE, "--run-id", RUN,
                                  "--set-aside", "rehearsal-evidence", "--root", str(self.workspace)])
        return status, errors.getvalue()

    def refused(self, reason: str, **kwargs: str) -> None:
        args = {"tag": TAG, "tree": TREE, "run_id": RUN, **kwargs}
        with self.assertRaisesRegex(bundle.BundleError, reason):
            bundle.check_bundle(self.root, **args)

    def test_the_files_a_rehearsal_keeps_pass_and_its_records_are_set_aside(self) -> None:
        # Catches a check that refuses the layout the rehearsal keeps, so that every promotion fails, and a promoted
        # release that publishes the rehearsal's provenance, which names the rehearsed commit and run, as its own.
        listed = bundle.check_bundle(self.root, tag=TAG, tree=TREE, run_id=RUN)
        self.assertEqual(len(listed), 23, "five platforms of four packages, two conformance files, the provenance")
        self.assertEqual(self.promote(), (0, ""))
        self.assertEqual(sorted(path.name for path in (self.workspace / "rehearsal-evidence").iterdir()),
                         ["SHA256SUMS", "release-notes.md", "release-provenance.json"])
        self.assertEqual(sorted(path.name for path in (self.workspace / "release-evidence").iterdir()),
                         ["conformance-status.json", "conformance-status.md"])
        for folder in bundle.FOLDERS[:4]:
            self.assertEqual(len(list((self.workspace / folder).iterdir())), 5, folder)
        self.assertFalse((self.workspace / "release-notes.md").exists())
        self.assertEqual(list(self.root.iterdir()), [], "everything moved out of the download folder")

    def test_a_bundle_entry_outside_the_release_folders_is_refused(self) -> None:
        # Review finding of 4 November 2026: download-artifact extracted the bundle into the checkout, so an entry
        # scripts/check_release_bundle.py could replace the checker before it ran. Catches a check that accepts, or a
        # download that lets into the checkout, any entry other than the release folders and the release text.
        (self.root / "scripts").mkdir()
        (self.root / "scripts" / "check_release_bundle.py").write_text("print('every bundle passes')\n")
        self.refused("the bundle holds scripts, which is no release file")
        status, errors = self.promote()
        self.assertEqual(status, 1)
        self.assertIn("the bundle holds scripts, which is no release file", errors)
        self.assertEqual((self.workspace / "scripts" / "check_release_bundle.py").read_text(),
                         "# the checker of the checkout\n")
        self.assertFalse((self.workspace / "vsix-artifacts").exists(), "nothing moves into the checkout")

    def test_a_release_folder_already_in_the_checkout_is_refused(self) -> None:
        # Catches a promotion that mixes the rehearsal's files with files already in the checkout.
        (self.workspace / "vsix-artifacts").mkdir()
        status, errors = self.promote()
        self.assertEqual(status, 1)
        self.assertIn("vsix-artifacts is in the workspace already", errors)
        self.assertTrue((self.root / "runtime-artifacts").is_dir(), "nothing moved")

    def test_a_file_changed_after_the_rehearsal_is_refused(self) -> None:
        # Catches a release that publishes bytes no rehearsal built and tested, or a download that broke a package.
        (self.root / "vsix-artifacts" / f"trust-lsp-linux-x64-{VERSION}.vsix").write_bytes(b"something else")
        self.refused(f"vsix-artifacts/trust-lsp-linux-x64-{VERSION}.vsix does not have the digest")

    def test_a_listed_file_that_is_missing_is_refused(self) -> None:
        # Catches a release that publishes fewer files than the rehearsal checked.
        (self.root / "toolchain-artifacts" / "trust-toolchain-win32-x64.tar.gz").unlink()
        self.refused("toolchain-artifacts/trust-toolchain-win32-x64.tar.gz is listed in release-evidence/SHA256SUMS "
                     "but is not a file")

    def test_a_file_that_the_checksums_do_not_list_is_refused(self) -> None:
        # The Publish job releases every `vsix-artifacts/*.vsix`. Catches a file the rehearsal never checked that would be
        # published with the others.
        (self.root / "vsix-artifacts" / "trust-lsp-linux-x64-0.30.0.vsix").write_bytes(b"an older package")
        self.refused("vsix-artifacts/trust-lsp-linux-x64-0.30.0.vsix is in the bundle but not in")

    def test_a_bundle_without_every_platform_is_refused(self) -> None:
        # 0.30.0's first rehearsal attempt kept its files although the darwin-arm64 build was cancelled (3 November
        # 2026). Catches a promoted release that lacks a platform although its checksums and provenance agree.
        with tempfile.TemporaryDirectory() as folder:
            self.root = Path(folder)
            write_bundle(self.root, [platform for platform in generator.PLATFORMS if platform != "darwin-arm64"])
            self.refused("does not hold exactly one vsix for darwin-arm64, runtime for darwin-arm64, "
                         "language_server for darwin-arm64, toolchain_bundle for darwin-arm64")

    def test_the_files_of_another_tag_tree_or_run_are_refused(self) -> None:
        # Catches a release that promotes the files of another version, of a commit with another tree, or of a run other
        # than the passed rehearsal the plan found.
        self.refused("not of v0.30.0", tag="v0.30.0")
        self.refused("not the released commit's tree " + "f" * 40, tree="f" * 40)
        self.refused("not by rehearsal run 41707764200", run_id="41707764200")

    def test_a_promoted_releases_files_are_refused(self) -> None:
        # Catches a chain of promotions: a release publishes only what a rehearsal built and tested.
        record_path = self.root / "release-evidence" / "release-provenance.json"
        record = json.loads(record_path.read_text(encoding="utf-8"))
        record_path.write_text(json.dumps({**record, "promoted": True}), encoding="utf-8")
        subprocess.run(["bash", "-c", action_script("Generate release checksums")], cwd=self.root, check=True,
                       capture_output=True)
        self.refused("not those of a promoted release")

    def test_a_path_outside_the_release_folders_is_refused(self) -> None:
        # Catches checksums that point the check, or the release, at a file outside the bundle's release folders.
        sums = self.root / "release-evidence" / "SHA256SUMS"
        listed = sums.read_text(encoding="utf-8")
        for name in ("../release-notes.md", "/etc/hostname", "release-notes.md", "vsix-artifacts/../release-notes.md"):
            sums.write_text(listed + f"{'0' * 64}  {name}\n", encoding="utf-8")
            self.refused("which is not a file of a release folder")

    def test_a_provenance_that_differs_from_the_checksums_is_refused(self) -> None:
        # Catches a record that vouches for other bytes than those checked: here the rehearsal's record gives a package a
        # digest that is not the file's, while SHA256SUMS lists the file's own.
        record_path = self.root / "release-evidence" / "release-provenance.json"
        record = json.loads(record_path.read_text(encoding="utf-8"))
        record["artifacts"][0]["sha256"] = "0" * 64
        record_path.write_text(json.dumps(record), encoding="utf-8")
        subprocess.run(["bash", "-c", action_script("Generate release checksums")], cwd=self.root, check=True,
                       capture_output=True)
        self.refused("does not list exactly the files of release-evidence/SHA256SUMS with their digests")

    def test_a_bundle_without_its_release_text_is_refused(self) -> None:
        (self.root / "release-notes.md").unlink()
        self.refused("the bundle has no release text")

    def test_a_refused_bundle_fails_the_step_and_says_why(self) -> None:
        # Catches a check that prints a warning and lets the release publish.
        (self.root / "runtime-artifacts" / "trust-runtime-linux-x64.tar.gz").write_bytes(b"other")
        status, errors = self.promote()
        self.assertEqual(status, 1)
        self.assertIn("::error::release bundle: runtime-artifacts/trust-runtime-linux-x64.tar.gz does not have", errors)
        self.assertFalse((self.workspace / "rehearsal-evidence").exists())
        self.assertFalse((self.workspace / "runtime-artifacts").exists(), "nothing unchecked moves into the checkout")


if __name__ == "__main__":
    unittest.main()
