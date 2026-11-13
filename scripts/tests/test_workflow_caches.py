"""Only the jobs that gain the most from a Rust cache keep one, and only a run on `main` saves it (specification 22,
"Pull-request checks and change detection").

GitHub keeps 10 GB of caches per repository, evicts the least recently used, and lets a pull request's cache serve
only that pull request. Pull requests that saved their own Rust caches pushed out `main`'s, which every pull request
restores: on 13 November 2026, 17 of the 21 Test jobs of the seven newest CI runs found no cache.

With only `main` saving, `main`'s own caches still exceeded the 10 GB, because every job that builds Rust kept one,
the scheduled ones too: at 18:55 UTC the Ubuntu Test, VS Code, HMI Screen Tests and Editor Expansion caches had been
evicted. Over six of `main`'s runs, a restored cache shortened each Test job by about 10 to 40 minutes, Conformance
and Clippy by about 12 to 14, Docs Captures' refresh by 7 and HMI Screen Tests by about 5, and every other job by
under five or not at all. Runtime Validation runs on `main` only when a release builds there, so it saves at most once per version.

Wrong behaviour caught: a Rust cache step that a pull request's run saves, in any workflow; a Rust cache in a job
outside the list, which pushes the caches past GitHub's 10 GB again; a job of the list that lost its cache."""

from __future__ import annotations

import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / ".github" / "workflows"
MAIN_ONLY = "${{ github.ref == 'refs/heads/main' }}"

# Specification 22: the jobs that keep a Rust cache, as `<workflow>#<job>`.
CACHED_JOBS = {
    "ci.yml#test",
    "ci.yml#conformance",
    "ci.yml#clippy",
    "ci.yml#hmi-screen-tests",
    "docs-captures.yml#refresh",
    "release.yml#runtime-vm-validation",
}


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
    def test_only_the_jobs_a_cache_saves_minutes_keep_one(self) -> None:
        self.assertEqual(set(rust_cache_steps()), CACHED_JOBS)

    def test_only_mains_runs_save_the_rust_caches(self) -> None:
        steps = rust_cache_steps()
        # A search that finds no step would pass without checking anything.
        self.assertIn("ci.yml#test", steps)
        for job, job_steps in steps.items():
            for step in job_steps:
                with self.subTest(job=job):
                    self.assertEqual((step.get("with") or {}).get("save-if"), MAIN_ONLY)


if __name__ == "__main__":
    unittest.main()
