# Phase 18 ignored-test execution-contract mapping closure

Date: 2026-07-29

## Scope and result

This coherent batch eliminates the complete ignored-test registry class from
the catalog denominator:

```text
perf_soak       15
lab_required     5
manual           3
total           23
```

All 23 live scanner facts now have exact generated catalog mappings to:

```text
VERIFICATION_IGNORED_TEST_EXECUTION_001
```

The mappings prove only exact registry identity and execution routing. They
preserve each discovery's ignored-test registry ID, class, reason, unblock
condition, native environment or reviewed topology, and retained-artifact
boundary. They do not claim that the ignored product test ran, that its product
assertions are proved, or that a structural metadata check substitutes for the
required native lane.

The extraction review observes 161 assertions and eight unresolved helper
observations across the 23 sources. All are deliberately classified outside
this structural proof. Product assertions claimed as proved: zero. Native
ignored tests executed: zero.

The exact denominator moves from
4,401 = 4,295 `catalog_mapped` + 106 `reviewed_nonmapping` to
4,401 = 4,318 `catalog_mapped` + 83 `reviewed_nonmapping`. All 83 remaining
facts are `not_catalogable_product_oracle`.

This is P18-003/P18-005 progress, not global row closure. The 23 native
performance/soak, hardware-lab, and manual/parent-harness executions remain
explicit P18-004/P18-007/downstream proof debt. The remaining 83 product
authorities, 203 `gap_open` coverage cells, dynamic reachability, assertion
strength, adequacy campaigns, and final broad candidate gate are also open.

## Authority and proof boundary

`SPEC_VERIFICATION_INVENTORY_001` now requires every ignored or conditional
test fact to retain:

- its exact live `verification/ignored-tests.toml` identity and scanner join;
- its ignore class, reason, unblock condition, native environment or topology,
  and public-claim boundary;
- a source-SHA-bound terminal result from the native execution lane; and
- the retained artifact required by that lane.

The contract explicitly rejects catalog presence, registry validation, a
default-suite filter, a missing-environment skip, or a parent-harness child
being undiscovered as product proof. Manual children must run through their
named parent or documented manual command; performance and soak tests must run
through their declared measurement lane; lab-required tests must run against
the reviewed physical topology.

These are verification-governance rules. They do not change runtime, LSP,
PLCopen, or IEC behavior and require no IEC decision, IEC deviation, changelog
entry, or version bump.

## Exact checkout and bootstrap

- Canonical local program checkout:
  `/home/johannes/projects/trust-platform`
- Canonical local branch:
  `plc-verification-program`
- Exact selection checkpoint:
  `763ec19df6cdaa3aba677326f40d3ab14cbbee74`
- Remote primary agent-file source:
  `trust-builder:/home/johannes/projects/trust-platform`
