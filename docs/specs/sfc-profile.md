# SFC Profile

This document defines the current truST scope for Sequential Function Chart
(SFC) support.

## Scope

- IEC SFC keywords such as `STEP`, `TRANSITION`, and `ACTION` are reserved by
  the lexer.
- truST ships a visual SFC editor/profile in the public docs and editor
  workflows.
- Textual SFC body syntax is not currently part of the Structured Text parser.
- As a bounded source-analysis facility, the parser and HIR accept an
  IEC-shaped textual `ACTION name: ... END_ACTION` declaration directly in a
  `PROGRAM` or `FUNCTION_BLOCK`. This does not include steps, transitions,
  associations, qualifiers, or action-control execution.

## Textual ACTION analysis boundary

The front end retains textual action declarations so imported or partially
authored sources receive syntax, name-resolution, type, and control-flow
diagnostics. This is not a promise of textual-SFC execution:

- the required declaration shape and semantic rules are specified by
  `04-pou-declarations.md`;
- an action name is not directly callable from ordinary ST;
- the runtime compiler must reject every source containing a textual action
  declaration rather than silently discard or execute its body; and
- no step association, qualifier, activation state, or scan-cycle behavior is
  defined for textual actions.

This fail-closed boundary remains in force until a later product specification
defines and verifies textual SFC as an executable language. The optional SFC
feature is currently unsupported, so this boundary is not an IEC deviation.

## Current Authoring Model

Use the visual-editor workflow for current SFC authoring guidance:

- `docs/public/develop/visual-editors/sfc.md`

A visual SFC program (`*.sfc.json` or `*.sfc`) executes only through its
generated Structured Text companion and runtime entry
(`17-visual-editors-runtime-unification.md` §3); the companion is plain
Structured Text, so the fail-closed textual-SFC boundary above is unaffected.
The editor has no executor of its own. Its file format, structure checks,
canvas edits and companion are implemented in `crates/trust-visual/src/sfc/`
(`25-vscode-product-contract.md` §11.4).

The reserved-keyword and visual-authoring boundary is a documented truST
product profile. It is outside the ST parser's IEC behavior and is not an IEC
deviation.

## Execution of a visual SFC

The companion of a visual SFC is one `FUNCTION_BLOCK` that executes the chart
by IEC 61131-3 Ed.3 §6.7 (steps §6.7.2, transitions §6.7.3, actions §6.7.4,
rules of evolution §6.7.5). This section states how each element of the chart
maps onto that clause and the choices the clause leaves to the implementer.

### What Validate refuses

The companion is made only of a chart that the editor's Validate accepts;
saving or generating a chart with findings shows the findings and writes no
companion. Validate refuses:

- a chart without an initial step, or a network of steps (the steps joined by
  transitions) with more than one initial step (§6.7.2: each network has
  exactly one initial step);
- a step without a name, step names that are not unique, or a step whose
  flag `<name>_active` is not an identifier (the name is trimmed; names are
  compared ignoring case, as identifiers are);
- a transition between nodes that do not exist, or without a condition;
- a transition `priority` that is present and not a whole number from 0
  (Table 61 feature 2b), written as a number or as text: the editor's
  Priority field keeps its text as typed, the engine reads a text such as
  `"2"` as the number it holds, and `null` or a blank text, which an emptied
  field leaves, names no priority;
- a parallel split without at least two branches, exactly one incoming
  transition and one transition to each branch, and a parallel join without at
  least two branches, one transition from each branch and exactly one outgoing
  transition;
- an action whose name is not an identifier, whose qualifier is not one of
  Table 59, or whose qualifier is time-related (`L`, `D`, `SD`, `DS`, `SL`)
  without a duration (§6.7.4.5);
- a duration that is not a `TIME` duration literal (`T#2s`, `TIME#1m30s`,
  Table 8) or is negative; a number is a duration in milliseconds (`300` is
  `T#300ms`);
- one action with two different bodies (statements);
- two time-related associations of one action in one step: they are always
  active together, which §6.7.4.6 makes an error;
- a name the function block would declare twice (a variable, a step flag, a
  step, a step's timer `SFC_S<n>`, an action, a part of an action's control, a
  transition flag), ignoring case.

### Steps (§6.7.2, Table 54)

- Each step is an IEC step. Its activity is the Boolean `<name>_active` of the
  function block, so conditions and statements can read it. An initial step's
  flag starts `TRUE`.
