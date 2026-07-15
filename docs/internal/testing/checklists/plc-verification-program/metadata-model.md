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
  gate-inventory.toml
  cases/
    bytecode_vm/
  schemas/
    invariant.schema.json
    suite.schema.json
    gate-inventory.schema.json
    catalog.schema.json
    ignored-test.schema.json
    invariant-seed-manifest.schema.json
    invariant-seed-audit-report.schema.json
    risk-register.schema.json
    evidence.schema.json
    spec-source.schema.json
    spec-gap.schema.json
    spec-matrix.schema.json
    matrix.schema.json
    case-file.schema.json
    case-artifact.schema.json
    bytecode-validator-mutation-report.schema.json
    ignored-test-inventory-report.schema.json
    spec-completeness-report.schema.json
    conformance-alignment-report.schema.json
  matrix.toml
  spec-matrix.toml
  test-catalog.toml
  ignored-tests.toml
  invariant-seeds.toml
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

### Generated Existing-Test Discovery Report

`verification/schemas/generated-test-catalog.schema.json` is the dedicated
closed schema for `test-catalog-scanner v1`. It is intentionally separate from
`catalog.schema.json`, which owns reviewed test intent.

The generated report contains:

- full generator provenance, input paths, and a digest over every scanned
  input;
- a complete/incomplete scan status and severity-bearing diagnostics;
- deterministic inferred facts with semantic discovery ID, native identity,
  source kind, literal name, path and line, nullable package, conservative
  command hint plus authority, ignore state and reason, and lexical reference
  candidates. Structured Text declarations retain their
  `TEST_PROGRAM`/`TEST_FUNCTION_BLOCK` kind in the native identity and use a
  conservative `trust-dev test --project ... --filter ...` hint;
- recomputed record/file/ignore/diagnostic counts by source kind;
- an explicit list of excluded hand-owned fields.

At-rest validation applies the committed closed JSON schema, then recomputes
discovery IDs and summary counts, requires canonical ordering and
workspace-relative paths, checks per-source package/command/authority claims,
checks input contents against the report digest, and binds the Markdown summary
to the JSON SHA-256. A scan
with missing roots, unreadable inputs, malformed manifests, duplicate native
IDs, or invalid target paths is incomplete and the CLI exits nonzero. Dynamic
or unsupported declarations remain visible warning debt and do not enable
enforcement in this slice.

Discovery facts must not contain `area`, `owner`, `status`, `test_class`,
`invariants`, expected results, suite tiers, hardware/network requirements,
oracle refs, expected failure modes, or evidence destinations. Those fields
remain review-owned by `verification/test-catalog.toml` beginning at
`VERIF-P2-004`.

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

- `active`: current, but usable as an oracle only when `oracle_eligible = true`
  and the authority rules also allow it.
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

Each metadata record must use the version accepted by its owning closed schema
and validator. A registry migration updates all committed records together;
older versions are then rejected unless an explicit compatibility path remains.

Schema changes must be handled explicitly:

- additive optional fields can stay on the current schema version,
- new required fields require a schema version bump,
- renamed enum values require a migration note and validator compatibility path,
- removed fields require a migration note and stale-field diagnostic,
- generated reports must include the schema version they read.

Test-catalog schema v2 is the first migration under this rule. It adds the
required `subject_kind`, `expected_failure_mode`, `evidence_destination`, and
`last_reviewed` fields. All committed test records migrate together; catalog
v1 records are rejected after the migration. The mechanical existing-test
report remains schema v1 but uses generator v2 because its hand-owned-field
exclusion list changed.

Spec-source schema v2 adds the required `locator_kind`, `version`,
`last_reviewed`, and `conflicts_with` fields and separates tracked files from
non-redistributable external references. All committed spec-source records
migrate together; spec-source v1 records are rejected after the migration.

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
  For every other status, `oracle.ref` must resolve to an active non-public
  normative/reviewed specification source, not merely avoid naming a gap.
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
schema_version = 2
id = "SPEC_BYTECODE_FORMAT_001"
title = "truST bytecode format"
area = "bytecode_vm"
owner = "trust-runtime"
status = "mapped"
authority = "normative_product"
source_status = "active"
oracle_eligible = true
visibility = "public"
locator_kind = "tracked_file"
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
- `oracle_eligible`
- `visibility`
- `locator_kind`
- `version`
- `last_reviewed`
- `covers`
- `known_limitations`
- `conflicts_with`

Specification-source schema v2 is closed: unknown fields and v1 records fail
validation. `locator_kind` selects exactly one locator contract:

- `tracked_file` requires `path`. The path must be canonical,
  workspace-relative, a tracked regular file, and contain no symlink component.
  External-reference fields are forbidden.
