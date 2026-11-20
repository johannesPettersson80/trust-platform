# PLCopen Motion

You run a single virtual axis with the PLCopen motion blocks and command its moves from Live Values. It teaches
how a PLCopen motion command starts: on the rising edge of its `Execute` input.

## What you learn

- `MC_Power` powers an axis, `MC_MoveAbsolute` moves it to a position, and `MC_ReadStatus` and
  `MC_ReadActualPosition` report what it does (PLCopen Motion Control, part 1).
- A motion command starts on the rising edge of `Execute`, not while it stays TRUE, and its `Done` reports the end
  of the move.
- The axis's status bits and values at their addresses, read in Live Values.

## What you need

- truST in VS Code. No hardware: the program runs on the Simulator, with a virtual axis.
- About 10 minutes.

## Step by step

1. In the truST side panel, choose **Start from example**, then **PLCopen Motion**. Pick a folder under **Select
   destination folder** and keep the name: VS Code opens the copy. Press **Start**. The status bar says
   `truST: Simulator running`, and **Live Values** opens.
2. Live Values shows the axis after its first move. Under **Outputs**, `P1.motion_demo_power_on` (`%QX0.0`),
   `P1.motion_demo_target_ok` (`%QX0.1`) and `P1.motion_demo_standstill` (`%QX0.2`) are TRUE. Under **Memory**,
   `P1.motion_demo_cycles` (`%MW0`) is 1, `P1.motion_demo_error` (`%MW2`) is 0 and `P1.motion_demo_position`
   (`%MD4`) is 25.0. The axis is powered and stands at 25.
3. On the row of `P1.motion_demo_move` (`%IX0.0`) under **Inputs**, click the **FALSE** button under **Actions**
   so that it says **TRUE**, and press **Write**. The axis moves to the other end: in the run, the position read
   0.0 and the moves 2, with the axis at its target and at standstill.
4. Press **Write** again, with **TRUE** still chosen. Nothing moves, and the moves stay at 2: the request is TRUE
   already, and only its rising edge starts a move.
5. Choose **FALSE** and press **Write**, then choose **TRUE** and press **Write** again. The axis moves back to
   25.0, and the moves count 3.
6. Press **Stop** in the side panel when you are done.

## What happened

**The axis and its commands.** A PLCopen axis is `Disabled` until `MC_Power` with `Enable` TRUE powers it; its
`Status` turns TRUE and the axis stands still (`Standstill`). A rising edge on `MC_MoveAbsolute`'s `Execute`
starts a move to `Position`: the axis is in motion until it reaches the position, `Done` turns TRUE, and it
stands still again. An `Execute` that stays TRUE starts nothing new.

In words: the program powers the axis, moves it to 25 once, and then waits. Each time the request
`P1.motion_demo_move` rises from FALSE to TRUE, an `R_TRIG` turns the edge into one pulse, and the program's step
chain (`Step` 30, 10, 20, 30) sets the new target, gives `Execute` one scan of TRUE, and counts the move when
`Done` comes.

As a formula, the target of move n is:

> x(n) = 25 − x(n − 1), with x(1) = 25

so the axis goes 25, 0, 25, 0, … . On a real drive a move from x₀ to x₁ at the velocity v takes |x₁ − x₀| / v,
here 25 / 10 = 2.5 s at the program's constant velocity of 10 per second. This example's virtual axis has no
drive behind it, so it reaches each target at once.

Worked with the run: move 1 went to x(1) = 25, so Live Values read 25.0. The request's first rising edge started
move 2 to x(2) = 25 − 25 = 0, and the reading was 0.0 with 2 moves. Writing TRUE again over TRUE was no edge, and
the reading stayed 0.0 with 2 moves. FALSE and then TRUE was a new edge, so move 3 went to x(3) = 25 − 0 = 25, read
as 25.0 with 3 moves.

## Try it yourself

- Open `src/Main.st` and set a breakpoint on `Target := REAL#25.0 - Target;`. Each Write of TRUE after FALSE stops
  the program there once. A Write of TRUE while it is TRUE does not stop it, because `Request.Q` is TRUE for one
  scan only.
- Instead of writing the request, press **Force** with **TRUE** chosen while it is FALSE: the force is a rising
  edge too, and the axis moves. **Release** leaves the request TRUE, so write FALSE before the next move.

## Reference

Files:

- `src/Main.st`: the program `Main`, with the axis, its PLCopen blocks and the step chain.
- `src/config.st`: the configuration `MotionStarter`, with the task `MainTask` (every 10 ms), the instance `P1`,
  and the addresses of its signals (`VAR_CONFIG`).
- `libraries/plcopen_motion`: the bundled PLCopen motion library, single-axis profile.
- `io.toml`: the simulated I/O driver.
- `runtime.toml`: the runtime's settings.

Signals:

| Variable | Address | Type | Meaning |
| --- | --- | --- | --- |
| `P1.motion_demo_move` | `%IX0.0` | BOOL | The request: its rising edge moves the axis to the other end. |
| `P1.motion_demo_power_on` | `%QX0.0` | BOOL | `MC_Power.Status`: the axis is powered. |
| `P1.motion_demo_target_ok` | `%QX0.1` | BOOL | The actual position is within 0.1 of the target. |
| `P1.motion_demo_standstill` | `%QX0.2` | BOOL | `MC_ReadStatus.Standstill`: the axis stands still. |
| `P1.motion_demo_cycles` | `%MW0` | INT | The moves completed. |
| `P1.motion_demo_error` | `%MW2` | INT | 0 while healthy; 1 after a block reported an error. |
| `P1.motion_demo_position` | `%MD4` | REAL | `MC_ReadActualPosition.Position`: the axis position. |

This example proves no physical drive and needs none: a real axis needs a drive and its driver, set up and
commissioned separately.