- Each step whose name is an identifier is also a variable `<name>` of the
  structure type `FB_<base>_SFC_STEP` (written after the function block), with
  the members of Table 54 as IEC spells them:
  - `<name>.X` (feature 3a), the step flag: the same value as `<name>_active`,
    written after the evolution of each scan and `TRUE` from the start for an
    initial step;
  - `<name>.T` (feature 4), the step elapsed time (§6.7.2): the elapsed time
    `ET` of the step's timer `SFC_S<n>` (a `TON`, specification 08; `<n>` the
    step's position in the chart), which each scan calls first with the step's
    flag. `T` starts again when the step is entered and keeps its value while
    the step is inactive; under the TON rule of specification 08 §5 a step
    entered in one scan reads `T#0s` in the next and the task interval in the
    one after. It counts up to
    `T#24d20h31m23s647ms`, the largest `TIME` of 32-bit milliseconds, as
    CODESYS and TwinCAT measure a step's time.
- `<name>_active` stays for the programs that read it; `<name>.X` is the same
  flag. A step whose name is a keyword (such as `ON` or `STEP`) has only its
  flag: a keyword is no variable name under the IEC rules, so `<name>.X` would
  not compile.
- Errors not detected: writing a step flag (§6.7.2 error 2), and a network
  without an initial step, whose steps never become active.

### Transitions (§6.7.3, Table 61)

- A transition between two steps is an IEC transition with one preceding and
  one following step (Table 61 feature 1).
- A parallel split is the double line of a simultaneous divergence after a
  single transition (feature 4a): the transition into the split and the
  split's transitions to its branches form one IEC transition, whose preceding
  step is the source of the transition into the split and whose following
  steps are all the split's branches.
- A parallel join is the double line of a simultaneous convergence before one
  transition (feature 4c): the join's transitions from its branches and its
  outgoing transition form one IEC transition, whose preceding steps are all
  the join's branches and whose following step is the target of the outgoing
  transition; when that target is a parallel split, its following steps are
  that split's branches.
- The condition of an IEC transition is the conjunction of the conditions of
  its parts: the transition into a split, then the split's transitions in
  branch order; for a join, its transitions in branch order, then its outgoing
  transition. A condition `TRUE` (any case) or `1` is always true (§6.7.3) and
  adds nothing; any other condition is a Structured Text Boolean expression.
- Features 4b and 4d (a simultaneous divergence after a selection convergence,
  and a simultaneous convergence before a selection) are drawn with a step
  between the two, since a split has one incoming and a join one outgoing
  transition.

### Rules of evolution (§6.7.5)

Each execution of the function block (one scan of its task):

1. runs the actions (below), with the steps active at the start of the scan;
2. tests every transition against the steps active at that point: a
   transition is enabled when all its preceding steps are active, and clears
   when it is enabled and its condition is `TRUE`;
3. lets a transition that shares a preceding step with a transition before it
   in priority order clear only when that transition does not clear (a
   selection divergence, Table 61 features 2a and 2b). The priority order is
   the transitions' `priority` numbers, lowest first (feature 2b; a text
   counts as the number it holds), then the
   transitions without a number in file order, which is the order in which
   they were drawn. An IEC transition made of several parts has the lowest
   number of its parts and the file position of its first part. Every
   selection is therefore prioritized, and the error §6.7.5 names for
   non-prioritized transitions that are true together cannot arise;
4. clears all clearing transitions together: every preceding step of a
   clearing transition is deactivated, then every following step of a
   clearing transition is activated (a step that is both left and entered is
   active).

A step activated in a scan runs its actions in the next scan before its own
following transitions are tested, so every step is active for at least one
scan and a chain of true transitions advances one step per scan. Unsafe and
unreachable charts (§6.7.5, Figure 26) are not detected; a step activated
while it is active stays active.

### Actions (§6.7.4, Tables 56 to 59)

- An action is identified by its name, ignoring case: every entry with that
  name in any step is an association of that one action with that step
  (Table 57 feature 1). The entry's qualifier is the association's qualifier
  (Table 59; an entry without one is `N`), and its `time` is the duration of
  a time-related qualifier.
- The action's body is its statements in Structured Text (Table 56 feature
  3s). Entries of one action may leave the body empty, for example an `R`
  entry that resets a stored action; the entries that have one have the same.
  A body of only blanks and comments is empty. An action without statements
  still has its flags, which conditions and other actions can read.
