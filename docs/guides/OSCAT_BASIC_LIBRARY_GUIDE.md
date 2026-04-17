# OSCAT BASIC Library Guide

This guide is the user-facing reference for the currently shipped OSCAT BASIC
compatibility slice in truST.

If you want a runnable consumer first, read
[`examples/oscat_basic_smoke/README.md`](../../examples/oscat_basic_smoke/README.md).
That walkthrough shows how a normal project wires the package through
`[dependencies]`, when to call `OSCAT_BASIC_Constants()`, and how to consume the
current helpers from scan-driven Structured Text.

## Package Layout

- `libraries/oscat_basic/src/03_data_types/oscat_basic_data_types.st`: shipped
  data types including `CALENDAR`, `COMPLEX`, `CONSTANTS_LOCATION`,
  `CONSTANTS_SETUP`, `ESR_DATA`, `FRACTION`, `HOLIDAY_DATA`, `REAL2`, `SDT`,
  `TIMER_EVENT`, `VECTOR_3`, and the shared `CONSTANTS_*` carrier records
- `libraries/oscat_basic/src/04_other_functions/oscat_basic_other_functions.st`:
  `OSCAT_BASIC_Constants()`, `STATUS_TO_ESR`, `OSCAT_VERSION`, `ESR_COLLECT`,
  `ESR_MON_B8`, `ESR_MON_R4`, `ESR_MON_X8`, and the shared carrier globals
- `libraries/oscat_basic/src/05_mathematics/oscat_basic_mathematics.st`:
  scalar math helpers
- `libraries/oscat_basic/src/06_arrays/oscat_basic_arrays.st`:
  Chapter 6 array mutation, ordering, and statistics helpers
- `libraries/oscat_basic/src/08_arithmetics_with_double_precision/oscat_basic_double_precision.st`:
  `R2_*` helpers
- `libraries/oscat_basic/src/09_arithmetic_functions/oscat_basic_arithmetic_functions.st`:
  linear/polynomial/ramp and averaging helpers
- `libraries/oscat_basic/src/10_geometric_functions/oscat_basic_geometric_functions.st`:
  the current geometry slice
- `libraries/oscat_basic/src/12_time_and_date/oscat_basic_time_and_date.st`:
  time, date, and calendar helpers
- `libraries/oscat_basic/src/13_string_functions/oscat_basic_string_functions.st`:
  the shipped string/date-label/formatting/search helpers
- `libraries/oscat_basic/src/14_memory_modules/oscat_basic_memory_modules.st`:
  `FIFO_*` and `STACK_*`
- `libraries/oscat_basic/src/15_pulse_generators/oscat_basic_pulse_generators.st`:
  trigger, clock, scheduler, and sequence FBs
- `libraries/oscat_basic/src/16_logic_modules/oscat_basic_logic_modules.st`:
  gate/bit helpers plus `CRC_GEN`, `MATRIX`, and `PIN_CODE`
- `libraries/oscat_basic/src/17_latches_flip_flop_and_shift_register/oscat_basic_latches_flip_flop_and_shift_register.st`:
  latch, counter, flip-flop, selector, and shift-register helpers
- `libraries/oscat_basic/src/19_signal_processing/oscat_basic_signal_processing.st`:
  range/scale helpers and `DELAY`
- `libraries/oscat_basic/src/21_measuring_modules/oscat_basic_measuring_modules.st`:
  `T_PLC_MS()` and `T_PLC_US()`
- `libraries/oscat_basic/src/22_calculations/oscat_basic_calculations.st`:
  engineering conversions and unit-conversion FBs
- `libraries/oscat_basic/src/25_buffer_management/oscat_basic_buffer_management.st`:
  byte-buffer mutation, search, and extraction helpers
- `libraries/oscat_basic/src/26_list_processing/oscat_basic_list_processing.st`:
  separator-prefixed list helpers and iterator FB
- `libraries/oscat_basic/src/<chapter>/`: empty placeholder directories also
  exist for the currently unported manual chapters so future work lands in the
  correct chapter from the start

The conformance fixtures mirror the same manual layout:

- `crates/trust-runtime/tests/fixtures/oscat_basic/core/src/<chapter>/tests.st`
- `crates/trust-runtime/tests/fixtures/oscat_basic/negative_public_surface/src/<chapter>/tests.st`

This is an incremental port, not the full OSCAT BASIC catalog.

The Chapter 3 carrier record types and the Chapter 4 carrier globals now use
the normal project-wide type catalog path in `trust-hir`; they no longer rely
on a same-file co-location workaround.

## Dependency Setup

Add the package to your project `trust-lsp.toml`:

```toml
[project]
include_paths = ["src"]
stdlib = "iec"

[dependencies]
OSCATBasic = { path = "../../libraries/oscat_basic", version = "0.1.0" }
```

## Usage Rules

1. Call `OSCAT_BASIC_Constants()` once during initialization before you read
   `MATH`, `PHYS`, or `LANGUAGE`, and before you use helpers that depend on
   those carriers such as `F_TO_OM`, `DEG_TO_DIR`, and `DIR_TO_DEG`.
2. It is safe to call `OSCAT_BASIC_Constants()` every scan if you want a simple
   guard; it only populates the carriers on the first successful call.
3. The shipped PLC clock helpers are compatibility shims:
   - `T_PLC_MS()` returns `TIME_TO_DWORD(TIME())`
   - `T_PLC_US()` returns `TIME_TO_DWORD(TIME()) * 1000`
4. `T_PLC_US()` in this phase is millisecond-derived compatibility behavior,
   not a true sub-millisecond hardware timer.
5. The current library preserves upstream OSCAT naming where practical, even
   when the names are not idiomatic IEC names.
6. `SEQUENCE_4` and `SEQUENCE_8` expose the current step as `STATE` in truST;
   upstream OSCAT uses the identifier `STEP`, but `STEP` is reserved in truST.
7. The shipped buffer helpers are typed truST ports: they operate on
   `ARRAY[*] OF BYTE` via `VAR_IN_OUT` parameters, and `CRC_GEN` accepts
   `POINTER TO ARRAY[*] OF BYTE`. Raw `POINTER TO BYTE` arithmetic remains
   outside the supported truST pointer model.
