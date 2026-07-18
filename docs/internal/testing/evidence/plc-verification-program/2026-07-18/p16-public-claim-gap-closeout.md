# Phase 16 Public-Claim Gap Closeout

- Review date: 2026-07-18
- Source checkpoint: `98861a8734763817542ba68fa17b54df729a2e79`
- Owning specification: `SPEC_RELEASE_EVIDENCE_001`
- Evidence posture: claim-boundary validation only; no new product behavior,
  native-target execution, physical-hardware qualification, or CI enforcement

## Platform Support Matrix

The public claim is limited to native CI and/or published artifacts for the
advertised target matrix. GitHub release `v0.24.34`, published 2026-07-15,
contains checksummed runtime archives and target-specific VSIX packages for
Linux x86-64/AArch64, macOS x86-64/AArch64, and Windows x86-64:

- release: <https://github.com/johannesPettersson80/trust-platform/releases/tag/v0.24.34>
- successful five-target release build and publish run:
  <https://github.com/johannesPettersson80/trust-platform/actions/runs/29426801469>
- successful Linux, macOS, and Windows CI run:
  <https://github.com/johannesPettersson80/trust-platform/actions/runs/29440442398>

`TEST_RELEASE_PLATFORM_MATRIX_TRACE_001` binds the documented tier vocabulary
and target matrix. `TEST_PLATFORM_PATH_TRACE_001` and
`TEST_VSIX_TARGET_IDENTITY_TRACE_001` remain mapped but are not promoted:
native archive execution and native VSIX installation across every target are
not established. Raspberry Pi remains Linux AArch64 artifact support, and
PREEMPT_RT remains a deployment configuration rather than latency
certification.

## Behavior-Locked Claim

The README limits “behavior locked” to explicitly mapped invariants, tests,
suites, and durable evidence. The runtime and debugger mapping contracts have
current lock-baseline/lock-compare pairs, and the complete mapped-test
execution recorded in `p16-mapped-test-execution.json` passed all 242 mapped
tests. This validates only the explicit mapping statement. It does not assert
repository-wide behavior coverage, release validation, or IEC conformance.

## Source Build

The documented shipped-binary build was executed from a clean trust-builder
checkout without a sibling `open-ot-ref` directory:

```text
test ! -e ../open-ot-ref &&
cargo build --release -p trust-lsp -p trust-runtime -p trust-debug
```

The build succeeded at `1be81590c89114f1a0abb76d04a5f4f7fe41fb91` and is
recorded by `EVID_SOURCE_BUILD_PUBLIC_CLAIM_EXECUTION_20260718`. Optional
OpenOT examples, conformance fixtures, and telemetry tests remain outside this
claim and may require the sibling checkout as documented.

## Hardware Claim Boundary

The public documentation uses the reviewed evidence vocabulary `mock`,
`loopback`, `simulation`, `interoperability`, and `device_in_loop`. Only a
named, reviewed device-in-loop topology can qualify physical hardware. The
current documentation explicitly says optional device gates that skip provide
no hardware proof. This closeout validates that wording boundary; it records
no device-in-loop result and qualifies no Raspberry Pi, fieldbus, PLC, broker,
or other physical target.

## Closeout

The four gaps are closed against the normative release-evidence contract and
their exact mapped tests. Higher evidence tiers that are not part of the public
wording remain outside the validated invariants rather than being inferred
from artifact names, mock tests, skipped hardware gates, or Linux-only runs.
