# A4 run-1 correction source review

Independent source inspection found no actionable correction defect in the ten
paths pinned by [the manifest](run-1-correction-review.json). The six files authored
by this reviewer are explicitly excluded; this is not self-approval or execution proof.

- Assignment: extracted array/structure branches preserve type checks, charging,
  clone-before-recursion order, depth increments, canonical member order and errors.
  No initializer/default recipe invocation was introduced. Compared against the
  retained pre-format source and inspected the current helper bodies.
- Provenance: selector matching still requires function, genre and replacement;
  exact selection wins, a unique relocation is accepted, and missing/ambiguous
  matches fail before accumulated source writes. New synthetic Rust assertions
  exercise these paths and the improved id/path/count diagnostic. They are unrun.
- Retain mutation: active source owner now matches core/retain.rs, which the hosted
  retain snapshot imports. The hosted warm-restart oracle remains bound. Existing
  digest/selector text is intentionally stale until the planned batch refresh;
  no claim of successful discovery or current metadata validation is made.
- Architecture: the new VM namespace note describes real child modules, not a
  blanket function waiver. The three reported oversized functions are split in
  separate corrections. Existing moved dispatcher/materialization entries retain
  their prior ownership rationale; this review does not claim architecture passes.
- Lease scripts: all three current script bytes match the reviewed A1 integration
  copies. Shared path normalization/ownership checks, mounted-volume requirement,
  post-lock revalidation and busy-lease exit 75 are preserved. The missing helper
  dependency diagnosed by the run-1 guard tests is present. Scripts were not run.

Canonical AGENTS.md, CLAUDE.md and complete skills matched all 21 files before
work. Source and existing run-1 logs were read only; only these review records
were written. No builds, tests, formatter, validator, mutation discovery, script
execution, commits or pushes were performed during this review.

Aggregate source SHA256: `ed1c583387e92fd8cd66a3d349c97c93623ac0e27242520ab60a0f178280dc51`.
