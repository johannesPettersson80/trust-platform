# Memory marker counter

You watch a counter that lives in the memory word `%MW0`, then stop the program at a breakpoint and run it
one statement at a time. It teaches when a scan reads and writes a variable that sits at an address.

## What you learn

- A variable located at `%MW0` takes the word's value when a scan starts, and the word takes the variable's
  value when the scan ends, so the count lasts from one scan to the next.
- Live Values shows the memory word and the output word while the program runs.
- A breakpoint holds the scan before a statement. The **Variables** show the values there, and **Step Over**
  runs one statement.

## What you need

- truST in VS Code. No hardware: the program runs on the Simulator, with simulated I/O.
- About 15 minutes.

## Step by step

1. In the truST side panel, choose **Start from example**, then **Memory marker counter**. Pick a folder
   under **Select destination folder** and keep the name under **New project folder name**: VS Code opens the
   copy. Press **Start** in the side panel. The status bar says `truST: Simulator running`, and **Live
   Values** opens.
2. Live Values shows `P1.Counter` (`%MW0`) under **Memory** and `P1.CounterLatched` (`%QW0`) under
   **Outputs**. The counter goes up by about 20 a second, and `P1.CounterLatched` is always one below it. In
   the run, read every half second, `%MW0` went 58, 67, 79, 88, 97 and 109 while `%QW0` showed 57, 66, 78,
   87, 96 and 108.
3. Open `src/Main.st` and click in the margin to the left of line 8, `Counter := Counter + 1;`. A red dot
   marks the breakpoint, and the program stops before that line: the editor highlights it, the status bar
   says `truST: Simulator paused`, and the **Run and Debug** view shows **Paused on breakpoint** under
   **Call Stack**. Open **Locals** under **Variables**: in the run, `Counter` was 125 and `CounterLatched`
   was 125.
4. Press **Step Over** (F10) in the debug toolbar. Line 8 runs, the scan ends, and the next scan stops at its
   first line, line 7, with **Paused on step**. **Locals** shows `Counter = 126` and `CounterLatched = 125`.
5. Click the red dot to remove the breakpoint, and press **Continue** (F5). The scans run again: in the run,
   `%MW0` went from 157 to 196 in 2 s, with `%QW0` one below it.
6. Press **Stop** in the side panel when you are done.

## What happened

**A located variable in the scan.** `src/Configuration.st` puts `P1.Counter` at `%MW0` and
`P1.CounterLatched` at `%QW0` (`VAR_CONFIG`). The declared name and the address name one location (IEC
61131-3 §6.5.5.3), and the runtime moves the value between the two at fixed points of each scan
(specification 10, 4.7):

1. the scan starts and `Counter` takes the value of `%MW0`;
2. the program runs: `CounterLatched := Counter;` copies it, and `Counter := Counter + 1;` adds one;
3. the scan ends: `%MW0` takes `Counter`, and `%QW0` takes `CounterLatched`.

In words: the program reads the count once, at the start, and the end of the scan stores the new count where
the next scan will read it. Between scans, `%QW0` holds the count that the scan started with and `%MW0` the
count it ended with.

As a formula, for scan k:

> L(k) = C(k − 1) and C(k) = C(k − 1) + 1, so between scans L(k) = C(k) − 1

where C(k) is the value of `%MW0` after scan k and L(k) the value of `%QW0` after it. The task `Cycle`
starts a scan every T = 50 ms, so the count grows by 1 / 0.05 s = 20 a second.

Worked with the run: from 58 to 109 in step 2 is 51 scans, 51 × 50 ms = 2.55 s, and the readings came
about every half second, 2.5 s apart in all. At the breakpoint, step 1 had given `Counter` 125 and
`CounterLatched := Counter;` had copied it, so both were 125. **Step Over** ran `Counter := Counter + 1;`
(126), the end of the scan wrote 126 into `%MW0` and 125 into `%QW0`, and the next scan read 126 back and
stopped before its `CounterLatched := Counter;`, so `CounterLatched` was still 125.

## Try it yourself

- Change `Counter := Counter + 1;` to `Counter := Counter + 5;` in `src/Main.st`, then press **Stop** and
  **Start**. The count grows by 5 a scan, 100 a second, and `P1.CounterLatched` stays 5 below `P1.Counter`.
- Set the breakpoint on line 7 instead. The program stops before the copy: **Locals** shows
  `CounterLatched` one below `Counter`, as Live Values did between scans.
- On the row of `P1.Counter`, type 41 under **Actions** and press **Force**. The panel says **I/O force
  active at %MW0.**, and both rows show 41: the force writes 41 into the word when the scan starts and again
  when it ends, whatever the program added. Press **Release**, and the count goes on from 41: in the run the
  next reading showed 44 and 43.

## Reference

Files:

- `src/Main.st`: the program `MainProgram`, and the program `Screens` that serves the screen.
- `src/Configuration.st`: the configuration `MarkerCounter`, with the task `Cycle` (every 50 ms) that runs
  the instance `P1`, the task `ScreenTask` (every 250 ms) that runs `Screens`, and the addresses of `P1`'s
  variables (`VAR_CONFIG`).
- `io.toml`: the simulated I/O driver.
- `runtime.toml`: the runtime's settings.
- `hmi/pages/overview.toml`: a screen with both values. It opens in the HMI Builder; press **Live** while the
  Simulator runs to see the values move. To open it in a browser, enable `[runtime.web]` in `runtime.toml`,
  and the runtime serves it at `/hmi`.
- `.vscode/launch.json`: the **truST Simulator** launch configuration that **Start** uses.

Signals:

| Variable | Address | Type | Meaning |
| --- | --- | --- | --- |
| `P1.Counter` | `%MW0` | INT | The count, one more each scan. |
| `P1.CounterLatched` | `%QW0` | INT | The count the scan started with. |

From the command line, with `trust-runtime` installed, the same project runs headless: `trust-runtime build
--project <folder>`, then `trust-runtime play --project <folder>`. In a second terminal, `trust-runtime ctl
--project <folder> io-write %MW0 41` writes 41 into the memory word once. The next scan latches 41 into
`%QW0` and writes 42 back into `%MW0`, and `trust-runtime ctl --project <folder> io-read` shows both.

To adapt it: give `Counter` another address in `VAR_CONFIG`, and keep a word's two bytes apart from every
other address you use (specification 11, 6.4). `INT` counts up to 32767; a count that must run for days
needs a `DINT`.
