# Phase 18 governed gate and fuzz structural mapping closure

Date: 2026-07-29

## Scope and result

This coherent batch eliminates both governed structural inventory classes from
the catalog denominator:

```text
gate_inventory_owned     59
fuzz_program_owned        8
total                    67
```

All 67 live scanner facts now have exact generated catalog mappings to two new
verification-governance invariants:

```text
VERIFICATION_GATE_SURFACE_EXECUTION_001
VERIFICATION_FUZZ_TARGET_EXECUTION_001
```

The mappings prove the structural inventory and routing contracts only. They
bind every exact discovery identity to its live gate-inventory or fuzz-program
row, source digest, native command or workflow job, declared suite and
enforcement posture, environment, surface associations, artifact/corpus
policy, and crash-regression handoff. They do not claim that a local metadata
test executed a GitHub job, ran a native fuzz campaign, proved a transitive
product property, or established universal crash freedom.

The exact denominator moves from
4,401 = 4,228 `catalog_mapped` + 173 `reviewed_nonmapping` to
4,401 = 4,295 `catalog_mapped` + 106 `reviewed_nonmapping`. The remaining
population is:

```text
not_catalogable_product_oracle     83
ignored_test_registry_owned        23
```

This is P18-003/P18-005 progress, not global row closure. Native execution of
the 59 gate/workflow surfaces and eight fuzz targets remains explicit
P18-004/P18-007/downstream proof debt. The remaining 106 facts, 203
`gap_open` coverage cells, dynamic reachability, mutation adequacy, and final
broad candidate gate are also still open.

## Authority and proof boundary

`SPEC_VERIFICATION_INVENTORY_001` now defines two explicit contracts:

- gate and workflow catalog presence proves only exact inventory identity,
  routing, posture, native environment, and artifact contract;
- fuzz target catalog presence proves only exact program identity, production
  surface association, native campaign command, tiers, storage policy, and
  crash-regression handoff;
- an assigned GitHub workflow job requires its complete exact-SHA GitHub result
  for native execution proof; a local replay is not a substitute;
- a fuzz target requires a bounded exact-SHA native fuzz campaign for execution
  proof; a structural metadata pass is not a campaign; and
- report-only, conditional, supporting, advisory, excluded, machine-local, and
  clean-bounded-campaign states cannot be escalated into stronger proof.

These are verification-governance contracts. They do not change product,
runtime, LSP, PLCopen, or IEC behavior and require no IEC decision, IEC
deviation, changelog entry, or version bump.

## Exact checkout and bootstrap

- Canonical local program checkout:
  `/home/johannes/projects/trust-platform`
- Canonical local branch:
  `plc-verification-program`
- Exact selection checkpoint:
  `d4b499cc91a8ea66386b168f86e1587e118f294b`
- Remote primary agent-file source:
  `trust-builder:/home/johannes/projects/trust-platform`
- Remote primary source mode:
  detached HEAD at `3479807f26f56b8347d588d658cd9301d6735050`
- Validation destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-governed-67-d4b499cc`
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

The required destination copies of `trust-test-authoring`,
`trust-remote-builder`, `st-lsp-solid`, `trust-release-hygiene`, and
`trust-lsp-iec` were read before specification, validator, mapping, and
focused validation work. The local commit objects were transferred to the
builder as a Git bundle because the remote primary checkout did not yet contain
`d4b499cc`; no branch was pushed or switched.

The live scanner then generated exactly 4,401 facts from 764 source files with
zero scanner warnings:

```text
live-scan.json
  241f7330393ef01b63494b2579dd09d09672b632d9a503029b86c45cb68968cb
live-scan.md
  224ea095d56a0184c6f568efea3b4ce2fccc586bf27eebaa1c66ee152f732f83
```

The scanner records `dirty:d4b499cc91a8ea66386b168f86e1587e118f294b`
because the reviewed specification and invariant sources were present in the
isolated checkout when the exact source scan was captured.

## Focused structural execution

The 59 gate and workflow facts share one complete structural validation
command:

```text
python3 -m unittest \
  scripts.verification.gate_inventory_tests \
  scripts.verification.metadata_validator.suite_contracts_tests
```

Result:

```text
36/36 pass in 4.585s
stdout sha256 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
stderr sha256 f8f0b4af4538f8d87e58f0227f83b041c942eeb30b367bf94b5ded7546f48659
```

The eight root fuzz targets share one complete structural program/source
validation command:

```text
python3 -m unittest \
  scripts.verification.fuzz_program_contract_tests \
  scripts.verification.fuzz_program_source_contract_tests
```

Result:

```text
36/36 pass in 271.425s
stdout sha256 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
stderr sha256 362abcffb676dce33271ea84446e99b03e1745d8fd73d958144c1cefc72f026d
```

An initial broader fuzz command also included
`scripts.verification.fuzz_program_report_tests`. Its report setup stopped on
the known repository-wide metadata backlog rather than a fuzz-contract
assertion. That global red is retained, not counted as the focused red-green
pair, and not reported as passing:

```text
fuzz-program-report-global-red.stderr.txt
  2f2e205d3742f5f85ce12d4845025849613ef4665b40b0c44ce2a0c8483395b5
