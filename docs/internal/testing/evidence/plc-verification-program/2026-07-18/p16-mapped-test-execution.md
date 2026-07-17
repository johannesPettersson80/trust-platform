# Phase 16 Mapped-Test Execution

- Source revision: `b7e3ab47172ff2e857c22a4173d78a639a6616dd`
- Catalog: `verification/test-catalog.toml`
- Catalog SHA-256: `e20a592afd7dc8ff3c8195dbf6840614b909384b16171f1386c3f5a9ea6e12b3`
- Platform: `linux-x86_64` (`trust-builder`)
- Executed at: `2026-07-17T22:07:31+00:00`
- Durable JSON: `p16-mapped-test-execution.json`
- JSON SHA-256: `c70479523fca5640947d6446f497e3b5bdc67d4d78cb0dc7654a59834d8baee2`

## Result

The clean source revision contained 242 catalog rows with `status = "mapped"`
and a committed command. A clean detached `trust-builder` worktree executed
each exact command independently with a 30-minute timeout and a separate log.
The result set and command bindings were then joined back to the catalog and
required to be exhaustive and one-to-one before the durable JSON was written.

| Mapped tests | Passed | Failed | Timed out | Accumulated command time |
| ---: | ---: | ---: | ---: | ---: |
| 242 | 242 | 0 | 0 | 591,885 ms |

No failing or timed-out mapped test was observed. Therefore this execution
creates no red proof, requires no product fix, and does not manufacture a
red/green pair. Existing per-test proof records remain the proof plane; this
artifact is execution evidence for the complete mapped denominator only.

The JSON records, for every test ID, the exact catalog command, exit status,
timeout state, duration, and SHA-256 of the retained remote log. The source
catalog has the same SHA-256 at the current implementation revision, so the
mapped denominator did not change after execution.

## Boundaries

- This result says nothing about scanner facts that have no mapped catalog row.
- It does not imply CI enforcement, broad-gate proof, native-platform proof,
  device-in-loop proof, or release proof.
- It does not close `VERIF-P16-006`, `VERIF-P8-002`, or `VERIF-P9-005`.
- No runtime or product behavior changed as a result of this sweep.

## Verification

```text
sha256sum docs/internal/testing/evidence/plc-verification-program/2026-07-18/p16-mapped-test-execution.json
jq -e '.source_commit == "b7e3ab47172ff2e857c22a4173d78a639a6616dd" and .summary == {"failed":0,"mapped_tests":242,"passed":242,"timed_out":0} and (.results | length) == 242 and ([.results[].test_id] | length == (unique | length)) and all(.results[]; .exit_status == 0 and (.timed_out | not))' docs/internal/testing/evidence/plc-verification-program/2026-07-18/p16-mapped-test-execution.json
git show b7e3ab47172ff2e857c22a4173d78a639a6616dd:verification/test-catalog.toml | sha256sum
sha256sum verification/test-catalog.toml
```