- `external_reference` forbids `path` and requires `external_ref`,
  `expected_local_path`, `retrieval_expectation`, `publication_date`,
  `redistributable`, and `absence_blocks_proof`. It requires
  `authority = "normative_external"`, `oracle_eligible = false`,
  `redistributable = false`, and `absence_blocks_proof = true`. The expected
  local path must be canonical, ignored, untracked, and under
  `docs/internal/standards/`.

`SPEC_IEC_61131_3_ED3_EXTERNAL_001` records IEC 61131-3 Edition 3 through the
`external_reference` branch. Its authorized local copy is expected at
`docs/internal/standards/iec61131-3.txt`, but that path is an availability
expectation only. The record remains non-oracle while the copy is absent, and
the local standard bytes must not enter committed report provenance or proof
input closures without a separate reviewed content-binding mechanism.

Public claims are committed spec-source records with `authority =
"public_claim"`. They must include `claim_text` and `surface_ref` fields, and
they must set `oracle_eligible = false`; they cannot serve as test, case,
behavior, or invariant oracles. Review artifacts that record finding provenance
without defining behavior must also set `oracle_eligible = false`. Such sources
may support risk provenance, but cannot close a spec gap or justify a test
deferral.

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
- `affected_tests`
- `candidate_spec_sources`
- `resolution_status`
- `closeout_evidence`
- `last_reviewed`

Allowed `resolution_status` values:

- `open`
- `decision_recorded`
- `spec_updated`
- `test_mapped`
- `closed`
- `rejected`

A gap may use `resolution_status = "closed"` only when an active tracked
normative-product/reviewed-decision/reviewed-deviation source owns the resolved
contract, mapped tests exist or an active reviewed deferral is named, and
durable closeout evidence back-links the gap and exact test disposition. A
closed gap must no longer be referenced from required specs, invariant
top-level refs/coverage/behavior, tests, or risks. Safety-critical invariants
cannot be `validated` while an open gap links them in either direction.

### Invariant Seed Manifest

`verification/invariant-seeds.toml` is a closed wrapper record:

```toml
schema_version = 1

[[seeds]]
seed_id = "VM_SEAM_TYPE_001"
canonical_invariant_id = "VM_SEAM_DECLARED_TYPE_001"
board_row = "VERIF-P4-002"
origin = "preexisting"
```

Every one of the 44 written seed IDs must appear exactly once and in reviewed
source order. `canonical_invariant_id` must resolve to the exact area/path; only
the reviewed `VM_SEAM_TYPE_001`/`VM_SEAM_TYPE_002` merge is many-to-one. The
five P4-000 seeds additionally name a unique `p4_000_risk_id`. New Phase 4
records must have empty tests/evidence, and every mapped invariant remains S0
with `status` in `gap_open` or `spec_gap`. The live audit binds active tracked
oracle/review sources and never treats a public claim as a behavior oracle.

### Required Specification Matrix Record

Record shape, canonical area values, `blocks` semantics, and validator rules
for `verification/spec-matrix.toml` live in `spec-matrix-model.md`.

### Test Catalog Record

```toml
schema_version = 2
id = "TEST_BYTECODE_CONTAINER_INVALID_MAGIC"
subject_kind = "generated_test"
discovery_id = "DISC_88F921D24D3708CEF3E1"
discovery_source_kind = "rust_integration_test"
test_class = "negative_malformed_input"
area = "bytecode_vm"
path = "crates/trust-runtime/tests/bytecode_container.rs"
name = "header_validation"
command = "cargo test -p trust-runtime --test bytecode_container header_validation -- --exact"
owner = "trust-runtime"
status = "mapped"
invariants = ["VM_SEAM_VALID_001"]
oracle_ref = "SPEC_BYTECODE_FORMAT_001"
spec_gap_ref = "SPEC_GAP_VM_ERROR_MODEL_001"
expected_result = "Decoding rejects non-STBC magic before execution; exact stable error-code semantics remain open."
expected_failure_mode = "A container with non-STBC magic is accepted or reaches execution."
evidence_destination = "target/gate-artifacts/cases/TEST_BYTECODE_CONTAINER_INVALID_MAGIC.json"
suite_tiers = []
requires_hardware = false
requires_network = false
duration_class = "fast"
last_reviewed = "2026-07-10"
```

Required fields:

- `schema_version`
- `id`
- `subject_kind`
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
- `expected_failure_mode`
- `evidence_destination`
- `last_reviewed`

`subject_kind` is a closed discriminator:

- `generated_test` requires `discovery_id`, `discovery_source_kind`, and
  `name`. The live staleness checker resolves exactly one current scanner fact
  and requires its source kind, path, and name to match. Line-only movement is
  not stale.
