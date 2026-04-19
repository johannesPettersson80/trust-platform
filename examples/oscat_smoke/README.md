# OSCAT Smoke Example

Docs category: `docs/public/examples/libraries-and-motion.md`

This example is the reference consumer for `libraries/oscat`.

Use it when you want to see how a normal truST project should consume the
currently shipped OSCAT manual-aligned package through `[dependencies]`.

## What The Example Covers

The program intentionally exercises several different parts of the shipped
library surface:

- `OSCAT_BASIC_Constants()` as the one-time loader for `MATH`, `PHYS`, and
  `LANGUAGE`
- scalar engineering conversions such as `C_TO_K`, `C_TO_F`, `KMH_TO_MS`,
  `MS_TO_KMH`, `F_TO_OM`, `OM_TO_F`, `F_TO_PT`, and `PT_TO_F`
- time conversion helpers `DAY_TO_TIME`, `HOUR_TO_TIME`, `MINUTE_TO_TIME`, and
  `SECOND_TO_TIME`
- calendar/date helpers such as `DAY_OF_DATE`, `DAY_OF_YEAR`, `MONTH_BEGIN`,
  `MONTH_END`, `YEAR_END`, `DATE_ADD`, `WORK_WEEK`, `LEAP_DAY`, and `EASTER`
- date-time component helpers `HOUR_OF_DT`, `MINUTE_OF_DT`, `SECOND_OF_DT`,
  plus the TOD/DT constructor helpers `HOUR`, `MINUTE`, `SECOND`,
  `HOUR_TO_TOD`, and `SET_DT`
- string/date-label helpers, case/predicate helpers, and formatting helpers
  such as `MONTH_TO_STRING`, `WEEKDAY_TO_STRING`, `DT_TO_STRF`,
  `DWORD_TO_STRF`, `CAPITALIZE`, `CLEAN`, `CODE`, `DEC_TO_BYTE`,
  `DEC_TO_DWORD`, `DEC_TO_INT`, `BYTE_TO_STRB`, `BYTE_TO_STRH`,
  `DWORD_TO_STRB`, `DWORD_TO_STRH`, `BIN_TO_BYTE`, `BIN_TO_DWORD`,
  `HEX_TO_BYTE`, `HEX_TO_DWORD`, `OCT_TO_BYTE`, `OCT_TO_DWORD`,
  `MIRROR`, `REPLACE_ALL`, `REPLACE_CHARS`, `REPLACE_UML`,
  `CHARCODE`, `CHARNAME`, `TICKER`,
  `DEL_CHARS`, `TO_UML`, `TRIM1`, `UPPERCASE`, `IS_ALPHA`, and `FINDP`
- logic FBs `LTCH`, `COUNT_BR`, and `TOGGLE`
- direction helpers and live direction-table access through `DEG_TO_DIR`,
  `DIR_TO_DEG`, and `LANGUAGE.DIRS[...]`
- OSCAT clock helpers `T_PLC_MS()` and `T_PLC_US()`
- conversion FBs `ENERGY`, `SPEED`, and `TEMPERATURE`

This is still a smoke example, not a full catalog demo. Its job is to show the
normal package-consumer wiring and the current recommended startup pattern.

## Files To Study

- `trust-lsp.toml`: project settings and dependency on `libraries/oscat`
- `src/Configuration.st`: minimal task/program wiring
- `src/Main.st`: startup constants load plus direct helper and FB usage

## How The Example Works

### 1. Project Dependency

The project consumes the library exactly the same way an application project
would:

```toml
[dependencies]
OSCAT = { path = "../../libraries/oscat", version = "0.1.0" }
```

### 2. One-Time Constant Loading

The first line of real work is:

```st
ConstantsReady := OSCAT_BASIC_Constants();
```

That call populates the shared OSCAT-style carriers:

- `MATH`
- `PHYS`
- `LANGUAGE`

Call it before you read `LANGUAGE.DIRS` or use helpers that depend on the
carrier values, such as `F_TO_OM`, `DEG_TO_DIR`, or `DIR_TO_DEG`.

### 3. Scalar And Time Conversion Helpers

`src/Main.st` then demonstrates several pure functions:

- temperature: `C_TO_K`, `C_TO_F`
- speed: `KMH_TO_MS`, `MS_TO_KMH`, `MS_TO_BFT`
- angular frequency: `F_TO_OM`, `OM_TO_F`
- period/frequency bridge: `F_TO_PT`, `PT_TO_F`
- geographic conversion: `GEO_TO_DEG`
- duration helpers: `DAY_TO_TIME`, `HOUR_TO_TIME`, `MINUTE_TO_TIME`,
  `SECOND_TO_TIME`