8. `CALENDAR` keeps the upstream field shape but uses IEC-compliant local field
   names `LOCAL_DT`, `LOCAL_DATE`, and `LOCAL_TOD` in place of upstream
   `LDT`, `LDATE`, and `LTOD`, because those upstream names are reserved IEC
   date/time keywords in truST.

## Shared Constants And Carriers

### Scalar globals

- `STRING_LENGTH : INT = 250`
- `LIST_LENGTH : INT = 250`

### `CONSTANTS_MATH`

Loaded into the global `MATH` carrier by `OSCAT_BASIC_Constants()`.

Fields:
- `PI`
- `PI2`
- `PI4`
- `PI05`
- `PI025`
- `PI_INV`
- `E`
- `E_INV`
- `SQ2`

### `CONSTANTS_PHYS`

Loaded into the global `PHYS` carrier by `OSCAT_BASIC_Constants()`.

Fields:
- `C`
- `E`
- `G`
- `T0`
- `RU`
- `PN`

### `CONSTANTS_LANGUAGE`

Loaded into the global `LANGUAGE` carrier by `OSCAT_BASIC_Constants()`.

Fields:
- `DEFAULT : INT`: default language table index
- `LMAX : INT`: highest shipped language table index
- `DIRS : ARRAY[1..3, 0..15] OF STRING[3]`: compass-direction lookup table

`DIRS[language, sector]` is live data in the shipped library surface, so callers
can use it directly in addition to `DEG_TO_DIR` / `DIR_TO_DEG`.

## Data Type Reference

### `CALENDAR`

Type: `STRUCT`

Important fields:
- `UTC`
- `LOCAL_DT`
- `LOCAL_DATE`
- `LOCAL_TOD`
- `YEAR`
- `MONTH`
- `DAY`
- `WEEKDAY`
- `OFFSET`
- `DST_EN`
- `DST_ON`
- `LANGUAGE`
- `SUN_RISE`
- `SUN_SET`
- `NIGHT`
- `HOLIDAY`
- `WORK_WEEK`

Compatibility note:
- Upstream OSCAT names the local date/time fields `LDT`, `LDATE`, and `LTOD`.
  truST keeps strict IEC keyword reservation, so the shipped port uses
  `LOCAL_DT`, `LOCAL_DATE`, and `LOCAL_TOD` instead.

### `OSCAT_BASIC_Constants`

Type: `FUNCTION`

Signature:

```st
OSCAT_BASIC_Constants() : BOOL
```

Behavior:
- populates `MATH`, `PHYS`, and `LANGUAGE` on first call
- returns `TRUE`
- leaves the already-loaded values in place on later calls

Usage notes:
- Call this before reading `LANGUAGE.DIRS`.
- Call this before helpers that depend on `MATH.PI2` or `PHYS.T0`.

### Chapter 4 Other Functions

| Surface | Summary |
| --- | --- |
| `STATUS_TO_ESR(status, adress, DT_in, TS)` | Builds one `ESR_DATA` record and classifies the byte-status into error (`1`), status (`2`), or debug (`3`) buckets. |
| `OSCAT_VERSION(IN)` | Returns the OSCAT BASIC version marker `333`, or the upstream release-date epoch marker when `IN = TRUE`. |
| `ESR_MON_B8` | Watches up to 8 BOOL inputs and emits up to 4 edge-change ESR records per scan. |
| `ESR_MON_R4` | Watches up to 4 REAL inputs with per-channel sensitivity thresholds and emits ESR records with the sampled REAL payload bytes. |
| `ESR_MON_X8` | Watches up to 8 BYTE status channels and filters error/status/debug events by `Mode`. |
| `ESR_COLLECT` | Merges up to 8 monitor output arrays into one rolling 32-slot ESR buffer. |

## Function Reference

### Core engineering conversions

| Function | Summary |
| --- | --- |
| `BFT_TO_MS(BFT)` | Beaufort scale to meters per second. |
| `C_TO_F(celsius)` | Celsius to Fahrenheit. |
| `C_TO_K(Celsius)` | Celsius to Kelvin using `PHYS.T0`. |
| `F_TO_C(fahrenheit)` | Fahrenheit to Celsius. |
| `F_TO_OM(F)` | Frequency in hertz to angular frequency using `MATH.PI2`. |
| `F_TO_PT(F)` | Frequency in hertz to `TIME` period through the shipped millisecond time bridge. |
| `GEO_TO_DEG(D, M, SEC)` | Degrees/minutes/seconds to decimal degrees. |
| `K_TO_C(Kelvin)` | Kelvin to Celsius using `PHYS.T0`. |
| `KMH_TO_MS(kmh)` | Kilometers per hour to meters per second. |
| `MS_TO_BFT(MS)` | Meters per second to Beaufort scale. |
| `MS_TO_KMH(ms)` | Meters per second to kilometers per hour. |
| `OM_TO_F(OM)` | Angular frequency to hertz using `MATH.PI2`. |
| `PT_TO_F(PT)` | `TIME` period to frequency in hertz through the shipped millisecond time bridge. |

### Direction and range helpers

| Function | Summary |
| --- | --- |
| `DEG_TO_DIR(DEG, N, L)` | Degrees to a compass label from `LANGUAGE.DIRS`. `N` selects 4/8/16-sector resolution; `L = 0` falls back to `LANGUAGE.DEFAULT`. |
| `DIR_TO_DEG(DIR, L)` | Compass label back to nominal degrees using the selected language table. |
| `BYTE_TO_RANGE(X, low, high)` | Map `BYTE` to a real-valued range. |
| `WORD_TO_RANGE(X, low, high)` | Map `WORD` to a real-valued range. |
| `RANGE_TO_BYTE(X, low, high)` | Clamp and scale a real-valued range into `BYTE`. |
| `RANGE_TO_WORD(X, low, high)` | Clamp and scale a real-valued range into `WORD`. |
| `SCALE(X, K, O, MX, MN)` | Linear scale with clamp. |

### Mathematical helpers