```

The original shell timing capture concatenated epoch seconds and nanoseconds
on this builder, so the raw arithmetic was not milliseconds. The accepted
duration files are regenerated from the authoritative unittest terminal
summaries above.

## Structural non-assertion review rule: red and green

The first atomic transaction correctly stopped because the existing mapping
review validator required at least one oracle-bound product assertion for
every accepted discovery. That rule remains mandatory for Rust, Structured
Text, VS Code, and ordinary product behavior tests, but it made structural
gate/workflow/fuzz inventory proof impossible.

A new focused test first established the required assertion red:

```text
test:
  MappingReviewContractTests.
  test_structural_inventory_acceptance_uses_metadata_oracle_without_assertions

expected red:
  gate_script, github_workflow_job, and fuzz_target each fail with
  "acceptance requires an oracle-bound assertion"

result:
  1 selected test, 3 expected subtest failures
  stderr sha256 a9e09f098256bd59a2a5e4f6ccc26b57329375313b79b057fdecbb8a53af43db
```

The minimum validator change permits an empty assertion review only when all
of these conditions hold:

- source kind is exactly `gate_script`, `github_workflow_job`, or
  `fuzz_target`;
- the proposal itself contains zero assertions;
- the accepted catalog class is exactly `metadata_validation`;
- catalog area and owner are exactly `verification`;
- every selected invariant belongs to the verification area; and
- the exact command, behavior references, source digest, reviewers, and
  unresolved-helper checks still pass.

The same focused test then passed:

```text
1/1 pass
stderr sha256 d10ffbd9f0faa72cd6837063279524e199b59fbc3d035051302a1e8dbc4b36ad
```

It also proves that an ordinary `rust_unit_test` and a structural fact
misclassified as `behavior_lock` remain red. The complete mapping-review
module passes 9/9.

The validator planner ran before implementation with `--intent bugfix` and
all five then-known validator/specification paths. It exits 6 on the accepted
global metadata backlog:

```text
validator-planner.stdout.txt
  e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
validator-planner.stderr.txt
  37eb3a1158a3802034891324c2818b93f553af7efe6b1447ee36b48e6f83d91a
```

## Atomic mapping and independent review

The fail-closed transaction joins each selected denominator row to the exact
live gate or fuzz inventory record, creates an empty assertion inventory bound
to the exact source digest, applies a two-reviewer structural decision, appends
the generated catalog row, updates the two invariant test lists, and changes
only the selected denominator rows.

Transaction helper:

```text
/tmp/p18_apply_governed_gate_fuzz_67.py
  e61f6988f18cc8475b1a24870b199c701073983d0444613f8f759004f4d334d0
```

The focused mapping suite passes 73/73:

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

stderr sha256 870a0dff965ced4ab21d75c927f71b6102e83223fc56e27cb47c02b32ff35216
```

Independent diff audit helper:

```text
/tmp/p18_audit_governed_gate_fuzz_67_decision_diff.py
  52d591ba884965fcd73d14e64548a13866285839705c69b2cdc25f450dbde488
```

The audit proves:

```text
new catalog mappings:                  67
changed denominator rows:              67
new proposal/decision root records:   134
unchanged pre-existing root records: 7,054
```

The repository-wide staleness gate remains red with 1,620 accepted global
Phase 18 metadata findings. Exact isolation across all 67 discovery IDs and 67
accepted catalog IDs finds zero target-linked findings. The retained
staleness artifact digest is:

```text
45fc605ff356aa891fa1d61214066d83585082e979878638bacce913294241c4
```

The complete metadata validator also remains red on the accepted global Phase
18 backlog. Exact filtering of its full output finds zero findings naming
either new invariant:

```text
metadata-validator.stdout.txt
  e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
metadata-validator.stderr.txt
  4bb140c3ef416a4a7f1cefb252adda1eb2fe030975a923b23b855b7298c52002
```

The machine-readable review lock is
`verification/p18/review-locks/governed-gate-fuzz-67-closure-lock.json` with
digest
`1a559d1e226bb5ba09fd36f208d6809e1ad7c4ebbbe671ee21235bfbdf219631`.
All 18 lock-bound metadata digests and all 27 lock-bound artifact digests
match their live files.

## Staged validation

The pre-change cohort planner was run with `--intent docs` and every planned
cohort path. It exits 6 on the known global metadata backlog:

```text
planner-before.stdout.txt
  e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
planner-before.stderr.txt
  674406f637ff94adb590caf9b37ec8b26ad1262a08e86cac09e95c5af98b4e6b
```

The mandatory complete changed-path planner was run with `--intent docs` and
every one of the 12 changed paths in this batch, including the canonical board,
evidence, review lock, both validator files, both invariants, the specification,
and all four atomic mapping documents. It exits 6 before emitting a plan
because the repository-wide metadata validator remains red with the accepted
Phase 18 backlog. Planner stdout is empty:

```text
complete-planner.stdout.txt
  e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
complete-planner.stderr.txt
  674406f637ff94adb590caf9b37ec8b26ad1262a08e86cac09e95c5af98b4e6b
```

That global red remains open debt and is not reported as passing. The complete
134-identifier staleness isolation still finds zero findings for this accepted
batch.

The canonical local checkout contains unrelated uncommitted runtime edits.
Their complete binary diff digest remains
`01458d43c50cd7dfe5cc6eb4649ac1a5a7b8595e61e26d2ce3e76c0b52de4fb4`.
Those user changes were neither overwritten nor included in this transaction.

No workspace-wide `just fmt`, `just clippy`, `just test`, or `just test-all`
command ran. Native workflow-job, gate, and fuzz-campaign execution also
remains explicitly deferred to the owning downstream rows rather than being
falsely inferred from structural metadata validation.