- `case_table_artifact` is limited to `metadata_validation` rows whose `path`
  equals a `case_file` under `verification/cases/**`; scanner identity fields
  are forbidden.
- `mutation_shard_runner` is limited to the reviewed bytecode-validator
  mutation runner and its closed shard contract; scanner identity fields are
  forbidden.

The two artifact subject kinds exist because the Phase 1 rows are verification
tools/data outside the P2-001 native-test scan surface. They are not a generic
escape hatch for native tests.

### Existing-test refactor assessment and proposals

The Phase 2A assessment is report-only. It recomputes scanner facts, catalog
bindings, suite metadata, VS Code registration, committed case inputs, and the
architecture program's inclusive existing-file review threshold. Mechanical
size, exact content, normalized content, and same-table case-input shape are
candidate signals. They never authorize a move, split, rename, fixture merge,
or behavior change. Free-form body similarity, fixture helper functions, and
helper-only files are explicitly `not_assessed` in v1.

Catalog schema v2 has no authorized per-invariant `coverage_dimensions` field.
Therefore every valid catalog row with multiple invariant IDs is reported as a
missing-dimension candidate; source text or an unknown catalog field cannot
upgrade it. Proposal `coverage_dimensions` are narrower: v1 accepts only
`malformed_input_class:<id>` values exactly backed by that catalog row's
reviewed `malformed_input_class_ids`.

`verification/test-refactor-proposals.toml` uses closed schema v1. A proposal
records one catalog test, its source and target scanner identities, source
paths, the reviewed assessment decision input, exact before/after commands,
invariants, explicit coverage dimensions, fixture ownership, stale-path
updates, `expected_behavior_delta = "none"`, lifecycle, rationale, and a
SOLID/KISS/DRY review. `no_refactor_needed` is a terminal reviewed decision
only when the live assessment reports no signal for its source paths. Change
dispositions remain unsupported by the mechanical v1 assessment; a signal is
not operation-specific authorization. Such a change plan may remain
`status = "proposed"` (or be rejected), but cannot advance. The single-target model rejects
`disposition = "split"` until a reviewed multi-target contract exists.

`verification/test-catalog-redirects.toml` is append-only identity history for
validated moves and renames. Each edge references exactly one completed,
validated proposal and its paired behavior-lock evidence. V1 permits one edge
per catalog test. A second historical edge is blocked until `prove.py` can emit
proposal-scoped lock IDs; it is not simulated with invented evidence IDs.
Forks, merges, cycles, live old identities, missing endpoints, and terminal
catalog/scanner mismatch fail.

Move/rename completion requires production-valid `lock_baseline` and
`lock_compare` evidence with the same current catalog command, one catalog
test, ordered distinct source revisions and run IDs, exit status zero, the
proposal's exact invariant IDs, the committed case-ID set, identical passing
per-case summaries and result digest, and the catalog's current
`case_file_digest`. This deliberately blocks
completion for command-changing refactors and scanner-only catalog rows that
lack `case_file` and `case_file_digest`; Phase 2A does not invent a
command-only proof model.

`case_file` and `case_file_digest` are optional. If either is present, both are
required. `case_file_digest` is the SHA-256 digest of the committed case file and
the validator recomputes it. Editing cases to make a test pass must be visible
as a metadata diff.

Pilot mutation catalog rows use `test_class = "mutation"` and add a closed
bytecode-validator-only contract: `mutation_shard_id`, `mutation_runner`,
`mutation_tool`, `mutation_tool_version`, `mutation_case_semantics`,
`mutation_out_of_scope_case_ids`, `mutation_out_of_scope_reason`, and one or
more `[[tests.mutations]]` tables.
Each mutation names a validator source file/function, cargo-mutants selector,
build/test commands, committed `related_case_ids`, and a survivor action. The
validator requires the measured and out-of-scope IDs to form a disjoint,
exhaustive partition of the cataloged case file. Whether those cases are still
blocked or have become runnable, the only allowed semantics are
`association_only_case_ids_not_executed_by_mutation_runner`; mutation results
are adequacy evidence, not expected-behavior proof or a case-execution claim.

The generic Phase 10 control plane is separate from that bytecode-only catalog
exception. `verification/mutation-program.toml` defines the six reviewed shard
IDs in checklist order, at most two exact single-file selectors per shard,
focused build/test commands, invariant ownership, and explicit
`association_ids`. Each association must partition the shard's committed case
or current scanner identities exactly; it is not a killed-by or executed-test
claim. The static manifest and closed schema are validated through the primary
metadata validator.

