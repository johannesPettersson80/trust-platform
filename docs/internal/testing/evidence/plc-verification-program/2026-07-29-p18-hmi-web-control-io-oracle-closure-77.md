# Phase 18 HMI, Web IDE, control, and I/O assertion-oracle closure

Date: 2026-07-29

## Scope and result

This coherent batch completed semantic assertion review and exact focused
execution for the remaining 77-test HMI, Web IDE, Runtime Cloud, control, and
I/O population selected at
`65652cfba7a131681d9cbacdaa6705ad1b5955f8`.

The accepted result is:

- all 77 discoveries are catalog-mapped and leave
  `assertion_oracle_unresolved`;
- all 1,493 extracted assertions are classified exhaustively: 524 are direct
  product oracles and 969 are setup or preconditions;
- the 524 oracle assertions have 524 exact behavior bindings;
- no assertion is accepted as a non-oracle observation, and no unresolved
  helper or assertion-empty identity remains;
- exact focused execution passes all 13 selected runtime-library tests, all 20
  selected `hmi_readonly_integration` tests, all 18 selected
  `web_ide_integration` tests, and all 26 selected
  `web_io_config_integration` tests;
- six bounded invariants record the newly established HMI, unified Web IDE,
  configuration UI, Runtime Cloud state, Runtime Cloud dispatch, and Runtime
  Cloud rollout authority;
- the existing Runtime Cloud action-policy invariant gains the audited
  live-allowlist update behavior;
- all 27 newly introduced coverage cells reconcile to accepted mappings, so
  the repository-wide `gap_open` count remains 203 rather than increasing;
  and
- the live denominator moves from 4,401 = 4,038 `catalog_mapped` + 363
  `reviewed_nonmapping` to 4,401 = 4,115 `catalog_mapped` + 286
  `reviewed_nonmapping`.

This is P18-003/P18-005 slice progress only. It does not close the remaining
286 nonmapping facts, dynamic reachability, mutation adequacy, the 203
remaining coverage-cell debts, rendered browser acceptance, or the final
broad candidate gate.

## Specification and invariant authority

No product code changed in this batch. The mapping transaction was preceded by
an explicit product-authority review:

- `docs/guides/HMI_OPERATOR_FIRST_SPECIFICATION.md` moved from Draft to Active
  normative authority and now defines the required layout, process-SVG,
  transport, performance, responsive, and standalone-export contracts;
- `verification/spec-sources.toml` registers that active HMI authority as
  `SPEC_HMI_OPERATOR_FIRST_001`;
- `docs/internal/runtime/trust-runtime-web-ide-full-specification.md` defines
  the unified product shell and runtime-integration contract;
- `docs/specs/11-runtime-engine.md` defines bounded HMI history persistence,
  the unified Web IDE shell, standalone configuration UI behavior, Runtime
  Cloud state/dispatch/rollout behavior, and audited WAN-allowlist update
  ordering; and
- the six new invariants are:
  `RT_HMI_BROWSER_PRODUCT_001`, `RT_WEB_IDE_UNIFIED_SHELL_001`,
  `RT_CONFIG_UI_WORKSPACE_001`, `RT_RUNTIME_CLOUD_STATE_PROJECTION_001`,
  `RT_RUNTIME_CLOUD_DISPATCH_001`, and
  `RT_RUNTIME_CLOUD_ROLLOUT_001`.

The exact accepted mapping population spans these 13 invariants:

```text
RT_ADS_ONBOARDING_ROUTE_SECURITY_001
RT_COMM_APPLY_MUTATION_001
RT_COMM_DISCOVERY_BROWSE_001
RT_CONFIG_SCHEMA_001
RT_CONFIG_UI_WORKSPACE_001
RT_HMI_BROWSER_PRODUCT_001
RT_RUNTIME_CLOUD_ACTION_POLICY_001
RT_RUNTIME_CLOUD_DISPATCH_001
RT_RUNTIME_CLOUD_ROLLOUT_001
RT_RUNTIME_CLOUD_STATE_PROJECTION_001
RT_WEB_IDE_PRODUCT_SURFACE_001
RT_WEB_IDE_UNIFIED_SHELL_001
UI_STATUS_001
```

The connector-status golden tests bind the existing cross-surface
`UI_STATUS_001` authority. They do not create new protocol-area mappings
against the HMI-owned `SPEC_CONNECTOR_STATUS_001` source.

## Rejected drafts and clean rebuild

Three drafts were rejected before acceptance:

1. The first authority draft used the legacy coverage dimension
   `permission_or_policy` for Runtime Cloud state. Targeted staleness
   validation rejected it; the accepted authority uses
   `auth_or_permission`.
2. The next draft used coverage rationales that did not contain a canonical
   catalog-reconciliation marker. The transaction therefore preserved 26
   false `gap_open` cells and raised the count from 203 to 229. That lock was
   rejected and the transaction was rebuilt from the exact base with
   canonical pre-transaction rationales.
3. Full identifier isolation then found eight new area findings across four
   connector-status catalog rows. The proposed protocol invariant and the
   HMI-owned connector-status specification did not share an area. Those four
   tests were rebound to `UI_STATUS_001`, the transaction was rebuilt again,
   and the final staleness result contains zero findings for any of the 77
   discoveries, 77 catalog identities, test names, or 13 selected invariants.

No rejected lock or partially reconciled catalog was copied into the accepted
checkout.

## Exact checkout and bootstrap

- Canonical local program checkout:
  `/home/johannes/projects/trust-platform`
