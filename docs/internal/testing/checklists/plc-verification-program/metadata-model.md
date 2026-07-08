# Verification Metadata Model

This document owns the future `verification/` control-plane data model. It fixes
the schema drift risk by naming every committed metadata file, every schema, and
the required fields for each record type.

## Storage Model

Executable tests stay where their native tooling expects them. Metadata and
generated reports describe those tests; they do not centralize them.

Committed metadata:

```text
verification/
  README.md
  invariants/
    compiler.toml
    runtime-safety.toml
    bytecode-vm.toml
    protocols.toml
    editor-safety.toml
    hmi-ui.toml
    release.toml
    supply-chain-platform.toml
  suites/
    veryquick.toml
    pr.toml
    nightly.toml
    release.toml
    hardware-lab.toml
  schemas/
    invariant.schema.json
    suite.schema.json
    catalog.schema.json
    ignored-test.schema.json
    risk-register.schema.json
    evidence.schema.json
    spec-source.schema.json
    spec-gap.schema.json
  test-catalog.toml
  ignored-tests.toml
  risk-register.toml
  evidence-index.toml
  spec-sources.toml
  spec-gaps.toml
```

Generated artifacts:

```text
target/gate-artifacts/**
gate-artifacts/** in CI workspace
docs/internal/testing/evidence/<area>/<yyyy-mm-dd>/ for durable manual/lab proof
```

Generated reports are CI artifacts unless a specific reviewed summary is
intentionally committed.

Durable evidence is one of:

- a committed repo file,
- a CI artifact with named workflow/run and retention,
- a public release object.

A path matched by `git check-ignore` is not durable evidence and must fail
metadata validation when referenced as durable proof.

## Common Fields

Every committed metadata record must include:

- `schema_version`
- `id`
- `title`
- `owner`
- `status`
- `area`
- `updated_at` or `last_reviewed`
- `notes` when the status is not self-explanatory

Domain-specific aliases such as `suite_id` or `test_id` may exist for display or
command generation, but `id` is the canonical key used by references.

Every generated report must include:

- generator name and version,
- command,
- commit or dirty-worktree marker,
- timestamp,
- platform,
- input metadata paths,
- output artifact paths.

## Machine Status Vocabulary

Use snake_case in machine-readable metadata. Human checklist row IDs may use
hyphens, but states must not.

Allowed record statuses:

- `planned`
- `mapped`
- `gap_open`
- `spec_gap`
- `test_written`
- `implemented`
- `validated`
- `blocked`
- `deferred`
- `rejected`
- `unproven`

Legacy prose labels such as `gap-open`, `spec-gap`, or `lab-required` must be
normalized to machine states when written to metadata.

Coverage cell states:

- `covered`
- `covered_by_fuzz`
- `not_applicable`
- `blocked`
- `spec_gap`
- `gap_open`
- `deferred`

Ignored-test classes:

- `red_protective`
- `lab_required`
- `perf_soak`
- `manual`
- `flaky_quarantined`
- `obsolete`
- `unknown`

## Risk Values

- `safety_critical`
- `wrong_result`
- `silent_corruption`
- `false_status`
- `data_loss`
- `security`
- `compatibility`
- `performance`
- `usability`
- `maintenance`
- `supply_chain`
- `platform`

## Oracle Kinds

- `iec_standard`
- `protocol_standard`
- `trust_contract`
- `design_contract`
- `public_doc`
- `external_reference`
- `hardware_observation`
- `reviewed_decision`
- `reviewed_deviation`
- `release_contract`

## Specification Source Authority

Authority levels:

- `normative_external`: IEC, protocol standards, LSP spec, vendor manuals.
- `normative_product`: truST-owned behavior contract implementation must follow.
- `reviewed_decision`: documented ambiguity decision.
- `reviewed_deviation`: documented deviation from a standard or vendor behavior.
- `public_claim`: marketing/docs/user promise that needs proof or narrowing.

Source status values:

- `active`: usable as an oracle subject to authority rules.
- `draft`: useful but not yet binding.
- `stale`: source appears outdated.
- `superseded`: source has an explicit replacement.
- `missing`: expected source does not exist.
- `conflicting`: conflicts with another source.

Precedence rule:

1. A `reviewed_deviation` wins over the external standard for truST behavior,
   but must cite the external source and justify the deviation.
