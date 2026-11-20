# Rust function block

Run a Structured Text program that calls a function block written in Rust: `RAMP` moves a speed
towards its setpoint at a limited rate, and you watch it climb in Live Values. You learn how Rust and
Structured Text meet in one program.

## What you learn

- How a function block is written in Rust: `#[trust_fb]` and its `#[input]` and `#[output]` fields.
- How truST declares it to Structured Text in `src/generated/rust_pous.st`.
- How Structured Text calls it as it calls its own function blocks, in the same scan.

## What you need

- truST in VS Code. No Rust of your own: truST's toolchain builds the crate. No hardware.
- About 10 minutes.

## Step by step

The values below are those of the program run scan by scan: the test
`crates/trust-runtime/tests/example_rust_function_block.rs` checks them.

1. In the truST view, choose **Start from example**, pick **Rust function block** and open the copy.
2. Open `rust/src/lib.rs`. `RAMP` is a struct with `#[trust_fb(name = "RAMP")]`: its inputs are
   `target` and `rate` (units per second, 10.0 unless the call gives another), its outputs `out` and
   `done`. Its `body` runs at each call: it moves `out` towards `target` by at most `rate` times the
   task's interval, and sets `done` once `out` has arrived.
3. Open `src/generated/rust_pous.st`. truST wrote `FUNCTION_BLOCK RAMP` there from the Rust: the same
   inputs and outputs, with `rate`'s 10.0, the standard's `EN` and `ENO`, and no body. The file is
   read-only: you change the Rust, and truST writes the declaration again when you save.
4. Open `src/Main.st`. It calls the block as any function block:
   `SpeedRamp(target := Setpoint, rate := 10.0);`, then copies `SpeedRamp.out` to `Speed` and
   `SpeedRamp.done` to `AtSpeed`. `src/config.st` runs `Main` every 100 ms.
5. Press **Start**. truST builds the crate with the program; the first build of a project takes
   longer, as it builds the SDK. The status bar shows `truST: Simulator running` and **Live Values**
   opens.
6. In Live Values, watch `MainInstance.Speed`. The task runs at the start and then every 100 ms, so
   `Speed` is 1.0 at once and climbs by 1.0 a run: 10.0 at 0.9 s, and 50.0 at 4.9 s, the 50th run,
   when `MainInstance.AtSpeed` turns `TRUE`.
7. Write 20.0 to `MainInstance.Setpoint`. `Speed` falls by 1.0 every 100 ms, `AtSpeed` is `FALSE`
   again, and three seconds later `Speed` is 20.0 and `AtSpeed` is `TRUE`.

## What happened

**The ramp.** At each call, `RAMP` may move `out` by one step: `rate × interval`, the rate in units per
second times the task's interval in seconds. Here that is 10 × 0.1 s = 1.0. From 0 to the setpoint
50.0 that takes 50 runs of the 100 ms task; the first runs at the start, so the 50th is at 4.9 s. From
50.0 down to 20.0 it takes 30 runs, three seconds. The last step lands on the setpoint exactly, and `done` turns `TRUE`.

**The interval.** The block reads the task's interval from its context, `cx.interval()`, so the same
block ramps at the same rate on a faster or slower task: on a 10 ms task each step is 0.1.

**One program.** truST compiled the Rust and the Structured Text into one program image. `RAMP` runs
inside the scan, between the statements that call it, and Live Values reads its variables as it reads
any function block's.

## Try it yourself

- In `src/Main.st`, call the block with `rate := 25.0`, and press **Start** again: `Speed` moves 2.5 a
  run and reaches 50.0 at its 20th run, 1.9 s after the start.
- In `rust/src/lib.rs`, add an output `#[output] pub moving: bool,` and set it in `body` with
  `self.moving = !self.done;`. Save: truST writes the new output into the declaration, and
  `SpeedRamp.moving` is there for `src/Main.st` to read.

## Reference

| File | What it holds |
|---|---|
| `rust/src/lib.rs` | the function block `RAMP` |
| `rust/Cargo.toml` | the crate, with `trust-plc` at this release's version |
| `src/generated/rust_pous.st` | `RAMP`'s declaration, which truST writes from the Rust |
| `src/Main.st` | the program that calls `RAMP` |
| `src/config.st` | the configuration: `Main` every 100 ms |

- [Rust POUs](https://johannespettersson80.github.io/trust-platform/develop/rust-pous/): writing Rust
  POUs and the rules they keep.
- Specification 48, "Rust in truST projects" (`docs/specs/48-rust-pous.md`).
