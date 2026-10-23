# A4 run 5 evidence reconciliation

Read-only reconciliation on 2026-10-10, by the assignment/reference correction
agent. This reviews saved execution evidence and source identity; it is not an
independent correctness review of this agent's own implementation. No build,
test, formatter, validator or mutation discovery was launched for this review.
Only this review note was written.

Canonical AGENTS.md, CLAUDE.md and the complete `.codex/skills` tree match
`/home/johannes/projects/trust-platform` in the A4 worktree
`/home/johannes/projects/trust-platform-portability-a4`, branch
`feat/runtime-portability-a4`, base `77b91381fcf1b1850611f88b397bbfe8d45e3523`.

## Evidence reconciled

Raw evidence: `.artifacts/runtime-portability-a4/run-5/raw`.
The ledger has 28 required and 2 advisory rows, all PASS with exit 0;
`result.txt` reports zero required failures. STARTED/environment and FINISHED
record the completed Rust 1.95.0 builder run, ending at 06:52:06 UTC.

| Saved log | Passed assertions | Result summaries | Failed / ignored |
| --- | ---: | ---: | --- |
| core-all-features.txt | 301 | 27 | 0 / 0 |
| core-portable.txt | 263 | 27 | 0 / 0 |
| core-i686.txt | 183 | 6 | 0 / 0 |
| runtime-unit.txt | 3841 | 1 | 0 / 0 |
| runtime-integration.txt | 525 | 64 | 0 / 0 |

Counts are sums of the logs' native result summaries, not a claim that different
feature/target runs contain disjoint tests. Provenance-helper tests report 4
passes; focused mutation tooling reports 16 and mutation contract tooling 20.
Advisory metadata reports 1029 validated records. Other gate outcomes agree
with the complete ledger; earlier failed runs remain failed historical evidence.

## Source identity

`formatted-source-manifest.json` has 478 records and SHA-256
`c14b104c6b93d047148b87af4beb26a1fb20d920687dc8f26011f0cdbc8a9458`.
All 210 crate-path records match current local source and the tested archive:
175 files match their byte hashes, and all 35 null/deletion records remain
absent from both local source and the archive. No crate mismatch was found.

The saved `source-snapshot.tar.gz` SHA-256 is
`014970fdbb4a06e7dec1eb63c59349624617bf7d021b9d824272ada8a99302cd`.

Compared all seven paths in `run-4-reference-types-review.json` between
`../pre-format-source.tar.gz` and the tested source archive. Each pre-format
file reproduces its independently reviewed SHA. Policy and spec 12 are
byte-identical. The other five diffs contain formatting only: wrapping,
indentation, optional trailing commas, blank lines, and alphabetic ordering
of the two sibling module declarations. No semantic correction was introduced
after that independent source review.

## Disposition and limits

No evidence or source-identity blocker found for the run 5 scope result.
Closeout documentation is being updated separately after the frozen run and is
not covered by the crate parity statement. This reconciliation is not an
exact-SHA release guard, Windows/macOS native execution, MCU firmware link or
board execution. No commit, push, release or later scope is authorized by it.