2. A `reviewed_decision` wins when the standard is ambiguous.
3. A `normative_external` source wins when no reviewed decision/deviation exists.
4. A `normative_product` source wins for truST-specific behavior not governed by
   an external standard.
5. A `public_claim` never wins over normative sources and may never serve as
   the oracle for a `safety_critical` invariant. It creates a proof obligation,
   gets narrowed, or becomes a spec/public-claim gap.
6. Sources with `source_status` in `draft`, `stale`, `superseded`, `missing`, or
   `conflicting` cannot be used as final test oracles for safety-critical
   behavior.

If two sources at the same authority level conflict, create a `spec_gap` with
gap class `conflicting_sources`.

## Schema Versioning

All metadata records start with `schema_version = 1`.

Schema changes must be handled explicitly:

- additive optional fields can stay on the current schema version,
- new required fields require a schema version bump,
- renamed enum values require a migration note and validator compatibility path,
- removed fields require a migration note and stale-field diagnostic,
- generated reports must include the schema version they read.

The validator must fail closed when it sees:

- unknown required version,
- unknown machine status,
- missing required fields,
- stale file/test paths or test names,
- unknown suite IDs,
- unknown invariant IDs,
- unknown evidence IDs,
- cyclic redirects,
- public claims with no invariant/proof/gap,
- durable evidence paths matched by `git check-ignore`,
- empty-string sentinels used as null values,
- `status = "test_written"` with no mapped test or no red/protective evidence
  for a behavior-changing fix,
- `status = "implemented"` with no mapped test or no targeted green evidence,
- `status = "validated"` unless all of the following are true:
  `proof_level` is `G1`, `G2`, or `R1`; `tests` is non-empty;
  `evidence_refs` is non-empty; `spec.status = "specified"`; all evidence refs
  resolve to evidence records; and safety-critical, wrong-result,
  silent-corruption, and false-status invariants have no applicable coverage
  cell in `gap_open` or `spec_gap`.

## Record Shapes

### Invariant Record

```toml
schema_version = 1
id = "RT_SAFE_STOP_001"
title = "Safe outputs are applied on deliberate stop"
area = "runtime_safety"
risk = "safety_critical"
status = "planned"
owner = "trust-runtime"
last_reviewed = "2026-07-08"

claim = "On deliberate stop or contained fault, configured safe outputs are applied or runtime reports not_safe."

proof_level = "S0"
source_refs = ["crates/trust-runtime/src/runtime/core/lifecycle.rs"]
tests = ["TEST_RUNTIME_SAFE_OUTPUTS_ON_STOP"]
gates = ["pr", "release"]
evidence_refs = []

failure_modes = [
  "worker_queue_full",
  "device_reconnecting",
  "worker_down",
  "sigterm",
]

missing = ["hardware_lab_real_output_module"]

[spec]
status = "specified"
source_refs = ["SPEC_RUNTIME_SAFE_STATE_001"]

[oracle]
kind = "design_contract"
ref = "docs/internal/architecture/runtime-safety-fail-closed-contract.md"

[[coverage.cells]]
dimension = "happy_path"
state = "gap_open"
rationale = "Initial seed; test mapping not complete."

[[coverage.cells]]
dimension = "hardware_or_network_fault"
state = "blocked"
rationale = "Requires hardware-lab proof for real output module."
```

All top-level keys appear before the `[spec]`, `[oracle]`, and
`[[coverage.cells]]` tables; TOML assigns later bare keys to the most recently
opened table, so key-after-table layouts silently produce wrong records.

Required invariant fields:

- `schema_version`
- `id`
- `title`
- `area`
- `risk`
- `status`
- `owner`
- `claim`
- `spec.status`
- `spec.source_refs` or `spec_gap_refs`
- `oracle.kind`
- `oracle.ref`
- `proof_level`
- `tests`
- `gates`
- `missing`
- `coverage.cells`

Allowed `spec.status` values:

- `specified`
- `ambiguous`
- `missing`
- `conflicting`
- `external_blocked`

### Specification Source Record

```toml
schema_version = 1
id = "SPEC_BYTECODE_FORMAT_001"
title = "truST bytecode format"
area = "bytecode_vm"
owner = "trust-runtime"
status = "mapped"
authority = "normative_product"
source_status = "active"
visibility = "internal"
path = "docs/specs/12-bytecode.md"
version = "current"
last_reviewed = "2026-07-08"

covers = [
  "bytecode_container",
  "instruction_stream_shape",
]

known_limitations = [
  "validator semantic contract is incomplete and tracked by SPEC_GAP_BYTECODE_VALIDATOR_001",
]

conflicts_with = []
```