- Actions run in chart order: in the order of their first association, with
  the steps in file order and each step's actions in order.

### Action control (§6.7.4.6, Figures 22 and 23, Table 60)

truST implements Table 60 feature 1, action control with final scan
(Figures 22 a and 23 a), and only the parts an action uses (§6.7.4.6).

- Each action has a flag structure named after the action with the action
  flag `Q` and the activation `A`. The action's statements run in a scan while
  `A` is `TRUE`, and read `<action>.Q` to know whether the action is still
  active. Transition conditions and other actions may read `<action>.Q` and
  `<action>.A` as well (truST allows the access that NOTE 4 leaves to the
  implementer); a program does not write them.
- An input of the action control is `TRUE` while at least one association
  with that qualifier is active, that is, while its step is active. Then
  (Figure 23 a):
  - `Q` is `NOT R AND (N OR S_FF.Q1 OR (L AND NOT L_TMR.Q) OR D_TMR.Q OR
    P_TRIG.Q OR SD_TMR.Q OR DS_FF.Q1 OR (SL_FF.Q1 AND NOT SL_TMR.Q))`, with
    `S_FF` = `RS(S, R)`, `L_TMR` = `TON(L, T)`, `D_TMR` = `TON(D, T)`,
    `P_TRIG` = `R_TRIG(P)`, `SD_FF` = `RS(SD, R)` timed by
    `SD_TMR` = `TON(SD_FF.Q1, T)`, `DS_TMR` = `TON(DS, T)` setting
    `DS_FF` = `RS(DS_TMR.Q, R)`, and `SL_FF` = `RS(SL, R)` timed by
    `SL_TMR` = `TON(SL_FF.Q1, T)`;
  - `A` is `Q` or a falling edge of `Q`, a rising edge of `P1` or a falling
    edge of `P0`: the statements run once more in the scan after `Q` falls
    (the final scan, with `Q` `FALSE`), and once for each `P1` or `P0` pulse
    (with `Q` `FALSE`). A `P` action therefore runs twice: once with `Q`
    `TRUE` and once in its final scan.
- A falling edge is a change from `TRUE` in the previous scan to `FALSE`, and
  the previous value at a start is `FALSE`: no action runs a final scan or a
  `P0` pulse in the first scan (`docs/IEC_DECISIONS.md`, 2026-10-26).
- Each timer times with the duration of its own association. An action with
  several associations of one time-related qualifier and different durations
  uses the duration of the first of them in chart order that is active, and
  keeps the last one while none is. A stored delay or limit (`SD`, `SL`) so
  keeps its duration after its step ends, where §6.7.4.6 sets `T` to `t#0s`
  and Table 46 NOTE leaves the effect of that change of `PT` to the
  implementer.
- Errors of §6.7.4.6 not detected while the program runs: more than one active
  time-related association of an action (on steps active together; each timer
  then uses its own association's duration), `SD` while `SL_FF.Q1`, and `SL`
  while `SD_FF.Q1`.

### What the companion declares

- `VAR`: the chart's variables; then the step flags; the flag structure of
  each action and the parts of its control (`<action>_S_FF : RS`,
  `<action>_L_TMR : TON`, `<action>_P_TRIG : R_TRIG`, …, `<action>_P0_M : BOOL`
  for the previous `P0` input, and `<action>_<qualifier>_PT : TIME` where an
  action's durations differ); and one `SFC_T<n> : BOOL` per IEC transition,
  numbered in priority order, which is `TRUE` after a scan in which the
  transition cleared.
- After the function block, when the chart has actions,
  `TYPE <function block>_ACTION : STRUCT Q : BOOL; A : BOOL; END_STRUCT;
  END_TYPE`, the type of the actions' flag structures.

## Ownership

- Lexer/token reservation: `01-lexical-elements.md`
- Textual ACTION analysis and compilation boundary:
  `04-pou-declarations.md`
- Visual-editor/runtime alignment: `17-visual-editors-runtime-unification.md`
- Execution of a visual SFC (this document) and its implementation:
  `crates/trust-visual/src/sfc/` (`chart.rs`, `generate.rs`, `validate.rs`);
  its executed tests: `crates/trust-lsp/src/handlers/commands/visual_sfc_run_tests.rs`
- User-facing authoring workflow: `docs/public/develop/visual-editors/sfc.md`