The bytecode shard may bind the validated legacy report. Other shards remain
`planned` until separately executed and cannot contain outcomes. A measured
survivor requires one matching `survivor_resolutions` record with an owner,
resolved allowed action (`add_test`, `unreachable_defensive_rationale`, or
`dead_code_removal`), rationale, and durable tracked reference. A shard whose
contract requires the delivered build cannot become measured without a bound
artifact SHA-256 and direct-execution confirmation.

`invariants` may be empty only while `status` is `planned` or `gap_open`.
Records with `status` in `mapped`, `test_written`, `implemented`, or
`validated` must reference known invariant IDs and an `oracle_ref` or
`spec_gap_ref`.

Stale-test validation must verify both `path` and `name` against scanner output.
A renamed or deleted test function inside an existing file fails validation.
`python3 scripts/check_test_catalog_staleness.py` performs the live scan and
join; it does not trust a previously generated target artifact.

The catalog schema's `subject_kind` and `discovery_source_kind` enums are
drift-pinned to the Python validator vocabularies, alongside `test_class`.
Changing only the schema or only the validator must fail metadata validation.

### Test-Class Completeness Report

`VERIF-P2-007` produces a generated JSON report under
`target/gate-artifacts/verification/test-class-completeness.json` and a concise
Markdown summary. Its closed schema is
`verification/schemas/test-class-completeness-report.schema.json`.

The report owns two independent joins:

- scanner classification: an inferred fact is classified only when exactly one
  `subject_kind = "generated_test"` row names its current `discovery_id`;
- mapped-area class completeness: each `required_test_classes` value in
  `verification/matrix.toml` is complete only when the area has at least one
  catalog row whose status is `mapped`, `test_written`, `implemented`, or
  `validated` and whose scanner fact, when present, is neither `ignored` nor
  `conditional`.

`case_table_artifact` and `mutation_shard_runner` rows never classify scanner
facts. `planned`, `gap_open`, and other non-runnable rows remain visible beside
their class but do not satisfy it. The generator must not infer hand-owned
fields from paths, names, command hints, or lexical reference candidates.
`report_status = "complete"` means generation, schema, live joins, input digest,
and Markdown binding succeeded; it does not mean the catalog is complete.
Unmapped facts and missing classes are debt and do not change the report command
to a failing exit. Corrupt metadata, stale identities, scanner errors, schema
drift, report tampering, and digest mismatches do fail. At-rest validation also
pins the canonical command and timestamp shape, requires the source commit to
exist, and verifies test sources, matrix/catalog data, report tooling, scanner
join modules, validator wiring, and relevant schemas against that clean commit.
Historical
platform text cannot be rederived on another host and remains an explicit
evidence-review field.

### Coverage-Matrix Gap Report

`VERIF-P2-008` writes
`target/gate-artifacts/verification/coverage-matrix-gaps.json`; its closed
schema is `verification/schemas/coverage-matrix-gap-report.schema.json`.
Completeness is assessed only for invariants in planning-matrix areas whose
status is `mapped`. The denominator is each mapped invariant crossed with that
area's `required_case_families`.

A required slot is either `assigned` to one committed coverage cell or
`missing_cell`. A missing cell has no coverage state; the report must not
invent `gap_open`, `spec_gap`, or `not_applicable`. Recorded cells preserve one
of `covered`, `covered_by_fuzz`, `not_applicable`, `blocked`, `spec_gap`,
`gap_open`, or `deferred` exactly as authored. Catalog-bound case files are
reported as planning observations only and never upgrade a cell.

### Malformed-Input Taxonomy And Coverage

`verification/malformed-input-taxonomy.toml` is the reviewed bytecode/VM pilot
taxonomy. It is intentionally limited to the inventoried
`bytecode_container_instruction_stream` surface and is mirrored exactly by
`verification/malformed-input-taxonomy.md`. Compound prose categories are
atomized into stable class IDs. `required` classes name an active same-area
oracle; `spec_gap` classes name an open/actionable same-area gap; blocked,
deferred, and not-applicable dispositions carry their required rationale or
reviewed reference.

Only `generated_test` rows with test class `negative_malformed_input` or `fuzz`
may set `malformed_input_class_ids`. Negative rows require a nonempty reviewed
binding; artifact rows and unrelated classes forbid it. The report never
derives a class from a test name, path, command, lexical reference, case ID, or
mutation association. For a `required` class, an effectively runnable native
mapping derives `covered`, a fuzz-only mapping derives `covered_by_fuzz`, and
no mapping derives `gap_open`. An authored `spec_gap`, `blocked`, `deferred`,
or `not_applicable` disposition is never promoted by test presence.

`VERIF-P2-009` writes
`target/gate-artifacts/verification/malformed-input-coverage.json`; its schema
is `verification/schemas/malformed-input-coverage-report.schema.json`.

