"""The docs site takes the newest screenshots of main (specification 22, "Docs Captures"). Each
expected choice follows from the runs, artifacts and files each test gives."""

from __future__ import annotations

import os
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import docs_captures_artifact as captures  # noqa: E402

REPO = "owner/trust-platform"
KEPT = [{"name": "docs-captures", "expired": False}]


def run(run_id: int, event: str = "push", **changes) -> dict:
    value = {
        "id": run_id,
        "path": ".github/workflows/docs-captures.yml",
        "event": event,
        "conclusion": "success",
        "head_branch": "main",
        "head_repository": {"full_name": REPO},
    }
    value.update(changes)
    return value


def write(path: Path, data: bytes, modified: float) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    os.utime(path, (modified, modified))


class DocsCapturesArtifactTests(unittest.TestCase):
    def test_the_newest_screenshots_of_a_trusted_main_run_are_taken(self) -> None:
        # GitHub lists the newest run first: the schedule's run 2 wins over the push's run 1.
        runs = [run(2, "schedule"), run(1, "push")]
        self.assertEqual(captures.newest_run(REPO, runs, {1: KEPT, 2: KEPT}.__getitem__), 2)

    def test_a_pull_request_a_fork_another_branch_or_a_failed_run_never_counts(self) -> None:
        # Catches a site published with screenshots that a pull request's code rendered, which would
        # let any pull request change the published site, and with a failed or expired upload.
        runs = [
            run(10, "pull_request"),
            run(11, head_repository={"full_name": "someone/fork"}),
            run(12, conclusion="failure"),
            run(13, path=".github/workflows/other.yml"),
            run(14, head_branch="work/next"),
            run(15),
            run(16),
            run(17),
        ]
        artifacts = {
            10: KEPT,
            11: KEPT,
            12: KEPT,
            13: KEPT,
            14: KEPT,
            # Run 15 kept only an expired upload; run 16's change detection skipped the captures.
            15: [{"name": "docs-captures", "expired": True}],
            16: [{"name": "other", "expired": False}],
            17: KEPT,
        }
        self.assertEqual(captures.newest_run(REPO, runs, artifacts.__getitem__), 17)

    def test_without_any_screenshots_of_main_the_images_in_git_stay(self) -> None:
        self.assertIsNone(captures.newest_run(REPO, [run(10, "pull_request")], {10: KEPT}.__getitem__))
        self.assertIsNone(captures.newest_run(REPO, [], {}.__getitem__))

    def test_only_images_are_copied_each_to_its_own_path(self) -> None:
        with tempfile.TemporaryDirectory() as source, tempfile.TemporaryDirectory() as dest:
            source_path, dest_path = Path(source), Path(dest)
            write(source_path / "terminal" / "ctl-status.gif", b"GIF89a", 1000)
            write(source_path / "browser" / "ide.png", b"\x89PNG", 1000)
            write(source_path / "notes.txt", b"not an image", 1000)
            copied = captures.copy_images(source_path, dest_path)
            self.assertEqual(copied, ["browser/ide.png", "terminal/ctl-status.gif"])
            self.assertEqual((dest_path / "terminal" / "ctl-status.gif").read_bytes(), b"GIF89a")
            self.assertFalse((dest_path / "notes.txt").exists())

    def test_the_artifact_holds_only_what_the_captures_wrote(self) -> None:
        # Catches an artifact that also carries the images nobody renders: laid over a newer commit,
        # an older artifact would put back an older brand or hand-made image.
        with tempfile.TemporaryDirectory() as folder:
            images, out, mark = Path(folder) / "images", Path(folder) / "out", Path(folder) / "mark"
            write(images / "brand" / "logo.png", b"brand", 1000)
            write(mark, b"", 2000)
            write(images / "terminal" / "ctl-status.gif", b"GIF89a", 3000)
            write(images / "browser" / "ide.png", b"\x89PNG", 3000)
            write(images / "browser" / "notes.txt", b"not an image", 3000)
            self.assertEqual(captures.collect(images, mark, out), 0)
            self.assertEqual(
                sorted(path.relative_to(out).as_posix() for path in out.rglob("*") if path.is_file()),
                ["browser/ide.png", "terminal/ctl-status.gif"],
            )

    def test_collecting_nothing_fails(self) -> None:
        # The captures ran but wrote no screenshot: the run must fail instead of uploading nothing.
        with tempfile.TemporaryDirectory() as folder:
            images, out, mark = Path(folder) / "images", Path(folder) / "out", Path(folder) / "mark"
            write(images / "terminal" / "ctl-status.gif", b"GIF89a", 1000)
            write(mark, b"", 2000)
            self.assertEqual(captures.collect(images, mark, out), 1)


if __name__ == "__main__":
    unittest.main()
