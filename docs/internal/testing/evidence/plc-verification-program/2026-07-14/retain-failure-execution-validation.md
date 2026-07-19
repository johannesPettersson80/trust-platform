# Runtime retain failure execution validation

Date: 2026-07-14

Final validated commit: `fe4de3a382d398fb4d98b42c19dbe177d1a46baf`

## Outcome

This vertical found and fixed a product defect. A retain snapshot containing an
initial compatible entry and a later incompatible entry returned an error after
already applying the initial entry. The runtime now validates and stages every
entry before committing any retained value, so rejection is atomic.

The product contract is written in `docs/specs/11-runtime-engine.md`. The
hand-authored case
`RT_SAFE_RETAIN_001_SNAPSHOT_REJECTS_ATOMICALLY_ON_LATE_INCOMPATIBLE_VALUE`
executes through the real runtime harness and is cataloged as
`TEST_RUNTIME_RETAIN_FAILURE_ATOMICITY_001`.

`SPEC_GAP_RUNTIME_RETAIN_FAILURE_001` is closed. `RT_SAFE_RETAIN_001` is
implemented at G2 with targeted red/green proof and a successful broad remote
gate. The invariant still names `retain_failure_matrix_depth` as missing; this
slice does not claim complete coverage of every persistence failure mode.

## Causal chain

| Step | Commit or evidence |
| --- | --- |
| Specify retain failure transaction boundaries | `57d09335ba1f0c8dfe72536f012d6158ef5b9074` |
| Reproduce partial snapshot application | `d751d7139f1a267b4505ecbd5f86a375c9c1dae6` |
| Fix snapshot application atomicity | `c720a658669d2e4d69c9a49e91d702a1cd7af9b7` |
| Bind the hand-authored case and cataloged runner | `4d0cb4527cd01911a567190424660ba9b76c64c6` |
| Producer-authentic red proof | `EVID_TEST_RUNTIME_RETAIN_FAILURE_ATOMICITY_001_RED` at `ec1d23b7be9c36e46599ea1154e2568e7d9a65cc` |
| Producer-authentic green proof | `EVID_TEST_RUNTIME_RETAIN_FAILURE_ATOMICITY_001_GREEN` at `8ba872b86c363e30517b7f1b33cb702788b0dd2f` |
| Close the specification gap at targeted proof | `dbfa07ff370cfc366ea86f6b8d0dda8aef2a8d81` |
| Broad PR gate evidence | `EVID_BROAD_REMOTE_PR_20260714_9913AD73A11F` |
| Promote the invariant to G2 | `560427043dc4e202f407a485f2f103d80ba6503a` |
| Final report source checkpoint | `bf29bc01115d0b603294ca31a8016419a24fabba` |
| Final report/evidence commit | `fe4de3a382d398fb4d98b42c19dbe177d1a46baf` |

The red artifact failed only the atomic-rejection case with
`accepted_first expected 70, observed 111`. The paired green artifact used the
same case file and proof-contract digest and passed the formerly red case.

## Validation

On `trust-builder`, the producer-authentic broad gate ran from clean commit
`dbfa07ff370cfc366ea86f6b8d0dda8aef2a8d81`:

- `just fmt`: passed.
- `just clippy`: passed.
- `just test-all`: passed.
- Cataloged retain trace: 1/1 passed.
- Gate duration: 739,325 ms.
- Disk preflight: passed with 65,493,148 KiB available under the home
  filesystem and 3,404,544 KiB under `/tmp`.

The required runtime verticals passed at the G2 commit:

- `cargo test -p trust-runtime --test api_smoke`: 3/3.
- `cargo test -p trust-runtime --test debug_control`: 20/20.
- `cargo test -p trust-runtime --test complete_program`: 1/1.
- `cargo test -p trust-runtime --test runtime_reliability`: 4/4.

Final clean-checkpoint validation at
`fe4de3a382d398fb4d98b42c19dbe177d1a46baf`:

- Canonical focused verification suite: 765/765 passed.
- Verification tooling self-tests: 33/33 passed.
- Metadata/fence gate: 389 records, zero changed product paths.
- Ignored-test join: 42 discovered, 42 registered, 17 unknown, zero
  catalog-mapped.
- Catalog staleness: 73 committed records against 3,878 scanner facts.
- VS Code registration: 456 facts, 38 files, 38 registrations.
- Refactor proposals: 1 proposal, zero redirects.
- All 15 generated report pairs passed their at-rest validators.
- `git diff --check`: passed.

## Final report digests

All reports were regenerated from clean commit
`bf29bc01115d0b603294ca31a8016419a24fabba` with timestamp
`2026-07-14T10:39:52+02:00`.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `0915ad7422fc7564dbc10cc41bfe6db44bf23e7905ac0e0a051cdb81bfa96b8e` |
| Coverage-matrix gaps | `983e2c531b603a64267b5bd93b41dd99311b92ab1488a062b915de776bab4871` |
| Malformed-input coverage | `4eb6c8261295fc6b95f6cd52e20d2c6eb3499531803d02e10617a8cad24e2ed4` |
| Unmapped-test debt | `05ec3a117537a0f19b06fe9d457521b8679eaf1d2baea86e62e1f1340d889d96` |
| Test-refactor assessment | `2397814b8e2b3aa9c5ca66dee7be24d95d257c2830e54a712088ccff6291d8c8` |
| Ignored-test inventory | `1c2773a647c83c516b7e54ca4dd9645038f379ecaaba0bee7ddf0969e30b2ad9` |
| Phase 5 suite audit | `10764850a53a4c9ea3373ca003895824c5704f3da8f86b29faca626e5039722d` |
| Invariant-seed audit | `d03a8cb8943725bee518db4afa0c52fdfa8a6de359829ad47d0b876b4afc4954` |
| Specification completeness | `f3fe095cc8e9f3064d9026ad846d97208f30b4f255a1160743365e2cc627102d` |
| Requirement/oracle audit | `ab0892151472f3dd0df5b16ad3531ed34dc29260dbbe99424360b679fa1d0714` |
| Conformance alignment | `3be48b179e501b13469605c7cf9f5f8d286394ada768df02c46c09a93ced3a78` |
| Runtime-anomaly audit | `0cb7267b53e2b4b2efb85174141370c18fd24c31bf7e0416bdb9fe57e8132962` |
| Fuzz-program audit | `a8253fbd5c39484a368ee3e5ba685444d9b2d5cdeca417f5bb7ee6b9d0f46438` |
| Mutation program | `649396f5ed2815b8a5325e597ca59d2c6235e61a212aa629ec8875027b14aaee` |
| Specification-source audit | `64268d9a6c2404916e575c52d2cd71a16727e8ce613fddaa1d24f44cf2f68b06` |

## Remaining posture

- Specification gaps: 34 total, 31 open, 3 closed.
- Invariants: 53 total, 48 at S0 and 5 at G2.
- Requirement/oracle audit: 12 eligible and 41 missing.
- Test catalog: 68 generated-test facts mapped and 3,810 scanner facts
  unmapped; five additional catalog rows are non-scanner artifacts.
- Conformance: 21 cases, zero explicitly linked.

These are visible program debts, not claims closed by this retain slice.