### Fuzz Program Inventory And Surface-Gap Report

`verification/fuzz-program.toml` is the Phase 9 hand-reviewed association
plane. Its closed schema is `verification/schemas/fuzz-program.schema.json`.
It inventories the live fuzz and fuzz-like executable population without
turning source names, paths, commands, or successful execution into invariant
coverage or proof.

The required surface IDs are ordered and exhaustive for this first report:

- `st_lexer_parser`
- `hir_lowering_input`
- `plcopen_xml`
- `bytecode_container_instructions`
- `protocol_payloads`
- `config_files`
- `lsp_incremental_edits`
- `hmi_schema_payloads`

The initial live inventory has eleven executable facts: five parsed cargo-fuzz
targets and six bounded Rust fuzz/property smokes. The cargo-fuzz targets are
`syntax_parse`, `hir_semantic`, `ams_frame`, `boundary_noop`, and
`command_dispatch`. The Rust population is
`vm_malformed_bytecode_fuzz_smoke_budget`,
`mesh_payload_encode_decode_fuzz_smoke_budget`,
`t0_shm_header_fuzz_rejects_corruption_budget`,
`runtime_cloud_api_payload_fuzz_smoke_budget`,
`wan_allowlist_parser_fuzz_smoke_budget`, and
`test_initializer_recovery_property_smoke_for_generated_positional_shapes`.
Cargo targets bind to their parsed manifest identity. Rust smokes bind to their
exact live scanner discovery identity. A name or lexical candidate alone cannot
create a target record or surface association.

Every target has exactly one `primary_tier` from `pr_smoke`, `nightly`, or
`manual_extended`. `additional_tiers` is sorted, duplicate-free, uses the same
vocabulary, and cannot repeat the primary tier. It records another real
execution profile, such as the root parser/HIR targets' extended nightly mode;
it does not imply suite inheritance. Tier records preserve their reviewed
enforcement posture, so a planned nightly route and a live required job do not
become equivalent.

For a live execution basis, command text is not sufficient. The validator binds
the raw reviewed shell-script and workflow bytes by content digest, retains the
scripts' tracked executable modes, requires unique reviewed workflow trigger
and job blocks, and checks effective Cargo default-workspace membership for the
parser property smoke. Comments, echoes, line-ending normalization, dead
control flow, uncalled functions, or another workflow block cannot preserve an
execution claim after its reviewed source changes.
Every bounded Rust smoke must resolve with `ignore_state = "not_ignored"` in the
fresh scanner join. Ignored or conditional facts are rejected rather than
remaining wired or planned through a filtered command that can exit without
executing the test.
The Rust scanner remains a lexical census and does not evaluate arbitrary
`cfg` expressions or prove parent-module reachability. Consequently `wired`
means the reviewed command path is required and source-bound; it does not mean
this report observed the test execute or pass.

The generated surface state is one of:

- `cargo_fuzz_target`: at least one reviewed direct cargo-fuzz association;
- `smoke_only`: direct bounded smoke exists, but no direct cargo-fuzz target;
- `partial_only`: every reviewed association exercises only part of the named
  surface or stops below its required boundary;
- `unmapped`: no reviewed target association exists.

Only the exact association records derive these states. They do not derive an
oracle, expected result, catalog mapping, invariant coverage state such as
`covered_by_fuzz`, passing campaign, or product claim. `smoke_only`,
`partial_only`, and `unmapped` remain surface-gap rows in the first report.

Corpus and artifact storage is deliberately separate from deterministic
regression storage. Generated seed corpora, developer corpora, coverage output,
and crash artifacts stay under the owning fuzz workspace's ignored
`corpus/`, `coverage/`, `artifacts/`, or `target/` paths. Named CI uploads remain
run artifacts rather than committed regression proof unless an evidence record
binds the run and retention. The Phase 9 audit validates path and ignore
posture, but does not enumerate, hash, assess, or claim completeness for corpus
contents. A minimized crash must ultimately be promoted to a tracked native
test or fixture with a deterministic command and reviewed identity; the ignored
crash artifact is not that regression.

`VERIF-P9-006` writes
`target/gate-artifacts/verification/fuzz-program-audit.json` plus the generated
`target/gate-artifacts/verification/fuzz-program-audit.md`; its closed schema is
`verification/schemas/fuzz-program-audit-report.schema.json`. The report is
report-only and records that no fuzz campaign ran. `VERIF-P9-005` remains open
because no enforced crash ledger currently joins every minimized artifact to a
tracked deterministic regression. A policy statement or an empty artifact
directory cannot close that universal handoff rule.

### Unmapped-Test Debt Report

