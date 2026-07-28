# Phase 18 protocol exact-behavior campaign

Date: 2026-07-29

## Scope and result

This coherent campaign reviewed all 29 frozen MQTT, OPC UA, and Modbus
behavior demands. It bound those demands to 32 exact tests and 35 behavior
links across seven protocol invariants.

The accepted transaction records:

- 29 reviewed behavior demands;
- 32 exact assigned tests and 35 assignment links;
- 29 previously cataloged tests;
- three promoted OPC UA server integration tests;
- one refreshed mapping decision for the strengthened Modbus reconnect test;
- four exact executable command groups;
- 32 assertion inventories containing 295 extracted assertions;
- 99 direct `assert*` or `expect_err` assertion oracles;
- zero assertion-empty selected tests;
- zero missing specification authority;
- 38 residual confirmed missing behavior tests.

The frozen denominator remains 4,399 unique live facts. The three promotions
change the terminal equations from 3,918 mapped and 481 reviewed nonmapping to
3,921 mapped and 478 reviewed nonmapping. This is large-batch progress across
`P18-003` through `P18-006`; it does not close those rows globally.

## Exact checkouts and bootstrap

- Canonical agent-file source:
  `/home/johannes/projects/trust-platform`
- Local branch: `plc-verification-program`
- Exact accepted campaign and assertion source commit:
  `8f3139a0a424ba7e89fa95c868113daaa7883898`
- Accepted metadata transaction commit:
  `0c129691cd22d830e537d5c1f2e25ad3396f98d7`
- Validation destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-runtime-cloud-23`
- Builder host: `scena-rust-builder`
- Builder checkout mode: detached HEAD

At every destination revision, `AGENTS.md` and the complete
`.codex/skills/**` directory were manually copied from the canonical checkout
and verified byte-for-byte before execution. The final focused validation ran
at the exact metadata transaction commit.

## Test-first behavior and transaction proof

The Modbus reconnect test first passed as the existing non-spinning behavior
lock. A new lower-bound assertion was then added for
`RT_MODBUS_DRIVER_001#behavior:7`. At exact commit
`e0fbf974acfde5cd33c9342cd5d9cf2d702e39cd`, the focused builder command
failed by assertion with `attempts=1`: the old stimulus ended before the
worker's 100 ms reconnect boundary. The corrected test waits through that
boundary and issues one new scan request. Exact descendant commit
`5217a1ec21941676299d4853cc2ff6b0933f80a3` passed the same selected test
1/1 without production-code changes.

The assignment, campaign, assertion-selection, and mapping transaction each
began with focused zero-result stubs. Their tests failed at the requested
29-demand, 32-test, four-group, and complete replacement-set boundaries before
implementation.

The first 32/32 protocol execution was invalidated when the mapping quarantine
correctly rejected all three OPC UA promotions: their shared helper used a
dynamic runtime TOML read, leaving an unresolved external oracle. The fixture
loader now embeds the three reviewed TOMLs with `include_str!` before writing
and loading the runtime copy. This binds the fixture digests into the source
closure. The accepted rerun remains 32/32 and all three promoted inventories
have zero unresolved helpers. The invalidated pre-closure artifacts are
retained separately under `pre-fixture-closure/`.

The atomic mapping compiler then promoted the three tests, refreshed the
Modbus mapping proposal and decision, added all exact behavior references,
updated two invariant debt lists, and validated the 38-demand residual before
one recoverable apply step.

The paired local focused suite passed 42/42:

```text
python3 -m unittest \
  scripts.verification.mapping_catalog_updates_tests \
  scripts.verification.runtime_other_behavior_mapping_tests \
  scripts.verification.runtime_other_behavior_proof_tests \
  scripts.verification.runtime_other_behavior_campaign_tests \
  scripts.verification.runtime_cloud_behavior_proof_tests \
  scripts.verification.runtime_cloud_behavior_campaign_tests \
  scripts.verification.runtime_cloud_assertions_tests \
  scripts.verification.runtime_cloud_behavior_mapping_tests \
  scripts.verification.protocol_behavior_proof_tests \
  scripts.verification.protocol_behavior_campaign_tests \
  scripts.verification.protocol_behavior_assertions_tests \
  scripts.verification.protocol_behavior_mapping_tests
```

The same command passed 42/42 in 12.511 seconds on the exact clean builder
checkout at `0c129691cd22d830e537d5c1f2e25ad3396f98d7`.

## Exact builder execution

The accepted builder campaign passed 32/32 selected tests in four exact
`cargo nextest` groups. Its campaign digest is:

```text
sha256:b401ca4e04fc075c259005c1a43e1be278db78496ab6defa14a7928022bfb79d
```

The exact plan digest is:

```text
sha256:6cd7f6076812145b712fd3f86a4576e5e60f57f025ac50583da8774c705b03c9
```

The accepted assertion inventory digest is:

```text
sha256:afbb026f73d7f175638389fd4d3a6a8d467f5242c793311e6fa2542e0b152ba2
```

Four already-cataloged tests retain one explicit unresolved helper annotation
each. Every selected test still has a direct reviewed oracle, but those helper
annotations remain downstream reachability and mutation-strength debt. This
campaign does not convert them into `P18-007` proof.

The durable exact plan, campaign, raw output, assertion inventories,
post-transaction gap report, and invalidated pre-closure artifacts are
retained under:

```text
docs/internal/testing/evidence/plc-verification-program/2026-07-29/
  p18-protocol-exact-behavior-29/
```

The accepted assignments and evidence digests are also bound in
`verification/p18/review-locks/protocol-29-semantic-lock.json`.

## Scoped and global validation state

The post-transaction frozen gap report has digest:

```text
sha256:52321a661f9577d30cd71f3ee102aec8eb26957ffc6685cb9451246266566e7a
```

It reports 4,399 frozen live tests, 3,921 `catalog_mapped`, 478 owned
`reviewed_nonmapping`, zero existing tests lacking specification authority,
and 38 confirmed missing behavior tests.

The mandatory planner remained red on the unchanged repository-wide Phase 18
metadata baseline (1,299 findings). It must be rerun against the complete final
changed-path set before any push. No workspace-wide `just fmt`, `just clippy`,
`just test`, or `just test-all` command ran. Those remain reserved for the
single frozen candidate at `VERIF-P18-016`.