Required fields:

- `schema_version`
- `id`
- `title`
- `area`
- `owner`
- `status`
- `authority`
- `source_status`
- `visibility`
- `path` or `external_ref`
- `covers`
- `known_limitations`

Public claims are committed spec-source records with `authority =
"public_claim"`. They must include `claim_text` and `surface_ref` fields, and
they cannot serve as final safety-critical oracles.

### Spec Gap Record

```toml
schema_version = 1
id = "SPEC_GAP_VM_OWNER_001"
title = "Bytecode instance-owner contract is not written"
area = "bytecode_vm"
risk = "silent_corruption"
owner = "trust-runtime"
status = "spec_gap"
gap_class = "missing_behavior"
blocking_question = "Should ambiguous instance ownership be rejected or represented with explicit owner metadata?"
affected_invariants = ["VM_SEAM_OWNER_001"]
affected_tests = []
candidate_spec_sources = ["SPEC_BYTECODE_FORMAT_001"]
resolution_status = "open"
closeout_evidence = []
last_reviewed = "2026-07-08"
```

Required fields:

- `schema_version`
- `id`
- `title`
- `area`
- `risk`
- `owner`
- `status`
- `gap_class`
- `blocking_question`
- `affected_invariants`
- `candidate_spec_sources`
- `resolution_status`
- `last_reviewed`

Allowed `resolution_status` values:

- `open`
- `decision_recorded`
- `spec_updated`
- `test_mapped`
- `closed`
- `rejected`

### Test Catalog Record

```toml
schema_version = 1
id = "TEST_BYTECODE_UNKNOWN_OPCODE_REJECTED"
test_class = "negative_malformed_input"
area = "bytecode_vm"
path = "crates/trust-runtime/tests/bytecode_validation.rs"
name = "rejects_unknown_opcode"
command = "cargo test -p trust-runtime --test bytecode_validation rejects_unknown_opcode"
owner = "trust-runtime"
status = "mapped"
invariants = ["VM_SEAM_VALID_001"]
oracle_ref = "SPEC_BYTECODE_FORMAT_001"
spec_ref = "SPEC_BYTECODE_FORMAT_001"
expected_result = "load rejects before execution with a specific unknown-opcode error"
suite_tiers = ["veryquick", "pr"]
requires_hardware = false
requires_network = false
duration_class = "fast"
last_reviewed = "2026-07-08"

[evidence]
red_or_protective = []
green = []
```

Required fields:

- `schema_version`
- `id`
- `test_class`
- `area`
- `path`
- `command`
- `owner`
- `status`
- `invariants`
- `oracle_ref` or `spec_gap_ref`
- `expected_result`
- `suite_tiers`
- `requires_hardware`
- `requires_network`
- `duration_class`

`invariants` may be empty only while `status` is `planned` or `gap_open`.
Records with `status` in `mapped`, `test_written`, `implemented`, or
`validated` must reference known invariant IDs and an `oracle_ref` or
`spec_gap_ref`.

Stale-test validation must verify both `path` and `name` against scanner output.
A renamed or deleted test function inside an existing file fails validation.

### Suite Record

```toml
schema_version = 1
id = "pr"
title = "PR Required"
area = "suite"
owner = "verification"
status = "planned"
last_reviewed = "2026-07-08"
purpose = "Protect mainline for normal changes."
duration_class = "medium"
environment = "trust-builder"
commands = [
  "just fmt",
  "just clippy",
  "just test",
]
evidence_destination = "target/gate-artifacts/pr/"
includes = ["veryquick"]
excludes = ["hardware_lab"]
```

Required fields:

- `schema_version`
- `id`
- `title`
- `area`
- `owner`
- `status`
- `last_reviewed`
- `purpose`
- `duration_class`
- `environment`
- `commands`
- `evidence_destination`

### Ignored Test Record

```toml
schema_version = 1
id = "IGNORED_VM_FRAME_REF_ESCAPE_001"
test_id = "TEST_VM_FRAME_REF_ESCAPE_REJECTED"
owner = "trust-runtime"
status = "planned"
ignore_class = "red_protective"
reason = "Documents known unsafe behavior before implementation slice starts."
unblock_condition = "SEAM verifier rule implemented and targeted green proof captured."
linked_rows = ["VM_SEAM_REF_001"]
area = "bytecode_vm"
last_reviewed = "2026-07-08"
```