Those values are stored in ordinary globals so you can inspect them in the
debugger or in a runtime snapshot.

### 4. Direction Helpers And Live Carrier Access

The example also demonstrates both forms of the current direction surface:

- helper-based lookup through `DEG_TO_DIR` and `DIR_TO_DEG`
- direct table access through `LANGUAGE.DIRS[LANGUAGE.DEFAULT, 4]`

That second form matters because the runtime/compiler work for nested
`field[index]` access was part of what unblocked the live OSCAT
`LANGUAGE.DIRS[...]` surface.

### 5. Calendar And Date-Time Helpers

The smoke example also exercises the shipped date-oriented surface:

- `DAY_OF_DATE`, `DAY_OF_YEAR`
- `MONTH_BEGIN`, `MONTH_END`, `YEAR_END`
- `DATE_ADD`, `WORK_WEEK`
- `LEAP_DAY`, `EASTER`
- `HOUR_OF_DT`, `MINUTE_OF_DT`, `SECOND_OF_DT`
- `HOUR`, `MINUTE`, `SECOND`
- `HOUR_TO_TOD`, `SET_DT`

This keeps the example aligned with the larger date/time surface now shipped in
`libraries/oscat`.

### 6. String Formatting And Cleanup Helpers

The smoke example also covers the shipped label/formatting layer:

- `MONTH_TO_STRING`, `WEEKDAY_TO_STRING`
- `DT_TO_STRF`
- `DWORD_TO_STRF`
- `CAPITALIZE`, `CLEAN`, `DEL_CHARS`
- `CODE`, `TO_UML`
- `DEC_TO_BYTE`, `DEC_TO_DWORD`, `DEC_TO_INT`
- `BYTE_TO_STRB`, `BYTE_TO_STRH`, `DWORD_TO_STRB`, `DWORD_TO_STRH`
- `BIN_TO_BYTE`, `BIN_TO_DWORD`, `HEX_TO_BYTE`, `HEX_TO_DWORD`,
  `OCT_TO_BYTE`, `OCT_TO_DWORD`
- `MIRROR`, `REPLACE_ALL`, `REPLACE_CHARS`, `REPLACE_UML`
- `CHARCODE`, `CHARNAME`, `TICKER`
- `TRIM1`
- `UPPERCASE`, `IS_ALPHA`
- `FINDP`

### 7. Logic FB Helpers

The smoke example also drives a small part of the shipped OSCAT logic surface:

- `LTCH`
- `COUNT_BR`
- `TOGGLE`

The shipped library surface is larger than the smoke example here: the current
port also includes selector/register FBs, the full current gate-logic helper
surface, and the full current generator/memory/logic-others surfaces (`A_TRIG`,
`B_TRIG`, `D_TRIG`, `CRC_GEN`, `MATRIX`, `PIN_CODE`, `FIFO_*`, `STACK_*`,
`CLICK_*`, `CLK_*`, `GEN_*`, `SCHEDULER*`, `SEQUENCE_*`, `TONOF`, `TP_X`),
the buffer/list helper surface, and the current geometry/`REAL2` math helpers.

### 8. Clock Helpers

The example reads both shipped OSCAT clock helpers:

- `T_PLC_MS()`
- `T_PLC_US()`

The current `T_PLC_US()` implementation is a compatibility shim on top of the
millisecond time bridge. It is useful for OSCAT compatibility, but it is not a
claim of true microsecond runtime precision in this phase.

### 9. Conversion Function Blocks

The example instantiates three shipped FBs:

- `ENERGY`
- `SPEED`
- `TEMPERATURE`

Like upstream OSCAT-style conversion blocks, they accept one or more input-unit
fields and emit normalized `Y*` outputs every scan.

## Build And Validate

Build the consumer example from repo root:

```bash
trust-runtime build --project examples/oscat_smoke --sources src
```

Validate the shipped OSCAT surface with the conformance fixture:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project crates/trust-runtime/tests/fixtures/oscat/core
```

The example has no hardware dependency and no custom runtime backend
requirement. It is intentionally small enough to use as a quick package-consumer
sanity check.

## Library Reference

For the current public carriers, conversion/date helpers,
clock-compatibility notes, and unit-conversion FB surface, see:

- `docs/guides/OSCAT_LIBRARY_GUIDE.md`

The library source of truth remains:

- `libraries/oscat/`

The upstream OSCAT BASIC reference material used for this port is:

- `docs/internal/references/OSCAT/OSCAT_BASIC/`
