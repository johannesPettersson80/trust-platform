# P18 portable numeric-kind authority

Date: 2026-08-13
Active rows: `VERIF-P18-003`, `VERIF-P18-005`

## Scope

This focused slice retires the remaining portable numeric-kind authority gap
without changing the public `trust_runtime_core::numeric` API or cataloged test
identities. Numeric kind classification and the closed widening matrix move to
`numeric/kind.rs`; execution-domain conversions remain in `numeric.rs`.

The work is behavior-preserving. It does not claim assertion reachability,
mutation sensitivity, broad workspace validation, or Phase 18 completion.

## Trace boundary and ownership split

The first nine-test trace request stopped before compilation because its closed
source route excluded `trust-runtime-core/src/error.rs` and the nested value
construction test module. The reduced six-test run at
`d53471c24114e915134df9d47d1610b297850e85` completed all six commands with
exit code zero. Its artifact is
`sha256:3998b4c67acb1676c60a5892dca0645695ccbcc968d85e54487c7ce1edbeacf1`.
It produced no executable authority row for the public `NumericKind` type,
because a type declaration is not a runtime execution point. The trace was
therefore retained as negative routing evidence and was not promoted into
semantic proof.

The implementation instead separates the exact semantic owner. Commit
`6585f2f4ff60843f1e3cea416033318ab384783d` adds
`numeric/kind.rs` for `NumericKind`, `numeric_kind`, `wider_numeric`, and the
private widening predicate. `numeric.rs` keeps normalization/conversion and
reexports the same public API. The cataloged test names remain under
`numeric::tests`. The runtime execution diagram and manifest were regenerated;
diagram drift passed. Targeted rustfmt and
`cargo check -p trust-runtime-core --no-default-features` passed. A package-wide
fmt check exposed unrelated pre-existing formatting drift outside this slice
and did not rewrite it.

## Identity refresh

The refresh prerequisite first corrected three out-of-order denominator review
rows. Its 19 focused denominator/candidate tests passed, and commit
`58017a96` records only that canonical ordering repair.

At `6585f2f4`, the clean 17,489-fact report contained exactly 16 findings: four
stale pre-split identities and their four missing successors, with two messages
per identity. The refresh proposal is
`sha256:5a61e7390f02b6f7171815322353844940dbfa4fbfdcb9375466c03fa4c54da5`
with operation-set digest
`sha256:fe48c63cbd71fabefd9625155d9b8b0fff088ee1ef303ca51cacbb1e3e194364`.

An attempted automatic transfer was rejected in check mode because moving to a
new path and qualified symbol is not an exact successor. It made no changes.
The corrected complete selection,
`sha256:5d34e10093e6122d2562351eee81536b2d1c7df31c4b0893898e3670d0b2399c`,
removes all four stale identities and registers all four successors against the
existing historical runtime-safety gap. Check mode and the atomic live
post-state validator passed. The application is
`sha256:ae884a9c76975cf32b0912f431b7fc65afdf918caa22eed13f1f620f4970fe0a`;
34 focused refresh/contract tests passed. Commit `e803fedc` is the recoverable
identity checkpoint.

## Semantic review

The clean report at
`e803fedc86e3152632e254da9f0bf4458a17a5ac` contains 17,489 facts, zero
findings, 13,840 `spec_gap`, and 3,389 `specified`. Its complete review-only
proposal contains 25 groups: 100 singular candidate facts, 95 ambiguous
candidate facts, and 13,645 facts with no candidate.

Only `CODEAUTH_GROUP_EC508E...`, the four-fact
`crates/trust-runtime-core/src/numeric/kind.rs` group, is accepted. Its only
candidate is `RT_CORE_NUMERIC_KIND_WIDENING_001`, whose exact documented
decision table and exhaustive cataloged behavior lock govern all four facts.
The other 24 groups retain their prior `ambiguous_authority`, `mixed_semantics`,
or `claim_mismatch` decisions unchanged.

