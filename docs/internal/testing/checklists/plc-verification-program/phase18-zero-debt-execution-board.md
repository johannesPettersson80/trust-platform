# Phase 18 Post-Closure Behavior Delta

Status: corrective reconstruction in progress.

Current row: `VERIF-P18-DELTA-003`.

## Accepted Baseline

The PLC verification program was completed and independently accepted on
2026-07-19. The accepted Phase 16 closure is the historical baseline:

- 44 specification gaps closed and 0 open;
- 55 invariants, with 0 at S0;
- 4,036 discovered test facts partitioned into 258 catalog mappings and 3,778
  reviewed nonmappings, with 0 unreviewed facts;
- 242 mapped commands passed; and
- final remote `fmt`, `clippy`, `test-all`, metadata, and 16 report validations
  passed.

Evidence:
`docs/internal/testing/evidence/plc-verification-program/2026-07-19/p16-final-closure.md`.

A later Phase 17 change introduced a per-function production-code index and
converted 15,133 implementation facts into blanket specification gaps without
identifying missing expected behavior. Phase 18 then treated those synthetic
rows, and every reviewed nonmapping test, as new work. That expansion is not
part of the accepted behavior-level model and is being removed.

## Retained Model

The program tracks observable product behavior:

```text
written specification
  -> behavior invariant
  -> executable test or explicit proof
  -> suite and evidence
```

A `spec_gap` is valid only when expected product behavior is genuinely unknown
or undecided. A function, method, helper, file, or test name is not by itself a
behavior requirement or a specification gap.

A `reviewed_nonmapping` row is accepted denominator inventory. It remains a
native executable test, but it is not claimed as proof for a hand-owned
behavior invariant and does not need to be routed again unless a real invariant
requires it.

## Corrective Census

After removing the campaign framework and auditing the retained metadata, the
current corrective candidate contains:

- 45 specification-gap records: 44 closed and
  `SPEC_GAP_CURRENT_DT_CLOCK_BOUNDARY_001` honestly reopened at `test_mapped`;
- 78 registered specification sources;
- 431 behavior invariants: 371 mapped, 32 `gap_open`, 23 implemented, and 5
  validated;
- 4,405 catalog records;
- 8,692 live denominator rows: 4,397 catalog-mapped and 4,295
  reviewed-nonmapping; and
- 6 behavior invariants with no mapped executable test.

The 32 `gap_open` invariants have tests, but their prior current-proof promotion
depended on stale or post-baseline campaign evidence. Historical red/green and
lock artifacts are retained with `proof_contract_binding = "source_revision"`;
they do not prove the current source revision. A new current targeted proof is
required before any of those invariants can return to `implemented` or G1.

The six actual invariant-to-test authority debts are:

- `IEC_PROJECT_EXAMPLE_CONFORMANCE_001`;
- `RT_CONNECTOR_REPORT_PROJECTION_001`;
- `RT_EXECUTION_LANGUAGE_CONFORMANCE_001`;
- `RT_HOST_SECURITY_WEB_CONFORMANCE_001`;
- `RT_STDLIB_CORE_HELPER_002`; and
- `RT_STDLIB_FB_RUNTIME_002`.

They remain mapped/S0 with an explicit missing catalog mapping. Do not bind a
test across product areas or infer a mapping from a function or test name.

## Other Genuine Proof Work

The corrective audit also retains these non-substitutable proof obligations:

- `RT_TYPED_POINT_MAP_002`: external interoperability evidence;
- `RT_GPIO_PARAMETER_HANDOFF_002`: physical GPIO evidence;
- `RT_WEB_SETUP_IO_PROJECTION_002`: rendered setup-page smoke;
- `DEV_TEST_EXECUTION_001`: direct runtime-error execution;
- `RT_CONTROL_AUDIT_VISIBILITY_002`: disabled-feature behavior;
- `RT_HMI_RUNTIME_SURFACE_001`: authorization behavior;
- `RT_OPERATOR_CONSOLE_PROJECTION_001`: input routing;
- `RT_IO_CYCLE_BINDING_001`: grouped and alias binding;
- `VERIFICATION_CASE_ARTIFACT_HELPER_001`: all four contract cells; and
- three stale focused mutation shards: bytecode validator, HIR diagnostics,
  and retain restart.

Hardware, rendered, and third-party interoperability proof must be recorded as
external/manual work. Unit tests or simulator runs must not replace it.

## Execution Rows

- [x] `VERIF-P18-DELTA-000` Recover and verify the accepted Phase 16 closure.
- [x] `VERIF-P18-DELTA-001` Remove the per-function code index, blanket code
  gaps, mapping accelerator, generated burn-down ledgers, one-off campaign
  tools, and framework-only tests while preserving product code, written
  specifications, real tests, and valid behavior mappings.
- [x] `VERIF-P18-DELTA-002` Audit every post-baseline invariant/test binding.
  Demote invalid cross-area mappings, reopen promotions that rely only on stale
  proof, retain historical evidence as source-revision evidence, and record the
  six actual no-test authority debts above.
- [ ] `VERIF-P18-DELTA-003` Freeze deterministic case and mutation inputs.
  Regenerate only mechanically stale case tables, refresh reviewed mutation
  selectors/digests from live source, execute the three stale mutation shards
  from a clean exact-commit remote checkout, and retain raw artifacts.
- [ ] `VERIF-P18-DELTA-004` Resolve the feasible behavior/test debts listed
  above test-first. Keep hardware, rendered, interoperability, and any genuine
  specification decision explicitly open until their required environment or
  authority exists.
- [ ] `VERIF-P18-DELTA-005` Run canonical behavior-level audits: metadata,
  scanner/catalog staleness, requirement/oracle traceability, specification
  completeness, and denominator integrity. Reviewed nonmapping remains an
  accepted disposition and is not universal mapping debt.
- [ ] `VERIF-P18-DELTA-006` Reconcile the program entrypoint, implementation
  board, metadata model, README, architecture checklist, and diagram manifest;
  obtain an independent read-only review of the exact final diff.
- [ ] `VERIF-P18-DELTA-007` Run the frozen candidate's focused tests and final
  remote `fmt`, `clippy`, and `test-all`; commit the corrected checkpoint and
  stop without pushing.

## Stop Gates

- [x] Do not treat functions, helpers, files, or scanner facts as
  specification gaps.
- [x] Do not treat reviewed nonmapping tests as missing tests.
- [x] Do not relabel product areas or fabricate current evidence to make a
  validator pass.
- [ ] Do not promote an invariant while its current-revision proof is missing.
- [ ] Do not close a listed external/manual obligation with a weaker proof
  class.
- [ ] Do not commit while canonical metadata, test-staleness, or frozen remote
  candidate checks fail.

## SOLID/KISS/DRY Acceptance

- [x] The retained model has one behavior-level chain and no parallel
  per-function authority plane.
- [x] Existing scanners inventory tests; they do not invent behavior or
  expected results.
- [x] Specification, invariant, catalog, suite, and evidence ownership remain
  separate.
- [ ] Final documentation and the diagram describe only the retained model.
- [ ] The final exact candidate and independent review are recorded.