`VERIF-P2-010` writes
`target/gate-artifacts/verification/unmapped-test-debt.json`; its closed schema
is `verification/schemas/unmapped-test-debt-report.schema.json`. The debt set
is the exact subtraction of current scanner facts by reviewed
`generated_test.discovery_id`. Case-table and mutation-runner artifact rows do
not classify scanner facts. Each debt row contains only inferred identity:
discovery ID, source kind, path, name, and ignore state.

All three Phase 2 reports are report-only. Debt returns success; corrupt
metadata, stale joins, unsafe or symlinked input paths, dirty source commits,
noncanonical JSON, or Markdown that differs from the JSON-derived rendering
fail. Source provenance binds report code and semantic inputs without binding
the mutable evidence index or the report's own follow-up evidence row. Full
metadata validation remains a separate current-tree health prerequisite.

### Conformance Alignment Report

`VERIF-P7-001` and `VERIF-P7-003` through `VERIF-P7-006` write
`target/gate-artifacts/verification/conformance-alignment.json`; its closed
schema is `verification/schemas/conformance-alignment-report.schema.json`.
The durable Markdown is a JSON-bound audit record, not a committed conformance
run result.

The case denominator is every live `conformance/cases/**/manifest.toml` fact
from the production test scanner. Each case row binds its discovery ID,
manifest and optional `program.st`, matching expected JSON artifact, category,
kind, and digests. Catalog and invariant associations arise only from an exact
`generated_test.discovery_id` join. Names, paths, descriptions, program text,
expected output, and lexical reference candidates cannot create mappings.

Zero explicit invariant coverage is a valid report result. It keeps
`VERIF-P7-002` open and must not be repaired by inventing a broad invariant,
semantic oracle, or area assignment. The ten v2 categories have explicit
`coverage_gap` rows that record case/artifact presence, missing invariant
mapping, and `semantic_oracle_state = "not_assessed"`. These are audit debt
rows, not registered specification gaps and not evidence that product behavior
is unspecified.

The communication posture is structural: the single
`connector_status_trace` case must have eight scripted in-process steps, no
program source or network-shaped fields, and the reviewed pure projection call
path. The audit performs no socket or hardware execution. Publication posture
requires generated JSON/Markdown under the CI `gate-artifacts/` upload, only
`.gitkeep` tracked beneath `conformance/reports/`, and no generated result
summary embedded in the public page. Expected artifacts remain committed test
inputs, not passing proof.

Generation and at-rest validation follow the common clean-full-SHA, complete
input-closure, canonical JSON, exact Markdown, source-tree membership, live
recomputation, full metadata validation, and semantic-tamper rules. The report
creates no proof, closes no gap, changes no enforcement, and leaves
`VERIF-STOP-014` open.

### Runtime Anomaly Taxonomy And Audit

`verification/runtime-anomaly-taxonomy.toml` is a closed Phase 8 control-plane
record. Its dedicated schema is
`verification/schemas/runtime-anomaly-taxonomy.schema.json`. The primary
metadata validator delegates to the per-taxonomy contract; it does not run the
live Rust source scan.

Root fields are:

- `schema_version`, `id`, `title`, `area`, and `last_reviewed`;
- the constant honesty fields `mapping_basis`, `proof_posture`,
  `fault_interface_status`, and `production_hook_policy`;
- `spec_gap_reviews`, `classes`, and `mappings`.

Each class carries `id`, `title`, a stimulus description, `primary_suite`,
`conditional_suites`, `injection_boundary`, and rationale. Class IDs and order
are exact. Each mapping carries a unique mapping ID and discovery ID, exact
scanner source kind/path/name binding, association kind, injection mechanism,
assertion summary, limitations, and review date. Mapping prose is rejected if
it claims proof, completed coverage, or validation.

The live report is
`target/gate-artifacts/verification/runtime-anomaly-audit.json`, with closed
schema `verification/schemas/runtime-anomaly-audit-report.schema.json`. It
rescans all Rust test facts through the production scanner, resolves every
mapping exactly once, and joins ignored facts to the Phase 3 registry. Only
`direct` plus `not_ignored` derives `mapped_runnable`; all other mapped classes
derive `mapped_non_runnable_or_partial`, and classes with no association derive
`unmapped`. The non-runnable and unmapped partition is reproduced as explicit
gap rows.

The report binds the taxonomy, both schemas, scanner inputs, mapped sources,
ignored registry, specification review inputs, suite records, validator code,
and exact Markdown. It does not claim that every semantically relevant test in
the scanner denominator was reviewed. Debt exits successfully; malformed or
stale identities, registry mismatch, schema drift, provenance drift, or report
tampering fail.

