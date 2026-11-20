# Conveyor demo

You run a small conveyor program on the Simulator, watch it in Live Values, and stop the line from its stop
input. It teaches the scan cycle: the runtime runs the whole program again every 50 ms, and each run starts
from the values the last one left.

## What you learn

- A PLC program runs in cycles. Each cycle reads the inputs, runs the program once from top to bottom and
  writes the outputs, and the variables keep their values to the next cycle.
- A variable can sit at an address of the process image: an input bit (`%IX0.0`), an output bit (`%QX2.0`),
  an output word (`%QW0`) or a memory word (`%MW0`).
- In Live Values, **Force** holds an input at a value, **Release** ends the force without putting another
  value back, and **Write** sets an input once.

## What you need

- truST in VS Code. No hardware: the program runs on the Simulator, with simulated I/O.
- About 10 minutes.

## Step by step

1. In the truST side panel, choose **Start from example**, then **Conveyor demo**. Pick a folder under
   **Select destination folder** and keep the name `conveyor` under **New project folder name**: VS Code opens
   the copy. Press **Start** in the side panel. The status bar says `truST: Simulator running`, and **Live
   Values** opens.
2. Live Values lists the program's four variables under **I/O**: under **Inputs** `P1.e_stop` (`%IX0.0`),
   under **Outputs** `P1.running` (`%QX2.0`) and `P1.conveyor_speed` (`%QW0`), and under **Memory**
   `P1.part_count` (`%MW0`). `P1.running` is TRUE and `P1.e_stop` is FALSE. Watch for a few seconds: the
   speed climbs in steps of 5 and falls back to 0, and the count goes up by about one a second. In the run,
   read every half second, the speed showed 80, 35, 80, 20, 80, 20, 65, 20, 65, 5, 65 and 5, and the count
   went from 2 to 8 in 5.5 s.
3. On the row of `P1.e_stop`, click the **FALSE** button under **Actions** so that it says **TRUE**, then
   press **Force**. The row turns yellow and says **FORCED**, and the panel says **I/O force active at
   %IX0.0.** and offers **Release all forces (1)**. `P1.running` turns FALSE, `P1.conveyor_speed` drops to 0,
   and `P1.part_count` stays where it was: 8 in the run.
4. Press **Release** on the same row. The row says **LIVE** again, but `P1.e_stop` stays TRUE and the line
   stays stopped: Release ends the force and writes no value back, and nothing else drives this simulated
   input.
5. Click the **TRUE** button so that it says **FALSE**, and press **Write**. The panel says **I/O write
   queued for %IX0.0.**, `P1.e_stop` turns FALSE and `P1.running` TRUE, the speed climbs from 0 again, and
   the count goes on: from 8 to 11 in 3.5 s in the run.
6. Press **Stop** in the side panel when you are done.

## What happened

**The scan cycle.** A PLC runs its program in cycles, called scans. Each scan reads the inputs into the
process image, runs the program once from top to bottom, and writes the outputs from the process image. The
task `Cycle` in `src/Configuration.st` starts a scan every 50 ms (`INTERVAL := T#50ms`). Live Values counts
the scans in its top right corner: in the run it showed **scan #180**, 180 × 50 ms = 9 s after the program
started.

In words: the program does not wait and does not loop. It looks at its values once, changes them a little
and ends, and the next scan does the same again. `conveyor_speed` keeps its value from one scan to the next,
so `conveyor_speed := conveyor_speed + 5` adds 5 each scan. When the speed has reached 100, the next scan
counts a part and sets the speed back to 0.

The time a part takes:

> N = 100 / 5 + 1 = 21 scans, and t = N × T = 21 × 50 ms = 1.05 s

where N is the number of scans per part (20 scans bring the speed from 0 to 100, and one more counts the part
and sets the speed to 0), T the cycle time of the task, and t the time per part. The line makes
1 / 1.05 s ≈ 0.95 parts a second.

Worked with the run: at the first reading of step 2 the speed was 80, four scans before 100, so part 3 was
counted five scans later, 0.25 s after it. Part 8 is five parts after part 3, 5 × 1.05 s = 5.25 s later, at
about 0.25 s + 5.25 s = 5.5 s. The last reading, at 5.5 s, shows the count 8 and the speed 5: one scan after
the reset that counted part 8.

**Force, Release and Write** (specification 11, 6.9.2). A force writes its value into the variable on every
scan, once the inputs have been read, so the program reads TRUE whatever the input says: `running :=
NOT e_stop` gives FALSE, the `IF` takes its `ELSE` branch and sets the speed to 0, and the count, which only
changes while the line runs, holds. Release ends the force without writing another value. On real I/O the
next scan reads the wired input; on the Simulator nothing drives `%IX0.0`, so it keeps TRUE until you write
FALSE. A write is applied once, at the next scan.

## Try it yourself

- Change the step `conveyor_speed + 5` to `conveyor_speed + 10` in `src/Main.st`, then press **Stop** and
  **Start**. A part now takes 100 / 10 + 1 = 11 scans, 11 × 50 ms = 0.55 s, so the count goes up by about two
  a second.
- Instead of forcing, choose **TRUE** on `P1.e_stop` and press **Write**. The line stops in the same way, and
  the row stays **LIVE**: the written value stays because nothing else writes this input, until you write
  FALSE.
- Press **Stop** and **Start** while the count is above 0. The count starts from 0 again: the project keeps
  no values over a restart (`[runtime.retain] mode = "none"` in `runtime.toml`).

## Reference

Files:

- `src/Main.st`: the program `MainProgram`.
- `src/Configuration.st`: the configuration `ConveyorCell`, with the task `Cycle` (every 50 ms), the program
  instance `P1`, and the addresses of its variables (`VAR_CONFIG`).
- `io.toml`: the simulated I/O driver and the safe state, which sets `%QX2.0` FALSE when the runtime stops or
  faults.
- `runtime.toml`: the runtime's settings: the resource `Simulator`, control and debugging.

Signals:

| Variable | Address | Type | Meaning |
| --- | --- | --- | --- |
| `P1.e_stop` | `%IX0.0` | BOOL | The stop input: TRUE stops the line. |
| `P1.running` | `%QX2.0` | BOOL | The line runs: `NOT e_stop`. |
| `P1.conveyor_speed` | `%QW0` | INT | The speed: from 0 to 100 in steps of 5 per scan. |
| `P1.part_count` | `%MW0` | INT | The parts counted. |

`%QW0` is bytes 0 and 1 of the output image and `%QX2.0` is bit 0 of byte 2, so the two outputs share no
byte (specification 11, 6.4): writing one never changes the other.

To adapt it:

- Give the variables other addresses in `VAR_CONFIG`; keep each word's two bytes apart from every bit you use.
- To run it on real I/O, choose a driver in **Devices & Connections**, as the `raspberry-pi` example does.
- `e_stop` only stands for a stop input. A real emergency stop is a safety circuit that removes power on its
  own (IEC 60204-1), not an input that a PLC program reads.