The shipped Chapter 5 `Mathematics` surface is now complete.

| Function | Summary |
| --- | --- |
| `AGDF(X)` | Inverse Gudermannian helper. |
| `ATAN2(Y, X)` | Quadrant-aware angle in radians. |
| `BETA(X, Y)` | Beta-function helper using the shipped `GAMMA` approximation. |
| `BINOM(N, K)` | Binomial coefficient (`N over K`). |
| `CAUCHY(X, T, U)` | Cauchy distribution density. |
| `CAUCHYCD(X, T, U)` | Cauchy distribution cumulative density. |
| `COTH(X)` | Hyperbolic cotangent. |
| `DIFFER(in1, in2, X)` | `TRUE` when the absolute REAL delta exceeds the threshold `X`. |
| `ERF(X)` / `ERFC(X)` | Error-function helpers. |
| `EXPN(X, N)` | Integer-power helper for `X^N`. |
| `FACT(X)` / `FIB(X)` | Factorial and Fibonacci helpers. |
| `GAUSS(X, U, SI)` / `GAUSSCD(X, U, SI)` | Gaussian density and cumulative-density helpers. |
| `GAMMA(X)` | Stirling-style approximation for the gamma function. |
| `GCD(A, B)` / `REAL_TO_FRAC(X, N)` | Greatest-common-divisor and nearest-fraction helpers. |
| `GDF(X)` / `GOLD(X)` | Gudermannian and golden-function helpers. |
| `LAMBERT_W(X)` / `LANGEVIN(X)` | Lambert-W and Langevin helpers. |
| `RDM(last)` / `RDM2(last, low, high)` / `RDMDW(last)` | OSCAT random helpers for `REAL`, bounded `INT`, and `DWORD`. |
| `RND(X, N)` / `ROUND(IN, N)` | Truncating and rounded decimal helpers. |
| `SIGMOID(X)` / `SIGN_I(IN)` / `SIGN_R(IN)` | Sigmoid and sign helpers. |
| `SINC(X)` / `SQRTN(X, N)` / `TANC(X)` | `sin(x)/x`, nth-root, and `tan(x)/x` helpers. |
| `WINDOW(low, in, high)` / `WINDOW2(low, in, high)` | Exclusive and inclusive range-window checks. |

### Array helpers

The shipped Chapter 6 `Arrays` surface is now complete.

| Function | Summary |
| --- | --- |
| `_ARRAY_ABS(PT, SIZE)` | In-place absolute value for each `REAL` array element. |
| `_ARRAY_ADD(PT, SIZE, X)` | In-place additive offset for each `REAL` array element. |
| `_ARRAY_INIT(PT, SIZE, INIT)` | Fill a `REAL` array with one value. |
| `_ARRAY_MEDIAN(PT, SIZE)` | Sort in place and return the median. |
| `_ARRAY_MUL(PT, SIZE, X)` | In-place scalar multiply for each `REAL` array element. |
| `_ARRAY_SHUFFLE(PT, SIZE)` | Randomly shuffle the elements in place. |
| `_ARRAY_SORT(PT, SIZE)` | Sort a `REAL` array in ascending order. |
| `ARRAY_AVG(PT, SIZE)` / `ARRAY_GAV(PT, SIZE)` / `ARRAY_HAV(PT, SIZE)` | Arithmetic, geometric, and harmonic means. |
| `ARRAY_MAX(PT, SIZE)` / `ARRAY_MIN(PT, SIZE)` / `ARRAY_SPR(PT, SIZE)` | Maximum, minimum, and spread. |
| `ARRAY_SDV(PT, SIZE)` / `ARRAY_VAR(PT, SIZE)` | Standard deviation and sample variance. |
| `ARRAY_SUM(PT, SIZE)` | Sum all `REAL` elements. |
| `ARRAY_TREND(PT, SIZE)` | Difference between the two half-array averages. |
| `IS_SORTED(PT, SIZE)` | `TRUE` when the array is already ascending. |

### Time conversion helpers

| Function | Summary |
| --- | --- |
| `DAY_TO_TIME(IN)` | Real-valued day count to `TIME` through the shipped millisecond time bridge. |
| `HOUR_TO_TIME(IN)` | Real-valued hour count to `TIME` through the shipped millisecond time bridge. |
| `HOUR_TO_TOD(IN)` | Real-valued hour count to `TOD` through the shipped millisecond time bridge. |
| `MINUTE_TO_TIME(IN)` | Real-valued minute count to `TIME` through the shipped millisecond time bridge. |
| `SECOND_TO_TIME(IN)` | Real-valued second count to `TIME` through the shipped millisecond time bridge. |

### Calendar and date helpers

| Function | Summary |
| --- | --- |
| `DATE_ADD(IDATE, D, W, M, Y)` | Adds day/week/month/year deltas to a date. Day/week offset is applied before month/year component adjustment. |
| `DAY_OF_DATE(IDATE)` | Days since `DATE#1970-01-01`. |
| `DAY_OF_MONTH(IDATE)` | Day component of a `DATE`. |
| `DAY_OF_YEAR(IDATE)` | Ordinal day inside the year. |
| `DAYS_DELTA(date_1, date_2)` | Signed whole-day delta from `date_1` to `date_2`. |
| `DAYS_IN_MONTH(IDATE)` | Days in the month of the input date. |
| `DAYS_IN_YEAR(IDATE)` | `365` or `366` for the input date's year. |
| `EASTER(year)` | Gregorian Easter Sunday for the given year. |
| `LEAP_DAY(IDATE)` | `TRUE` when the date is February 29 in a leap year. |
| `LEAP_OF_DATE(IDATE)` | `TRUE` when the input date falls in a leap year. |
| `LEAP_YEAR(yr)` | Leap-year predicate. |
| `MONTH_BEGIN(IDATE)` | First day of the input date's month. |
| `MONTH_END(IDATE)` | Last day of the input date's month. |
| `MONTH_OF_DATE(IDATE)` | Month component of a `DATE`. |
| `SET_DATE(YEAR, MONTH, DAY)` | Constructs a `DATE` from components. |
| `WORK_WEEK(idate)` | ISO 8601 work-week number for the input date. |
| `YEAR_BEGIN(y)` | First day of the given year. |
| `YEAR_END(y)` | Last day of the given year. |
| `YEAR_OF_DATE(IDATE)` | Year component of a `DATE`. |

