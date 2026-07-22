# P18 signed integer materialization slice

Date: 2026-07-22

Status: exact authority, mapping, executable boundary matrix, and targeted
behavior-lock proof complete. Dynamic assertion reachability, mutation
sensitivity, and broad remote proof remain open. No Phase 18 global row or stop
gate is closed by this slice.

## Scope and authority

- Phase 18 slice:
  `P18-CODE-CODE_F72E3E7BC892973CF9344497078A35D93380745CDC5DB4820F5CFB302AAEE14A`
- Exact code fact:
  `CODE_F72E3E7BC892973CF9344497078A35D93380745CDC5DB4820F5CFB302AAEE14A`
- Exact symbol: `trust_runtime_core::numeric::signed_from_i128`
- Reviewed body digest:
  `sha256:a46696e724d430be4d6a2f50f477a60811b4aa2fc22372dd0473afa3abf03168`
- Oracle:
  `SPEC_RUNTIME_SEMANTICS_001#6-8-signed-integer-result-materialization`
- Invariant: `RT_SIGNED_INTEGER_MATERIALIZATION_001`

The reviewed contract requires exact destination-tagged SINT, INT, DINT, and
LINT materialization for representable values, `RuntimeError::Overflow` at the
first value outside each signed destination range, and
`RuntimeError::TypeMismatch` for every unsigned or floating destination. The
code body did not change in this slice; the work replaces a historical gap with
reviewed authority and executable proof rather than inferring expected behavior
from the implementation.

## Executable contract

Two mapped tests cover the closest owning boundary:

- `TEST_RUNTIME_SIGNED_INTEGER_MATERIALIZATION_001` is the direct unit matrix
  in `trust-runtime-core`. It checks minimum, zero, maximum, adjacent underflow,
  adjacent overflow, exact value tags, and all six non-signed categories.
- `TEST_RUNTIME_SIGNED_INTEGER_MATERIALIZATION_TRACE_001` runs the same four
  reviewed behavior partitions through the shared `verification-cases`
  artifact contract. Its generated case file is
  `verification/cases/runtime_safety/RT_SIGNED_INTEGER_MATERIALIZATION_001.toml`
  with digest
  `sha256:eacff93189df0af63914f4d21801eed3a5ae413e8e9b42270cff2120796e6053`.

The case file regenerates exactly with `gen_cases_v2.py v1`. The scanner found
the integration test as `DISC_EF59D3F89B6C090FA842`; the committed denominator
maps it to the trace test without an ignore or nonmapping disposition.

## Producer-authentic lock proof

The isolated builder checkout was
`trust-builder:/home/johannes/projects/trust-platform-p18-substrate-186b9359`.
Before each detached-HEAD transition, canonical
`/home/johannes/projects/trust-platform/AGENTS.md` and the complete
`.codex/skills/**` tree were manually copied to the destination. Their hashes
matched as `d0b35bc3edf19dc1d276202b6058ac6ad2835c6552fafbe71c04eb262a88530e`
and `a595ab028d85241bc031148f44604245e027d9c320d858150bc460aaf740b25f`.

The baseline ran from clean source commit
`68d42d053dec2a4695ae7ab679d2b053fed01736`; the comparison ran from clean
descendant commit `7ce3fe5cb71defc3e4cf1224cc074ded93179f0c`.

- Baseline:
  `EVID_TEST_RUNTIME_SIGNED_INTEGER_MATERIALIZATION_TRACE_001_LOCK_BASELINE`
- Comparison:
  `EVID_TEST_RUNTIME_SIGNED_INTEGER_MATERIALIZATION_TRACE_001_LOCK_COMPARE`
- Proof contract digest:
  `sha256:99e1bacba21f1c6fb9c57e5be34acb12d7a5a620e3662178b2e6ef7708ce203b`
- Stable case-result digest:
  `sha256:7706fae16c008e635f073fb7b32d135d1fb4dbd0055b2faed361649a6f7420d7`
- Result: all four generated partitions passed in both runs; no case was
  blocked, skipped, filtered out, or failed.

Focused remote validation on x86_64 Linux also passed `cargo fmt --all --
--check`, the exact direct unit test, the exact case-backed integration test,
case regeneration, the 273-to-4,042 catalog staleness join, and all 887 current
metadata records. The warmed target cache and checkout-local temporary
directory were used because the builder had 27 GB free on `/home/johannes` and
1.6 GB on `/tmp`.

