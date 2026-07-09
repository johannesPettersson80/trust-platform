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
    bytecode_vm/
      VM_SEAM_SUBRANGE_001.toml
      VM_SEAM_DECLARED_TYPE_001.toml
    runtime_safety/
      RT_SAFE_STOP_001.toml
  suites/
    veryquick.toml
    pr.toml
    nightly.toml
    release.toml
    hardware-lab.toml
  cases/
    bytecode_vm/
  schemas/
    invariant.schema.json
    suite.schema.json
    catalog.schema.json
    ignored-test.schema.json
    risk-register.schema.json
    evidence.schema.json
    spec-source.schema.json
    spec-gap.schema.json
    spec-matrix.schema.json
    matrix.schema.json
    case-file.schema.json
    case-artifact.schema.json
  matrix.toml
  spec-matrix.toml
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

## TOML Container Convention

Use two TOML shapes deliberately:

- Invariants are one record per file:
  `verification/invariants/<area>/<INVARIANT_ID>.toml`. This keeps behavior
  tables reviewable and avoids TOML wrapper scoping such as
  `[invariants.spec]` or `[[invariants.behavior]]`.
- Flat registries use plural wrapper arrays:
  `[[spec_sources]]` in `verification/spec-sources.toml`, `[[spec_gaps]]` in
  `verification/spec-gaps.toml`, `[[required_specs]]` in
  `verification/spec-matrix.toml`, `[[evidence]]` in
  `verification/evidence-index.toml`, and the same pattern for future flat
  registry files.

Bare single-record examples in this document are writable as invariant files or
as the body of one wrapped registry entry after adding the matching
`[[...]]` header. Do not mix top-level single-record and plural-wrapper shapes
in one file.

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

Canonical area values and `verification/spec-matrix.toml` semantics live in
`spec-matrix-model.md`.

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
- unknown area values,
- cyclic redirects,
- public claims with no invariant/proof/gap,
- durable evidence paths matched by `git check-ignore`,
- empty-string sentinels used as null values,
- evidence records with unknown `kind`, invalid commit markers, no `suite_id`
  or `release_object`, or missing kind-specific fields,
- unknown `contract_kind`,
- a `decision_table` invariant with missing behavior rows for applicable
  coverage dimensions,
- a behavior row that supplies expected behavior without an oracle ref or uses a
  spec-gap ref as if it were an oracle,
- a behavior row with expression-language partitions such as `when = "..."`
  instead of the v1 explicit partition keys,
- a required spec-matrix tag that resolves to neither an active source nor an
  open spec gap,
- a spec-matrix source whose `covers` list does not contain the required tag,
- a spec-matrix source whose authority is not allowed by `expected_authority`,
- a test catalog record whose `case_file_digest` does not match the committed
  case file,
- a `not_applicable` coverage cell or waived matrix row without `decision_ref`,
- a safety-critical, wrong-result, silent-corruption, or false-status red/green
  proof whose producer is not `prove.py vN` or an explicitly approved gate,
- a `prove.py green` evidence record that does not pair to a red record for the
  same test, case file digest, and formerly-red case IDs,
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
contract_kind = "state_machine"
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
- `contract_kind`
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

Allowed `contract_kind` values:

- `decision_table`: data-shaped input partition with typed outcomes. This is
  the v1 case-generation target.
- `state_machine`: lifecycle or resource-state behavior.
- `protocol_trace`: ordered protocol exchange or wire-status behavior.
- `fault_scenario`: injected runtime, network, hardware, or OS fault.
- `ui_journey`: visible user workflow and acceptance-board truth.
- `security_policy`: authz, RBAC, secret handling, or supply-chain policy.
- `perf_budget`: latency, jitter, throughput, memory, or size budget.
- `release_matrix`: release artifact, platform, version, or public-claim
  contract.

The invariant record is the structured specification unit. Do not add a
separate behavior-record layer between spec sources and invariants. Spec sources
point at prose or external standards; invariant behavior rows carry the
machine-readable decisions that tools can derive tests from.