### Date-time component helpers

| Function | Summary |
| --- | --- |
| `HOUR(ITOD)` | Hour component of a `TOD`. |
| `MINUTE(ITOD)` | Minute component of a `TOD`. |
| `SECOND(ITOD)` | Seconds-plus-milliseconds component of a `TOD` as `REAL`. |
| `HOUR_OF_DT(XDT)` | Hour component of a `DT`. |
| `MINUTE_OF_DT(XDT)` | Minute component of a `DT`. |
| `SECOND_OF_DT(XDT)` | Second component of a `DT`. |
| `SET_TOD(HOUR, MINUTE, SECOND)` | Builds a `TOD` from components via the shipped time helpers. |
| `SET_DT(YEAR, MONTH, DAY, HOUR, MINUTE, SECOND)` | Builds a `DT` from date and time components. |

### String and formatting helpers

| Function | Summary |
| --- | --- |
| `MONTH_TO_STRING(MTH, LANG, LX)` | Month name lookup from the shipped `LANGUAGE.MONTHS` / `MONTHS3` tables. |
| `WEEKDAY_TO_STRING(WDAY, LANG, LX)` | Weekday name lookup from the shipped `LANGUAGE.WEEKDAYS` / `WEEKDAYS2` tables. |
| `DT_TO_STRF(DTI, MS, FMT, LANG)` | OSCAT-style date-time formatter using `#` tokens and shipped language tables. |
| `CHR_TO_STRING(C)` | Single-byte character code to one-character `STRING`. |
| `FILL(C, L)` | Repeated fill-character string builder. |
| `FIX(STR, L, C, M)` | String pad/truncate helper; `M=0` pad right, `M=1` pad left, `M=2` center. |
| `REAL_TO_STRF(IN, N, D)` | Fixed-scale real formatter with configurable decimal separator. |
| `DWORD_TO_STRF(IN, N)` | Decimal `DWORD` formatter with left zero-padding/truncation. |
| `CAPITALIZE(STR)` | Uppercases the first character after each space boundary. |
| `CLEAN(IN, CX)` | Keeps only characters that appear in `CX`. |
| `COUNT_CHAR(STR, CHR)` | Counts occurrences of the byte-character `CHR` inside `STR`. |
| `COUNT_SUBSTRING(SEARCH, STR)` | Counts non-overlapping substring matches. |
| `CODE(STR, POS)` | Returns the byte code at 1-based position `POS`, or `0` when out of range. |
| `DEL_CHARS(IN, CX)` | Removes all characters that appear in `CX`. |
| `TO_UML(IN)` | Maps selected byte codes to ASCII digraph replacements (`Ae`, `oe`, `ss`, ...). |
| `DEC_TO_BYTE(DEC)` | Decimal text to `BYTE`, ignoring non-digit characters. |
| `DEC_TO_DWORD(DEC)` | Decimal text to `DWORD`, ignoring non-digit characters. |
| `DEC_TO_INT(DEC)` | Decimal text to `INT`, preserving a leading-minus marker before the first digit. |
| `BYTE_TO_STRB(IN)` | Renders a `BYTE` as an 8-character binary string with the high-order bit on the left. |
| `BYTE_TO_STRH(IN)` | Renders a `BYTE` as a 2-character uppercase hexadecimal string. |
| `DWORD_TO_STRB(IN)` | Renders a `DWORD` as a 32-character binary string with the high-order bit on the left. |
| `DWORD_TO_STRH(IN)` | Renders a `DWORD` as an 8-character uppercase hexadecimal string. |
| `BIN_TO_BYTE(BIN)` | Binary text to `BYTE`, ignoring non-binary separator characters. |
| `BIN_TO_DWORD(BIN)` | Binary text to `DWORD`, ignoring non-binary separator characters. |
| `HEX_TO_BYTE(HEX)` | Hexadecimal text to `BYTE`, accepting upper/lowercase digits and ignoring separators. |
| `HEX_TO_DWORD(HEX)` | Hexadecimal text to `DWORD`, accepting upper/lowercase digits and ignoring separators. |
| `OCT_TO_BYTE(OCT)` | Octal text to `BYTE`, ignoring non-octal separator characters. |
| `OCT_TO_DWORD(OCT)` | Octal text to `DWORD`, ignoring non-octal separator characters. |
| `FLOAT_TO_REAL(FLT)` | Permissive real parser that accepts `,` or `.` plus optional `e`/`E` exponent text. |
| `FSTRING_TO_BYTE(IN)` | Formatted byte parser accepting `2#`, `8#`, `16#`, or decimal text. |
| `FSTRING_TO_DWORD(IN)` | Formatted dword parser accepting `2#`, `8#`, `16#`, or decimal text. |
| `FSTRING_TO_DT(SDT, FMT)` | Parses a formatted date-time string using OSCAT-style `#` field markers. |
| `FSTRING_TO_MONTH(MTH, LANG)` | Parses a month name/abbreviation or numeric month using the loaded language tables. |
| `FSTRING_TO_WEEK(WEEK, LANG)` | Parses a comma-separated weekday list into the OSCAT weekday bitmask (`bit 6 = Monday`, `bit 0 = Sunday`). |
| `FSTRING_TO_WEEKDAY(WDAY, LANG)` | Parses a weekday abbreviation or weekday number into `1..7`. |
| `MIRROR(STR)` | Reverses the input string character order. |
| `REPLACE_ALL(STR, SRC, REP)` | Replaces every non-overlapping occurrence of `SRC` with `REP`. |
| `REPLACE_CHARS(STR, SRC, REP)` | Replaces each character found in `SRC` with the character at the same position in `REP`. |
| `REPLACE_UML(STR)` | Expands characters through the shipped `TO_UML` mapping while preserving ASCII text. |
| `CHARCODE(STR)` | Maps HTML-style entity names such as `euro` or `uuml` to the shipped byte code, or returns the byte value directly for 1-character input. |
| `CHARNAME(C)` | Maps shipped byte codes back to their HTML-style entity names, or returns the character itself when no shipped name exists. |
| `EXEC(STR)` | Evaluates a simple one-operator expression string and returns the result as text. |
| `TO_LOWER(IN)` | ASCII uppercase byte to lowercase byte helper. |
| `TO_UPPER(IN)` | ASCII lowercase byte to uppercase byte helper. |
| `LOWERCASE(STR)` | Whole-string ASCII lowercase conversion helper. |
| `UPPERCASE(STR)` | Whole-string ASCII uppercase conversion helper. |
| `ISC_ALPHA(IN)` | `TRUE` when the input byte is an ASCII letter. |
| `ISC_CTRL(IN)` | `TRUE` when the input byte is an ASCII control character (`0..31` or `127`). |
| `ISC_HEX(IN)` | `TRUE` when the input byte is an ASCII hex digit. |
| `ISC_LOWER(IN)` | `TRUE` when the input byte is an ASCII lowercase letter. |
| `ISC_NUM(IN)` | `TRUE` when the input byte is an ASCII decimal digit. |
| `ISC_UPPER(IN)` | `TRUE` when the input byte is an ASCII uppercase letter. |
| `IS_ALNUM(STR)` | `TRUE` when every character is an ASCII letter or digit and the string is non-empty. |
| `IS_ALPHA(STR)` | `TRUE` when every character is an ASCII letter and the string is non-empty. |
| `IS_CC(STR, CMP)` | `TRUE` when every character of `STR` is present in `CMP` and `STR` is non-empty. |
| `IS_CTRL(STR)` | `TRUE` when every character is an ASCII control character and the string is non-empty. |
| `IS_HEX(STR)` | `TRUE` when every character is an ASCII hex digit and the string is non-empty. |
| `IS_LOWER(STR)` | `TRUE` when every character is an ASCII lowercase letter and the string is non-empty. |
| `IS_NCC(STR, CMP)` | `TRUE` when no character from `CMP` appears in `STR`. |
| `IS_NUM(STR)` | `TRUE` when every character is an ASCII decimal digit and the string is non-empty. |
| `IS_UPPER(STR)` | `TRUE` when every character is an ASCII uppercase letter and the string is non-empty. |
| `FIND_CHAR(STR, POS)` | First position at or after `POS` containing a non-control character. |
| `FIND_CTRL(STR, POS)` | First position at or after `POS` containing an ASCII control character. |
| `FIND_NONUM(STR, POS)` | First position at or after `POS` that is not `0..9` or `.`. |
| `FIND_NUM(STR, POS)` | First position at or after `POS` that is `0..9` or `.`. |
| `FINDB(STR1, STR2)` | Right-to-left substring search returning the last matching start position. |
| `FINDB_NONUM(STR)` | Last position that is not `0..9` or `.`. |
| `FINDB_NUM(STR)` | Last position that is `0..9` or `.`. |
| `FINDP(STR, SRC, POS)` | Forward substring search starting at `POS`. |
| `TRIM(STR)` | Removes all spaces from a string. |
| `TRIM1(STR)` | Collapses repeated spaces to one and strips edges. |
| `TRIME(STR)` | Strips leading and trailing spaces. |

