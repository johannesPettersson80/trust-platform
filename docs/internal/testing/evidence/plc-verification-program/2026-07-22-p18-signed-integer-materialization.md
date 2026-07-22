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

## Exact risk acknowledgement

`RISK_ACK_F72E3E7BC892973CF9344497078A35D93380745CDC5DB4820F5CFB302AAEE14A`
acknowledges the scanner's `fault_mapping` signal for the exact unchanged body
digest. The acknowledgement is now bound to this reviewed specification,
invariant, unit matrix, and producer-authentic lock pair. It does not suppress
the signal and does not claim assertion reachability, mutation sensitivity, or
broad-gate proof.

## Architecture and remaining debt

The implementation reuses the existing `verification-cases` helper and keeps
the artifact runner isolated in a `trust-runtime-core` integration test. The
direct unit matrix remains beside the pure numeric helper. No production
ownership, data flow, execution flow, public API, or diagram changed; this is a
small test-only dependency and proof binding, so no architecture diagram update
is applicable. The structure keeps one oracle, one generated decision table,
and two intentionally different consumers: direct boundary assertions and the
durable proof-artifact adapter.

The post-edit planner still reports `missing_tests` when all support files and
the crate manifest are classified together because the manifest route expands
the request across control-security, supply-chain, runtime, and verification
areas and requires both behavior-lock and metadata-validation classes in each
area. That over-broad result is retained as open planner evidence; it is not
used to claim this slice or any global row complete.

Remaining exact debt:

- dynamic reachability and assertion-strength proof under `VERIF-P18-007`;
- mutation sensitivity and zero viable survivors under `VERIF-P18-009`;
- causal broad remote evidence, then final frozen-candidate gates under
  `VERIF-P18-016`;
- all other code/test facts and every global Phase 18 equation remain open.
