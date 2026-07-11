# Evidence and Traceability Metadata

This companion to `metadata-model.md` owns the detailed evidence record,
traceability report, and verification-tooling self-test rules. It was split out
before Phase 1B behavior-row work to keep each metadata document reviewable.

## Evidence Record

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
# Manual is allowed for non-red/green notes and reports. It cannot close
# high-risk red/green proof.
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

Optional proof fields for `prove.py` or approved gate output:

- `proof_kind`: `red`, `green`, `lock_baseline`, `lock_compare`,
  `protective_red`, or `none`.
- `failure_kind`: stable failure classification such as `assertion_failure`,
  `expected_rejection`, `compile_error`, `harness_panic`, `timeout`, or `none`.
- `case_artifact_path`, `case_artifact_digest`, `case_file_digest`,
  `case_result_digest`, `paired_red_evidence`, `formerly_red_case_ids`,
  `paired_lock_baseline`, `decision_ref`, and `per_case_summary`.

Optional mutation-report fields for the Phase 1B bytecode-validator pilot:

- `mutation_test_id`
- `mutation_report_path`
- `mutation_report_digest`

Allowed evidence kinds:

- `committed_file`
- `ci_artifact`
- `release_object`
- `lab_report`

Evidence rules:

- Every evidence record must name either `suite_id` or `release_object`.
  `suite_id` names the suite bucket that owns the evidence, not necessarily a
  final milestone gate. Supporting local proof that is not a gate uses the
  `supporting_local` suite bucket until Phase 5 defines final suite
  composition.
- `commit` is the tested commit, or `dirty:<base-commit>` when the worktree was
  dirty; a bare dirty marker without the base commit is invalid.
- The generated existing-test catalog JSON under
  `target/gate-artifacts/verification/` is a reproducible working artifact, not
  durable evidence and not proof of a product claim. Its committed dated
  Markdown summary may be indexed as `proof_kind = "none"` with empty invariant
  and test links after the JSON/input digests and counts validate at rest.
- The generated test-class completeness JSON uses the same working-artifact
  rule. Its committed dated Markdown summary may be indexed with
  `proof_kind = "none"` and empty invariant/test links only after the validator
  recomputes the live scanner/catalog/matrix joins, input digest, class counts,
  and Markdown-to-JSON digest. Missing mappings/classes are visible debt and do
  not become behavior proof.
- `committed_file` paths must be git-tracked; `ci_artifact` refs name
  `workflow`, `run_id`, `artifact`, and `retention_days` when the artifact is
  the only proof; `release_object` evidence names `release_object` and `url`;
  `lab_report` evidence names `device_model`, `firmware`, `topology`, and
  `env_vars`.
- For safety-critical, wrong-result, silent-corruption, and false-status
  behavior-changing proof, the producer is an allowlist, not a blocklist:
  `producer` must be `prove.py vN` or an approved gate recorded in suite
  metadata. Values such as `manual`, `codex`, `claude`, `fable`, or any
  unrecognized agent/tool name are treated as manual provenance and cannot close
  red/green proof for those risks.
- `prove.py green` must pair to the red evidence record for the same test and
  same case-file digest. It must prove every formerly-red case is now green, no
  previously-green case regressed, no case was skipped without a waiver, and
  the case-file digest did not change unless the changed case table has its own
  reviewed decision or spec-gap closeout.

### Bytecode-Validator Mutation Report Contract

The `VERIF-P1B-013` report is a narrow pilot contract, not the generic Phase 10
mutation-report format tracked by `VERIF-P10-003`.

- The runner uses cargo-mutants single-file candidates because cargo-mutants
  package discovery does not traverse the validator's `include!()` fragments.
- Product source is mutated only inside an isolated archive of the tested Git
  commit. A dedicated target is cleaned for `trust-runtime` before baseline,
  before every mutant, and after source restoration so mutant artifacts cannot
  contaminate later proof.
- Results use the closed vocabulary `caught`, `survived`, `unviable`, `timeout`,
  or `error`; compile failures are `unviable`, never `caught`. Signals and known
  disk, quota, temporary-directory, or permission failures are infrastructure
  errors and abort the shard instead of being counted as mutant outcomes.
