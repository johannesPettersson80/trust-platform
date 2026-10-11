# A4 independent source findings and cycle implementation

Reviewed the evolving A4 tree based on 77b91381f. No builds, tests, formatting or
validators were run by this agent. The reviewer subsequently became the author of
the cycle files below; those files require a different independent reviewer.

Initial findings (source inspection, not execution):

1. Instance promotion and recursive lifetime checks omitted physical ancestors.
2. Supplied NULL parameter values were confused with omitted arguments.
3. Dynamic instance construction omitted deeper inheritance ancestors.
4. Method-static declarations were not resolved through their physical template.
5. Module statics ran before program construction/configuration overrides.
6. Unscheduled background programs were omitted from resource cycles.
7. Initial SINGLE task state was not seeded from constructed global BOOL values.

The retain-save boundary before output publication was also absent.
Root owns findings 1–5. This subtask implements 6–7 and the retain boundary in
engine/cycle.rs, engine/mod.rs and engine/services.rs. It reuses core task readiness
and stable priority sorting; it does not introduce another periodic scheduling rule.
Program invocation resolves configured root names to template IDs explicitly.

ExecutionServices defaults to no configured retain store. Claiming a store without
implementing its save hook fails closed. A successful save hook represents the
platform's documented persistence/enqueue policy, not a universal power-loss guarantee.
Snapshots match hosted retained globals/program-variable keys and exclude hidden edge
phase state and live instance/reference handles. Save failure latches a resource fault
and prevents output-image publication.

Authored, unrun regressions: core source_free_cycle and hosted
source_free_cycle_contract. They cover background execution, completed-program retain
snapshots, save failure/fail-closed defaults, a changed output withheld after failure,
initially-TRUE SINGLE behavior, and task-before-background ordering.

Startup seeding belongs after configuration actions and before module-static startup;
restart preserves the existing reset-to-false SINGLE baseline. Root is coordinating
that exact constructor boundary while correcting finding 5.

## Follow-up aggregate assignment implementation

This agent additionally authored engine/assignment.rs, policy/access/context
integration, a ReferenceContext normalization hook and shared dispatcher/call
binding call sites. Ordinary assignments recursively validate aggregate shape,
array dimensions/count, enum identity and nested runtime reference/instance
compatibility, applying scalar/string normalization without defaults. Reused the
existing scalar coercion helper solely to validate enum base range. Work/allocation
checks occur before copied aggregate data is committed. Tests are authored in
trust-runtime/tests/source_free_assignment.rs and remain unrun. These new authored
files also require a different independent reviewer. No validation was launched.

Assignment follow-up: bounded-string normalization now charges text traversal and
both temporary String/final string allocation before normalization. Shared store
composition returns Result<bool, RuntimeError>; source-free storage commits no
longer repeat normalization after the dispatcher has prepared the value. Typed
budget/deadline failures propagate as their original RuntimeError. Access aliases
also normalize only once. Added alias/subrange/union and aggregate OUT/IN_OUT cases,
plus a string-budget atomicity case. These are authored tests, not passing evidence.
