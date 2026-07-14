# Runtime Reload Transaction Gap Closeout

Date: 2026-07-14

## Scope

This record closes `SPEC_GAP_RUNTIME_RELOAD_TRANSACTION_001`, whose blocking
question asked which runtime, status, I/O, retain, and debug changes form one
online-change transaction and what must happen on partial failure. The owning
contract is now written in `docs/specs/11-runtime-engine.md` section 6.7.1 and
is reflected at the debug boundary in `docs/specs/13-debug-adapter.md`.

The closeout does not claim complete reload fault coverage. Bytecode/resource
commit failures, retained-value migration failure after preparation, and broad
remote validation remain visible in `RT_RELOAD_001.missing`.

## Product Finding And Proof

The committed retain-load trace found a real partial-apply defect. The runtime
returned the injected retain-store error only after replacing the executable,
resetting the scan counter, clearing the active debug force, and resetting PLC
storage.

- Red revision: `bba0607c9d228db7843429952bd98a598e20f879`
- Red evidence: `EVID_TEST_RUNTIME_RELOAD_TRANSACTION_001_RED`
- Red observations: count changed from `1` to `0`, the cycle counter changed
  from `1` to `0`, and the next scan executed the new `+10` program.
- Green revision: `813b93e9998524b6f659d63202ba1141d44331f0`
- Green evidence: `EVID_TEST_RUNTIME_RELOAD_TRANSACTION_001_GREEN`

The fix reads the retained snapshot before mutating live runtime state. A read
failure therefore rejects online change before bytecode replacement or warm
restart. Red and green use the same case file and execution-contract digests.

## Honest Posture

`RT_RELOAD_001` advances only to targeted `G1`. The trace proves the
retain-store read-failure partition. It does not prove atomic rollback for
every later bytecode, resource, or retained-value migration failure, and it
does not establish broad remote-gate evidence.
