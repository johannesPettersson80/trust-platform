# LD Specification Detailed Checklist (IEC 61131-3 Ed.3)

Use this checklist to review and sign off LD specification quality, implementation profile
alignment, and test/evidence traceability.

Repository anchor:
- `/home/johannes/projects/trust-platform-merge-run`

Truth policy:
- Keep `[x]` only when implementation (or doc), validation, and evidence are all present.
- If any one of those is missing, reset to `[ ]`.
- Every checked item should include evidence in the `Evidence` field.

---

## 1. Governance and Baseline

- [x] IEC baseline fixed to Edition 3.0 (2013-02) in LD spec docs.
  Evidence: `docs/specs/15-ladder-diagram.md:3`, `docs/specs/README.md:31`.
- [x] LD normative vs implementation-profile split is explicit and consistent.
  Evidence: `docs/specs/README.md:7`, `docs/specs/README.md:8`, `docs/specs/15-ladder-diagram.md:20`, `docs/specs/16-ladder-profile-trust.md:5`.
- [x] Ambiguities are captured in `docs/IEC_DECISIONS.md`.
  Evidence: `docs/IEC_DECISIONS.md:5`, `docs/IEC_DECISIONS.md:35`.
- [x] Deviations/extensions are captured in `docs/IEC_DEVIATIONS.md`.
  Evidence: `docs/IEC_DEVIATIONS.md:5`, `docs/IEC_DEVIATIONS.md:40`.
- [x] Checklists follow truth policy (no unchecked reality marked as done).
  Evidence: `docs/internal/testing/checklists/ld-specification-detailed-checklist.md:9`, `docs/internal/testing/checklists/ld-full-iec-implementation-checklist.md:9`.

## 2. Normative LD Language Spec Completeness

Target file:
- `docs/specs/15-ladder-diagram.md`

- [x] Conformance language (`MUST/SHOULD/MAY`) is present.
  Evidence: `docs/specs/15-ladder-diagram.md:22`.
- [x] IEC anchors for LD semantics are listed (Section 8.2, relevant FB tables).
  Evidence: `docs/specs/15-ladder-diagram.md:30`.
- [x] Network/rung model and evaluation ordering are defined.
  Evidence: `docs/specs/15-ladder-diagram.md:39`, `docs/specs/15-ladder-diagram.md:41`.
- [x] Left-to-right power flow and branch semantics are defined.
  Evidence: `docs/specs/15-ladder-diagram.md:45`, `docs/specs/15-ladder-diagram.md:47`, `docs/specs/15-ladder-diagram.md:102`.
- [x] Contact semantics (`NO`, `NC`) are defined.
  Evidence: `docs/specs/15-ladder-diagram.md:90`.
- [x] Coil semantics (`NORMAL`, `SET`, `RESET`, `NEGATED`) are defined.
  Evidence: `docs/specs/15-ladder-diagram.md:95`.
- [x] Function/function-block invocation rules are defined.
  Evidence: `docs/specs/15-ladder-diagram.md:108`.
- [x] Timer/counter semantic expectations map to IEC Table 45/46.
  Evidence: `docs/specs/15-ladder-diagram.md:126`, `docs/specs/15-ladder-diagram.md:127`.
- [x] Scan-cycle model (read/evaluate/commit/publish) is defined.
  Evidence: `docs/specs/15-ladder-diagram.md:132`.
- [x] Required diagnostics are enumerated.
  Evidence: `docs/specs/15-ladder-diagram.md:145`.
- [x] Edition 2 vs 3 compatibility statement is present.
  Evidence: `docs/specs/15-ladder-diagram.md:157`.
- [x] Symbolic-first variable policy is explicit.
  Evidence: `docs/specs/15-ladder-diagram.md:60`.
- [x] Local/global variable separation is defined via IEC sections (`VAR*`, `VAR_GLOBAL`, `VAR_EXTERNAL`).
  Evidence: `docs/specs/15-ladder-diagram.md:65`, `docs/specs/15-ladder-diagram.md:67`, `docs/specs/15-ladder-diagram.md:78`.

## 3. Implementation Profile Completeness

Target file:
- `docs/specs/16-ladder-profile-trust.md`

