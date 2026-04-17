# OSCAT BASIC For truST

This package is the first executable OSCAT BASIC compatibility slice for truST.

Source layout now mirrors the OSCAT BASIC manual chapters under `src/`, and the
runtime conformance fixtures mirror the same chapter structure under
`crates/trust-runtime/tests/fixtures/oscat_basic/*/src/`. New porting work
should add tests to the matching chapter first and then land implementation in
that same chapter directory.

Current scope:

- chapter 3 data types including `CALENDAR`, `COMPLEX`,
  `CONSTANTS_LOCATION`, `CONSTANTS_SETUP`, `ESR_DATA`, `FRACTION`,
  `HOLIDAY_DATA`, `REAL2`, `SDT`, `TIMER_EVENT`, `VECTOR_3`, and the shared
  `CONSTANTS_*` carrier records
- shared OSCAT-style `MATH` and `PHYS` constants, loaded by `OSCAT_BASIC_Constants()`
- shared `LANGUAGE` direction tables with live `LANGUAGE.DIRS[...]` access
- chapter 4 helper surface `STATUS_TO_ESR`, `OSCAT_VERSION`, `ESR_COLLECT`,
  `ESR_MON_B8`, `ESR_MON_R4`, and `ESR_MON_X8`
- chapter 6 `Arrays` surface `_ARRAY_ABS`, `_ARRAY_ADD`, `_ARRAY_INIT`,
  `_ARRAY_MEDIAN`, `_ARRAY_MUL`, `_ARRAY_SHUFFLE`, `_ARRAY_SORT`,
  `ARRAY_AVG`, `ARRAY_GAV`, `ARRAY_HAV`, `ARRAY_MAX`, `ARRAY_MIN`,
  `ARRAY_SDV`, `ARRAY_SPR`, `ARRAY_SUM`, `ARRAY_TREND`, `ARRAY_VAR`, and
  `IS_SORTED`