### Clock helpers

| Function | Summary |
| --- | --- |
| `T_PLC_MS()` | Returns the current runtime timebase as a `DWORD` number of milliseconds. |
| `T_PLC_US()` | Returns the current runtime timebase as a `DWORD` number of microseconds using the shipped millisecond-derived compatibility rule. |

### String Message FBs

| Function Block | Summary |
| --- | --- |
| `TICKER` | Scrolls a fixed-width window across a text input; `PT = T#0ms` advances one step per call for deterministic scan-driven tests. |
| `MESSAGE_4R` | Rotates across up to four message strings on clock edges / timer expiry and exposes the selected index in `MN`. |
| `MESSAGE_8` | Priority-selects one of eight message strings from `IN1..IN8`, with `IN1` highest priority. |

### Buffer Helpers

| Function | Summary |
| --- | --- |
| `_BUFFER_CLEAR(PT, SIZE)` | Clears the first `SIZE` bytes of the caller-owned buffer to `0`. |
| `_BUFFER_INIT(PT, SIZE, INIT)` | Fills the first `SIZE` bytes of the caller-owned buffer with `INIT`. |
| `_STRING_TO_BUFFER(STR, POS, PT, SIZE)` | Copies `STR` into the caller-owned buffer at `POS` and returns the next write position. |
| `_BUFFER_INSERT(STR, POS, PT, SIZE)` | Shifts the tail of the caller-owned buffer and inserts `STR` at `POS`. |
| `_BUFFER_UPPERCASE(PT, SIZE)` | Uppercases the first `SIZE` bytes of the caller-owned buffer with ASCII-only `TO_UPPER`. |
| `BUFFER_COMP(PT1, SIZE1, PT2, SIZE2, START)` | Finds the first occurrence of buffer `PT2` inside buffer `PT1` starting at `START`, or returns `-1`. |
| `BUFFER_SEARCH(PT, SIZE, STR, POS, IGN)` | Searches a caller-owned byte buffer for `STR`; when `IGN` is `TRUE`, it compares against an uppercase pattern. |
| `BUFFER_TO_STRING(PT, SIZE, START, STOP)` | Extracts a byte range from the caller-owned buffer into a `STRING`. |

### List Helpers

| Function / FB | Summary |
| --- | --- |
| `LIST_ADD(SEP, INS, LIST)` | Appends one separator-prefixed element to the list string. |
| `LIST_CLEAN(SEP, LIST)` | Removes empty elements caused by repeated separators and trims a trailing separator. |
| `LIST_GET(SEP, POS, LIST)` | Returns the 1-based element at `POS` without mutating the list. |
| `LIST_INSERT(SEP, POS, INS, LIST)` | Inserts a separator-prefixed element before the 1-based position `POS`. |
| `LIST_LEN(SEP, LIST)` | Counts how many separator-prefixed elements are present. |
| `LIST_NEXT` | Iterates one list element per call through `LEL`; `RST` restarts at the first element and `NUL` reports end-of-list. |
| `LIST_RETRIEVE(SEP, POS, LIST)` | Returns the 1-based element at `POS` and removes it from the list. |
| `LIST_RETRIEVE_LAST(SEP, LIST)` | Returns the last element and removes it from the list. |

