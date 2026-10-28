# SFC Parallel Branches Examples

This directory contains SFC (Sequential Function Chart) programs with parallel branches. They run
as their Structured Text companion, by the SFC rules of IEC 61131-3 §6.7
(`docs/specs/sfc-profile.md`, "Execution of a visual SFC").

## Examples

### 1. Simple Parallel Example (`sfc_simple_parallel.sfc`)

Two tasks run at the same time and the chart continues when both have finished.

```
Start
  │
  ↓ ready
⫸ SPLIT ⫸
  │     │
TaskA  TaskB
  │     │
  ↓ NOT ExecuteTaskA.Q    ↓ NOT ExecuteTaskB.Q
⫷ JOIN ⫷
  │
 End
```

1. **Start** sets `ready := TRUE`; its transition enters TaskA and TaskB together.
2. **TaskA**'s action `ExecuteTaskA` has the qualifier `L` with `T#1s`: it runs for one second and
   writes `task_a_running := ExecuteTaskA.Q`, which is FALSE again in its last scan.
3. **TaskB**'s action `ExecuteTaskB` is limited to `T#2s` the same way.
4. The **join** leaves both branches together when both actions have ended (`NOT ExecuteTaskA.Q`
   and `NOT ExecuteTaskB.Q`), after two seconds, and **End** sets `done := TRUE`.

### 2. Parallel Process Example (`sfc_parallel_branches_example.sfc`)

An industrial process with three concurrent operations: Heat (`L`, `T#2s`), Mix (`L`, `T#3s`)
and Polish (`L`, `T#1500ms`). Each action switches its output with its flag
(`heater_on := HeatProcess.Q;`), and each branch's transition into the join is
`NOT <action>.Q`. The join waits for Mix, the longest, and Finish follows after three seconds.

## How it runs

- Each scan of the runtime entry's 20 ms task first runs the actions of the active steps, then
  clears the transitions whose steps are active and whose conditions hold.
- A parallel split enters all its branches in the same scan; a parallel join leaves them in the
  same scan, once every branch's last step is active and the join's conditions hold.
- An action runs one more scan after it ends, with `<action>.Q` FALSE, so `out := Action.Q;`
  switches the output off again.

## Expected Behavior

### Simple Parallel Example

| Time | Active Steps | Notes |
|------|--------------|-------|
| 0.02 s | TaskA, TaskB | Start's transition enters both branches |
| 1.02 s | TaskA, TaskB | ExecuteTaskA has ended; the join waits for TaskB |
| 2.02 s | End | ExecuteTaskB has ended; the join enters End |

### Parallel Process Example

| Time | Active Steps | Notes |
|------|--------------|-------|
| 0.02 s | Prepare | Init done |
| 0.04 s | Heat, Mix, Polish | The split enters three branches |
| 1.54 s | Heat, Mix, Polish | Polish has ended |
| 2.04 s | Heat, Mix, Polish | Heat has ended |
| 3.04 s | Finish | Mix has ended; the join enters Finish |

The times are those of the 20 ms task, whose timers count the scan in which they start;
`crates/trust-lsp/src/handlers/commands/visual_sfc_run_tests.rs` runs both examples scan by scan.

## How to Use

1. Open `sfc_simple_parallel.sfc` or `sfc_parallel_branches_example.sfc`; it opens in the SFC editor.
2. **Preview ST** shows the companion; saving or **Generate ST** writes `<name>.st` and
   `<name>.visual.runtime.st` beside the file.
3. Run the runtime entry from the truST sidebar and watch the step flags (`<step>_active`) and the
   action flags (`<action>.Q`) in Live Values.

## Key Concepts

### Parallel Split (Divergence)
- **Symbol**: ⫸ (horizontal double line)
- **Behavior**: Activates **ALL** branches **simultaneously** (IEC 61131-3 Table 61 feature 4a)

### Parallel Join (Convergence)
- **Symbol**: ⫷ (horizontal double line)
- **Behavior**: Continues when **ALL** branches have reached their last step and the conditions of
  the join hold (Table 61 feature 4c)

### Contrast with Alternative Branches
- **Alternative**: Only ONE transition from a step clears: the one with the lowest `priority`
  number, otherwise the one drawn first
- **Parallel**: ALL branches run concurrently

## Troubleshooting

**Issue**: Execution doesn't continue after parallel steps
- **Cause**: The join waits for every branch's last step and its conditions
- **Solution**: Check in Live Values which branch has not reached its last step

**Issue**: Saving shows findings instead of writing the companion
- **Cause**: Validate refuses the chart (for example a split with one branch, or an `L` action
  without a duration)
- **Solution**: Run **Validate** and fix each finding

## Creating Your Own Parallel SFC

1. **Add Steps**: Click **➕ Add Step**
2. **Add Parallel Split**: Click **⫸ Parallel Split**
3. **Add Branch Steps**: Create a step for each concurrent branch
4. **Connect Split to Branches**: Draw a transition from the split to each branch step
5. **Add Parallel Join**: Click **⫷ Parallel Join**
6. **Connect Branches to Join**: Draw a transition from each branch's last step to the join
7. **Add Next Step**: Create the step after the join and connect the join to it
8. **Set Conditions**: Edit the transition conditions; a timed wait is an `L` or `D` action whose
   flag `<action>.Q` the condition reads
9. **Validate**, then run the runtime entry