- [x] Schema v2 top-level contract is documented (`schemaVersion`, `metadata`, `variables`, `networks`).
  Evidence: `docs/specs/16-ladder-profile-trust.md:18`, `docs/specs/16-ladder-profile-trust.md:30`.
- [x] Supported LD node subset is fully listed.
  Evidence: `docs/specs/16-ladder-profile-trust.md:55`.
- [x] Runtime deterministic behavior and topology validation contract are documented.
  Evidence: `docs/specs/16-ladder-profile-trust.md:67`.
- [x] Variable/address runtime behavior is documented as profile-level, not normative limit.
  Evidence: `docs/specs/16-ladder-profile-trust.md:81`, `docs/specs/16-ladder-profile-trust.md:96`.
- [x] Runtime right-pane parity contract for Ladder/Statechart/Blockly is documented.
  Evidence: `docs/specs/16-ladder-profile-trust.md:98`.
- [x] PLCopen LD interop subset and diagnostics behavior are documented.
  Evidence: `docs/specs/16-ladder-profile-trust.md:119`, `docs/specs/16-ladder-profile-trust.md:127`.
- [x] Profile limitations are linked to `docs/IEC_DEVIATIONS.md`.
  Evidence: `docs/specs/16-ladder-profile-trust.md:141`.
- [x] Primary verification tests are listed.
  Evidence: `docs/specs/16-ladder-profile-trust.md:146`.

## 4. Coverage and Traceability

Target files:
- `docs/specs/coverage/ld-coverage.md`
- `docs/specs/coverage/iec-table-test-map.toml`

- [x] IEC anchors map to spec sections and concrete tests.
  Evidence: `docs/specs/coverage/iec-table-test-map.toml:53`, `docs/specs/coverage/ld-coverage.md:13`.
- [x] Each major LD semantic area has at least one automated test anchor.
  Evidence: `docs/specs/coverage/ld-coverage.md:13`, `docs/specs/coverage/ld-coverage.md:20`, `docs/specs/coverage/ld-coverage.md:22`.
- [x] Profile-limited areas are marked as `Profile-limited`.
  Evidence: `docs/specs/coverage/ld-coverage.md:18`, `docs/specs/coverage/ld-coverage.md:21`.
- [x] Manual-only checks (for example visual parity) have explicit status and evidence.
  Evidence: `docs/specs/coverage/ld-coverage.md:24`, `docs/internal/testing/evidence/ld-runtime-pane-parity-2026-02-27.md`.
- [x] Coverage docs avoid claiming implementation beyond tested/profile scope.
  Evidence: `docs/specs/coverage/ld-coverage.md:7`, `docs/specs/coverage/ld-coverage.md:9`.

## 5. Runtime and Editor Parity Verification

- [x] Shared runtime control contract is used by Ladder/Statechart/Blockly.
  Evidence: `docs/specs/16-ladder-profile-trust.md:100`, `editors/vscode/src/test/suite/visual-runtime-controller.test.ts:9`.
- [x] Right pane is resizable with persisted width behavior.
  Evidence: `editors/vscode/src/test/suite/visual-right-pane-resize.test.ts:22`, `editors/vscode/src/test/suite/visual-right-pane-resize.test.ts:55`.
- [x] Runtime panel bridge mode/state mapping is verified.
  Evidence: `editors/vscode/src/test/suite/visual-runtime-panel-bridge.test.ts:43`, `editors/vscode/src/test/suite/visual-runtime-panel-bridge.test.ts:50`.
- [x] Manual visual parity review against ST right pane is completed and documented.
  Evidence: `docs/internal/testing/evidence/ld-runtime-pane-parity-2026-02-27.md`.
- [x] Any parity gaps are tracked as unchecked and linked to issue/work item.
  Evidence: `docs/internal/testing/evidence/ld-runtime-pane-parity-2026-02-27.md` (`LD-RIGHT-PANE-PARITY-2026-02-27`, no open gaps).

## 6. LD Execution Semantics Verification

- [x] Deterministic series contact behavior verified.
  Evidence: `editors/vscode/src/test/suite/ladder-engine.test.ts:26`.
- [x] Parallel branch merge behavior verified.
  Evidence: `editors/vscode/src/test/suite/ladder-engine.test.ts:71`.
- [x] Buffered write commit boundary verified.
  Evidence: `editors/vscode/src/test/suite/ladder-engine.test.ts:137`.
