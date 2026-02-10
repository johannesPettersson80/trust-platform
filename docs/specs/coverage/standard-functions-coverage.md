# Standard Functions Coverage Checklist (IEC 61131-3 Ed 3, Tables 22-36, 43-46)

Use this checklist to track coverage of standard functions in trust-hir.

Legend:
- [ ] not implemented
- [x] implemented
- [~] partial

Status (current codebase; refactor-only check on 2026-01-23): All standard functions listed below are implemented in trust-hir. Runtime execution coverage is tracked in internal test checklists. As of 2026-01-30, trust-ide surfaces IEC-referenced standard function docs in hover/completion (`stdlib_docs`).

## Table 22 - Data Type Conversion Function Forms
- [x] `SRC_TO_DST` typed conversion
- [x] `TO_DST` overloaded conversion (deprecated)
- [x] `TRUNC` overloaded truncation (deprecated)
- [x] `TRUNC_DST` overloaded truncation
- [x] `SRC_TRUNC_DST` typed truncation (deprecated)
- [x] `SRC_BCD_TO_DST` typed BCD conversion
- [x] `BCD_TO_DST` overloaded BCD conversion
- [x] `DST_TO_BCD_SRC` typed BCD conversion
- [x] `TO_BCD_DST` overloaded BCD conversion

## Table 23 - Data Type Conversion of Numeric Data Types
- [x] LREAL -> {REAL, LINT, DINT, INT, SINT, ULINT, UDINT, UINT, USINT}
- [x] REAL -> {LREAL, LINT, DINT, INT, SINT, ULINT, UDINT, UINT, USINT}
- [x] LINT -> {LREAL, REAL, DINT, INT, SINT, ULINT, UDINT, UINT, USINT}
- [x] DINT -> {LREAL, REAL, LINT, INT, SINT, ULINT, UDINT, UINT, USINT}
- [x] INT -> {LREAL, REAL, LINT, DINT, SINT, ULINT, UDINT, UINT, USINT}
- [x] SINT -> {LREAL, REAL, LINT, DINT, INT, ULINT, UDINT, UINT, USINT}
- [x] ULINT -> {LREAL, REAL, LINT, DINT, INT, SINT, UDINT, UINT, USINT}
- [x] UDINT -> {LREAL, REAL, LINT, DINT, INT, SINT, ULINT, UINT, USINT}
- [x] UINT -> {LREAL, REAL, LINT, DINT, INT, SINT, ULINT, UDINT, USINT}
- [x] USINT -> {LREAL, REAL, LINT, DINT, INT, SINT, ULINT, UDINT, UINT}

## Table 24 - Data Type Conversion of Bit Data Types
- [x] BYTE <-> WORD
- [x] BYTE <-> DWORD
- [x] BYTE <-> LWORD
- [x] WORD <-> DWORD
- [x] WORD <-> LWORD
- [x] DWORD <-> LWORD

## Table 25 - Data Type Conversion of Bit and Numeric Types
- [x] LWORD_TO_LREAL
- [x] DWORD_TO_REAL
- [x] LWORD_TO_LINT
- [x] LWORD_TO_DINT
- [x] LWORD_TO_INT
- [x] LWORD_TO_SINT
- [x] LWORD_TO_ULINT
- [x] LWORD_TO_UDINT
- [x] LWORD_TO_UINT
- [x] LWORD_TO_USINT
- [x] DWORD_TO_LINT
- [x] DWORD_TO_DINT
- [x] DWORD_TO_INT
- [x] DWORD_TO_SINT
- [x] DWORD_TO_ULINT
- [x] DWORD_TO_UDINT
- [x] DWORD_TO_UINT
- [x] DWORD_TO_USINT
- [x] WORD_TO_LINT
- [x] WORD_TO_DINT
- [x] WORD_TO_INT
- [x] WORD_TO_SINT
- [x] WORD_TO_ULINT
- [x] WORD_TO_UDINT
- [x] WORD_TO_UINT
- [x] WORD_TO_USINT
- [x] BYTE_TO_LINT
- [x] BYTE_TO_DINT
- [x] BYTE_TO_INT
- [x] BYTE_TO_SINT
- [x] BYTE_TO_ULINT
- [x] BYTE_TO_UDINT
- [x] BYTE_TO_UINT
- [x] BYTE_TO_USINT
- [x] BOOL_TO_LINT
- [x] BOOL_TO_DINT
- [x] BOOL_TO_INT
- [x] BOOL_TO_SINT
- [x] BOOL_TO_ULINT
- [x] BOOL_TO_UDINT
- [x] BOOL_TO_UINT
- [x] BOOL_TO_USINT
- [x] LREAL_TO_LWORD
- [x] REAL_TO_DWORD
- [x] LINT_TO_LWORD
- [x] LINT_TO_DWORD
- [x] LINT_TO_WORD
- [x] LINT_TO_BYTE
- [x] DINT_TO_LWORD
- [x] DINT_TO_DWORD
- [x] DINT_TO_WORD
- [x] DINT_TO_BYTE
- [x] INT_TO_LWORD
- [x] INT_TO_DWORD
- [x] INT_TO_WORD
- [x] INT_TO_BYTE
- [x] SINT_TO_LWORD
- [x] SINT_TO_DWORD
- [x] SINT_TO_WORD
- [x] SINT_TO_BYTE
- [x] ULINT_TO_LWORD
- [x] ULINT_TO_DWORD
- [x] ULINT_TO_WORD
- [x] ULINT_TO_BYTE
- [x] UDINT_TO_LWORD
- [x] UDINT_TO_DWORD
- [x] UDINT_TO_WORD
- [x] UDINT_TO_BYTE
- [x] UINT_TO_LWORD
- [x] UINT_TO_DWORD
- [x] UINT_TO_WORD
- [x] UINT_TO_BYTE
- [x] USINT_TO_LWORD
- [x] USINT_TO_DWORD
- [x] USINT_TO_WORD
- [x] USINT_TO_BYTE

