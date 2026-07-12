# Execution Slice 001 - Readiness and First Spec-to-Green Vertical

Status: readiness accepted; product execution queued.
Drafted: 2026-07-12 by the program reviewer (Fable), at the user's direction.
Reconciled: 2026-07-12 by the implementer after current-HEAD reproduction and
contract audit. Reviewer and implementer must remain different agents.
Readiness closure evidence:
[p16-execution-readiness-closure-validation.md](../../evidence/plc-verification-program/2026-07-12/p16-execution-readiness-closure-validation.md).
Independent acceptance:
[p16-execution-readiness-independent-acceptance.md](../../evidence/plc-verification-program/2026-07-12/p16-execution-readiness-independent-acceptance.md).

## Purpose

First close the missing execution contracts without changing product behavior:

    durable proof output -> clean revision ancestry -> honest trace cases
    -> evidence-bound promotion

Only then execute one proven product defect through the complete pipeline:

    written timer decisions -> state-machine trace cases -> cataloged runner
    -> durable red proof -> minimal TOF fix -> paired green
    -> broad remote gate -> evidence-supported promotion

This ordering prevents the first product proof from depending on hand-edited
evidence, ambiguous case provenance, or a proof level that metadata does not
actually enforce.

## Current-HEAD Reproduction

The original draft treated two review findings as known product defects. The
implementation audit changed that premise:

- `VM_SEAM_DECLARED_TYPE_001`: the alleged declared-storage defect is not
  reproducible on `25b05983a72df538f1d05f51ab5a1a08456905ba`. The existing
  store-normalization fix landed in `91d2ae09ab1d3daddfa31a504d766e5200e4c313`.
  Five declared-type runtime tests pass on the clean builder checkout,
  including integer-to-REAL, integer-to-DINT, and copy-in widening. This path
  is characterization/behavior-lock work. Do not manufacture red evidence or
  revert the fix.
- `IEC_TIMER_001`: the TOF defect is reproducible in source and existing tests.
  After TOF reaches `PT`, the next scan with `IN = FALSE` resets `ET` to zero;
  IEC 61131-3 Ed.3 section 6.6.3.5.5, Table 46, Figure 15(c) shows the
  post-expiry plateau at `PT` until the next rising input. TP already holds
  `ET = PT` while `IN = TRUE`; TON's basic hold is also conforming. TP
  short-input expiry and implementer-specific PT changes require reviewed
  scan-step decisions before any assertion.

The confirmed first product vertical is therefore TOF ET hold only. Declared
type remains in the wider Phase 16 backlog because its shared gap also owns
STRING bounds, subranges, reference stores, and the separately blocked stable
error model.

## Readiness Gate

No runtime/compiler/LSP/IDE/UI product file may change until all four readiness
rows `VERIF-P16-000` through `VERIF-P16-000C` are complete and independent
acceptance closes `VERIF-P16-000D`.

- `VERIF-P16-000`: policy and slice scope match the current-HEAD audit. A
  passing reproduction cannot be represented as red.
- `VERIF-P16-000A`: `prove.py` emits directly to a durable tracked evidence
  destination with a clean full SHA. Red and green revisions are distinct and
  the red revision is an ancestor of green. Agents do not rewrite proof rows.
- `VERIF-P16-000B`: hand-authored state-machine trace provenance is a validated
  mode distinct from `gen_cases.py v1`; case and run artifacts are
  closed-schema and digest-bound.
- `VERIF-P16-000C`: `G1`, `G2`, and `R1` are rejected unless targeted, broad
  remote, and release/public evidence respectively is present and linked.
- `VERIF-P16-000D`: the canonical report gate sees changed product paths and
  reports them blocked until independent readiness acceptance is recorded.
  The check is intentionally nonblocking in CI until `VERIF-P16-007`.