#### Decision Table v1

`contract_kind = "decision_table"` supports single-input, data-shaped contracts
only. It intentionally does not support expression grammars, multi-input
interaction tables, protocol sequences, runtime fault scenarios, or connector
resource probes. Those use other `contract_kind` values and remain blocked until
their harness exists.

```toml
schema_version = 1
id = "VM_SEAM_SUBRANGE_001"
title = "Subrange writes reject out-of-range runtime values"
area = "bytecode_vm"
risk = "wrong_result"
status = "planned"
owner = "trust-runtime-core"
claim = "A runtime write to INT(0..100) accepts values in range and rejects values outside range without partial state change."
proof_level = "S0"
contract_kind = "decision_table"
tests = ["TEST_VM_SUBRANGE_WRITE_CASES"]
gates = ["veryquick", "pr"]
evidence_refs = []
missing = []
last_reviewed = "2026-07-08"

[input]
name = "value"
type = "INT"
declared = "INT(0..100)"

[spec]
status = "specified"
source_refs = ["SPEC_RUNTIME_SEMANTICS_001"]

[oracle]
kind = "trust_contract"
ref = "SPEC_RUNTIME_SEMANTICS_001#subrange-runtime-write"

[[behavior]]
partition = { min = 0, max = 100 }
outcome = "accept_value"
delta = { target = "set_to_input", siblings = "unchanged", retain = "unchanged", process_image = "unchanged", diagnostics = "none", status = "unchanged" }
oracle_ref = "SPEC_RUNTIME_SEMANTICS_001#runtime-write"

[[behavior]]
partition = { below = 0 }
outcome = "reject"
error_code = "SubrangeOutOfBounds"
delta = { target = "unchanged", siblings = "unchanged", retain = "unchanged", process_image = "unchanged", diagnostics = "stable_error", status = "fault_visible" }
no_partial_apply = true
fault_surface = "diagnostic"
oracle_ref = "SPEC_RUNTIME_SEMANTICS_001#runtime-write"

[[behavior]]
partition = { above = 100 }
outcome = "reject"
error_code = "SubrangeOutOfBounds"
delta = { target = "unchanged", siblings = "unchanged", retain = "unchanged", process_image = "unchanged", diagnostics = "stable_error", status = "fault_visible" }
no_partial_apply = true
fault_surface = "diagnostic"
oracle_ref = "SPEC_RUNTIME_SEMANTICS_001#runtime-write"
```

Allowed decision-table outcomes:

- `accept_value`
- `reject`
- `trap`
- `fault_visible`
- `not_safe_reported`
- `status_degraded`

Decision-table rules:

- Partitions use explicit keys such as `min`, `max`, `below`, `above`, `equals`,
  `wrong_type`, and `malformed`. No `when = "..."` expression language is
  allowed in v1.
- `equals` partitions are opaque symbolic labels, not behavior expressions.
  They must use stable `UPPER_CASE_LABEL` values. Case tooling may copy or map
  the label, but must not infer product behavior from the label text.
- `equals` partitions must also name `case_family`, and generated cases must
  carry the label as `input.scenario`, not as the typed input value. This keeps
  scenario labels from becoming hidden product behavior or fake happy paths.
- A behavior partition must use exactly one handled shape, except `{ min, max }`
  which is the v1 closed-range shape. Mixed partitions such as
  `{ above = 5, equals = "X" }` are invalid because they silently under-generate
  cases.
- Rows that name `spec_gap_ref` are blocked behavior placeholders, not oracles:
  they may name the partition and the gap, but must not carry `outcome`,
  `delta`, `error_code`, `no_partial_apply`, or `fault_surface`.
- Rows that name `oracle_ref` must resolve to an active non-public-claim spec
  source. Fragment suffixes such as `SPEC_ID#section` are allowed, but the base
  spec ID must exist in `verification/spec-sources.toml`.
- Rows that name `error_code` require an active stable-error-code-model spec
  source before they validate. Until that source exists, error-code behavior
  rows must stay blocked by a spec gap.