### Logic FBs

| Function Block | Summary |
| --- | --- |
| `LTCH` | Transparent latch with asynchronous reset. |
| `LTCH_4` | Four-channel transparent latch with asynchronous reset. |
| `STORE_8` | Eight-bit latched store with set-all, one-at-a-time clear, and asynchronous reset. |
| `COUNT_BR` | Rising-edge byte counter with independent `UP` / `DN` inputs, wraparound at `MX`, and configurable step width. |
| `COUNT_DR` | Rising-edge `DWORD` counter with independent `UP` / `DN` inputs, wraparound at `MX`, and configurable step width. |
| `TOGGLE` | Toggle flip-flop that changes state on each rising `CLK` edge and clears on reset. |
| `FF_D2E` | Dual D-type flip-flop with reset and rising clock trigger. |
| `FF_D4E` | Quad D-type flip-flop with reset and rising clock trigger. |
| `FF_DRE` | D-type flip-flop with asynchronous set/reset and rising clock trigger. |
| `FF_JKE` | JK flip-flop with asynchronous set/reset and rising clock trigger. |
| `FF_RSE` | Rising-edge set/reset latch with reset priority. |
| `SELECT_8` | Eight-way one-hot selector with set, step-up, step-down, and enable outputs. |
| `SHR_4E` | Four-stage rising-edge shift register with set-all and reset. |
| `SHR_4UDE` | Four-stage shift register that can shift up or down on each rising clock edge. |
| `SHR_8PLE` | Eight-bit serial/parallel shift register with optional parallel load and configurable direction. |
| `SHR_8UDE` | Eight-stage directional shift register with set-all and reset. |
| `A_TRIG` | Real-valued change trigger that fires when `ABS(IN - last)` exceeds `RES`. |
| `B_TRIG` | One-scan trigger on both rising and falling edges. |
| `CLICK_CNT` | Multi-click detector that pulses `Q` when the input edge count matches `N` before timeout `TC`. |
| `CLICK_DEC` | Multi-click decoder that raises one of `Q0..Q3` after the timeout window closes. |
| `CLK_DIV` | Free-running divider/counter that exposes eight output bits from an internal byte counter. |
| `CLK_N` | Scan pulse generator derived from the shipped PLC clock; `N` selects the bit position used for pulse generation. |
| `CLK_PULSE` | Periodic pulse generator with optional pulse-count limit and asynchronous reset. |
| `CYCLE_4` | Four-state cyclic sequencer with optional forced start state via `SL` / `SX`. |
| `D_TRIG` | `DWORD` change trigger exposing the unsigned delta to the previous input. |
| `DELAY` | Ring-buffer delay line for `REAL` inputs with up to 32 retained samples and reset-to-current-input behavior. |
| `FIFO_16` | Sixteen-entry `DWORD` FIFO buffer. |
| `FIFO_32` | Thirty-two-entry `DWORD` FIFO buffer. |
| `GEN_BIT` | Four-lane serial pattern generator that shifts bits out of up to four source `DWORD`s. |
| `GEN_SQ` | Square-wave generator based on the shipped PLC clock. |
| `MATRIX` | Four-row matrix keypad encoder with optional release-code reporting. |
| `PIN_CODE` | Keycode-sequence matcher that pulses `TP` after a complete configured PIN match. |
| `SCHEDULER` | Four-lane time scheduler that emits one-scan enables when each lane period elapses. |
| `SCHEDULER_2` | Four-lane cycle scheduler keyed off scan counts instead of elapsed time. |
| `SEQUENCE_4` | Four-step input-driven sequencer; the truST port exposes the current step as `STATE`. |
| `SEQUENCE_8` | Eight-step input-driven sequencer; the truST port exposes the current step as `STATE`. |
| `STACK_16` | Sixteen-entry `DWORD` LIFO stack. |
| `STACK_32` | Thirty-two-entry `DWORD` LIFO stack. |
| `TONOF` | Combined on-delay/off-delay output filter with separate `T_ON` and `T_OFF`. |
| `TP_X` | Retriggerable pulse FB with elapsed-time output `ET`. |

### Logic Helpers