The first registry is intentionally non-exhaustive. It reports useful exact
associations and class-level gaps, but `VERIF-P8-002` stays open until a
reviewed runtime-safety denominator records a disposition for every fact.

### Case File

Case files are committed data under `verification/cases/<area>/*.toml`. They are
not a new source of product truth. `generated_decision_table_v1` files are
derived from invariant behavior rows or deterministic mechanical transforms of
a committed seed artifact. `hand_authored_state_machine_v1` files copy reviewed
ordered stimuli and expected states from oracle-backed behavior without claiming
`gen_cases.py` provenance.

```toml
schema_version = 1
id = "CASES_VM_SUBRANGE_WRITE_001"
title = "Subrange write boundary cases"
area = "bytecode_vm"
owner = "trust-runtime-core"
status = "planned"
invariant = "VM_SEAM_SUBRANGE_001"
case_provenance_kind = "generated_decision_table_v1"
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
- `case_provenance_kind` when the legacy generated default is not used
- `generator` and `generator_digest` for `generated_decision_table_v1`, or
  `trace_definition_digest` for `hand_authored_state_machine_v1`
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
- Hand-authored state-machine files require a `state_machine` invariant. Each
  runnable case carries a non-empty ordered `trace`; every step has exactly
  `sequence`, non-empty `stimulus`, non-empty `expected`, and `oracle_ref`.
- `generated_decision_table_v1` keeps `source_digest` as the complete invariant
  file digest because regeneration is its provenance contract.
  `hand_authored_state_machine_v1` instead defines `source_digest` as the
  `invariant_execution_contract_v1` projection digest: invariant behavior,
  input, oracle, and every other unexcluded semantic field remain frozen, while
  `status`, `proof_level`, `tests`, `gates`, `evidence_refs`, `spec_gap_refs`,
  `missing`, and `last_reviewed` may progress without invalidating a previously
  reviewed trace. Coverage state, gap binding, and rationale may progress, but
  its ordered cell set, dimensions, decision references, and future unknown
  fields remain digest-bound. The provenance kind versions this distinction;
  unknown provenance kinds fail closed.
  Sequence numbers are contiguous from zero, trace values are canonical JSON
  values without TOML floats, and every oracle ref resolves to an active
  eligible source. Durations and other numeric trace values use integer units;
  even finite TOML floats are forbidden because Python and Rust JSON encoders
  need not serialize them identically for the trace-definition digest.
- The trace-definition digest is SHA-256 over canonical case IDs and trace
  definitions and is recomputed independently by the metadata validator and
  `verification-cases`. A blocked row may omit its trace only while its existing
  `spec_gap_ref` remains open; it cannot contribute to red/green closure.
- Generated provenance forbids trace fields. Hand-authored provenance forbids
  `generator` and `generator_digest`; relabeling one mode as the other fails.
- The case-file root and each `[[case]]` table are closed contracts. Unknown
  fields fail both metadata validation and Rust helper deserialization. The
  `input` and `expect` tables, plus the trace `stimulus` and `expected` tables,
  remain explicitly dynamic value maps; their enclosing records are closed.

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
- `case_provenance_kind`
- `trace_definition_digest` (`null` for generated decision tables)
- `trust_verify_test_id`, `trust_verify_run_id`,
  `trust_verify_case_file_digest`, and `trust_verify_artifact_dir` (required
  keys that are `null` only for an unstamped standalone helper run)
- `cases`
- per-case `id`, `family`, `result`, `spec_gap_ref`, observed error/status,
  state-delta verdict, and before/after snapshots

The artifact is not durable evidence by itself. The test helper writes only
harness-observed facts. `prove.py` owns command, commit, platform, timing,
failure-kind classification, and evidence creation. If `prove.py` exports
`TRUST_VERIFY_*` environment variables for the helper to stamp into the case
artifact, `prove.py` must verify the stamped values before accepting the
artifact. The helper fails before case execution when only some stamp variables
are present or when stamped `TEST_ID`, case-file digest, or artifact directory
does not match the current run configuration.
`prove.py` additionally requires the artifact's provenance kind and trace digest
to equal the committed case file. The case-file digest still binds the complete
source; the explicit fields prevent one case mode from impersonating another.
The artifact root and each per-case result are closed contracts. `prove.py`
requires `helper_version = "verification-cases v1"` and exact equality between
`artifact.case_file` and the catalog `case_file`; a digest match cannot excuse
different path text or an unreviewed helper implementation.

For `prove.py lock`, the raw case-artifact digest is provenance only because
freshness stamps intentionally change between runs. The stable behavior-lock
comparison uses a deterministic `case_result_digest` derived from command exit
status plus per-case results, paired to the baseline through
`paired_lock_baseline`.

### Suite Record

```toml
schema_version = 2
id = "pr"
title = "Pull Request Verification"
area = "suite"
owner = "verification"
status = "mapped"
last_reviewed = "2026-07-10"
purpose = "Bind current PR entrypoints and their inventoried helper gates."
duration_class = "medium"
environment = "github_matrix"
commands = [
  "workflow job .github/workflows/ci.yml#fmt",
]
command_bindings = [
  "GATE_JOB_CI_FMT",
]
inventory_ids = [
  "GATE_JOB_CI_FMT",
]
evidence_destination = "ci-artifact:gate-*"
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
- `command_bindings`
- `inventory_ids`
- `evidence_destination`
- `includes`
- `excludes`
- `approved_proof_producers`

