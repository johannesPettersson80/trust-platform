"""Only a run on `main` saves the Rust build caches; any other run restores what `main` saved and saves none
(specification 22, "Pull-request checks and change detection").

GitHub keeps 10 GB of caches per repository and lets a pull request's cache serve only that pull request. Each pull
request saved its own Rust caches, about 7 GB, and pushed out `main`'s, which every pull request restores: on
13 November 2026, 17 of the 21 Test jobs of the seven newest CI runs, three of them `main`'s, found no cache and
compiled every dependency and cargo-nextest again. `main`'s Test jobs took 102, 59 and 59 minutes on macOS, Ubuntu and Windows without
the cache (run 43433092919) and 62, 37 and 51 with it (run 43409433450). Wrong behaviour caught: a Rust cache step that
a pull request's run saves, in any workflow."""

from __future__ import annotations

import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / ".github" / "workflows"
MAIN_ONLY = "${{ github.ref == 'refs/heads/main' }}"


def rust_cache_steps() -> dict[str, list[dict]]:
    """Every step of every workflow that uses the Rust cache action, by `<workflow>#<job>`."""
    found: dict[str, list[dict]] = {}
    for path in sorted(WORKFLOWS.glob("*.yml")):
        workflow = yaml.load(path.read_text(encoding="utf-8"), Loader=yaml.BaseLoader)
        for job_id, job in (workflow.get("jobs") or {}).items():
            for step in job.get("steps") or []:
                if str(step.get("uses", "")).startswith("Swatinem/rust-cache@"):
                    found.setdefault(f"{path.name}#{job_id}", []).append(step)
    return found


class RustCacheTests(unittest.TestCase):
    def test_only_mains_runs_save_the_rust_caches(self) -> None:
        steps = rust_cache_steps()
        # The CI workflow's Test jobs and the jobs of the other workflows that build Rust: a search that finds none
        # would pass without checking anything.
        self.assertIn("ci.yml#test", steps)
        self.assertIn("release.yml#runtime-vm-validation", steps)
        for job, job_steps in steps.items():
            for step in job_steps:
                with self.subTest(job=job):
                    self.assertEqual((step.get("with") or {}).get("save-if"), MAIN_ONLY)


if __name__ == "__main__":
    unittest.main()