| Function | Summary |
| --- | --- |
| `BCDC_TO_INT(IN)` | Two-digit packed BCD byte to `INT`. |
| `BIT_COUNT(IN)` | Counts the number of set bits in a `DWORD`. |
| `BIT_LOAD_B(IN, VAL, POS)` | Sets or clears one `BYTE` bit at `POS`. |
| `BIT_LOAD_B2(I, D, P, N)` | Sets or clears `N` consecutive `BYTE` bits starting at `P`, wrapping inside the byte. |
| `BIT_LOAD_DW(IN, VAL, POS)` | Sets or clears one `DWORD` bit at `POS`. |
| `BIT_LOAD_DW2(I, D, P, N)` | Sets or clears `N` consecutive `DWORD` bits starting at `P`, wrapping inside the word. |
| `BIT_LOAD_W(IN, VAL, POS)` | Sets or clears one `WORD` bit at `POS`. |
| `BIT_LOAD_W2(I, D, P, N)` | Sets or clears `N` consecutive `WORD` bits starting at `P`, wrapping inside the word. |
| `BIT_OF_DWORD(IN, N)` | Extracts bit `N` from a `DWORD`. |
| `BIT_TOGGLE_B(IN, POS)` | Toggles a `BYTE` bit at `POS`. |
| `BIT_TOGGLE_DW(IN, POS)` | Toggles a `DWORD` bit at `POS`. |
| `BIT_TOGGLE_W(IN, POS)` | Toggles a `WORD` bit at `POS`. |
| `BYTE_OF_BIT(B0..B7)` | Packs eight booleans into one byte. |
| `BYTE_OF_DWORD(IN, N)` | Extracts byte `N` from a `DWORD` (`N=0` is the low byte). |
| `BYTE_TO_BITS(IN)` | FB that exposes the eight individual bits of a byte as `B0..B7`. |
| `BYTE_TO_GRAY(IN)` | Binary byte to Gray code. |
| `CHECK_PARITY(IN, P)` | Checks whether parity bit `P` matches the current even-parity requirement for `IN`. |
| `CHK_REAL(X)` | Classifies a `REAL` as normal (`00`), `+inf` (`20`), `-inf` (`40`), or `NaN` (`80`). |
| `CRC_GEN(PT, SIZE, PL, PN, INIT, REV_IN, REV_OUT, XOR_OUT)` | Generates a CRC checksum over a caller-supplied byte buffer using the configured polynomial and reflection settings. |
| `DEC_2(D, A)` | Two-way decoder. |
| `DEC_4(D, A0, A1)` | Four-way decoder. |
| `DEC_8(D, A0, A1, A2)` | Eight-way decoder. |
| `DW_TO_REAL(X)` | Bit-pattern reinterpretation wrapper from `DWORD` to `REAL` via truST `DWORD_TO_REAL`. |
| `DWORD_OF_BYTE(B3, B2, B1, B0)` | Packs four bytes into one `DWORD`. |
| `DWORD_OF_WORD(W1, W0)` | Packs two words into one `DWORD`. |
| `GRAY_TO_BYTE(IN)` | Gray code back to binary byte. |
| `INT_TO_BCDC(IN)` | `INT` to two-digit packed BCD byte. |
| `MUX_2(D0, D1, A0)` | Two-input multiplexer. |
| `MUX_4(D0, D1, D2, D3, A0, A1)` | Four-input multiplexer. |
| `PARITY(IN)` | Returns `TRUE` when the number of set bits is odd. |
| `REAL_TO_DW(X)` | Bit-pattern reinterpretation wrapper from `REAL` to `DWORD` via truST `REAL_TO_DWORD`. |
| `REFLECT(D, L)` | Reverses the lowest `L` bits of a `DWORD`, leaving higher bits in place. |
| `REVERSE(IN)` | Reverses the bit order of a byte. |
| `SHL1(IN, N)` | Left-shifts a `DWORD` and fills introduced low bits with `1`. |
| `SHR1(IN, N)` | Right-shifts a `DWORD` and fills introduced high bits with `1`. |
| `SWAP_BYTE(IN)` | Swaps the high and low bytes of a `WORD`. |
| `SWAP_BYTE2(IN)` | Reverses the byte order of a `DWORD`. |
| `WORD_OF_BYTE(B1, B0)` | Packs two bytes into one `WORD`. |
| `WORD_OF_DWORD(IN, N)` | Extracts word `N` from a `DWORD` (`N=0` is the low word). |

### Math helpers

| Function | Summary |
| --- | --- |
| `ACOSH(X)` | Inverse hyperbolic cosine. |
| `ACOTH(X)` | Inverse hyperbolic cotangent. |
| `ASINH(X)` | Inverse hyperbolic sine. |
| `ATANH(X)` | Inverse hyperbolic tangent. |
| `CEIL(X)` | Ceiling to `INT`. |
| `CEIL2(X)` | Ceiling to `DINT`. |
| `CMP(X, Y, N)` | Decimal-digit comparison helper. |
| `COSH(X)` | Hyperbolic cosine. |
| `D_TRUNC(X)` | Truncate toward zero to `DINT`. |
| `DEC1(X, N)` | Wraparound decrement helper. |
| `DEG(rad)` | Radians to degrees modulo `360`. |
| `EVEN(IN)` | `TRUE` when the input is even. |
| `EXP10(X)` | Base-10 exponential. |
| `F_LIN(X, A, B)` | Linear equation helper `A * X + B`. |
| `F_LIN2(X, X1, Y1, X2, Y2)` | Linear interpolation/extrapolation through two points. |
| `FLOOR(X)` | Floor to `INT`. |
| `FLOOR2(X)` | Floor to `DINT`. |
| `F_POLY(X, C)` | Polynomial evaluation helper for the shipped 8-coefficient OSCAT form. |
| `F_POWER(A, X, N)` | Power-law helper `A * X^N`. |
| `F_QUAD(X, A, B, C)` | Quadratic helper `(A * X + B) * X + C`. |
| `FRMP_B(START, DIR, TD, TR)` | Byte ramp helper with 0..255 saturation and millisecond-based time scaling. |
| `FT_AVG(IN, E, N, RST)` | Stateful moving-average FB over `N` retained samples with enable-gating and reset-to-current-input behavior. |
| `FRACT(X)` | Fractional part helper. |
| `HYPOT(X, Y)` | Euclidean hypotenuse. |
| `INC(X, D, M)` | Wraparound increment helper. |
| `INC1(X, N)` | Increment with reset-to-zero at `N - 1`. |
| `INC2(X, D, L, U)` | Increment inside a bounded range. |
| `INV(X)` | Reciprocal with zero guard. |
| `MAX3(IN1, IN2, IN3)` | Max of three reals. |
| `MID3(IN1, IN2, IN3)` | Median of three reals. |
| `MIN3(IN1, IN2, IN3)` | Min of three reals. |
| `MODR(IN, DIVI)` | Real-valued modulo helper. |
| `MUL_ADD(X, K, O)` | Multiply-add helper. |
| `NEGX(X)` | Negation helper. |
| `RAD(DEG)` | Degrees to radians modulo `2π`. |
| `SGN(X)` | Sign helper returning `-1`, `0`, or `1`. |
| `SINH(X)` | Hyperbolic sine. |
| `TANH(X)` | Hyperbolic tangent. |

### Geometry Helpers