The readiness slice changes verification tooling, metadata schemas, program
documentation, tests, and durable `proof_kind = "none"` closure evidence only.
It does not create product proof, close a gap, promote an invariant, flip CI,
or change skills/agent instructions.

## Product Scope After Readiness

In scope:

- reviewed scan-step semantics for TP/TON/TOF and `*_LTIME`, limited to what
  the IEC text specifies plus explicit truST decisions for implementer-owned
  boundaries;
- a committed hand-authored timer trace case file and an artifact-producing
  shipped-path runner using real ST through `TestHarness::from_source`;
- a pre-fix red artifact whose cases include the failing TOF post-expiry hold
  and passing protective TP/TON observations;
- the minimal TOF product fix that preserves `ET = PT` after expiry while
  `IN = FALSE` until re-armed by a rising input;
- paired green proof, targeted and broad remote gates, and honest promotion;
- migration of the Phase 8 runtime-anomaly review from
  `existing_open_gap` to the resolved product source before the timer gap
  closes.

Out of scope:

- a declared-type product fix; current HEAD already passes that allegation;
- closing `SPEC_GAP_VM_VALUE_SEMANTICS_001` without resolving every owned
  surface and the stable error-model dependency;
- changing TP merely because it appeared in the old draft;
- asserting TP short-input, PT-change, restart, conditional-call, first-call,
  negative-time, or nonmonotonic-clock behavior before a reviewed decision;
- NaN ingress, warm-restart time reset, scan-thread panic containment,
  `REF(returnvar)` escape, rename/LSP invariants;
- CI enforcement, any guarded row not explicitly closed by this slice,
  `VERIF-STOP-012`, `VERIF-STOP-014`, or any skill/agent-instruction update.

## Product Preconditions

- The readiness gate above is complete and accepted.
- Clean branch from reviewed readiness HEAD; baseline commit recorded.
- `docs/internal/standards/iec61131-3.txt` is present. Cite Ed.3 section
  6.6.3.5.5, Table 46, and Figure 15; do not cite the local Ed.2 PDF as Ed.3.
- Timer decisions are committed before cases. At minimum decide PT resampling,
  `PT <= 0`, first-call delta, conditional/skipped calls, nonmonotonic/reset
  clock, warm/cold restart, TP retrigger/short-input scan boundary, and
  TIME/LTIME parity. Ambiguity goes to `docs/IEC_DECISIONS.md`; deliberate
  deviation goes to `docs/IEC_DEVIATIONS.md`.
- Remote-builder disk preflight per `AGENTS.md` before broad gates.

## Product Stop Rows

- `E1-STOP-001` No expected outcome is invented. Every case traces to the
  committed spec/decision/deviation and an oracle-eligible source.
- `E1-STOP-002` Red before fix. Product behavior does not change until durable
  producer-authentic red evidence exists at a clean pre-fix ancestor. If the
  case passes, stop and reclassify it as characterization.
- `E1-STOP-003` Product-code fence. The only product behavior change is the TOF
  implementation and its immediate state helper. Anything else stays a
  separately planned finding.
- `E1-STOP-004` Metadata freeze. Spec decisions, invariant behavior rows, case
  file, catalog row, command, and case digest are committed before red and do
  not change between red and green.
- `E1-STOP-005` Gap closure is atomic and last. Write the resolving spec while
  keeping the timer gap open, then close it only after runnable tests and
  closeout evidence exist, all live refs are removed, and the Phase 8
  open-gap contract has migrated in the same closure milestone.
- `E1-STOP-006` Promotion honesty. `G1` follows targeted green; `G2` follows a
  separately recorded broad remote gate. `validated` is forbidden while any
  applicable coverage cell remains `gap_open` or `spec_gap`.
- `E1-STOP-007` Scan-cycle honesty. Tests operate at scan-step granularity and
  make no continuous-time claim.
- `E1-STOP-008` Guarded-row closure. If any guarded row becomes genuinely
  closable, its checkbox, all validator pins, report/schema contracts, and
  closure evidence change in one commit.