- For `status = "spec_gap"`, `oracle.ref` must equal one of the record's
  `spec_gap_refs`; `oracle.kind` documents the expected future oracle category.
  For every other status, `oracle.ref` must not name a spec gap.
- Behavior rows must tile the declared input domain for finite/scalar domains or
  mark the missing partitions with `spec_gap_ref`.
- Every behavior row must use the structured `delta` field. Allowed keys in v1
  are `target`, `siblings`, `retain`, `process_image`, `diagnostics`, and
  `status`. Areas that cannot observe a key must set it to `not_applicable` and
  explain why in the invariant notes.
- Allowed `delta.target` values: `set_to_input`, `set_to_expected`,
  `unchanged`, `not_applicable`.
- Allowed `delta.siblings`, `delta.retain`, and `delta.process_image` values:
  `unchanged`, `expected_delta`, `not_applicable`.
- When any delta key uses `expected_delta`, the behavior row must define the
  actual expected change in oracle-cited notes or another structured field in
  the same record. `expected_delta` is not self-describing.
- Allowed `delta.diagnostics` values: `none`, `stable_error`,
  `expected_diagnostic`, `not_applicable`.
- Allowed `delta.status` values: `unchanged`, `fault_visible`,
  `status_degraded`, `not_safe_reported`, `not_applicable`.
- Accept outcomes require exact delta semantics: target value, sibling state,
  retain state, output/process-image state, and diagnostics/status expectations.
- Reject outcomes require zero-delta semantics unless the oracle explicitly
  permits a partial change: unchanged target and sibling state, no partial
  apply, visible diagnostic/status/fault surface, and stable error code.
- A row with `outcome = "blocked"` is not allowed. Use a coverage cell or case
  with `state = "blocked"` plus `spec_gap_ref`; do not pretend blocked behavior
  is an oracle.

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

User stories and public workflows are also specification sources when they
define expected behavior. They may use `authority = "normative_product"` when
they are truST-owned workflow contracts, or `authority = "public_claim"` when
they are claims from shipped/public docs that need proof or narrowing. Workflow
spec-source records must include `actor`, `entry_point`, `preconditions`,
`success_state`, `failure_status_behavior`, and `acceptance_evidence` fields
unless they are tracked as a spec gap.

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

### Required Specification Matrix Record

Record shape, canonical area values, `blocks` semantics, and validator rules
for `verification/spec-matrix.toml` live in `spec-matrix-model.md`.

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
case_file = "verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml"
case_file_digest = "sha256:<digest>"
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

`case_file` and `case_file_digest` are optional. If either is present, both are
required. `case_file_digest` is the SHA-256 digest of the committed case file and
the validator recomputes it. Editing cases to make a test pass must be visible
as a metadata diff.

`invariants` may be empty only while `status` is `planned` or `gap_open`.
Records with `status` in `mapped`, `test_written`, `implemented`, or
`validated` must reference known invariant IDs and an `oracle_ref` or
`spec_gap_ref`.

Stale-test validation must verify both `path` and `name` against scanner output.
A renamed or deleted test function inside an existing file fails validation.

### Case File

Case files are committed data under `verification/cases/<area>/*.toml`. They are
not a new source of product truth; they are derived from invariant behavior rows
or from deterministic mechanical transforms of a committed seed artifact.