- [x] Coil mode behavior verified.
  Evidence: `editors/vscode/src/test/suite/ladder-engine.test.ts:197`.
- [x] Compare/math behavior verified.
  Evidence: `editors/vscode/src/test/suite/ladder-engine.test.ts:312`.
- [x] Local/global symbol resolution and scoped fallback behavior verified.
  Evidence: `editors/vscode/src/test/suite/ladder-engine.test.ts` (`resolves local/global symbols with local-first precedence`).
- [x] Symbolic runtime write/force behavior verified.
  Evidence: `editors/vscode/src/test/suite/ladder-engine.test.ts` (`supports symbolic input force/write against declared variables`).
- [x] Timer behavior verified (`TON`, `TOF`, `TP`).
  Evidence: `editors/vscode/src/test/suite/ladder-engine.test.ts:407`, `editors/vscode/src/test/suite/ladder-engine.test.ts:500`.
- [x] Counter behavior verified (`CTU`, `CTD`, `CTUD` profile semantics).
  Evidence: `editors/vscode/src/test/suite/ladder-engine.test.ts:500`, `docs/IEC_DEVIATIONS.md:5`.
- [x] Invalid topology diagnostics verified.
  Evidence: `editors/vscode/src/test/suite/ladder-engine.test.ts:643`.
- [x] Unresolved symbol and non-assignable target diagnostics verified.
  Evidence: `editors/vscode/src/test/suite/ladder-engine.test.ts` (`emits diagnostics for unresolved symbols and non-assignable coil targets`).

## 7. Schema and Interop Verification

- [x] Schema v2 valid fixture accepted.
  Evidence: `editors/vscode/src/test/suite/ladder-schema.test.ts:25`.
- [x] Legacy schema fixture rejected with actionable message.
  Evidence: `editors/vscode/src/test/suite/ladder-schema.test.ts:36`.
- [x] PLCopen export/import roundtrip verified.
  Evidence: `editors/vscode/src/test/suite/plcopen-ld-interop.test.ts:114`.
- [x] Unsupported PLCopen construct diagnostics verified.
  Evidence: `editors/vscode/src/test/suite/plcopen-ld-interop.test.ts:129`.
- [x] Malformed PLCopen payload diagnostics verified.
  Evidence: `editors/vscode/src/test/suite/plcopen-ld-interop.test.ts:157`.

## 8. Documentation Index and Consistency

- [x] `docs/specs/README.md` includes both LD normative and profile docs.
  Evidence: `docs/specs/README.md:24`, `docs/specs/README.md:25`.
- [x] `docs/specs/README.md` usage guide references LD coverage files.
  Evidence: `docs/specs/README.md:62`.
- [x] No conflicting claims between `15-ladder-diagram.md` and `16-ladder-profile-trust.md`.
  Evidence: `docs/specs/15-ladder-diagram.md:19`, `docs/specs/15-ladder-diagram.md:195`, `docs/specs/16-ladder-profile-trust.md:3`.
- [x] Example docs are aligned with shipped behavior.
  Evidence: `examples/ladder/README.md` (updated right-pane `I/O`/`Settings`/`Tools` flow and runtime controls).

## 9. Release Gate Evidence

- [x] `just fmt`
  Evidence: `docs/internal/testing/checklists/ld-full-iec-implementation-checklist.md:83`.
- [x] `just clippy`
  Evidence: `docs/internal/testing/checklists/ld-full-iec-implementation-checklist.md:84`.
- [x] `just test`
  Evidence: `docs/internal/testing/checklists/ld-full-iec-implementation-checklist.md:85`.
- [x] `cd editors/vscode && npm run lint`
  Evidence: `docs/internal/testing/checklists/ld-full-iec-implementation-checklist.md:87`.
- [x] `cd editors/vscode && npm run compile`
  Evidence: `docs/internal/testing/checklists/ld-full-iec-implementation-checklist.md:87`.
- [x] `cd editors/vscode && npm test`
  Evidence: passed on 2026-02-27 (`96 passing`) including LD suites and snippets completion checks.

## 10. Sign-off

- Spec Owner:
- Runtime Owner:
- VS Code Owner:
- QA/Validation Owner:
- Date:
- Notes:
