# Phase 18 runtime-cloud exact-behavior campaign

Date: 2026-07-29

## Scope and result

This coherent campaign reviewed all 23 frozen behavior demands owned by the
runtime-cloud and managed-fleet authority group. It bound the demands to 52
existing catalog tests and 53 exact behavior links across seven invariants.
No new behavior test, denominator fact, specification, or production-code
change was required.

The accepted transaction records:

- 23 reviewed behavior demands;
- 52 exact assigned existing catalog tests and 53 assignment links;
- four exact executable command groups;
- seven reviewed invariants;
- 52 assertion inventories containing 439 extracted assertions;
- at least one direct assertion oracle in every selected test;
- 246 direct `assert*` or `expect_err` assertion oracles;
- zero missing specification authority;
- 67 residual confirmed missing behavior tests.

The frozen denominator remains 4,399 unique live facts: 3,918 are
`catalog_mapped` and 481 retain an explicit owned `reviewed_nonmapping`
disposition. This is large-batch progress across `P18-003` through `P18-006`;
it does not close those rows globally.

## Exact checkouts and bootstrap

- Canonical agent-file source:
  `/home/johannes/projects/trust-platform`
- Local branch: `plc-verification-program`
- Exact campaign and assertion source commit:
  `dce893b021f95d2156ce882c1d43c9c0a19ca7df`
- Accepted metadata transaction commit:
  `ae57d2c3c397fa22af6076d813f9d8e463ad73fd`
- Validation destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-runtime-cloud-23`
- Builder host: `scena-rust-builder`
- Builder checkout mode: detached HEAD

At each destination revision, `AGENTS.md` and the complete
`.codex/skills/**` directory were manually copied from the canonical checkout
and verified byte-for-byte before execution. The final focused validation ran
at the exact metadata transaction commit.

## Test-first control-plane proof

The mapping transaction began with focused tests that expected the complete
23-demand, 52-test replacement set. Before implementation, the tests failed at
that behavior boundary because the stub returned zero assignments and no
catalog or semantic-lock replacements.

The first builder execution then exposed an exact-name harness defect: three
split `web_io_config_integration` tests were addressed by bare names rather
than their `part_08` and `part_14` module-qualified identities. A focused
regression first failed on those three identities, then passed after the
verified full names were supplied. The failed harness attempt is retained
under `attempt-01/`; it is not counted as a behavior red or passing proof.

After atomic metadata application, the live pre-transaction resolver produced
an expected focused red because the 23 behavior demands were no longer gaps.
Applied-state reconstruction was then bound to the immutable semantic lock.
The paired local focused command passed 19/19:

```text
python3 -m unittest \
  scripts.verification.runtime_other_behavior_mapping_tests \
  scripts.verification.runtime_other_behavior_proof_tests \
  scripts.verification.runtime_other_behavior_campaign_tests \
  scripts.verification.runtime_cloud_behavior_proof_tests \
  scripts.verification.runtime_cloud_behavior_campaign_tests \
  scripts.verification.runtime_cloud_assertions_tests \
  scripts.verification.runtime_cloud_behavior_mapping_tests
```

The same command passed 19/19 in 10.454 seconds on the exact clean builder
checkout at `ae57d2c3c397fa22af6076d813f9d8e463ad73fd`.

## Exact builder execution

The accepted builder campaign passed 52/52 selected tests in four exact
`cargo nextest` groups. Its campaign digest is:

```text
sha256:96f7ddf53c3c3efa368c9e908664e914dfce09339f13167d3842ebe79bbea273
```

The exact plan digest is:

```text
sha256:28bbaee13779c969cb9422dfa2e9c1f263d6b43521ed7e2cab7955a0b920e72b
```

The 52-test assertion inventory digest is:

```text
sha256:b89eb62f6c545e0d2b02fdfa14daad63b430018ac9bb2459ecd3d9015156c9f8
```

All selected tests have a direct reviewed oracle. The extractor also reports
44 unresolved helper-call annotations across 23 tests. Those annotations are
retained honestly for the later dynamic-reachability and mutation-strength
work; this mapping campaign does not convert them into `P18-007` proof.

The durable exact plan, campaign, raw output, assertion inventory,
post-transaction gap report, and failed harness attempt are retained under:

```text
docs/internal/testing/evidence/plc-verification-program/2026-07-29/
  p18-runtime-cloud-exact-behavior-23/
```

The accepted assignment and evidence digests are also bound in
`verification/p18/review-locks/runtime-cloud-23-semantic-lock.json`.

## Scoped and global validation state

The post-transaction frozen gap report has digest:

```text
sha256:403994783a341efaa3b0a1d4d6ccf7655a74160aa37a69bc997d2b24e8c38406
```

It reports 4,399 frozen live tests, 3,918 `catalog_mapped`, 481 owned
`reviewed_nonmapping`, zero existing tests lacking specification authority,
and 67 confirmed missing behavior tests.

The mandatory planner was run before this control-plane implementation and
remained red on the unchanged repository-wide Phase 18 metadata baseline
(1,299 findings). It must be rerun against the complete final changed-path set
before any push. No workspace-wide `just fmt`, `just clippy`, `just test`, or
`just test-all` command ran. Those remain reserved for the single frozen
candidate at `VERIF-P18-016`.