- Validation destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-ignored-23-763ec19d`
- Validation destination mode:
  detached HEAD at the exact selection checkpoint
- Builder host:
  `scena-rust-builder`

The destination `AGENTS.md` and complete `.codex/skills/**` directory were
manually copied from the remote primary checkout before validation. Exact
verification passed:

```text
AGENTS.md
  76576b5464443f4128a67c325527d23e59da46c019da7b79f39454dabc250263
.codex/skills
  diff -qr against the remote primary source: no differences
```

The required private validation input was also restored manually from the
remote primary checkout:

```text
.claude/hooks/pre_tool_use_gate.py
  be49797cabae8807d39c41e3c0d75b1dd527ada4e2c8889120aab6d7f7ec911d
```

The required destination copies of `trust-test-authoring`,
`trust-remote-builder`, `st-lsp-solid`, `trust-release-hygiene`, and
`trust-lsp-iec` were read before specification, validator, mapping, and
focused validation work. The local commit objects were transferred to the
builder as an incremental Git bundle; no branch was pushed or switched.

Builder disk preflight reported 18 GiB free on `/home/johannes` and 1.7 GiB on
`/tmp`. That is sufficient for the scoped metadata work but below the program's
broad-gate target, so no broad compile or test gate was started.

The live scanner generated exactly 4,401 facts from 764 source files with zero
scanner warnings:

```text
live-scan.json
  efc5d9c6cb40446727919171da0bddb163e943c2a3571e89dfa1df115208f1d8
live-scan.md
  99ead561e8fb288965bbaa79031c580d6b27ceb9bb568b38244f8178b703a2a3
```

## Assertion inventory and proof boundary

Source-closure extraction of the complete 23-test population completed in
7.03 seconds:

```text
assertions-23.json
  8c445bde6a61b464c88e50297d05e7c2bd69df76055654fca719ead45bb314a7

observed assertions             161
unresolved helper observations    8
claimed as product proof           0
```

The observed assertions remain product-proof debt. The structural mappings
bind only the registry and execution contract; they do not promote those
assertions to oracle-bound evidence.

## Focused structural execution

All 23 mappings share this exact structural command:

```text
python3 -m unittest \
  scripts.verification.ignored_test_inventory_tests \
  scripts.verification.ignored_test_schema_drift_tests \
  scripts.verification.ignored_test_source_contract_tests
```

Result:

```text
17/17 pass in 0.199s
stdout sha256 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
stderr sha256 9f83171d84403da1013eec8262e4e4904f03d6e49a79212d83ddaba0664c1c33
```

A broader 22-test registry/hardware command was retained twice and remains
red. The first run exposed a missing private hook input in the isolated
checkout:

```text
22 tests in 201.918s; failure/error
stderr sha256 b45bd5ab7a5343d29697a92bd310baedabbdee6b479b6ff9eaabb341ea041728
```

After the hook was restored and verified, the same command reached the known
repository-wide evidence-index and metadata backlog:

```text
22 tests in 201.651s; failure/error
stderr sha256 a26dfcf856d2a0f322c7128442f3e77c9b22389b437eec582c175de54e338f27
```

Neither run is treated as a behavioral expected-red or a pass. The accepted
focused proof is the isolated 17-test structural suite above.

## Structural ignored-test review rule: red and green

The existing mapping-review validator correctly rejected these Rust tests
because their assertions were not oracle-bound and some source closures
contained unresolved helpers. That rule must remain mandatory for ordinary
product mappings, but the ignored-test registry itself needed a structural
execution-routing contract without falsely claiming product proof.

A new focused test first established the required assertion red:

```text
test:
  MappingReviewContractTests.
  test_ignored_inventory_acceptance_keeps_product_assertions_unproved

expected red:
  acceptance requires an oracle-bound assertion
  acceptance is blocked by unresolved assertion helpers

result:
  1 selected test; expected failure
  stderr sha256 30102c75a9b6224b97b3d7250d8aa9b6935fb47167a76a26efd30c5fd7308a8e
```

The minimum validator change permits this structural route only when:

- the discovery is an ignored or conditional Rust test;
- every observed assertion is exhaustively classified as setup/precondition
  or non-oracle observation, with no product behavior binding;
- the accepted catalog class is exactly `metadata_validation`;
- catalog area and owner are exactly `verification`;
- every selected invariant belongs to verification governance; and
- source digests, exact command, reviewers, and behavior references still
  validate.

Only this structural route can tolerate its recorded unresolved helper
observations. An ordinary, non-ignored Rust test still requires a real
oracle-bound assertion and rejects unresolved helpers.

The same focused test then passed:

```text
1/1 pass
stderr sha256 c346f1228fe88f4ab3cb336ffa610dc3a34ec7c7bb7d05df7a1c222f2d928881
```

The then-complete mapping-review module passed 10/10. After the transaction,
the complete mapping suite passed 74/74:

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

stderr sha256 3fb3d168534bfd1717406eb738be3673747b2d5471c0d7227277d3221b3adb02
```

The mandatory pre-change planner ran with `--intent docs` and all then-known
specification, validator-test, validator, and invariant paths. It exits 6 on
the accepted global Phase 18 metadata backlog:

```text
planner-before.stdout.txt
  e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
planner-before.stderr.txt
  0f24c780506b0fdafa67b64da6d5a118f27e528171f8df4ecc4d0d9c448724da
```

## Atomic mapping and independent review

The fail-closed transaction joins each selected denominator row to its exact
live ignored-test registry record, retains the complete assertion inventory as
unproved product evidence, applies a two-reviewer structural decision, appends
the generated catalog row, updates the invariant test list, and changes only
the selected denominator rows.

Transaction helper:

```text
/tmp/p18_apply_ignored_23.py
  e1608291738e04cc02b6b05730bb6e5188d0c45b55116b7e778b62c677a8d649
```

The transaction reports:

```text
ignored tests selected                 23
reviewed_nonmapping before            106
catalog mappings added                 23
reviewed_nonmapping after              83
native ignored tests executed           0
```

Independent diff audit helper:

```text
/tmp/p18_audit_ignored_23_decision_diff.py
  5dd5a8670d1d68efab3526165263761e84e59e734bebdc8ae4459914686fa561
```

The audit proves:

```text
exact ignored-registry joins           23
new catalog mappings                   23
changed denominator rows               23
new proposal/decision root records     46
unchanged pre-existing root records 7,188
```

Audit stdout:

```text
bb80433a7617f58369fd15e0ee0adaf61e8ae120294af079da8504ea368ea4d4
```

The repository-wide staleness gate remains red on accepted global Phase 18
metadata debt. Exact isolation across all 23 discovery IDs and all 23 accepted
catalog IDs finds zero target-linked findings:

```text
staleness.stdout.txt
  a97315c6cb7052d334de4db7774dc6d5eb111408fb8c17a1b2b0426107843ec1
target-isolation.json
  a7bed1e17518173b38e3fec4bfc2bbfbfb35067a3f5f5c4425cc401cff33d01f
```

The complete metadata validator also remains red on the accepted global Phase
18 backlog. Exact filtering finds zero findings naming
`VERIFICATION_IGNORED_TEST_EXECUTION_001`:

```text
metadata-validator.stdout.txt
  e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
metadata-validator.stderr.txt
  00ca116e0b07ddba15958b7c67c10d7be98f146309287f16837b7caf41902933
```

The machine-readable review lock is
`verification/p18/review-locks/ignored-test-execution-23-closure-lock.json`.
Its final digest and complete changed-path planner evidence are recorded below
after the canonical board and this evidence file enter the lock input closure.

## Staged validation

The mandatory complete changed-path planner covers the specification,
validator and focused test, invariant, four atomic mapping documents, review
lock, canonical board, and this evidence file. Its repository-wide outcome is
recorded here after the lock is refreshed:

```text
complete-planner.stdout.txt
  e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
complete-planner.stderr.txt
  f4eaf0154162f2b11ca7508e6e2acba7ac33e78e72bea935e1907307e4b0228a
review-lock sha256
  71d156a8f2543d45b9889b16a9e786db682b1345d7e826493dbc3243646fe8f5
```

That expected global red remains open debt and is not reported as passing.
Target isolation still finds zero findings for this accepted batch. All ten
lock-bound metadata digests and all 35 lock-bound artifact digests match their
live files.

The canonical local checkout contains unrelated uncommitted runtime edits.
Their complete binary diff digest remains
`01458d43c50cd7dfe5cc6eb4649ac1a5a7b8595e61e26d2ce3e76c0b52de4fb4`.
Those user changes were neither overwritten nor included in this transaction.

No workspace-wide `just fmt`, `just clippy`, `just test`, or `just test-all`
command ran. No ignored product test ran in its native performance/soak,
hardware-lab, or manual/parent-harness lane. Those executions remain explicit
downstream debt rather than being falsely inferred from structural metadata
validation.