The proposal is
`sha256:2f2031fc5bd98b802bbf0861063da02de810e11cb800668c5865b6e8f7e1344d`
with operation-set digest
`sha256:ace4cc124411a1f769466f005add6228e4564fb29c442bb65868b1335b079ec6`.
The complete selection is
`sha256:ff35aa91772f950f1affda1b4947dbd370871119412a0c06270cab1e70feaa97`.
Check mode and the atomic live reconstruction passed: one group accepted, 24
withheld, and four mappings promoted. The application artifact is
`sha256:eda97da22f23f3071910ec569fa5afdf94a7d09ec4888be4ddc7a88f91519837`.

The post-authority report at
`aae1beef07947569f8df1c475faf7fce3723c8a3` is
`sha256:32d669b0e2ff13d6198de13e2c48cf176cf1112ce8a43c5fd5ce910244a2937f`:
17,489 facts, zero findings, 13,836 `spec_gap`, and 3,393 `specified`. Relative
to the prior portable-core closeout, this retires the one remaining
`NumericKind` fact while preserving the three already specified function
facts through the guarded remove/register/promote route.

The invariant and evidence record use the governed `trust-runtime` product
owner, consistent with the four code mappings; `trust-runtime-core` remains the
test/component owner. This removes the slice's pre-existing owner-governance
mismatch rather than adding a waiver.

## Focused execution

On `scena-rust-builder` with Rust 1.97.0, Cargo 1.97.0, and nextest 0.9.140:

- Before and after the module split, the exact numeric-kind/widening test passed
  1/1 and the conversion-normalization behavior lock passed 1/1.
- After authority application, those same exact catalog commands passed 1/1
  each again.
- The focused authority-reuse, authority-candidate, and code-spec contract
  modules passed 23/23.
- The identity-refresh modules passed 34/34, and the ordering prerequisite
  modules passed 19/19.

These are focused behavior and metadata locks. The invariant stays at S0;
dynamic assertion reachability, mutation sensitivity, and the final broad
remote candidate remain open.

The canonical complete-slice planner was also run with `--intent refactor`
against all 14 changed paths from `d53471c2`. It stopped with exit 6 before
planning on 2,876 pre-existing repository-wide metadata errors. The full
metadata validator likewise remains red. After registering this evidence and
aligning the governed product owner, a slice filter over its 2,877-line output
contains no occurrence of this invariant, evidence ID, or evidence path. The
catalog-staleness checker remains globally red, including existing accepted
source-closure drift for many `numeric.rs` tests; that debt remains under
`VERIF-P18-005`. Passing the exact catalog command here does not claim that the
repository-wide catalog is current.

## SOLID, KISS, and DRY review

- SOLID: kind/widening owns one closed decision table; numeric conversions keep
  their independent execution-domain responsibility.
- KISS: one private module plus compatibility reexports preserves callers and
  test identities without adapters or duplicate public types.
- DRY: both metadata transactions use the repository's canonical scanner,
  proposal, selection, live reconstruction, and atomic projection tools.

## Bootstrap and proof boundary

Canonical source and local checkout:
`/home/johannes/projects/trust-platform`, branch
`plc-verification-program`. Isolated destination:
`trust-builder:/home/johannes/projects/trust-platform-p18-portable-d53471c2`.
Before evidence-producing work, the destination matched the active commit and
branch. `AGENTS.md` matched
`sha256:76576b5464443f4128a67c325527d23e59da46c019da7b79f39454dabc250263`;
the complete `.codex/skills/**` aggregate matched
`sha256:3f84c064eff30aca31d55c51034a05a7e2ae4a53d2c69fb6f1827585b7ac73a7`.

`VERIF-P18-003` and `VERIF-P18-005` remain globally open. No `just fmt`,
`just clippy`, `just test-all`, strict preflight, GitHub candidate matrix, or
`VERIF-P18-016` broad gate ran.
