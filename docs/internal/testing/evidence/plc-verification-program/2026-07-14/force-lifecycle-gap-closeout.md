# Force lifecycle coverage closeout

Date: 2026-07-14

Source commit: `4c97a8461032df65d3e10f04d1e9c14ff7422b5e`

## Result

The written debug-mutation lifecycle contract is now exercised at
the missing pause/resume, non-terminating disconnect, release, deliberate-stop,
fault, and authorization-change boundaries. All six new tests passed on a clean
`trust-builder` worktree. No new runtime defect was observed in these
dimensions; this batch adds missing coverage rather than changing product
behavior.

The earlier producer-authentic red/green proof remains bound to the unchanged
`TEST_RUNTIME_FORCE_LIFECYCLE_001` case file. The additional tests do not alter
that case file or its historical proof digests.

`SPEC_GAP_RUNTIME_FORCE_LIFECYCLE_001` remains `test_mapped`, not closed. Its
existing proof contract names the gap as the invariant oracle. Replacing that
oracle with `SPEC_RUNTIME_ENGINE_001` is required before metadata closure, but
doing so changes the proof-contract digest and would invalidate the committed
producer-authentic red/green pair. The frozen control plane has no reviewed
historical-proof migration for that transition. This record therefore exposes
`source_oracle_proof_contract_transition` as remaining debt instead of
rewriting proof history or manufacturing a new red run.

## Focused validation

Remote platform: `trust-builder`, Linux x86_64.

```text
CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p trust-runtime --test force_lifecycle_boundaries
```

Result: 5 passed, 0 failed.

```text
CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p trust-runtime --lib \
  control::tests::auth_token_change_preserves_force_until_authorized_release \
  -- --exact
```

Result: 1 passed, 0 failed.

```text
CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p trust-runtime --test force_lifecycle \
  force_lifecycle_trace_cases -- --exact
```

Result: 1 passed, 0 failed. This confirms the original proof runner and case
file remain unchanged and executable.

Local metadata checks at the same source state:

```text
python3 scripts/validate_verification_metadata.py
python3 scripts/check_test_catalog_staleness.py
```

Result: metadata validated with 423 records; live catalog staleness passed.

## Covered boundaries

- active forces persist across pause/resume;
- dropping a non-terminating debug-session handle does not clear runtime-owned
  force state;
- release removes the force without writing a replacement value;
- deliberate stop/safe-state handling clears queued writes and forces;
- fault handling clears queued writes and forces before explicit recovery;
- control-token rotation preserves an existing force, rejects the stale token,
  and permits release with the replacement token.

## Honesty boundaries

- This record does not claim a newly discovered product bug.
- It does not define the separate role/permission matrix tracked by
  `SPEC_GAP_DEBUG_AUTHORIZATION_001`.
- It does not change CI, suite definitions, approved proof producers, runtime
  behavior, or the historical force-lifecycle red/green evidence.
- Broad-gate evidence is recorded separately for this batch. It cannot by
  itself close the source-oracle proof-contract transition or promote this
  invariant while the gap remains open.