- Every measured mutant names committed `related_case_ids`. Every survivor also
  carries a non-empty action. Unknown, duplicate, overlapping, omitted, or
  unaccounted case IDs fail validation.
- `related_case_ids` are traceability labels, not an assertion that a blocked
  case ran or killed a mutant. Fields such as `killed_by_case_ids` or
  `executed_case_ids` are forbidden while the cases remain blocked.
- The report JSON pins the cataloged case-file digest, source commit, exact tool
  and runner versions, platform, passing baseline commands, per-mutant selector
  and build/test commands, raw exit/timeout fields, derived outcomes, summary
  counts, survivors, and out-of-scope case IDs. The evidence record pins the
  JSON path and SHA-256 digest, and the at-rest validator recomputes the report
  contract from the already-loaded catalog record.
- `prove.py lock` for refactors must pair `lock_compare` evidence to a
  `lock_baseline` evidence record and report any output delta.

### `prove.py` Contract

`prove.py` is the only default producer allowed to create high-risk red, green,
and lock evidence. It is deterministic wrapper tooling around cataloged
commands and generated case artifacts; it must not decide product behavior.

Command surface:

```text
prove.py red --test <TEST_ID>
prove.py green --test <TEST_ID> --red-evidence <EVIDENCE_ID>
prove.py lock --test <TEST_ID> --baseline
prove.py lock --test <TEST_ID> --compare <BASELINE_EVIDENCE_ID>
```

Catalog binding rules:

- `TEST_ID` resolves to exactly one `verification/test-catalog.toml` record.
- The catalog record must be at least `mapped` before `prove.py` can create
  proof; planned case-table rows are not runnable tests.
- `prove.py` must refuse a `TEST_ID` named by an ignored record's optional
  `test_id` field unless a reviewed decision explicitly authorizes collecting
  diagnostic-only evidence for that ignored test. The lookup is indexed by
  catalog test identity, never by the ignored record's `IGNORED_*` ID. Records
  without `test_id` classify uncataloged source facts and do not fabricate a
  catalog mapping.
- The catalog record's `command` is the command `prove.py` runs. Agents do not
  run a different command and hand-write evidence.
- If the catalog row names `case_file`, it also names `case_file_digest`; the
  emitted case artifact must match both.
- Artifact trust is by `test_id` and digest, not path text. The expected helper
  artifact path is workspace-root `target/gate-artifacts/cases/<TEST_ID>.json`
  unless the catalog/command contract explicitly specifies another artifact
  directory.
- Before running the cataloged command, `prove.py` must delete or quarantine any
  pre-existing artifact for the requested `TEST_ID`. A missing artifact after a
  command that was expected to emit one is `metadata_error` or `infra_error`,
  never proof.
- `prove.py` must bind artifacts to the run it just started by exporting
  `TRUST_VERIFY_TEST_ID`, `TRUST_VERIFY_RUN_ID`, `TRUST_VERIFY_CASE_FILE_DIGEST`,
  and `TRUST_VERIFY_ARTIFACT_DIR`. The `verification-cases` helper extension in
  `VERIF-P1B-008` must stamp those values into the case artifact. `prove.py`
  must reject artifacts with missing or mismatched stamps.
- `TRUST_VERIFY_RUN_ID` must be unique and non-empty for the proof run.
- `TRUST_VERIFY_ARTIFACT_DIR` is matched as exact path text by the helper.
  `prove.py` must export the same normalized absolute path text it passes to
  the helper; trailing-slash and symlink spelling differences fail closed.
- Cataloged Rust test commands that export `TRUST_VERIFY_*` must filter to the
  single intended test. A binary that runs multiple `run_case_file` tests under
  one stamped `TEST_ID` will correctly fail on the non-target tests.
- `prove.py` runs each cataloged command once. Retrying until the desired color
  appears is forbidden; flaky behavior is recorded as evidence, not hidden.

Case-artifact checks:

- The artifact parses as `case-artifact.schema.json`.
- `artifact.test_id` equals the requested `TEST_ID`.
- `artifact.case_file_digest` equals the catalog row's `case_file_digest`.
- `artifact.case_file` names the catalog row's `case_file` or a canonical
  equivalent; the digest remains the authority.