| Function | Summary |
| --- | --- |
| `CIRCLE_A(RX, AX)` | Circle-sector area for radius `RX` and angle `AX` in degrees. |
| `CIRCLE_C(RX, AX)` | Circle-arc length for radius `RX` and angle `AX` in degrees. |
| `CIRCLE_SEG(RX, HX)` | Circular-segment area from radius `RX` and segment height `HX`. |
| `CONE_V(RX, HX)` | Cone volume. |
| `ELLIPSE_A(R1, R2)` | Ellipse area from semi-axes `R1` and `R2`. |
| `ELLIPSE_C(R1, R2)` | Ellipse circumference approximation from semi-axes `R1` and `R2`. |
| `SPHERE_V(RX)` | Sphere volume. |
| `TRIANGLE_A(S1, A, S2, S3)` | Triangle area either from three sides (`A = 0`) or from `S1`, `S2`, and included angle `A`. |

### Double-Precision Helpers

`REAL2` is the shipped two-field carrier used by the current double-precision
helpers:

```st
TYPE REAL2 :
STRUCT
    R1 : REAL;
    RX : REAL;
END_STRUCT
END_TYPE
```

| Function | Summary |
| --- | --- |
| `R2_SET(X)` | Constructs a `REAL2` from a plain `REAL`. |
| `R2_ABS(X)` | Absolute-value helper for `REAL2`. |
| `R2_ADD(X, Y)` | Adds a `REAL` to a `REAL2`. |
| `R2_ADD2(X, Y)` | Adds one `REAL2` to another. |
| `R2_MUL(X, Y)` | Multiplies a `REAL2` by a plain `REAL`. |

## Function Block Reference

The shipped FB surface includes both stateful helper FBs and stateless
unit-conversion FBs.

### `DELAY`

Type: `FUNCTION_BLOCK`

`VAR_INPUT`:
- `IN : REAL`
- `N : INT`
- `RST : BOOL`

`VAR_OUTPUT`:
- `OUT : REAL`

Usage notes:
- Retains up to 32 samples of `IN`.
- `RST` reloads the internal ring buffer with the current input value and
  immediately sets `OUT := IN`.
- `N = 0` acts as the zero-delay path (`OUT := IN`).

### `FT_AVG`

Type: `FUNCTION_BLOCK`

`VAR_INPUT`:
- `IN : REAL`
- `E : BOOL := TRUE`
- `N : INT := 32`
- `RST : BOOL`

`VAR_OUTPUT`:
- `AVG : REAL`

Usage notes:
- Maintains a moving average over the retained sample window.
- `RST` and first-call initialization reload the average to the current input.
- `E = FALSE` freezes the current average and does not advance the internal
  delay line.

The unit-conversion FBs listed below are stateless scan functions: they read
one or more unit inputs, normalize to a base unit, and emit `Y*` outputs every
scan.

### `ENERGY`

Type: `FUNCTION_BLOCK`

`VAR_INPUT`:
- `J : REAL`
- `C : REAL`
- `Wh : REAL`

`VAR_OUTPUT`:
- `YJ : REAL`
- `YC : REAL`
- `YWh : REAL`

Usage notes:
- Use this when you want to accept one or more upstream OSCAT energy-unit
  inputs and publish all supported outputs from a single block call.

### `LENGTH`

Type: `FUNCTION_BLOCK`

`VAR_INPUT`:
- `m : REAL`
- `p : REAL`
- `inch : REAL`
- `ft : REAL`
- `yd : REAL`
- `mile : REAL`
- `sm : REAL`
- `fm : REAL`

`VAR_OUTPUT`:
- `Ym : REAL`
- `Yp : REAL`
- `Yin : REAL`
- `Yft : REAL`
- `Yyd : REAL`
- `Ymile : REAL`
- `Ysm : REAL`
- `Yfm : REAL`

Usage notes:
- The field names intentionally match the upstream OSCAT naming instead of being
  renamed to new truST-specific aliases.

### `PRESSURE`

Type: `FUNCTION_BLOCK`

`VAR_INPUT`:
- `mws : REAL`
- `torr : REAL`
- `att : REAL`
- `atm : REAL`
- `pa : REAL`
- `bar : REAL`

`VAR_OUTPUT`:
- `Ymws : REAL`
- `Ytorr : REAL`
- `Yatt : REAL`
- `Yatm : REAL`
- `Ypa : REAL`
- `Ybar : REAL`

### `SPEED`

Type: `FUNCTION_BLOCK`

`VAR_INPUT`:
- `ms : REAL`
- `kmh : REAL`
- `kn : REAL`
- `mh : REAL`

`VAR_OUTPUT`:
- `Yms : REAL`
- `Ykmh : REAL`
- `Ykn : REAL`
- `Ymh : REAL`

### `TEMPERATURE`

Type: `FUNCTION_BLOCK`

`VAR_INPUT`:
- `K : REAL`
- `C : REAL := -273.15`
- `F : REAL := -459.67`
- `Re : REAL := -218.52`
- `Ra : REAL`

`VAR_OUTPUT`:
- `YK : REAL`
- `YC : REAL`
- `YF : REAL`
- `YRe : REAL`
- `YRa : REAL`

Usage notes:
- The shipped implementation supports the upstream omission-style defaults on
  the `C`, `F`, and `Re` inputs.

## Example Pattern

The current recommended startup pattern is:

```st
PROGRAM Main
VAR
    ConstantsReady : BOOL;
    Kelvin : REAL;
    DirectionLabel : STRING[3];
    DaySpan : TIME;
    MonthEndDate : DATE;
    WeekNumber : INT;
    HourPart : INT;
    PlcMs : DWORD;
END_VAR

ConstantsReady := OSCAT_BASIC_Constants();
Kelvin := C_TO_K(Celsius := REAL#25.0);
DirectionLabel := DEG_TO_DIR(DEG := INT#90, N := INT#1, L := INT#1);
DaySpan := DAY_TO_TIME(IN := REAL#1.5);
MonthEndDate := MONTH_END(IDATE := DATE#2024-02-29);
WeekNumber := WORK_WEEK(idate := DATE#2026-04-15);
HourPart := HOUR_OF_DT(XDT := DT#2026-04-15-13:14:15);
PlcMs := T_PLC_MS();
END_PROGRAM
```

For a fuller worked consumer, see
[`examples/oscat_basic_smoke/README.md`](../../examples/oscat_basic_smoke/README.md).