`verification/gate-inventory.toml` owns command facts. Every workflow job and
root gate script is live-bound or explicitly excluded. A suite's
`inventory_ids` is the exact exhaustive direct reverse join for that suite;
`command_bindings` must contain every directly assigned inventory row whose
`command_role` is `entrypoint` and no `helper` or `reference` row, and
`commands` must equal the corresponding ordered inventory command projection.
This avoids duplicating owners, environments, durations, artifact posture, and
enforcement facts in suite files.

Milestone suite records are `mapped`, non-placeholder, and nonempty. This does
not mean they have run or produced proof. `veryquick` is explicitly
`trust_builder`-owned. Hardware entrypoints require
`TRUST_DIT_REQUIRE_HARDWARE=1`; the strict device-in-loop script is the
entrypoint and the skip-capable scheduled workflow remains a helper reference.
Release entrypoints require durable evidence, a named CI artifact, or a bound
CI job result and cannot use `target/**` as durable output. The
`supporting_local` bucket may be commandless because its inventory rows are not
milestone entrypoints.

`includes` and `excludes` are validated identifiers/display constraints only.
The validator does not recursively compose commands or claim inherited
execution; `VERIF-P14-000B` owns that future policy.

### Ignored Test Record

```toml
schema_version = 2
id = "IGNORED_0123456789ABCDEF0123"
discovery_id = "DISC_0123456789ABCDEF0123"
discovery_source_kind = "rust_integration_test"
path = "crates/trust-runtime/tests/example.rs"
name = "historical_ignored_example"
ignore_state = "ignored"
ignore_reason = "reviewed source attribute text"
ignore_mechanism = "rust_attribute"
owner = "trust-runtime"
area = "bytecode_vm"
status = "gap_open"
ignore_class = "unknown"
reason = "Current behavior has not been re-established under this program."
unblock_condition = "Rerun at a reviewed commit, then remove the ignore or assign an evidence-backed class."
last_reviewed = "2026-07-10"
```

Required fields:

- `schema_version`
- `id`
- `discovery_id`
- `discovery_source_kind`
- `path`
- `name`
- `ignore_state`
- `ignore_reason`
- `ignore_mechanism`
- `owner`
- `area`
- `status`
- `ignore_class`
- `reason`
- `unblock_condition`
- `last_reviewed`

`discovery_id` is the required source identity. The live checker binds path,
name, source kind, raw ignore reason, state, and mechanism exactly while
deliberately ignoring line movement. `test_id` is optional; when present it
must resolve to a catalog row with the same discovery identity. Registry IDs
are not catalog test IDs, and uncataloged ignored facts must not invent one.

Class-specific fields:

- `red_protective`: nonempty, unambiguous `linked_rows` plus
  `expected_red_symptom`.
- `lab_required`: non-secret `required_env_vars`, `hardware_topology`, a
  tracked non-symlink `hardware_topology_ref`, and nonempty
  `public_claim_impact`. This impact is not a test-to-public-claim mapping.
- `flaky_quarantined`: `last_observed_failure`, `failure_signature`, and a
  tracked non-symlink `evidence_ref`.

Class-only fields are forbidden on other classes. `unknown` must use
`status = "gap_open"` and remains report-only until `VERIF-P14-000` defines
the grace rule; there is no ad hoc strict flag. Mechanical names, paths,
comments, reasons, and lexical references never infer a reviewed class.

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
source_refs = ["SPEC_BYTECODE_FORMAT_001"]
related_invariants = ["VM_SEAM_VALID_001"]
related_spec_gaps = ["SPEC_GAP_BYTECODE_VALIDATOR_001"]
related_spec_sources = ["SPEC_BYTECODE_FORMAT_001"]
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
- `source_refs`
- `related_invariants`
- `related_spec_gaps`
- `related_spec_sources`
- `evidence_refs`

Evidence record details, traceability report rules, and verification-tooling
self-test fixtures live in `metadata-evidence-traceability.md`.