- Artifact stamp fields match the `TRUST_VERIFY_*` values for this run.
- Every committed case ID appears exactly once.
- No unknown case IDs appear.
- A blocked case is allowed only while the catalog/invariant still names the
  referenced spec gap; blocked cases cannot close red/green proof.
- A skipped case requires a waiver/decision ref; unwaived skipped cases fail
  proof.
- Case artifact results are a closed vocabulary: `passed`, `failed`, `skipped`,
  or `blocked`. Unknown result strings are metadata errors. Green proof may only
  close when every case in the artifact is `passed`.

Failure-kind classification:

- `assertion_failure`: command ran, harness assertion failed, and the artifact
  identifies failed cases. Counts as red for bug fixes when required.
- `expected_rejection`: command failed because the oracle says load/compile
  rejection is expected. Requires an explicit oracle and a validator-backed
  catalog mechanism. Until that mechanism exists, `prove.py red` must reject
  catalog fields such as `expected_red_failure_kind` as metadata errors rather
  than treating arbitrary non-zero exits as red evidence.
- `compile_error`: Rust/build/tooling compile failure. Does not count as red
  unless the oracle expects compile/load failure.
- `harness_panic`: panic in harness/helper/runner outside expected assertion.
  Never counts as red.
- `metadata_error`: metadata/catalog/case/artifact shape failure. Never counts
  as red.
- `timeout`: never counts as red unless timeout behavior itself is the oracle.
- `infra_error`: disk, permission, missing-tool, or environment failure. Never
  counts as red.
- `none`: command/proof succeeded.

Failure classification must come from deterministic, documented signals. For
Rust commands, `prove.py` must distinguish cargo build/compile failure,
test-process failure, test assertion failure, harness panic, timeout, and
missing-artifact cases by a documented command runner contract. Ad hoc regex
matching over unstructured terminal output is not a proof contract.

`prove.py red` rules:

- Runs the cataloged command itself.
- Runs each command once with a bounded timeout. The P1B red-only
  implementation uses an 1800-second default so cold cargo builds do not
  spuriously time out; timeout is recorded as non-red `timeout`.
- Requires a case artifact when the catalog row names `case_file`.
- Writes evidence with `producer = "prove.py v1"`, `proof_kind` `red` or
  `protective_red`, `failure_kind`, `case_artifact_path`,
  `case_artifact_digest`, `case_file_digest`, `command_exit_status`, and
  `per_case_summary`.
- Red proof identifies red case IDs and failure source. `compile_error`,
  `harness_panic`, `metadata_error`, `timeout`, and `infra_error` may be
  recorded for debugging, but cannot advance proof status.
- `assertion_failure` red requires a same-run case artifact whose
  `per_case_summary` names at least one failed case ID. `expected_rejection`
  red remains reserved until the expected-rejection catalog field, rejection
  signal, and validator rules are implemented together. Red evidence with an
  empty failed-case set cannot feed green pairing.

`prove.py green` rules:

- Requires `--red-evidence` naming known red/protective-red evidence for the
  same `TEST_ID`.
- The paired record must have `proof_kind` `red` or `protective_red`,
  `producer = "prove.py vN"` or an approved gate, the same `TEST_ID` in
  `linked_tests` as the only linked test, the same `case_file_digest` as both
  the green record and the current catalog row, `failure_kind`
  `assertion_failure` or `expected_rejection`, and non-empty `per_case_summary`
  failed-case data.
- Runs the cataloged command itself.
- Requires the same `case_file_digest` as red evidence unless reviewed
  decision/spec-gap closeout permits a changed case table. That exception must
  be represented by `decision_ref` to an active reviewed decision/deviation.
- Every formerly-red case ID must now pass or be covered by explicit reviewed
  case-table change.
- No previously-passing case may regress.
- No case may be skipped without waiver.
- Command exit status and artifact status must agree. A green command with a
  failing artifact, or a failing command with a green artifact, is
  `metadata_error` until the command contract explains it.
- Writes evidence with `proof_kind = "green"`, `paired_red_evidence`,
  `formerly_red_case_ids`, `case_artifact_path`, `case_artifact_digest`,
  `case_file_digest`, `command_exit_status`, and `per_case_summary`.
