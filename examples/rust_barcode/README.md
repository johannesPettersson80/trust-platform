# Rust barcode reader

A packing line checks each box's GS1-128 label: a function block written in Rust reads the label's
text into its product number, batch and expiry date, and the program decides whether the box goes
on. You learn why text belongs in Rust, and how a Rust block reports a label it cannot read without
stopping the scan.

## What you learn

- How a Rust function block takes and gives text and dates: `Str<64>` for `STRING[64]` and `Date`
  for `DATE`.
- How Rust reads text within the scan's rules, with no heap and no panics: `get`,
  `split_at_checked` and `strip_prefix` in place of indexing, and an error code in place of a
  fault.
- How the Structured Text program turns the block's fields into the line's decision.

## What you need

- truST in VS Code. No Rust of your own: truST's toolchain builds the crate. No hardware.
- About 15 minutes.

## Step by step

The values below are those of the program run scan by scan: the test
`crates/trust-runtime/tests/example_rust_barcode.rs` checks them.

1. In the truST view, choose **Start from example**, pick **Rust barcode reader** and open the copy.
2. Open `rust/src/lib.rs`. `GS1_LABEL` takes `code`, the scanner's text, and gives `gtin`, `batch`,
   `expiry`, `gtin_ok` and `error`. Its `parse` reads the label's fields in any order: `01` and the
   GTIN's 14 digits, `17` and the expiry date as YYMMDD, `10` and the batch, up to 20 characters.
3. Open `src/Main.st`. `Labels` holds four labels as a scanner sends them, and `Label` (`%IW0`)
   names the one the simulated scanner reads. The program calls `Read(code := Labels[Label])` and
   accepts the box when its GTIN is the line's product, `09501101530003`, the GTIN's check digit is
   right, and the expiry is not before `TODAY`, 2026-11-20.
4. Press **Start**. truST builds the crate with the program, and **Live Values** opens. Under
   **Inputs**, `MainInstance.Label` (`%IW0`) is 0: no label. Under **Outputs**,
   `MainInstance.Accepted` (`%QX0.0`), `MainInstance.Expired` (`%QX0.1`),
   `MainInstance.Unreadable` (`%QX0.2`) and `MainInstance.WrongProduct` (`%QX0.3`) are `FALSE`,
   and `MainInstance.Problem` (`%QW2`) is 0.
5. Type 1 in the row of `MainInstance.Label` and press **Write**. `Accepted` is `TRUE`: the line's
   product, in date until 2026-12-31.
6. Write 2. `Expired` is `TRUE`: the label's expiry, `250600`, has the day 00, the end of June 2025.
7. Write 3. `Unreadable` is `TRUE`, and `Problem` stays 0: the block read the label, but the GTIN's
   last digit, 4, is not its check digit, 3.
8. Write 4. `WrongProduct` is `TRUE`: the GTIN `05012345678900` is a right one, with the check digit
   0, but of another product.

## What happened

**The label's text.** A GS1-128 label holds application identifiers, each a two-digit number and
its data: `01` and a 14-digit GTIN, the product's number; `17` and the expiry date as YYMMDD; `10`
and a batch of up to 20 characters. A field of variable length, as the batch, ends with the group
separator, byte 29, when another field follows it. Label 1 is `01` `09501101530003`, `17` `261231`
and `10` `ABC123`.

**The check digit.** The GTIN's last digit checks the 13 before it. Weighted 3 and 1 from the right,
they add up to a sum, and the check digit is what brings the sum to the next multiple of ten:

> check digit = (10 − (sum mod 10)) mod 10

Worked with label 1: 0×3 + 9×1 + 5×3 + 0×1 + 1×3 + 1×1 + 0×3 + 1×1 + 5×3 + 3×1 + 0×3 + 0×1 + 0×3
= 47, so the check digit is 10 − 7 = 3, the GTIN's last. Label 3 ends in 4, so a digit was misread
or misprinted, and the program does not trust it. Label 4's sum is 90, so its check digit is 0, which
it has.

**The day 00.** GS1 lets an expiry date name its month only, with the day 00, which is the month's
last day: `250600` is 2025-06-30, before the day the line runs. The block gives whole dates, so the
program compares `Read.expiry < TODAY` as any two dates.

**A bad label never stops the scan.** A scratched label must not stop the line: the block reads with
`get`, `split_at_checked` and `strip_prefix`, which give `None` where indexing would panic, and a
label it cannot read sets `error`: 1 no text, 2 a field it does not read, 3 a field too short or
too long, 4 a letter in a number, 5 an expiry date that is no date, 6 no GTIN. The build refuses a
module that could panic (specification 48 RU-14), so this is a rule, not only style.

**Why the day is fixed.** `TODAY` is 2026-11-20, so that the example gives the same verdicts on any
day. A plant reads its controller's clock.

## Try it yourself

- Add a fifth label, `'0109501101530003219876'`, a serial number with the identifier `21`, which the
  block does not read. Make `Labels` an `ARRAY[1..5]` and the check `Label <= 5`, press **Start**
  again and write 5: `Problem` is 2 and `Unreadable` is `TRUE`. Then teach `parse` the identifier
  `21`, a field of variable length of up to 20 characters, as `10` is.
- Set a breakpoint on `Problem := Read.error;` and write 1: when the program stops there, the debugger
  shows `Read`'s fields, the GTIN, the batch and the expiry date of label 1.

## Reference

| File | What it holds |
|---|---|
| `rust/src/lib.rs` | the function block `GS1_LABEL`, its reader and its tests |
| `rust/Cargo.toml` | the crate, with `trust-plc` at this release's version |
| `src/generated/rust_pous.st` | `GS1_LABEL`'s declaration, which truST writes from the Rust |
| `src/Main.st` | the line's labels, `Label` at `%IW0`, and the verdicts at `%QX0.0` to `%QX0.3` and `%QW2` |
| `src/config.st` | the configuration: `Main` every 100 ms |

- The crate's tests, five of them with the check digit and the day 00, run with `cargo test` in
  `rust/` with your own Rust.
- [Rust POUs](https://johannespettersson80.github.io/trust-platform/develop/rust-pous/): writing Rust
  POUs and the rules they keep.
- Specification 48, "Rust in truST projects" (`docs/specs/48-rust-pous.md`).