## Product Rows

These rows remain queued until the readiness gate closes. Flip board row
`VERIF-P16-001` only when every row below is complete.

- [ ] `VERIF-E1-000` Baseline freeze. Record clean commit, branch, current timer
  reproduction, and remote-builder disk preflight.
- [ ] `VERIF-E1-001` Write and commit the timer resolving spec and reviewed
  decisions. Correct the existing timer diagrams and prose, update the FB
  index, add explicit invariant behavior rows and coverage cells, and keep
  `SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001` open with a `spec_updated` posture.
- [ ] `VERIF-E1-002` Add the committed hand-authored timer trace case file and
  cataloged shipped-path runner. It must run every trace through real ST,
  produce one `verification-cases` artifact containing pass and fail cases,
  and exit nonzero when any case fails.
- [ ] `VERIF-E1-003` Capture durable pre-fix red with `prove.py red`. The TOF
  post-expiry case must fail for the specified value mismatch; protective TP
  and TON cases may pass in the same artifact. A plain failing Rust exit
  without a fresh bound case artifact is not behavioral red.
- [ ] `VERIF-E1-004` Implement the minimal TOF ET-hold fix. Update the existing
  shipped-path expectation that currently pins zero on the following scan.
  Do not change TP unless a separately specified failing case proves a defect.
- [ ] `VERIF-E1-005` Capture paired green at a clean descendant commit with the
  unchanged test/case contract. Run the targeted timer suites.
- [ ] `VERIF-E1-006` Migrate the Phase 8 timer allocation/restart review from
  the open-gap const to the resolved product source, tests first. Regenerate
  the runtime-anomaly report and every affected closure-bound report.
- [ ] `VERIF-E1-007` Close the timer gap atomically: set its resolution source
  and closeout evidence, remove invariant/risk/taxonomy live refs, and prove
  the aggregate affected-test set. Do not close while any referenced decision
  remains unresolved.
- [ ] `VERIF-E1-008` Run broad gates on `trust-builder`: `just fmt`, `just
  clippy`, `just test-all`, plus `api_smoke`, `debug_control`,
  `complete_program`, and `runtime_reliability`. Record broad evidence and
  promote only to the supported level; stop below `validated` if a coverage
  cell remains open.
- [ ] `VERIF-E1-009` Apply release hygiene for the product behavior change:
  changelog, standard-function coverage, decisions/deviations, version and
  merge-time tag/release obligations as required by `AGENTS.md`.
- [ ] `VERIF-E1-010` Record durable slice evidence, regenerate affected
  reports, update metadata/board, and obtain independent review.

## Minimum Validation

- `python3 scripts/validate_verification_metadata.py`
- `scripts/verification_metadata_gate.sh`
- `python3 scripts/run_verification_focused_tests.py`
- `python3 scripts/check_verification_tooling_selftests.py`
- targeted catalog command through the real shipped-path runner
- broad remote gates and runtime vertical suite after the product fix
- `git diff --check`
- clean `git status --short` at each recorded proof revision

## Reviewer Acceptance

1. Readiness proof output is producer-authentic, durable, clean-full-SHA, and
   ancestry checked; no proof row was hand-rewritten.
2. The case file identifies hand-authored state-machine provenance honestly and
   its artifact is bound to the same run consumed by `prove.py`.
3. Timer decisions cite the real Ed.3 text and separate standard requirements
   from truST-owned choices.
4. Red and green use the same committed spec, catalog command, case IDs, and
   case digest; red is a clean ancestor of green.
5. The product diff stays inside the TOF fence and matches only the failing
   case. TP/TON passing observations do not become invented fixes.
6. Phase 8 migration and timer-gap closure are atomic and fail closed.
7. Promotion is justified cell-by-cell and by targeted/broad evidence.
8. STOP-012/014, report-only CI, skills, and agent instructions remain
   unchanged until their own board rows genuinely permit change.