- Canonical local branch: `plc-verification-program`
- Exact accepted selection checkpoint:
  `65652cfba7a131681d9cbacdaa6705ad1b5955f8`
- Remote primary agent-file source:
  `trust-builder:/home/johannes/projects/trust-platform`
- Remote primary source state during bootstrap:
  detached at `3479807f26f56b8347d588d658cd9301d6735050`
- Validation destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-hmi3-65652cfb`
- Builder host: `scena-rust-builder`
- Validation checkout mode: detached HEAD at the exact selection checkpoint

The destination `AGENTS.md` and complete `.codex/skills/**` directory were
manually copied from the remote primary checkout before validation. Exact
verification passed:

```text
AGENTS.md
  76576b5464443f4128a67c325527d23e59da46c019da7b79f39454dabc250263
.codex/skills
  diff -qr against the remote primary source: no differences
```

The required destination copies of `trust-test-authoring`,
`trust-remote-builder`, `st-lsp-solid`, and `trust-release-hygiene` were read
before the transaction and focused validation.

The focused builder preflight reported 23 GiB free on `/home/johannes` and
2.4 GiB free on `/tmp`. That was sufficient for the metadata transaction and
focused Python validators. No broad compile or workspace gate ran.

## Source and execution chain

The assertion inventory was frozen at
`63532855e2eaedb714469817f8411c56c2743676`. Exact execution was retained from
`b636380db507db633f1bbb790fd1a2327a112d13`, and a separate source-equivalence
record revalidated all 896 source paths against the accepted selection
checkpoint `65652cfba7a131681d9cbacdaa6705ad1b5955f8`.

Exact execution:

```text
trust-runtime library shard
  selected: 13
  passed:   13
  failed:   0

hmi_readonly_integration shard
  selected: 20
  passed:   20
  failed:   0

web_ide_integration shard
  selected: 18
  passed:   18
  failed:   0

web_io_config_integration shard
  selected: 26
  passed:   26
  failed:   0

total
  selected: 77
  passed:   77
  failed:   0
```

Retained core artifact digests:

```text
assertions-77.json
  92c6c407976456627a65c606ac23c978872d44db28e473004044190375f8f575
assignments-77.json
  085a21db0fec02eb884d6ad70c43658f1b852a42f6cfbcc12d296954ba227091
live-scan.json
  c98808bbb4ef48de8656c4f0708c6d45e19713d9ec89c13a5328445fa2fab18f
execution-summary.json
  7e6a8732b40e43d4e987486d3897a72fdcd3b5008a87e98a1d54869e857c7df4
source-equivalence.json
  f2201ecbadc80f957a475400173b6d7018f345cc3c95af169f5eacaee43b93b9
staleness.txt
  45fc605ff356aa891fa1d61214066d83585082e979878638bacce913294241c4
```

The machine-readable review lock is
`verification/p18/review-locks/hmi-web-control-io-oracle-77-closure-lock.json`
with digest
`2365e7688bfb7bf737deb1cdffbba90b8c2d73c8335a2f466bf1671d68f22aa4`.

## Validation and staged cadence

On the manually bootstrapped exact builder checkout:

- the atomic semantic transaction validator passes;
- all 17 lock-bound metadata digests and all 13 lock-bound artifact digests
  match their live files;
- all six new invariants have only `covered` catalog-coverage cells;
- the repository-wide `gap_open` count is exactly 203;
- `git diff --check` passes;
- the independent root-record audit proves exactly 154 proposal/decision
  records were added and all 6,674 pre-existing root records remained
  byte-for-byte identical; and
- the focused mapping suite passes 72/72:

```text
python3 -m unittest \
  scripts.verification.mapping_catalog_updates_tests \
  scripts.verification.mapping_proposals_tests \
  scripts.verification.mapping_review_batch_tests \
  scripts.verification.mapping_review_tests \
  scripts.verification.mapping_transaction_tests \
  scripts.verification.test_catalog_denominator_tests \
  scripts.verification.mapping_toml_tests \
  scripts.verification.metadata_validator.mapping_metadata_tests
  72/72 pass
```

The repository-wide catalog-staleness gate remains red with 1,620 accepted
global Phase 18 metadata findings. Full isolation checks 244 batch identifiers
and finds zero target-linked findings. That global red remains debt and is not
reported as a passing gate.

The mandatory complete changed-path planner was also run with `--intent docs`
and every one of the 24 changed paths in this batch. It exits 6 before emitting
a plan because the same 1,620 global metadata findings block planning; the
planner stderr has digest
`674406f637ff94adb590caf9b37ec8b26ad1262a08e86cac09e95c5af98b4e6b`.
The same 244-identifier isolation finds zero batch-linked planner findings.
The planner's global red is retained as open Phase 18 debt and is not treated
as a passing result.

The canonical local checkout contains unrelated uncommitted runtime edits.
Their complete binary diff digest remained
`01458d43c50cd7dfe5cc6eb4649ac1a5a7b8595e61e26d2ce3e76c0b52de4fb4`
before and after the accepted metadata copy. Those user changes were neither
overwritten nor included in this transaction; the clean exact builder result
is the authoritative source-bound proof.

This mapping proof does not claim a real rendered HMI or Web IDE acceptance
pass. Browser-visible validation remains explicit downstream debt owned by
`VERIF-P18-014`.

No workspace-wide `just fmt`, `just clippy`, `just test`, or `just test-all`
command ran. Those remain reserved for the single frozen candidate at
`VERIF-P18-016`.