Required fields:

- `schema_version`
- `id`
- `test_id`
- `owner`
- `area`
- `status`
- `ignore_class`
- `reason`
- `unblock_condition`
- `last_reviewed`

### Risk Register Record

```toml
schema_version = 1
id = "RISK_BYTECODE_VALIDATOR_SCOPE_001"
title = "Validator coverage may lag VM assumptions"
area = "bytecode_vm"
risk = "silent_corruption"
owner = "trust-runtime"
status = "planned"
description = "Malformed bytecode could satisfy structural checks while violating VM semantic assumptions."
mitigation = "Map each VM assumption to crafted-bytecode negative tests and validator invariants."
related_invariants = ["VM_SEAM_VALID_001"]
related_spec_gaps = []
evidence_refs = []
last_reviewed = "2026-07-08"
```

Required fields:

- `schema_version`
- `id`
- `title`
- `area`
- `risk`
- `owner`
- `status`
- `last_reviewed`
- `description`
- `mitigation`
- `related_invariants`

### Evidence Record

```toml
schema_version = 1
id = "EVIDENCE_BYTECODE_VALIDATION_20260708_001"
title = "Bytecode validation targeted test run"
area = "bytecode_vm"
owner = "trust-runtime"
status = "planned"
kind = "committed_file"
path = "docs/internal/testing/evidence/plc-verification-program/2026-07-08/bytecode-validation-output.txt"
command = "cargo test -p trust-runtime --test bytecode_validation"
commit = "dirty:6a7492cae"
platform = "trust-builder-linux"
date = "2026-07-08"
suite_id = "pr"
producer = "manual"
generated_report_version = "none"
linked_invariants = ["VM_SEAM_VALID_001"]
linked_tests = ["TEST_BYTECODE_UNKNOWN_OPCODE_REJECTED"]
last_reviewed = "2026-07-08"
```

Required fields:

- `schema_version`
- `id`
- `title`
- `area`
- `owner`
- `status`
- `kind`
- `command` when command-backed
- `commit`
- `platform`
- `date`
- `suite_id` or `release_object`
- `producer`
- `generated_report_version`
- `linked_invariants`
- `linked_tests`
- `last_reviewed`

Allowed evidence kinds:

- `committed_file`
- `ci_artifact`
- `release_object`
- `lab_report`

Evidence rules:

- `commit` is the tested commit, or `dirty:<base-commit>` when the worktree was
  dirty; a bare dirty marker without the base commit is invalid.
- `lab_report` evidence additionally requires `device_model`, `firmware`,
  `topology`, and `env_vars` fields so hardware claims carry device identity.
- `committed_file` paths must be git-tracked; `ci_artifact` refs name workflow,
  run, and artifact, and note retention when the artifact is the only proof.

## Traceability Reports

The program must generate both directions:

Forward trace:

```text
spec source -> invariant -> test -> suite/gate -> evidence -> public claim
```

Reverse trace:

```text
public claim -> evidence -> suite/gate -> test -> invariant -> spec source
```

Traceability report requirements:

- Every safety-critical invariant has a spec source or spec gap.
- Every mapped test has at least one invariant.
- Every release/public claim has a proof path or visible gap.
- Every evidence artifact names command, commit/dirty marker, platform, and
  generated report version.
- Every spec gap lists blocked tests or explains why no test can be authored.

## Verification Tooling Self-Tests

The verification program must test its own tooling.

Required fixtures:

- known-good metadata set,
- missing required field,
- unknown status,
- stale path,
- durable evidence path matched by `git check-ignore`,
- unknown invariant ID,
- unknown suite ID,
- unknown evidence ID,
- public claim without proof/gap,
- ignored test with `unknown` after grace period,
- schema-version mismatch,
- mapped record with empty invariants,
- stale test name inside an existing file,
- `validated` with empty evidence,
- safety-critical `validated` with a `gap_open` or `spec_gap` coverage cell,
- proof level below status requirement,
- traceability cycle or redirect loop.

Required tooling tests:

- schema validator rejects known-bad fixtures,
- catalog scanner detects changed/deleted tests,
- spec-source scanner emits missing/stale/conflict reports,
- changed-file classifier maps paths to code areas and gates,
- report renderer output is stable enough for review,
- generated artifacts distinguish inferred facts from hand-authored intent.
