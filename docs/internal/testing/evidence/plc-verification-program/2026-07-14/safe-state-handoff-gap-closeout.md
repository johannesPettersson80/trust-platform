# Runtime Safe-State Handoff Gap Closeout

Date: 2026-07-14

## Scope

This record closes `SPEC_GAP_RUNTIME_SAFE_STATE_001`, whose blocking question
asked what must reach hardware and what must be reported as not safe when an
I/O worker is full, reconnecting, or down. The owning contract is now written
in `docs/specs/11-runtime-engine.md` under "Safe-state handoff confirmation".

The closeout does not claim proof on a real output module or exhaustive worker
disconnect coverage. Those obligations remain visible in the two invariants'
`missing` fields together with the pending broad remote gate.

## Product Finding And Proof

The focused safe-state traces found a real fail-open stop path. A protocol
driver could accept output bytes into a worker queue, report degraded or
faulted health, and still let the resource enter `Stopped` as though physical
safe-state delivery had been confirmed. The same path could also stop trying
later configured drivers after an earlier write error.

- Red revision: `7c8d3c00a7a5c6e1702aee75762373be6d73c9c4`
- Red evidence: `EVID_TEST_RUNTIME_SAFE_STATE_HANDOFF_001_RED`
- Red result: all three committed handoff cases failed
- Green revision: `d9222d3128b72fbdf99cf751f5c5c263499f6279`
- Green evidence: `EVID_TEST_RUNTIME_SAFE_STATE_HANDOFF_001_GREEN`

The fix attempts every configured driver and requires both a successful write
call and immediately observed `IoDriverHealth::Ok`. Otherwise deliberate stop
enters `Faulted` with the named unconfirmed-handoff error. The same case file,
trace-definition digest, and execution-contract digest bind red and green.

## Honest Posture

`RT_SAFE_STOP_001` and `RT_SAFE_IO_WORKER_001` advance only to targeted `G1`.
The proof covers the committed degraded, faulted, and continue-after-failure
cases. It does not claim a hardware-lab run or broad-builder proof; those are
the next evidence steps.
