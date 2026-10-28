# LD Coverage Matrix (IEC -> Spec -> Evidence)

This matrix tracks Ladder Diagram (LD) coverage against IEC 61131-3 Ed.3 anchors.

Legend:

- `Implemented`: automated evidence exists in the current test suite.
- `Profile-limited`: implemented with documented constraints; see deviations.
- `Pending manual sign-off`: behavior exists, but explicit manual parity sign-off is still required.
- `Not provided`: the truST profile has no such behavior; the evidence column says what happens instead.

Evidence names without a crate are tests of `crates/trust-visual`. Ladder executes only through its
generated Structured Text companion (`16-ladder-profile-trust.md` Section 4).

| Area | IEC Anchor | Spec Section | Evidence | Status |
| --- | --- | --- | --- | --- |
| Deterministic network/rung scan order | Section 8.2 (LD execution model) | `15-ladder-diagram.md` Sections 4, 8; `16-ladder-profile-trust.md` Section 4 | `trust-visual`: `ladder::tests::lowering::series_contacts_and_the_four_coil_kinds` (rungs in `order`), `ladder::power::tests::a_network_without_edges_is_a_series_chain_by_position` | Implemented |
| Series and parallel branch power semantics | Section 8.2 (LD networks and branching) | `15-ladder-diagram.md` Sections 4, 6 | `ladder::power::tests::parallel_legs_are_or_and_an_or_is_parenthesised_inside_and`, `ladder::tests::lowering::a_parallel_branch_feeds_its_coil_the_or_of_its_legs` | Implemented |
| Write commit boundary | Section 8.2 scan-cycle interpretation | `15-ladder-diagram.md` Section 8 | The companion writes in network order within the scan (Structured Text order); outputs are published by the runtime at the output sample (`03-variables.md` Section 5) | Profile-limited |
| Symbolic variable resolution and scope precedence | Section 6.5.2.2 + Section 8.2 operand resolution | `15-ladder-diagram.md` Section 5; `16-ladder-profile-trust.md` Section 5 | `ladder::tests::lowering::qualified_symbols_resolve_to_their_scope`, `ladder::references::tests::qualifiers_name_a_scope_in_any_case` | Implemented |
| Contact and coil variants | Section 8.2 LD element semantics | `15-ladder-diagram.md` Section 6 | `ladder::tests::lowering::series_contacts_and_the_four_coil_kinds` | Implemented |
| Timer FB invocation (`TON`, `TOF`, `TP`) | Table 46 | `15-ladder-diagram.md` Section 7 | `ladder::tests::lowering::blocks_call_their_instances_and_division_is_plain`, `a_block_on_the_left_rail_gets_constant_power`; timer behavior is the runtime's standard FBs (`08-standard-function-blocks.md`) | Implemented |
| Counter FB invocation (`CTU`, `CTD`, `CTUD`) | Table 45 | `15-ladder-diagram.md` Section 7 | `ladder::tests::lowering::counters_without_own_input_count_the_power_in_each_form`, `blocks_call_their_instances_and_division_is_plain` | Profile-limited |
| Invalid topology diagnostics | Section 8.2 deterministic execution requirements | `15-ladder-diagram.md` Section 9 | Malformed wires are refused by the schema check (`ladder::tests::members_of_the_wrong_shape_are_each_named`); a cycle or a wire from an unknown node carries no power instead of a diagnostic (`ladder::power::tests::a_node_fed_only_by_an_unknown_node_gets_no_power`) | Profile-limited |
| Unresolved/invalid operand diagnostics | Section 8.2 + diagnostics requirements | `15-ladder-diagram.md` Section 9 | `ladder::tests::lowering::every_untranslatable_operand_is_reported_with_its_element`, `an_output_that_cannot_be_written_is_reported` | Implemented |
| Schema v2 conformance and legacy rejection | Implementation profile contract | `16-ladder-profile-trust.md` Section 2 | `ladder::tests::a_legacy_program_is_refused_with_every_problem_named`, `an_unknown_enum_symbol_is_refused_not_coerced`; wiring `ladder-schema.test.ts` | Implemented |
| PLCopen LD import/export | PLCopen LD interchange | `16-ladder-profile-trust.md` Section 7 | PLCopen import skips an LD body with `PLCO216` (`25-vscode-product-contract.md` Section 11.3) | Not provided |
| Right-pane resize persistence | Editor authoring UI behavior | `16-ladder-profile-trust.md` Section 6 | `visual-right-pane-resize.test.ts` | Implemented |

## Profile Limits Tracked Elsewhere

- `docs/IEC_DEVIATIONS.md` for deliberate profile constraints.
- `docs/IEC_DECISIONS.md` for standard ambiguities and selected interpretations.
