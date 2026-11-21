# Rust signal filter and CRC

A tank's level sensor now and then gives a spike, and a Modbus slave's frame now and then arrives with
a bit changed on the line. Two function blocks written in Rust handle both: a median filter that drops
the spikes, and a CRC check that refuses a changed frame before the program takes its value. You learn
why loops, arrays and bit operations are plainer in Rust.

## What you learn

- How a Rust function block keeps state from one call to the next: `MEDIAN5`'s ring of the last five
  readings, which truST declares as the block's `VAR`.
- Why a median, not an average, filters spikes: a spike moves an average by a fifth of its size, and
  the median not at all.
- How a CRC tells a changed frame from a good one, and how the program keeps its last trusted value.

## What you need

- truST in VS Code. No Rust of your own: truST's toolchain builds the crate. No hardware.
- About 15 minutes.

## Step by step

The values below are those of the program run scan by scan: the test
`crates/trust-runtime/tests/example_rust_signal.rs` checks them.

1. In the truST view, choose **Start from example**, pick **Rust signal filter and CRC** and open the
   copy.
2. Open `rust/src/lib.rs`. `MEDIAN5` keeps the last five readings of `x` in a ring and gives their
   median, `y`, and their average, `mean`. `MODBUS_CRC` works out the CRC of a frame's bytes and
   compares it with the frame's last two.
3. Open `src/Main.st`. The simulated sensor gives the next of the twenty `Readings` every 5 scans,
   half a second: around 50.0, with a spike of 80.0 and a dip to 20.0 in every 10 s. A frame comes in
   when you write `Frame`.
4. Press **Start**. truST builds the crate with the program, and **Live Values** opens. Under
   **Outputs**, `MainInstance.Raw` (`%QD0`) changes every half second; `MainInstance.Level`
   (`%QD4`), the median, stays between 49.9 and 50.2; and so does `MainInstance.Mean` (`%QD8`), the
   average, until the spike.
5. Watch the spike: `Raw` reads 80.0 for half a second, and `Mean` jumps to 55.98 and stays near 56
   for 2.5 s, as long as the spike is among the last five readings, while `Level` stays at 50.0 and
   50.1. When `Raw` dips to 20.0, `Mean` falls to 44.02 and stays near 44, while `Level` stays at
   50.0 or 49.9.
6. Under **Inputs**, type 1 in the row of `MainInstance.Frame` (`%IW0`) and press **Write**: the frame
   comes in as it was sent. `MainInstance.FrameOk` (`%QX12.0`) is `TRUE` and `MainInstance.Register`
   (`%QW14`) is 42.
7. Write 2: the same frame comes in with one bit changed on the line. `FrameOk` is `FALSE`,
   `MainInstance.BadFrames` (`%QW16`) is 1, and `Register` keeps 42: the program does not take a
   value it cannot trust.
8. Write 1 again: `FrameOk` is `TRUE`, and `BadFrames` stays 1.

## What happened

**The median.** At 3.5 s the ring holds 50.3, 49.7, 80.0, 49.9 and 50.0. Sorted, they are 49.7,
49.9, 50.0, 50.3 and 80.0, and the median is the middle one, 50.0: the spike is at one end, and the
middle stays. Their average is 279.9 / 5 = 55.98: the spike moves it by a fifth of its size,

> average − level ≈ (spike − level) / 5 = (80 − 50) / 5 = 6

for the five readings it stays in the ring, 2.5 s. A median of five drops up to two spikes among
five readings; a third one would reach the middle.

**The ring.** The block keeps the five readings where they are and writes each new one over the
oldest, at `next`, which runs 0 to 4 and round again: nothing is moved. The first reading fills the
ring, so the filter starts at the sensor's value, not at zero. The median comes from a sorting
network of nine exchanges, which uses no index, so it holds the scan's rule that a Rust POU never
panics (specification 48 RU-14).

**The CRC.** A Modbus RTU frame ends with the CRC-16 of its other bytes, low byte first. The CRC
starts at 16#FFFF; each byte is XORed into it, and then, 8 times, the CRC is shifted right by one and
XORed with 16#A001 when the bit shifted out was 1. The frame `01 03 02 00 2A` (slave 1, function 3,
two bytes, the value 42) gives 16#9B39, which the slave sends as `39 9B`. With `2A` changed to `2B`
the frame's CRC would be 16#5BF8, not the 16#9B39 it carries: a CRC of 16 bits finds every change of
one bit, and almost every other.

**The last trusted value.** The program takes the register only from a frame whose CRC is right, and
otherwise counts the frame and keeps the value it had: a wrong value is worse than an old one.

## Try it yourself

- Change the spike in `Readings` from 80.0 to 200.0 and press **Start** again: `Mean` jumps to
  79.98, and `Level` stays where it was.
- Write 2, then 1, then 2: `BadFrames` counts 2, and `Register` keeps 42 throughout.

## Reference

| File | What it holds |
|---|---|
| `rust/src/lib.rs` | the function blocks `MEDIAN5` and `MODBUS_CRC`, and their tests |
| `rust/Cargo.toml` | the crate, with `trust-plc` at this release's version |
| `src/generated/rust_pous.st` | their declarations, which truST writes from the Rust |
| `src/Main.st` | the sensor's readings, the frame, and the outputs at `%QD0` to `%QW16` |
| `src/config.st` | the configuration: `Main` every 100 ms |

- The crate's tests, five of them with the CRC's check value 16#4B37 for "123456789", run with
  `cargo test` in `rust/` with your own Rust.
- Modbus over Serial Line, the specification and implementation guide, §6.2.2: the CRC.
- [Rust POUs](https://johannespettersson80.github.io/trust-platform/develop/rust-pous/): writing Rust
  POUs and the rules they keep.
- Specification 48, "Rust in truST projects" (`docs/specs/48-rust-pous.md`).