## Frozen slice-accounting validation

The refreshed focused accounting milestone ran in the clean detached checkout
at `865c3e6eb4f945cb26b7b9d87a7776a6833f328e`, after the assertion-reachability
architecture, board review, diagram, and canonical ledgers were rebound. It
passed:

- both exact Rust tests and `cargo fmt --all -- --check`;
- 890-record metadata validation;
- the canonical 15,298-code/4,042-test ledger with artifact digest
  `sha256:0c92d87c45db03c84af49f4fe5d525b0432abdd16640d03727049e2f5bd286f0`;
- the report-only P18 gate with zero contract findings;
- full code/spec accounting with 15,298 facts, zero findings, and zero blocking
  findings; and
- the specification-source audit with 401 documents, 14,516 public prose
  blocks, zero unreviewed blocks, and 126 report-only warnings.

The refreshed ledger reports 15,137 remaining `spec_gap` facts, 15,134
remaining `registered_backlog` facts, and 3,777 remaining
`reviewed_nonmapping` test facts. No broad `clippy` or `test-all` milestone ran;
the accepted board reserves those final gates for `VERIF-P18-016` on the one
eventual zero-debt candidate.

## Assertion reachability

The Phase 18 assertion-reachability producer executed the reviewed signed,
integer-normalization, and unsigned-materialization shards from clean detached
source `811e94b04463217ef6976e90388c33b467e7b437` on
`trust-builder-linux-x86_64`. Both signed-materialization commands passed on
the unmodified source. The producer then deleted only the `NumericKind::SInt`
match arm from
`signed_from_i128`; the mutant compiled, and each selected test failed with a
nonzero exit. The semantic validator reconstructed both results as `caught`.

Canonical raw artifact:
`verification/p18/assertion-reachability.json`, SHA-256
`20bbf0d59c8dd4e14b272ff3de1f4670f32d3ea019d63f60021c204d956da246`, input
closure `sha256:3088cfaaec235b2449677fb7b32646fd164cdd72325d7b70b514055cdd46c5ba`.
Its report-only summary is 6/269 proven, 263 unrun debt, six caught mutants,
and zero survivors, unviable outcomes, timeouts, or harness failures. This
closes this invariant's `assertion_reachability` item only. The separate
`mutation_sensitivity` and formal `VERIF-P18-009` adequacy campaign remain open
and budget-gated.

## Exact risk acknowledgement

`RISK_ACK_F72E3E7BC892973CF9344497078A35D93380745CDC5DB4820F5CFB302AAEE14A`
acknowledges the scanner's `fault_mapping` signal for the exact unchanged body
digest. The acknowledgement is now bound to this reviewed specification,
invariant, unit matrix, and producer-authentic lock pair. It does not suppress
the signal. The source-bound catcher evidence closes assertion reachability for
this exact slice but does not claim mutation adequacy or broad-gate proof.

## Architecture and remaining debt

The implementation reuses the existing `verification-cases` helper and keeps
the artifact runner isolated in a `trust-runtime-core` integration test. The
direct unit matrix remains beside the pure numeric helper. No production
ownership, data flow, execution flow, or public API changed. The Phase 18
verification-control-plane flow did change, so
`docs/diagrams/architecture/verification-p18-zero-debt.puml` now shows the
reviewed plan, isolated producer, canonical raw artifact, and focused semantic
validator. The product-test structure keeps one oracle, one generated decision
table, and two intentionally different consumers: direct boundary assertions
and the durable proof-artifact adapter.

The post-edit planner still reports `missing_tests` when all support files and
the crate manifest are classified together because the manifest route expands
the request across control-security, supply-chain, runtime, and verification
areas and requires both behavior-lock and metadata-validation classes in each
area. That over-broad result is retained as open planner evidence; it is not
used to claim this slice or any global row complete.

Remaining exact debt:

- dynamic reachability and assertion-strength proof for the other 263 currently
  mapped tests and every future mapping under global `VERIF-P18-007`;
- mutation sensitivity and zero viable survivors under `VERIF-P18-009`;
- causal broad remote evidence, then final frozen-candidate gates under
  `VERIF-P18-016`;
- all other code/test facts and every global Phase 18 equation remain open.