- Green `per_case_summary` may contain only `case_id:passed` entries. Any
  `failed`, `skipped`, `blocked`, malformed, or unknown-result entry keeps the
  proof open.
- The at-rest validator must reject green evidence whose paired record is not
  red/protective-red proof for the same test and digest, is not allowlist
  produced, has no failed-case summary, or leaves `formerly_red_case_ids` empty.

`prove.py lock` rules:

- `lock --baseline` runs the cataloged command and writes `proof_kind =
  "lock_baseline"` evidence with command exit status, case-file digest,
  raw case-artifact digest when present, deterministic `case_result_digest`,
  and `per_case_summary`.
  A lock baseline must be produced by `prove.py vN` or another approved proof
  producer, must be backed by a catalog `case_file`, and must have
  `command_exit_status = 0` with no failed or blocked cases.
- `lock --compare <BASELINE_EVIDENCE_ID>` runs the same cataloged command and
  compares to baseline. The comparison basis is command exit status,
  `case_file_digest`, `case_result_digest`, and per-case results. Raw terminal
  output is not the comparison basis unless the catalog row explicitly defines a
  deterministic normalized output artifact. Raw `case_artifact_digest` is
  recorded for provenance, but it is not stable enough for behavior-lock
  comparison because same-run freshness stamps include a unique
  `TRUST_VERIFY_RUN_ID`.
- `lock_compare` requires a baseline record with `proof_kind =
  "lock_baseline"`, an approved producer, the same `TEST_ID`, the same current
  catalog command, compatible case-file digest, `command_exit_status = 0`, and
  `per_case_summary`. Missing, wrong-kind, wrong-test, wrong-producer,
  command-drift, result-digest drift, or case-file-digest drift baseline records
  fail lock.
- The at-rest validator recomputes `case_result_digest` from
  `command_exit_status` and `per_case_summary` for lock records. A hand-edited
  pair with matching forged digest strings is still invalid.
- Any command-exit, case-result, or digest delta must be reported. Unreviewed
  deltas fail lock. A reviewed accepted delta requires `decision_ref` to an
  active reviewed decision/deviation.

Adversarial self-test fixtures:

- missing catalog test ID
- planned case-table row treated as runnable proof
- artifact missing
- artifact `test_id` mismatch
- artifact `case_file_digest` mismatch
- missing committed case
- duplicate or unknown case ID
- unwaived skipped case
- compile error misrecorded as red
- harness panic misrecorded as red
- metadata error misrecorded as red
- timeout misrecorded as red
- infra error misrecorded as red
- green paired to wrong red
- green paired to non-red evidence
- green paired to red evidence lacking per-case data
- green with changed case digest and no decision
- stale artifact from a previous run accepted
- hand-authored artifact accepted without same-run stamp
- command/artifact exit-status disagreement accepted as green
- proof for a test listed in `ignored-tests.toml`
- lock compare with output delta reported as success
- lock compare against missing or wrong-kind baseline
- lock compare baseline for a different `TEST_ID`
- lock compare case-file digest drift without `decision_ref`

Accepted risk: a hand-authored evidence record can forge `producer =
"prove.py v1"` text at rest. The defense is re-runnability, review,
same-run artifact stamping, and the later verification-gate/CI ratchet; the
producer string alone is provenance, not cryptographic proof.

Producer vocabulary:

- `manual`: human-authored evidence note.
- `codex`, `claude`, `fable`: agent-authored review or report note. These are
  provenance values, not proof producers for high-risk red/green closure.
- `prove.py vN`: verification prover output.
- `verification-gate vN`: CI/local metadata gate output.
- `approved-gate:<suite-or-script-id>`: named suite or gate with an explicit
  suite record and proof fields equivalent to `prove.py`.

Waiver and risk-change rules live in `spec-matrix-model.md`.

## Traceability Reports

### Phase 6 Requirement/Oracle Audit

Before bidirectional public-claim traceability can be exhaustive, Phase 6 uses a
bounded requirement/oracle audit over all committed invariant records. The
audit derives mappings only from each invariant's explicit `[spec].source_refs`,
`spec_gap_refs`, and `[oracle].ref` fields. Titles, paths, source text, test
names, and lexical candidates never create a mapping.

The reviewed row scopes are:

- `VERIF-P6-001`: `compiler_iec`;
- `VERIF-P6-002`: `runtime_safety`;
- `VERIF-P6-003`: `protocols`;
- `VERIF-P6-004`: `editor_safety`;
- `VERIF-P6-005`: `control_security` and `supply_chain_platform`.

`VERIF-P6-006` reports every committed invariant, including areas outside those
five mapping rows. An invariant has an available oracle only when its
`[oracle].ref` names an active, oracle-eligible, non-public-claim specification
source. A reference to an open specification gap is a visible missing-oracle
blocker, even when the invariant also lists an active candidate source. Public
claims may create obligations or context but can never be counted as oracles.

The report lists missing-oracle invariants in the four risk classes named by
`VERIF-P6-007`, but does not fail on them. That enforcement row remains blocked
until `VERIF-P14-000` defines its grace rule. The report scope is committed
verification metadata, not a scan of every possible product requirement.
`VERIF-P1A-002`, `VERIF-P1A-003`, `VERIF-P1A-006`, `VERIF-P1A-007`, and
`VERIF-P4A-005` therefore remain visible limits. Forward, reverse, and orphan
traceability rows `VERIF-P6-008` through `VERIF-P6-010` stay open until their
public-claim and live-test denominators can be exhaustive. Test, gate, and
evidence IDs shown in the Phase 6 ledger are copied explicit associations, not
a completed trace. Their registries are live-validated; the evidence index is
excluded from the report input digest to avoid a report/evidence digest cycle.

## Conformance Alignment Boundary

The Phase 7 alignment report traces the committed conformance corpus only as
far as explicit metadata permits:

```text
public suite contract -> category -> manifest -> expected artifact
                                      |
                                      +-> exact catalog discovery_id -> invariant
```

The right-hand link is absent for all 21 current cases. The report must show
`0/21` explicit invariant coverage and keep `VERIF-P7-002` open; case names,
descriptions, source text, expected JSON, and lexical candidates cannot fill
the link. The ten v2 category rows are alignment debt with semantic oracles
`not_assessed`, not specification-gap records and not behavior proof.

`SPEC_CONFORMANCE_CONTRACT_001` owns the public category, result, determinism,
and generated-report contract. It is deliberately `oracle_eligible = false`
for individual case behavior. Expected artifacts remain baselines checked by
the runner, not independent semantic authorities.

The report binds the scripted in-process communication case and the CI
artifact publication path. It does not run a case, use a socket or hardware,
commit a generated suite result, create evidence links, close a specification
gap, or upgrade an invariant.

The program must generate both directions:

Forward trace:

```text
user story/public workflow -> spec source -> invariant -> test -> suite/gate -> evidence -> public claim
```

Reverse trace:

```text
public claim/workflow -> evidence -> suite/gate -> test -> invariant -> spec source -> user story/public workflow
```

Traceability report requirements:

- Every safety-critical invariant has a spec source or spec gap.
- Every mapped test has at least one invariant.
- Every release/public claim has a proof path or visible gap.
- Every user-facing workflow claim has a workflow spec source or visible spec
  gap before implementation proof is mapped.
- Every evidence artifact names command, commit/dirty marker, platform, and
  generated report version.
- Every spec gap lists blocked tests or explains why no test can be authored.

## Verification Tooling Self-Tests

The verification program must test its own tooling.

Required fixtures: known-good metadata; missing required field; unknown status;
stale path; gitignored durable evidence; unknown invariant/suite/evidence IDs;
public claim without proof/gap; ignored test with `unknown` after grace period;
schema-version mismatch; mapped record with empty invariants; stale test name;
`validated` with empty evidence, open safety coverage, or low proof level;
traceability cycle; uncovered decision-table partition; stale case digest;
skipped case artifact; non-allowlisted high-risk proof; green evidence without
paired red evidence; compile error or harness panic misrecorded as red.

Required tooling tests:

- schema validator rejects known-bad fixtures,
- catalog scanner detects changed/deleted tests,
- spec-source scanner emits missing/stale/conflict reports,
- changed-file classifier maps paths to code areas and gates,
- report renderer output is stable enough for review,
- generated artifacts distinguish inferred facts from hand-authored intent.
