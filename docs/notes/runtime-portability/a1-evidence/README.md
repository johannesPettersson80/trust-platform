# A1 retained evidence

Run 3 passed all 24 planned steps. These files are byte copies of existing evidence, not new test executions. The base commit was `9a15065725c17da2c912055f1509368d3fd01d6c` with uncommitted source identified by `frozen-run3.json`; it is not a clean-commit validation record.

- [Ledger](run-3-ledger.tsv), [commands](run-3-commands.txt), [environment](run-3-environment.txt), [result](run-3-result.txt).
- [Native hosted tests](run-3-runtime-tests.txt): both cataloged periodic tests pass, including 40 activations/zero overruns for a 25 ms task on exact 10 ms logical samples.
- [Core tests](run-3-core-tests.txt): readiness and portable foundations pass.
- [Frozen file manifest](frozen-run3.json) and [copied-artifact hashes](artifact-sha256.json).
- [Reviewer messages and provenance limits](review-record.md).
- Earlier failed batch ledgers: [run 1](run-1-ledger.tsv), [run 2](run-2-ledger.tsv).

The registered closeout is supporting evidence (`proof_kind = none`), not a producer-authenticated green/red/lock proof. It closes the scheduling specification conflict using mapped native tests and retained logs, without raising the formal invariant proof level. The schema term `committed_file` identifies the intended repository-file evidence format; these new files are still uncommitted worktree changes.

The subsequent bookkeeping correction changes documentation and metadata after run 3. No run 4 or metadata-validator run was performed for it; the frozen manifest intentionally describes the tested snapshot rather than these later records. The full report-census suite remains known to contain separate historical drift. No assertion that all repository metadata now validates is made.

A1 is not push-ready: full runtime unit tests, complete LSP/debug suites and `just test-all` were not run as A1 gates; required pre-push/release checks still apply. Native macOS/Windows builds of the updated native crypto dependency and physical board qualification remain unverified. Commit from the A1 worktree, not the older primary checkout. No commit is authorized by this record.