```toml
schema_version = 1
id = "CASES_VM_SUBRANGE_WRITE_001"
title = "Subrange write boundary cases"
area = "bytecode_vm"
owner = "trust-runtime-core"
status = "planned"
invariant = "VM_SEAM_SUBRANGE_001"
generator = "gen_cases.py v1"
generator_digest = "sha256:<generator-source-digest>"
source_digest = "sha256:<invariant-record-digest>"
last_reviewed = "2026-07-08"

[[case]]
id = "SUBRANGE_WRITE_MIN"
family = "boundary_low"
input = { value = 0 }
expect = { outcome = "accept_value", delta = { target = "set_to_input", siblings = "unchanged", retain = "unchanged", process_image = "unchanged", diagnostics = "none", status = "unchanged" }, oracle_ref = "SPEC_RUNTIME_SEMANTICS_001#runtime-write" }

[[case]]
id = "SUBRANGE_WRITE_UPPER_PLUS_1"
family = "above_max"
input = { value = 101 }
expect = { outcome = "reject", error_code = "SubrangeOutOfBounds", delta = { target = "unchanged", siblings = "unchanged", retain = "unchanged", process_image = "unchanged", diagnostics = "stable_error", status = "fault_visible" }, no_partial_apply = true, fault_surface = "diagnostic", oracle_ref = "SPEC_RUNTIME_SEMANTICS_001#runtime-write" }
```

Required fields:

- `schema_version`
- `id`
- `title`
- `area`
- `owner`
- `status`
- `invariant`
- `generator`
- `generator_digest`
- `source_digest`
- `last_reviewed`
- each `case.id`
- each `case.family`
- each `case.input`
- each `case.expect` or `case.state = "blocked"` with `spec_gap_ref`

Case rules:

- `family` must be one of the coverage dimensions in `test-taxonomy.md`.
- Generated expected behavior may only be copied from a behavior row. Tools and
  agents must never synthesize an expected value from code or a guess.
- A case with `expect` must match an oracle-backed behavior row on the named
  invariant. Its `oracle_ref` must resolve through the same oracle rules as
  behavior rows and catalog rows.
- A blocked case must name a `spec_gap_ref`; it cannot be counted as covered.
- Wrong-type or malformed inputs must use `input.shape_descriptor`, not the
  invariant's typed input field. This prevents a runner from treating labels
  such as `WSTRING` or `BOOL_TO_REAL_SLOT` as literal test values.
- Transform-backed case files must name the committed seed artifact and
  generator. `gen_cases.py --check` must reproduce the committed table exactly.

### Case Artifact

Tests that consume a case file must emit a generated artifact under
`target/gate-artifacts/cases/<TEST_ID>.json` through the shared
`verification-cases` helper. The artifact lets `prove.py` verify that every
committed case was executed and observed.

The helper requires the expected case-file digest when constructing its run
configuration. It must fail before execution and before artifact write if the
digest does not match. The default artifact directory is the workspace-root
`target/gate-artifacts/cases`; explicit overrides should be absolute paths.

Required fields:

- `schema_version`
- `test_id`
- `case_file`
- `case_file_digest`
- `helper_version`
- `trust_verify_test_id`, `trust_verify_run_id`,
  `trust_verify_case_file_digest`, and `trust_verify_artifact_dir` when
  `prove.py` exports the matching `TRUST_VERIFY_*` environment variables
- `cases`
- per-case `id`, `result`, observed error/status, and state-delta verdict

The artifact is not durable evidence by itself. The test helper writes only
harness-observed facts. `prove.py` owns command, commit, platform, timing,
failure-kind classification, and evidence creation. If `prove.py` exports
`TRUST_VERIFY_*` environment variables for the helper to stamp into the case
artifact, `prove.py` must verify the stamped values before accepting the
artifact. The helper fails before case execution when only some stamp variables
are present or when stamped `TEST_ID`, case-file digest, or artifact directory
does not match the current run configuration.

For `prove.py lock`, the raw case-artifact digest is provenance only because
freshness stamps intentionally change between runs. The stable behavior-lock
comparison uses a deterministic `case_result_digest` derived from command exit
status plus per-case results, paired to the baseline through
`paired_lock_baseline`.

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
approved_proof_producers = []
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

Optional fields:

- `approved_proof_producers`: producer IDs such as
  `approved-gate:runtime-comms-conformance` that are allowed to close high-risk
  red/green proof when they emit evidence fields equivalent to `prove.py`.

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

Evidence record details, traceability report rules, and verification-tooling
self-test fixtures live in `metadata-evidence-traceability.md`.