- engineering conversion helpers, direction helpers, and range helpers
- time conversion helpers `DAY_TO_TIME`, `HOUR_TO_TIME`, `MINUTE_TO_TIME`, and `SECOND_TO_TIME`
- calendar/date helpers such as `DATE_ADD`, `DAYS_DELTA`, `EASTER`, `MONTH_BEGIN`, `WORK_WEEK`, and `YEAR_OF_DATE`
- date-time component helpers `HOUR_OF_DT`, `MINUTE_OF_DT`, `SECOND_OF_DT`, plus TOD/DT construction helpers `HOUR`, `MINUTE`, `SECOND`, `HOUR_TO_TOD`, `SET_TOD`, and `SET_DT`
- string/date-label helpers `MONTH_TO_STRING`, `WEEKDAY_TO_STRING`, and `DT_TO_STRF`
- string formatting, bit/hex rendering, binary/hex/octal decoding, formatted-string parsing, mirror/replacement, HTML-name/ticker, expression/message helpers, cleanup, character-code, decimal-decoder, case-conversion, predicate, and search helpers/FBs such as `CHR_TO_STRING`, `FILL`, `FIX`, `REAL_TO_STRF`, `DWORD_TO_STRF`, `BYTE_TO_STRB`, `BYTE_TO_STRH`, `DWORD_TO_STRB`, `DWORD_TO_STRH`, `BIN_TO_BYTE`, `BIN_TO_DWORD`, `HEX_TO_BYTE`, `HEX_TO_DWORD`, `OCT_TO_BYTE`, `OCT_TO_DWORD`, `FLOAT_TO_REAL`, `FSTRING_TO_BYTE`, `FSTRING_TO_DWORD`, `FSTRING_TO_DT`, `FSTRING_TO_MONTH`, `FSTRING_TO_WEEK`, `FSTRING_TO_WEEKDAY`, `MIRROR`, `REPLACE_ALL`, `REPLACE_CHARS`, `REPLACE_UML`, `CHARCODE`, `CHARNAME`, `TICKER`, `EXEC`, `MESSAGE_4R`, `MESSAGE_8`, `CAPITALIZE`, `CLEAN`, `CODE`, `DEC_TO_BYTE`, `DEC_TO_DWORD`, `DEC_TO_INT`, `DEL_CHARS`, `TO_UML`, `TRIM`, `TRIM1`, `TRIME`, `UPPERCASE`, `IS_ALPHA`, and `FINDP`
- logic helpers and FBs including `LTCH`, `LTCH_4`, `STORE_8`, `COUNT_BR`, `COUNT_DR`, `TOGGLE`, `FF_D2E`, `FF_D4E`, `FF_DRE`, `FF_JKE`, `FF_RSE`, `SELECT_8`, `SHR_4E`, `SHR_4UDE`, `SHR_8PLE`, `SHR_8UDE`, `DELAY`, the full current gate-logic helper surface (`DEC_*`, `MUX_*`, `BIT_*`, `BYTE_*`, `WORD_*`, `DWORD_*`, `SHL1`, `SHR1`, `SWAP_*`, `REAL_TO_DW`, `DW_TO_REAL`, `CHK_REAL`, `REFLECT`, `REVERSE`), the full current logic-generator surface (`A_TRIG`, `B_TRIG`, `D_TRIG`, `CLICK_CNT`, `CLICK_DEC`, `CLK_DIV`, `CLK_N`, `CLK_PULSE`, `CYCLE_4`, `GEN_BIT`, `GEN_SQ`, `SCHEDULER`, `SCHEDULER_2`, `SEQUENCE_4`, `SEQUENCE_8`, `TONOF`, `TP_X`), logic memory FBs `FIFO_16`, `FIFO_32`, `STACK_16`, `STACK_32`, and the current logic-others slice `CRC_GEN`, `MATRIX`, `PIN_CODE`
- buffer/list helpers including `_BUFFER_CLEAR`, `_BUFFER_INIT`, `_BUFFER_INSERT`, `_BUFFER_UPPERCASE`, `_STRING_TO_BUFFER`, `BUFFER_COMP`, `BUFFER_SEARCH`, `BUFFER_TO_STRING`, `LIST_ADD`, `LIST_CLEAN`, `LIST_GET`, `LIST_INSERT`, `LIST_LEN`, `LIST_NEXT`, `LIST_RETRIEVE`, and `LIST_RETRIEVE_LAST`
- the complete Chapter 5 `Mathematics` surface plus the current linear/polynomial/ramp, geometry, stateful averaging, and double-precision slices: Chapter 5 helpers now include the shipped inverse/error/distribution helpers (`AGDF`, `ERF`, `ERFC`, `GAUSS`, `GAUSSCD`), sequence/fraction helpers (`EXPN`, `FACT`, `FIB`, `GCD`, `REAL_TO_FRAC`), transcendental helpers (`GDF`, `GOLD`, `LAMBERT_W`, `LANGEVIN`, `SIGMOID`, `SINC`, `SQRTN`, `TANC`), rounding/random/window helpers (`RND`, `ROUND`, `RDM`, `RDM2`, `RDMDW`, `WINDOW`, `WINDOW2`), and the previously shipped linear/polynomial/ramp, geometry, and `REAL2` helpers `F_LIN`, `F_LIN2`, `F_POLY`, `F_POWER`, `F_QUAD`, `FRMP_B`, `FT_AVG`, `CIRCLE_A`, `CIRCLE_C`, `CIRCLE_SEG`, `CONE_V`, `ELLIPSE_A`, `ELLIPSE_C`, `SPHERE_V`, `TRIANGLE_A`, `R2_SET`, `R2_ABS`, `R2_ADD`, `R2_ADD2`, and `R2_MUL`
- OSCAT clock helpers `T_PLC_MS()` and `T_PLC_US()`
- unit-conversion function blocks `ENERGY`, `LENGTH`, `PRESSURE`, `SPEED`, and `TEMPERATURE`

User-facing reference material for the shipped surface lives in:

- `docs/guides/OSCAT_BASIC_LIBRARY_GUIDE.md`

Reference consumer walkthrough:

- `examples/oscat_basic_smoke/README.md`

Upstream reference source for this package lives in:

- `docs/internal/references/OSCAT/OSCAT_BASIC/upstream/oscat_basic_333.txt`
- `docs/internal/references/OSCAT/OSCAT_BASIC/manuals/oscat_basic333_en.pdf`
- `docs/internal/references/OSCAT/OSCAT_BASIC/license/oscat_license_agreement.html`

Conformance consumers for the currently shipped surface live under:

- `crates/trust-runtime/tests/fixtures/oscat_basic/core`
- `crates/trust-runtime/tests/fixtures/oscat_basic/negative_public_surface`

This is an incremental port, not the full OSCAT BASIC catalog.

IEC compatibility note:

- The shipped `CALENDAR` port uses `LOCAL_DT`, `LOCAL_DATE`, and `LOCAL_TOD`
  instead of upstream `LDT`, `LDATE`, and `LTOD`, because those upstream names
  collide with reserved IEC date/time keywords and truST keeps that rule
  strict.
