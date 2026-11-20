# Empty simulator: your first program

You start from an empty project, write a counter into its program, and watch it count on the Simulator. It
teaches the loop you will use for every program: edit, save, Start, and watch the values in Live Values.

## What you learn

- A truST project's program lives in `src/`, and **Start** compiles it and runs it on the Simulator.
- A variable with an address (`AT %MD0`) shows in Live Values while the program runs.
- After a change you press **Stop** and **Start** again, and the Simulator runs the new program from its start.

## What you need

- truST in VS Code. No hardware: the program runs on the Simulator, with simulated I/O.
- About 10 minutes.

## Step by step

1. In the truST side panel, choose **Start from example**, then **Empty simulator**. Pick a folder under
   **Select destination folder** and keep the name: VS Code opens the copy, with no hardware to set up.
2. Open `src/Main.st`. It holds an empty program, `PROGRAM Main` and `END_PROGRAM`. Replace it with:

   ```
   PROGRAM Main
   VAR
       Count AT %MD0 : DINT;
   END_VAR
       Count := Count + 1;
   END_PROGRAM
   ```

   and save the file.
3. Press **Start** in the side panel. The status bar says `Compile passed — 2 sources, no errors.` and
   `truST: Simulator running`, and **Live Values** opens with `Main.Count` (`%MD0`) under **Memory**. It counts:
   in the run, read every half second, 301, 346, 392, 452, 497 and 542, which is 241 in 2.5 s.
4. Press **Stop**. The status bar says `truST: Simulator stopped`.
5. Change `Count := Count + 1;` to `Count := Count + 10;`, save, and press **Start** again. The count starts from
   0 again and now grows by 10 a scan: in the run 310, 910, 1360, 1810, 2410 and 2860, which is 2550 in 2.5 s,
   every reading a multiple of 10.
6. Press **Stop** when you are done.

## What happened

**The scan.** `src/config.st` runs the program `Main` in the task `MainTask` every 10 ms (`INTERVAL := T#10ms`).
Each run of the program, a scan, adds the step to `Count`, and `Count` keeps its value from one scan to the next.
**Start** compiles the program and starts a new Simulator, which begins with `Count` at 0.

As a formula, after n scans:

> Count(n) = n × s, and n = t / T

where s is the step, t the time since the program started and T = 10 ms the task's interval, so the count grows
by s / T: 100 a second with a step of 1, and 1000 with a step of 10.

Worked with the run: in 2.5 s the formula gives 2.5 s / 0.01 s = 250 scans. With a step of 1 the run counted 241
between its first reading and its last, and with a step of 10 it counted 2550, against 250 × 10 = 2500. Live
Values refreshes about twice a second, so the time between two readings is a little more or less than half a
second.

**Why `AT %MD0`.** Live Values lists the values that have an address: inputs (`%I`), outputs (`%Q`) and memory
(`%M`) (specification 25, 4.3). `AT %MD0` puts `Count` at the memory double word 0, four bytes, enough for a
`DINT`. A `DINT` counts to 2147483647, about 248 days at 100 a second; an `INT` would reach its end, 32767,
after about 5½ minutes.

## Try it yourself

- Change the task's interval to `T#100ms` in `src/config.st`, then **Stop** and **Start**: the count grows by
  10 a second with a step of 1.
- Add a second counter at `%MD4` that counts down from 1000, `Down := Down - 1;`, with `Down AT %MD4 : DINT := 1000;`:
  Live Values lists both, each at its address.

## Reference

- `src/Main.st`: the program `Main`, empty until you write it.
- `src/config.st`: the configuration `Config`, with the resource `MainRes`, the task `MainTask` (every 10 ms) and
  the program instance `Main`.
- `io.toml`: the simulated I/O driver.
- `runtime.toml`: the runtime's settings: the resource `MainRes` with its 10 ms cycle.

From here, the [conveyor demo](../conveyor/README.md) teaches the scan cycle with inputs and outputs, and the
[tutorials](../tutorials/README.md) teach Structured Text from its first lines.