## Table 26 - Data Type Conversion of Date and Time Types
- [x] LTIME_TO_TIME
- [x] TIME_TO_LTIME
- [x] LDT_TO_DT
- [x] LDT_TO_DATE
- [x] LDT_TO_LTOD
- [x] LDT_TO_TOD
- [x] DT_TO_LDT
- [x] DT_TO_DATE
- [x] DT_TO_LTOD
- [x] DT_TO_TOD
- [x] LTOD_TO_TOD
- [x] TOD_TO_LTOD

## Table 27 - Data Type Conversion of Character Types
- [x] WSTRING_TO_STRING
- [x] WSTRING_TO_WCHAR
- [x] STRING_TO_WSTRING
- [x] STRING_TO_CHAR
- [x] WCHAR_TO_WSTRING
- [x] WCHAR_TO_CHAR
- [x] CHAR_TO_STRING
- [x] CHAR_TO_WCHAR

## Table 28 - Numerical and Arithmetic Functions (Single Input)
- [x] ABS
- [x] SQRT
- [x] LN
- [x] LOG
- [x] EXP
- [x] SIN
- [x] COS
- [x] TAN
- [x] ASIN
- [x] ACOS
- [x] ATAN
- [x] ATAN2

## Table 29 - Arithmetic Functions (Two or More Inputs)
- [x] ADD (extensible)
- [x] SUB
- [x] MUL (extensible)
- [x] DIV
- [x] MOD
- [x] EXPT
- [x] MOVE

## Table 30 - Bit Shift/Rotate Functions
- [x] SHL
- [x] SHR
- [x] ROL
- [x] ROR

## Table 31 - Bitwise Boolean Functions
- [x] AND (extensible)
- [x] OR (extensible)
- [x] XOR (extensible)
- [x] NOT

## Table 32 - Selection Functions
- [x] SEL
- [x] MAX (extensible)
- [x] MIN (extensible)
- [x] LIMIT
- [x] MUX (extensible)

## Table 33 - Comparison Functions
- [x] GT (extensible)
- [x] GE (extensible)
- [x] EQ (extensible)
- [x] LE (extensible)
- [x] LT (extensible)
- [x] NE (non-extensible)

## Table 34 - String Functions
- [x] LEN
- [x] LEFT
- [x] RIGHT
- [x] MID
- [x] CONCAT (extensible)
- [x] INSERT
- [x] DELETE
- [x] REPLACE
- [x] FIND

## Table 35 - Time/Duration Arithmetic Functions
- [x] ADD (overloaded)
- [x] ADD_TIME
- [x] ADD_LTIME
- [x] ADD_TOD_TIME
- [x] ADD_LTOD_LTIME
- [x] ADD_DT_TIME
- [x] ADD_LDT_LTIME
- [x] SUB (overloaded)
- [x] SUB_TIME
- [x] SUB_LTIME
- [x] SUB_DATE_DATE
- [x] SUB_LDATE_LDATE
- [x] SUB_TOD_TIME
- [x] SUB_LTOD_LTIME
- [x] SUB_TOD_TOD
- [x] SUB_LTOD_LTOD
- [x] SUB_DT_TIME
- [x] SUB_LDT_LTIME
- [x] SUB_DT_DT
- [x] SUB_LDT_LDT
- [x] MUL (overloaded)
- [x] MUL_TIME
- [x] MUL_LTIME
- [x] DIV (overloaded)
- [x] DIV_TIME
- [x] DIV_LTIME

## Table 36 - CONCAT/SPLIT and DAY_OF_WEEK
- [x] CONCAT_DATE_TOD
- [x] CONCAT_DATE_LTOD
- [x] CONCAT_DATE
- [x] CONCAT_TOD
- [x] CONCAT_LTOD
- [x] CONCAT_DT
- [x] CONCAT_LDT
- [x] SPLIT_DATE
- [x] SPLIT_TOD
- [x] SPLIT_LTOD
- [x] SPLIT_DT
- [x] SPLIT_LDT
- [x] DAY_OF_WEEK

## Non-IEC Extensions (MP-014 Test Framework)
- [x] ASSERT_TRUE
- [x] ASSERT_FALSE
- [x] ASSERT_EQUAL
- [x] ASSERT_NOT_EQUAL
- [x] ASSERT_GREATER
- [x] ASSERT_LESS
- [x] ASSERT_GREATER_OR_EQUAL
- [x] ASSERT_LESS_OR_EQUAL
- [x] ASSERT_NEAR

## Table 43 - Bistable Function Blocks
- [x] RS
- [x] SR

## Table 44 - Edge Detection Function Blocks
- [x] R_TRIG
- [x] F_TRIG

## Table 45 - Counter Function Blocks
- [x] CTU
- [x] CTD
- [x] CTUD
- [x] CTU_INT
- [x] CTD_INT
- [x] CTUD_INT
- [x] CTU_DINT
- [x] CTD_DINT
- [x] CTUD_DINT
- [x] CTU_LINT
- [x] CTD_LINT
- [x] CTUD_LINT
- [x] CTU_UDINT
- [x] CTD_UDINT
- [x] CTUD_UDINT
- [x] CTU_ULINT
- [x] CTD_ULINT
- [x] CTUD_ULINT

## Table 46 - Timer Function Blocks
- [x] TP
- [x] TON
- [x] TOF
- [x] TP_LTIME
- [x] TON_LTIME
- [x] TOF_LTIME
