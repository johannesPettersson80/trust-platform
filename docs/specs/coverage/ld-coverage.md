# LD Coverage Matrix (IEC -> Spec -> Evidence)

This matrix tracks Ladder Diagram (LD) coverage against IEC 61131-3 Ed.3 anchors.

Legend:

- `Implemented`: automated evidence exists in the current test suite.
- `Profile-limited`: implemented with documented constraints; see deviations.
- `Pending manual sign-off`: behavior exists, but explicit manual parity sign-off is still required.

| Area | IEC Anchor | Spec Section | Evidence | Status |
| --- | --- | --- | --- | --- |
| Deterministic network/rung scan order | Section 8.2 (LD execution model) | `15-ladder-diagram.md` Sections 4, 8 | `ladder-engine.test.ts` -> "validates deterministic series NO/NC contact semantics" | Implemented |
| Series and parallel branch power semantics | Section 8.2 (LD networks and branching) | `15-ladder-diagram.md` Sections 4, 6 | `ladder-engine.test.ts` -> "supports parallel branch semantics via topology edges" | Implemented |
| Buffered write commit boundary | Section 8.2 scan-cycle interpretation | `15-ladder-diagram.md` Section 8 | `ladder-engine.test.ts` -> "keeps buffered write commit semantics across networks" | Implemented |
| Symbolic variable resolution and scope precedence | Section 6.5.2.2 + Section 8.2 operand resolution | `15-ladder-diagram.md` Section 5 | `ladder-engine.test.ts` -> "resolves local/global symbols with local-first precedence", "supports symbolic input force/write against declared variables" | Implemented |
| Contact and coil variants | Section 8.2 LD element semantics | `15-ladder-diagram.md` Section 6 | `ladder-engine.test.ts` -> coil/contact behavior tests | Implemented |
| Timer FB behavior (`TON`, `TOF`, `TP`) | Table 46 | `15-ladder-diagram.md` Section 7 | `ladder-engine.test.ts` -> timer behavior tests | Implemented |
| Counter FB behavior (`CTU`, `CTD`, `CTUD`) | Table 45 | `15-ladder-diagram.md` Section 7 | `ladder-engine.test.ts` -> counter behavior tests | Profile-limited |
| Invalid topology diagnostics | Section 8.2 deterministic execution requirements | `15-ladder-diagram.md` Section 9 | `ladder-engine.test.ts` -> "rejects invalid topology with actionable diagnostics" | Implemented |
| Unresolved/invalid operand diagnostics | Section 8.2 + diagnostics requirements | `15-ladder-diagram.md` Section 9 | `ladder-engine.test.ts` -> "emits diagnostics for unresolved symbols and non-assignable coil targets" | Implemented |
| Schema v2 conformance and legacy rejection | Implementation profile contract | `16-ladder-profile-trust.md` Section 2 | `ladder-schema.test.ts` | Implemented |
| PLCopen LD import/export subset | PLCopen LD interchange subset | `16-ladder-profile-trust.md` Section 7 | `plcopen-ld-interop.test.ts` | Profile-limited |
| Runtime message contract parity (visual editors) | Shared runtime control behavior for editors | `16-ladder-profile-trust.md` Section 6 | `visual-runtime-controller.test.ts`, `visual-runtime-panel-bridge.test.ts` | Implemented |
| Right-pane resize persistence | Editor runtime UI behavior | `16-ladder-profile-trust.md` Section 6 | `visual-right-pane-resize.test.ts` | Implemented |
| Visual parity with ST right pane | Product UX parity requirement | `16-ladder-profile-trust.md` Section 6 | `docs/internal/testing/evidence/ld-runtime-pane-parity-2026-02-27.md` | Implemented |

## Profile Limits Tracked Elsewhere

- `docs/IEC_DEVIATIONS.md` for deliberate profile constraints.
- `docs/IEC_DECISIONS.md` for standard ambiguities and selected interpretations.
