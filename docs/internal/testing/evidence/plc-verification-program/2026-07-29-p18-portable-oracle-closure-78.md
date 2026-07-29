# Phase 18 portable assertion-oracle closure

Date: 2026-07-29

## Scope and result

This coherent batch completed semantic assertion review and exact focused
execution for the complete remaining portable assertion-oracle population at
`8a0a9fa2f9377a88b4fac4bbe5fbaaa2c0764443`.

The accepted result is:

- all 78 selected discoveries are catalog-mapped and leave
  `assertion_oracle_unresolved`;
- all 865 extracted assertions are classified exhaustively: 402 are direct
  product oracles and 463 are setup or preconditions;
- the direct oracle assertions have 403 exact behavior bindings;
- no assertion is accepted as a non-oracle observation, and no unresolved
  helper or assertion-empty identity remains;
- exact focused execution passes 78/78 selected tests across 20 owning target
  groups;
- 21 bounded invariants establish the accepted product, PLCopen, developer
  workflow, runtime, platform, and supply-chain authority, including six new
  invariants;
- all newly introduced coverage cells reconcile to accepted mappings, so the
  repository-wide `gap_open` count remains 203;
- an independent root-record audit proves exactly 78 discovery mappings and
  156 proposal/decision records were added while all 6,894 pre-existing root
  records remained byte-for-byte identical; and
- the live denominator moves from 4,401 = 4,148 `catalog_mapped` + 253
  `reviewed_nonmapping` to 4,401 = 4,226 `catalog_mapped` + 175
  `reviewed_nonmapping`.

This is P18-003/P18-005 slice progress only. It does not close the remaining
175 nonmapping facts, dynamic reachability, mutation adequacy, the 203
remaining coverage-cell debts, the Windows-only platform lane, or the final
broad candidate gate.

## Specification and invariant authority

No product code changed in this batch. Product specifications now explicitly
own:

- deterministic repeated build/check behavior and check-mode no-write
  guarantees;
- historian alert debounce and file-hook behavior;
- T0 shared-memory corruption rejection and hot-path exclusions;
- the Linux realtime host profile;
- runtime Agent API command conformance;
- initializer and reference architecture constraints;
- user-facing runtime value presentation;
- shipped project and OSCAT example behavior; and
- CI reliability and release-gate artifact aggregation.

PLCopen import/export tests remain bound to the existing reviewed PLCopen
product decisions. These are product and interchange contracts, not IEC
decisions or deviations.

The transaction used these 21 invariants:

```text
BROWSER_ANALYSIS_FOUNDATION_CONFORMANCE_001
DEV_CI_GATE_CONTRACT_001
DEV_DOCS_GENERATION_001
DEV_SHIPPED_PROJECT_EXAMPLES_001
OSCAT_EXAMPLE_PROJECTS_001
PLCO_CODESYS_METADATA_001
PLCO_EXPORT_ADAPTERS_001
PLCO_MIGRATION_REPORT_AND_EXTENSION_001
PLCO_ST_COMPLETE_INTERCHANGE_001
RT_ADS_CLI_AUTHORING_001
RT_AGENT_COMMAND_CONFORMANCE_001
RT_BUILD_COMMAND_001
RT_CONFIG_SCHEMA_001
RT_GPIO_DRIVER_001
RT_HISTORIAN_RECORDING_001
RT_INITIALIZER_ARCHITECTURE_001
RT_LINUX_REALTIME_PROFILE_001
RT_REALTIME_T0_TRANSPORT_001
RT_RETAIN_PERSISTENCE_CODEC_001
RT_RUNTIME_CLOUD_ACTION_POLICY_001
RT_RUNTIME_VALUE_PRESENTATION_001
```

The six new authorities are
`DEV_CI_GATE_CONTRACT_001`, `DEV_SHIPPED_PROJECT_EXAMPLES_001`,
`OSCAT_EXAMPLE_PROJECTS_001`, `RT_INITIALIZER_ARCHITECTURE_001`,
`RT_LINUX_REALTIME_PROFILE_001`, and
`RT_RUNTIME_VALUE_PRESENTATION_001`.

## Platform boundary

The only two remaining `assertion_oracle_unresolved` tests are Windows-only:

```text
path_to_uri_strips_extended_length_prefix
uri_to_path_decodes_drive_letter
```

The Linux assertion extractor resolves no syntax nodes for their
platform-gated bodies. They remain explicit Windows authority and execution
debt; they were not silently retired or represented as Linux proof.

## Exact checkout and bootstrap

- Canonical local program checkout:
  `/home/johannes/projects/trust-platform`
- Canonical local branch: `plc-verification-program`
- Exact selection and execution checkpoint:
  `8a0a9fa2f9377a88b4fac4bbe5fbaaa2c0764443`
- Remote primary agent-file source:
  `trust-builder:/home/johannes/projects/trust-platform`
- Validation destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-portable-78-8a0a9fa2`
- Builder host: `scena-rust-builder`
- Validation checkout mode:
  detached HEAD at the exact selection checkpoint

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
`trust-remote-builder`, `st-lsp-solid`, `trust-release-hygiene`, and
`trust-lsp-iec` were read before specification review, the transaction, and
focused validation.

The scanner checkpoint records
`dirty:8a0a9fa2f9377a88b4fac4bbe5fbaaa2c0764443` because reviewed authority
and metadata changes were present when the source scan was captured. The
atomic transaction independently required the exact selection checkpoint and
review-lock inputs before applying the accepted mapping.

## Source and execution chain

Exact `cargo nextest` expressions selected all 78 tests from 20 owning target
groups. The retained execution summary reports:

```text
target groups: 20
selected:      78
passed:        78
failed:         0
```

Every group retains its exact list command, exact run expression, resolved
binary and fully qualified test identities, raw JSONL output, stderr, and
SHA-256 digests.

Retained core artifact digests:

```text
assertions-78.json
  0f6ba2c9fb471062c7fe39dfc342dcb730e82b3822e620d08ecfe193c807631a
assignments-78.json
  4553cef2cb49cc313c4c0d6032793092e048aabfea4b10cd2fda314b1c49aa2b
live-scan.json
  505a087fe03f0936c174ce8b5dece6260688b5f56ad031c2ba70947c4c5a7766
execution-summary.json
  426d74f4927100a660dbffbca8fcbd5b09bd38b562bd289f37e4352e69f0ee6c
staleness.txt
  45fc605ff356aa891fa1d61214066d83585082e979878638bacce913294241c4
```

The machine-readable review lock is
`verification/p18/review-locks/portable-oracle-78-closure-lock.json` with
digest
`3791d7992756ba46581bc2c523c618a4dcd4a397f260b8538d8faf98f5aacec9`.
All 25 lock-bound metadata digests and all 44 lock-bound artifact digests
match their live files.

## Validation and staged cadence

On the manually bootstrapped exact builder checkout:

- the atomic semantic transaction passes;
- all six new invariants have only `covered` catalog-coverage cells;
- the repository-wide `gap_open` count is exactly 203;
- `git diff --check -- docs verification` passes;
- the independent root-record audit passes with 78 introduced discoveries,
  156 added proposal/decision records, and 6,894 unchanged pre-existing root
  records; and
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
global Phase 18 metadata findings. Full isolation checks all 156 batch
identifiers and finds zero target-linked findings. That global red remains
debt and is not reported as a passing gate.

The mandatory complete changed-path planner was run with `--intent docs` and
every one of the 33 changed paths in this batch. It exits 6 before emitting a
plan because the repository-wide metadata validator remains red with
pre-existing Phase 18 findings; the summarized stderr shows VM/compiler
invariant findings and reports 1,759 additional findings. Planner stdout is
empty. The retained artifact digests are:

```text
planner.stdout.txt
  e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
planner.stderr.txt
  f4eaf0154162f2b11ca7508e6e2acba7ac33e78e72bea935e1907307e4b0228a
```

The planner's global red is retained as open Phase 18 debt and is not treated
as a passing result. The complete staleness isolation described above still
finds zero findings for the 156 accepted discovery and catalog identities in
this batch.

The canonical local checkout contains unrelated uncommitted runtime edits.
Their complete binary diff digest remains
`01458d43c50cd7dfe5cc6eb4649ac1a5a7b8595e61e26d2ce3e76c0b52de4fb4`.
Those user changes were neither overwritten nor included in this transaction;
the clean exact builder result is the authoritative source-bound proof.

No workspace-wide `just fmt`, `just clippy`, `just test`, or `just test-all`
command ran. Those remain reserved for the single frozen candidate at
`VERIF-P18-016`.
