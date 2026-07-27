# Standard Functions

IEC 61131-3 Edition 3.0 (2013) - Section 6.6.2.5

This specification defines standard-function signatures for trust-hir and the
bounded runtime results explicitly written below.

## 1. Overview

Standard functions are predefined functions available in all IEC 61131-3 implementations.

### Function Index

| Name / Group | Category | Signature Shape | IEC ref | Status |
|--------------|----------|-----------------|---------|--------|
| `*_TO_*`, `TO_*`, `TRUNC_*`, `*_BCD_TO_*`, `*_TO_BCD_*` | Conversion | fixed or overloaded | Tables 22-27 | Implemented with documented extensions |
| `ABS`, `SQRT`, `LN`, `LOG`, `EXP`, `SIN`, `COS`, `TAN`, `ASIN`, `ACOS`, `ATAN`, `ATAN2` | Numerical | fixed arity | Table 28 | Implemented |
| `ADD`, `SUB`, `MUL`, `DIV`, `MOD`, `EXPT`, `MOVE` | Arithmetic | fixed/extensible | Table 29 | Implemented |
| `SHL`, `SHR`, `ROL`, `ROR` | Bit shift / rotate | fixed arity | Table 30 | Implemented |
| `AND`, `OR`, `XOR`, `NOT` | Bitwise boolean | fixed/extensible | Table 31 | Implemented |
| `SEL`, `MAX`, `MIN`, `LIMIT`, `MUX` | Selection | fixed/extensible | Table 32 | Implemented |
| `GT`, `GE`, `EQ`, `LE`, `LT`, `NE` | Comparison | fixed/extensible | Table 33 | Implemented |
| `LEN`, `LEFT`, `RIGHT`, `MID`, `CONCAT`, `INSERT`, `DELETE`, `REPLACE`, `FIND` | String | fixed/extensible | Table 34 | Implemented |
| `ADD_*`, `SUB_*`, `MUL_*`, `DIV_*`, `CONCAT_*`, `SPLIT_*`, `DAY_OF_WEEK` | Date / time | fixed or overloaded | Tables 35-36 | Implemented |
| `IS_VALID`, `IS_VALID_BCD` | Validate | fixed arity | Table 39 | Implemented |
| `REF` | Reference | fixed arity | Table 12 | Implemented |
| `LOWER_BOUND`, `UPPER_BOUND` | Array bound | fixed arity | IEC extension set | Implemented |
| `ASSERT_*` | Test assertions | fixed arity | non-IEC | truST test-workflow extension |

### Function Characteristics

- No internal state (stateless)
- Same inputs always produce same outputs
- Can be overloaded for different types
- Some have extensible inputs (e.g., ADD can take 2+ arguments)

## 2. Type Conversion Functions (Tables 22-27)

Conversion functions use the `SRC_TO_DST` form (Table 22). The overloaded `TO_DST` form exists but is deprecated.
Truncation forms:
- `TRUNC` (deprecated overloaded)
- `TRUNC_<DST>` (overloaded, e.g., `TRUNC_INT`)
- `<SRC>_TRUNC_<DST>` (typed, deprecated, e.g., `REAL_TRUNC_INT`)

When STRING/WSTRING is an input or output, the string shall conform to the external representation of the corresponding data type.

### 2.1 Numeric Conversions

#### Integer to Integer

| Function | From | To | Notes |
|----------|------|-----|-------|
| `*_TO_SINT` | ANY_INT | SINT | Truncation may occur |
| `*_TO_INT` | ANY_INT | INT | Truncation may occur |
| `*_TO_DINT` | ANY_INT | DINT | |
| `*_TO_LINT` | ANY_INT | LINT | |
| `*_TO_USINT` | ANY_INT | USINT | Unsigned conversion |
| `*_TO_UINT` | ANY_INT | UINT | |
| `*_TO_UDINT` | ANY_INT | UDINT | |
| `*_TO_ULINT` | ANY_INT | ULINT | |

#### Real to Integer

| Function | Notes |
|----------|-------|
| `REAL_TO_INT` | IEC 60559 round-to-nearest, ties to even |
| `LREAL_TO_INT` | IEC 60559 round-to-nearest, ties to even |
| `TRUNC` | Deprecated overloaded truncation toward zero |
| `TRUNC_*` | Overloaded truncation toward zero (e.g., `TRUNC_INT`) |
| `*_TRUNC_*` | Typed truncation toward zero (deprecated) |

#### Integer to Real

| Function | Notes |
|----------|-------|
| `INT_TO_REAL` | Exact for small integers |
| `*_TO_LREAL` | More precision |

#### Real to Real

| Function | Notes |
|----------|-------|
| `REAL_TO_LREAL` | Widening |
| `LREAL_TO_REAL` | Narrowing and precision loss; a finite source that overflows basic-single range returns `RuntimeError::Overflow` |

For an explicit numeric or binary-transfer conversion whose target is `REAL`
or `LREAL`, truST returns `RuntimeError::Overflow` if the computed result would
be NaN or either infinity. This includes `LREAL_TO_REAL` narrowing and the IEC
Table 25 binary transfers `DWORD_TO_REAL` and `LWORD_TO_LREAL`. Finite binary
transfers retain their exact bit-defined value. Text parsing rejects the
non-finite spellings specified in section 2.7, but that rejection does not
promise an exact runtime-error variant. The rejection policy is
implementer-specific under IEC 61131-3 section 6.6.2.5.2 and Table 23; see
`docs/IEC_DECISIONS.md#2026-07-22---non-finite-real-result-and-explicit-conversion-policy`.

For conversion into `SINT`, `INT`, `DINT`, or `LINT`, a representable integer
value is preserved exactly. A value outside the destination's IEC Table 10
range returns `RuntimeError::Overflow`; truST does not wrap, saturate,
truncate, or substitute an integer. This is the reviewed implementer-specific
conversion fault policy specified by
`docs/specs/10-runtime-semantics.md#6-8-signed-integer-result-materialization`;
it is not an IEC deviation.

### 2.2 Bit Data Type Conversions (Table 24)

Binary transfer between BYTE/WORD/DWORD/LWORD. If the target is wider, the rightmost bits are preserved and the remaining bits are set to zero. If the target is narrower, only the rightmost bits are kept.

- `BYTE_TO_WORD`, `BYTE_TO_DWORD`, `BYTE_TO_LWORD`
- `WORD_TO_BYTE`, `WORD_TO_DWORD`, `WORD_TO_LWORD`
- `DWORD_TO_BYTE`, `DWORD_TO_WORD`, `DWORD_TO_LWORD`
- `LWORD_TO_BYTE`, `LWORD_TO_WORD`, `LWORD_TO_DWORD`

### 2.3 Bit/Numeric Conversions (Table 25)

Binary transfer between bit strings and numeric types as listed in Table 25:
- Bit to numeric: `BYTE/WORD/DWORD/LWORD` to `SINT/INT/DINT/LINT/USINT/UINT/UDINT/ULINT/REAL/LREAL`
- Numeric to bit: `SINT/INT/DINT/LINT/USINT/UINT/UDINT/ULINT/REAL/LREAL` to `BYTE/WORD/DWORD/LWORD`

### 2.4 Date and Time Conversions (Table 26)

| Function | Description |
|----------|-------------|
| `LTIME_TO_TIME` | LTIME to TIME |
| `TIME_TO_LTIME` | TIME to LTIME |
| `LDT_TO_DT` | LDT to DT |
| `LDT_TO_DATE` | Extract DATE from LDT |
| `LDT_TO_LTOD` | Extract LTOD from LDT |
| `LDT_TO_TOD` | Extract TOD from LDT (precision loss possible) |
| `DT_TO_LDT` | DT to LDT |
| `DT_TO_DATE` | Extract DATE from DT |
| `DT_TO_LTOD` | Extract LTOD from DT |
| `DT_TO_TOD` | Extract TOD from DT |
| `LTOD_TO_TOD` | LTOD to TOD |
| `TOD_TO_LTOD` | TOD to LTOD |

Implementer extension note:
- truST additionally provides `TIME_TO_DWORD` and `DWORD_TO_TIME` as
  documented non-IEC conversion helpers.
- These extensions use milliseconds:
  `TIME_TO_DWORD(T#123ms) = DWORD#123`,
  `DWORD_TO_TIME(DWORD#123) = T#123ms`.

### 2.5 Character Type Conversions (Table 27)

| Function | Description |
|----------|-------------|
| `WSTRING_TO_STRING` | Convert WSTRING to STRING |
| `WSTRING_TO_WCHAR` | First character of WSTRING |
| `STRING_TO_WSTRING` | Convert STRING to WSTRING |
| `STRING_TO_CHAR` | First character of STRING |
| `WCHAR_TO_WSTRING` | Single-character WSTRING |
| `WCHAR_TO_CHAR` | Convert WCHAR to CHAR |
| `CHAR_TO_STRING` | Single-character STRING |
| `CHAR_TO_WCHAR` | Convert CHAR to WCHAR |

Implementer extension note:
- truST also accepts direct character-to-bitstring conversions such as
  `CHAR_TO_BYTE` and `WCHAR_TO_WORD` as documented vendor extensions.
- Other conversions involving STRING/WSTRING (for example, numeric to string)
  remain implementer-specific. When provided, they shall follow the external
  literal representation rules in 6.3.3.

### 2.6 BCD Conversions (Table 22)

| Function | Description |
|----------|-------------|
| `*_BCD_TO_**` | Typed BCD conversion from BYTE/WORD/DWORD/LWORD to USINT/UINT/UDINT/ULINT |
| `BCD_TO_**` | Overloaded BCD conversion (bit string to unsigned integer) |
| `**_TO_BCD_*` | Typed BCD conversion from USINT/UINT/UDINT/ULINT to BYTE/WORD/DWORD/LWORD |
| `TO_BCD_**` | Overloaded BCD conversion (unsigned integer to bit string) |

```
// Example
BCDValue := 16#0042;
UIntValue := BCD_TO_UINT(BCDValue);  // UIntValue = 42
```

### 2.7 Runtime Text-Conversion Representatives

IEC 61131-3 Ed.3 section 6.6.2.5.2 requires conversions involving
`STRING`/`WSTRING` to use the external representation of the source or
destination type, while conversion-error handling remains
implementer-specific. The bounded truST runtime contract contains these exact
representatives:

| Call | Result |
|------|--------|
| `DINT_TO_STRING(DINT#42)` | `STRING#'42'` |
| `REAL_TO_STRING(REAL#1.25)` | `STRING#'1.25'` |
| `DWORD_TO_STRING(DWORD#42)` | `STRING#'42'` |
| `STRING_TO_DINT(STRING#'42')` | `DINT#42` |

These representatives do not define canonical output spelling for every
numeric or bit-string value. `STRING_TO_REAL('NaN')` and
`STRING_TO_LREAL('inf')` reject without producing a runtime `REAL`/`LREAL`
value. This specification intentionally does not freeze the runtime-error
variant used for those two text-rejection cases.

## 3. Numerical Functions (Table 28)

### Basic Arithmetic

| Function | Description | Signature |
|----------|-------------|-----------|
| `ABS` | Absolute value | `ABS(x: ANY_NUM) : ANY_NUM` |
| `SQRT` | Square root | `SQRT(x: ANY_REAL) : ANY_REAL` |
| `LN` | Natural logarithm | `LN(x: ANY_REAL) : ANY_REAL` |
| `LOG` | Base 10 logarithm | `LOG(x: ANY_REAL) : ANY_REAL` |
| `EXP` | Exponential (e^x) | `EXP(x: ANY_REAL) : ANY_REAL` |

### Trigonometric Functions (Table 28)

| Function | Description | Domain | Range |
|----------|-------------|--------|-------|
| `SIN` | Sine | Radians | -1.0 to 1.0 |
| `COS` | Cosine | Radians | -1.0 to 1.0 |
| `TAN` | Tangent | Radians | Real |
| `ASIN` | Arc sine | -1.0 to 1.0 | -π/2 to π/2 |
| `ACOS` | Arc cosine | -1.0 to 1.0 | 0 to π |
| `ATAN` | Arc tangent | Real | -π/2 to π/2 |
| `ATAN2` | Arc tangent (y/x) | Real, Real | -π to π |

```
// Examples
Y := SIN(X);              // X in radians
Angle := ATAN2(DY, DX);   // Four-quadrant arctangent
```

### Arithmetic Functions (Table 29)

| Function | Description | Signature |
|----------|-------------|-----------|
| `ADD` | Addition | `ADD(IN1, IN2, ...: ANY_NUM) : ANY_NUM` |
| `MUL` | Multiplication | `MUL(IN1, IN2, ...: ANY_NUM) : ANY_NUM` |
| `SUB` | Subtraction | `SUB(IN1, IN2: ANY_NUM) : ANY_NUM` |
| `DIV` | Division | `DIV(IN1, IN2: ANY_NUM) : ANY_NUM` |
| `MOD` | Modulo | `MOD(IN1, IN2: ANY_INT) : ANY_INT` |
| `EXPT` | Exponentiation | `EXPT(IN1: ANY_REAL, IN2: ANY_NUM) : ANY_REAL` |
| `MOVE` | Assignment | `MOVE(IN: ANY) : ANY` |

**Note**: ADD and MUL are extensible (can take more than 2 inputs).

The `EXPT` standard-function signature remains the IEC `ANY_REAL`-base
contract shown above. The reviewed `INT#2 ** INT#3` host-evaluator extension
is separate and is recorded in
[`IEC_DEVIATIONS.md`](../IEC_DEVIATIONS.md#2026-07-27---integer-base-exponentiation);
it does not widen the standard-function signature.

For finite `REAL` operands, `EXP` and `EXPT` return a value only when the
result remains finite at IEC basic single width. A result outside that finite
range raises `RuntimeError::Overflow` before assignment storage and leaves the
target unchanged. The runtime does not clamp or store infinity or NaN. `LREAL`
and other exceptional numerical-function behavior remain outside this rule;
see `docs/specs/10-runtime-semantics.md` and
`docs/IEC_DECISIONS.md#2026-07-22---non-finite-real-result-and-explicit-conversion-policy`.

## 4. Bit Shift Functions (Table 30)

| Function | Description | Signature |
|----------|-------------|-----------|
| `SHL` | Shift left | `SHL(IN: ANY_BIT, N: ANY_INT) : ANY_BIT` |
| `SHR` | Shift right | `SHR(IN: ANY_BIT, N: ANY_INT) : ANY_BIT` |
| `ROL` | Rotate left | `ROL(IN: ANY_BIT, N: ANY_INT) : ANY_BIT` |
| `ROR` | Rotate right | `ROR(IN: ANY_BIT, N: ANY_INT) : ANY_BIT` |

```
// Examples
X := 2#1100_0000;
Y := SHL(X, 2);    // Y = 2#0000_0000 (bits shifted out)
Z := ROL(X, 2);    // Z = 2#0000_0011 (bits rotated)
```

## 5. Bitwise Boolean Functions (Table 31)

| Function | Description | Signature |
|----------|-------------|-----------|
| `AND` | Bitwise AND | `AND(IN1, IN2, ...: ANY_BIT) : ANY_BIT` |
| `OR` | Bitwise OR | `OR(IN1, IN2, ...: ANY_BIT) : ANY_BIT` |
| `XOR` | Bitwise XOR | `XOR(IN1, IN2, ...: ANY_BIT) : ANY_BIT` |
| `NOT` | Bitwise NOT | `NOT(IN: ANY_BIT) : ANY_BIT` |

**Note**: AND, OR, XOR are extensible.

At runtime, shift and rotate functions preserve the input bit-string type and
width. Bitwise Boolean functions return the common participating bit-string
width; a narrower input is zero-extended to that width. The reviewed mixed
representative is
`OR(BYTE#16#0F, WORD#16#00F0) = WORD#16#00FF`.

```
// Examples
Mask := 16#FF00;
Data := 16#1234;
Result := AND(Data, Mask);   // Result = 16#1200
Result := OR(Data, 16#00FF); // Result = 16#12FF
```

## 6. Selection Functions (Table 32)

| Function | Description | Signature |
|----------|-------------|-----------|
| `SEL` | Binary selection | `SEL(G: BOOL, IN0, IN1: ANY) : ANY` |
| `MAX` | Maximum | `MAX(IN1, IN2, ...: ANY_ELEMENTARY) : ANY_ELEMENTARY` |
| `MIN` | Minimum | `MIN(IN1, IN2, ...: ANY_ELEMENTARY) : ANY_ELEMENTARY` |
| `LIMIT` | Bounded value | `LIMIT(MN, IN, MX: ANY_ELEMENTARY) : ANY_ELEMENTARY` |
| `MUX` | Multiplexer | `MUX(K: ANY_INT, IN0, IN1, ...: ANY) : ANY` |

```
// SEL: Returns IN0 if G=FALSE, IN1 if G=TRUE
Result := SEL(Condition, ValueIfFalse, ValueIfTrue);

// MAX/MIN
MaxValue := MAX(A, B, C, D);
MinValue := MIN(A, B, C, D);

// LIMIT: Clamps IN between MN and MX
Output := LIMIT(0, Input, 100);  // 0 <= Output <= 100

// MUX: Returns IN[K]
Selected := MUX(Index, Value0, Value1, Value2, Value3);
```

The runtime returns the selected input with its runtime value identity intact,
including an enumerated value selected by `SEL`. Named invocation binds the
documented formal names; the reviewed executed-ST representative
`SEL(G := TRUE, IN0 := INT#4, IN1 := INT#7)` returns `INT#7`.

## 7. Comparison Functions (Table 33)

| Function | Description | Signature |
|----------|-------------|-----------|
| `GT` | Greater than | `GT(IN1, IN2, ...: ANY_ELEMENTARY) : BOOL` |
| `GE` | Greater or equal | `GE(IN1, IN2, ...: ANY_ELEMENTARY) : BOOL` |
| `EQ` | Equal | `EQ(IN1, IN2, ...: ANY_ELEMENTARY) : BOOL` |
| `LE` | Less or equal | `LE(IN1, IN2, ...: ANY_ELEMENTARY) : BOOL` |
| `LT` | Less than | `LT(IN1, IN2, ...: ANY_ELEMENTARY) : BOOL` |
| `NE` | Not equal | `NE(IN1, IN2: ANY_ELEMENTARY) : BOOL` |

**Note**: For GT, GE, EQ, LE, LT with multiple inputs, checks if sequence is monotonic. `NE` is not extensible.

For values of the same enumerated type, runtime `EQ` and `NE` compare the
enumerated value identity. The reviewed representatives are `EQ(RED, RED) =
TRUE` and `NE(RED, GREEN) = TRUE`.

```
// Examples
InOrder := GT(A, B, C);      // TRUE if A > B > C
AllEqual := EQ(X, Y, Z);     // TRUE if X = Y = Z
Different := NE(A, B);       // TRUE if A <> B
```

## 8. String Functions (Table 34)

String declaration syntax (`STRING[n]`, `WSTRING[n]`) and character indexing are
owned by `02-data-types.md`. This section owns the callable string functions.

| Function | Description | Signature |
|----------|-------------|-----------|
| `LEN` | Length | `LEN(IN: ANY_STRING) : INT` |
| `LEFT` | Left substring | `LEFT(IN: ANY_STRING, L: ANY_INT) : ANY_STRING` |
| `RIGHT` | Right substring | `RIGHT(IN: ANY_STRING, L: ANY_INT) : ANY_STRING` |
| `MID` | Middle substring | `MID(IN: ANY_STRING, L, P: ANY_INT) : ANY_STRING` |
| `CONCAT` | Concatenate | `CONCAT(IN1, IN2, ...: ANY_STRING) : ANY_STRING` |
| `INSERT` | Insert string | `INSERT(IN1, IN2: ANY_STRING, P: ANY_INT) : ANY_STRING` |
| `DELETE` | Delete substring | `DELETE(IN: ANY_STRING, L, P: ANY_INT) : ANY_STRING` |
| `REPLACE` | Replace substring | `REPLACE(IN1, IN2: ANY_STRING, L, P: ANY_INT) : ANY_STRING` |
| `FIND` | Find position | `FIND(IN1, IN2: ANY_STRING) : INT` |

```
// Examples
Str := 'Hello World';
Length := LEN(Str);                    // 11
Left5 := LEFT(Str, 5);                 // 'Hello'
Right5 := RIGHT(Str, 5);               // 'World'
Mid := MID(Str, 5, 7);                 // 'World' (5 chars starting at pos 7)
Full := CONCAT('Hello', ' ', 'World'); // 'Hello World'
Inserted := INSERT('AC', 'B', 2);      // 'ABC'
Deleted := DELETE('ABCD', 2, 2);       // 'AD' (delete 2 chars at pos 2)
Replaced := REPLACE('ABCD', 'XX', 2, 2); // 'AXXD'
Pos := FIND('ABCABC', 'BC');           // 2 (first occurrence)
```

**Position Notes**:
- Position 1 is the first character
- FIND returns 0 if not found
- Runtime positions and lengths count Unicode scalar values, not UTF-8 bytes.
  Thus `LEN('ÄBC') = 3`, `LEFT('ÄBC', 1) = 'Ä'`, and
  `FIND('ÄBC', 'B') = 2`. The same element-count rule applies to the reviewed
  `WSTRING` length.

## 9. Date and Time Functions (Tables 35-36)

### Time Arithmetic

| Function | Description |
|----------|-------------|
| `ADD` | Overloaded time/date addition (see Table 35) |
| `ADD_TIME` | TIME + TIME → TIME |
| `ADD_LTIME` | LTIME + LTIME → LTIME |
| `ADD_TOD_TIME` | TOD + TIME → TOD |
| `ADD_LTOD_LTIME` | LTOD + LTIME → LTOD |
| `ADD_DT_TIME` | DT + TIME → DT |
| `ADD_LDT_LTIME` | LDT + LTIME → LDT |
| `SUB` | Overloaded time/date subtraction (see Table 35) |
| `SUB_TIME` | TIME - TIME → TIME |
| `SUB_LTIME` | LTIME - LTIME → LTIME |
| `SUB_DATE_DATE` | DATE - DATE → TIME |
| `SUB_LDATE_LDATE` | LDATE - LDATE → LTIME |
| `SUB_TOD_TIME` | TOD - TIME → TOD |
| `SUB_LTOD_LTIME` | LTOD - LTIME → LTOD |
| `SUB_TOD_TOD` | TOD - TOD → TIME |
| `SUB_LTOD_LTOD` | LTOD - LTOD → LTIME |
| `SUB_DT_TIME` | DT - TIME → DT |
| `SUB_LDT_LTIME` | LDT - LTIME → LDT |
| `SUB_DT_DT` | DT - DT → TIME |
| `SUB_LDT_LDT` | LDT - LDT → LTIME |
| `MUL_TIME` | TIME * ANY_NUM → TIME |
| `MUL_LTIME` | LTIME * ANY_NUM → LTIME |
| `DIV_TIME` | TIME / ANY_NUM → TIME |
| `DIV_LTIME` | LTIME / ANY_NUM → LTIME |

**Notes**:
- Overloaded `ADD`/`SUB` apply only within the TIME/DT/DATE/TOD set or the LTIME/LDT/LDATE/LTOD set.
- Result range overflow is an error; output ranges are Implementer specific.

### Date/Time Component Functions

| Function | Description |
|----------|-------------|
| `CONCAT_DATE_TOD` | Combine DATE and TOD into DT |
| `CONCAT_DATE_LTOD` | Combine DATE and LTOD into LDT |
| `CONCAT_DATE` | YEAR, MONTH, DAY → DATE |
| `CONCAT_TOD` | HOUR, MINUTE, SECOND, MILLISECOND → TOD |
| `CONCAT_LTOD` | HOUR, MINUTE, SECOND, MILLISECOND → LTOD |
| `CONCAT_DT` | YEAR, MONTH, DAY, HOUR, MINUTE, SECOND, MILLISECOND → DT |
| `CONCAT_LDT` | YEAR, MONTH, DAY, HOUR, MINUTE, SECOND, MILLISECOND → LDT |
| `SPLIT_DATE` | DATE → YEAR, MONTH, DAY |
| `SPLIT_TOD` | TOD → HOUR, MINUTE, SECOND, MILLISECOND |
| `SPLIT_LTOD` | LTOD → HOUR, MINUTE, SECOND, MILLISECOND |
| `SPLIT_DT` | DT → YEAR, MONTH, DAY, HOUR, MINUTE, SECOND, MILLISECOND |
| `SPLIT_LDT` | LDT → YEAR, MONTH, DAY, HOUR, MINUTE, SECOND, MILLISECOND |
| `DAY_OF_WEEK` | DATE → 0=Sunday..6=Saturday |

**Notes**:
- `SPLIT_*` output types are `ANY_INT`; the Implementer specifies concrete types.
- Additional inputs/outputs (for example, microsecond/nanosecond) are Implementer specific.

```
// Examples
NewTime := ADD_TIME(T#1h, T#30m);           // T#1h30m
EndTime := ADD_TOD_TIME(TOD#08:00:00, T#2h); // TOD#10:00:00
Duration := SUB_DT_DT(EndDateTime, StartDateTime);
DoubleTime := MUL_TIME(BaseTime, 2);
HalfTime := DIV_TIME(BaseTime, 2);
```

## 10. Validate Functions (Table 39)

| Function | Description | Signature |
|----------|-------------|-----------|
| `IS_VALID` | Returns `FALSE` for invalid real values such as NaN or infinity | `IS_VALID(IN: REAL/LREAL) : BOOL` |
| `IS_VALID_BCD` | Returns `FALSE` if any BCD nibble is greater than `9` | `IS_VALID_BCD(IN: BYTE/WORD/DWORD/LWORD) : BOOL` |

IEC 61131-3 Ed.3 Figure 5 places `BOOL` under the broader `ANY_BIT` hierarchy,
but the Table 39 validation-function narrative defines `IS_VALID_BCD` for
`BYTE`, `WORD`, `DWORD`, and `LWORD`. truST follows that stricter Table 39
domain and rejects `BOOL` for `IS_VALID_BCD`.

```
VAR
  R : REAL;
  W : WORD := WORD#16#1234;
  Ok : BOOL;
END_VAR

Ok := IS_VALID(R);
Ok := IS_VALID_BCD(W);
```

## 11. Reference Functions

| Function | Description | Signature |
|----------|-------------|-----------|
| `REF` | Get reference | `REF(IN: ANY) : REF_TO ANY` |

```
VAR
  MyInt: INT := 42;
  pInt: REF_TO INT;
END_VAR

pInt := REF(MyInt);
```

## 12. Array Bound Functions

| Function | Description | Signature |
|----------|-------------|-----------|
| `LOWER_BOUND` | Lower array bound | `LOWER_BOUND(arr: ARRAY, dim: INT) : DINT` |
| `UPPER_BOUND` | Upper array bound | `UPPER_BOUND(arr: ARRAY, dim: INT) : DINT` |

```
VAR
  Data: ARRAY[5..15] OF INT;
  Lo, Hi: DINT;
END_VAR

Lo := LOWER_BOUND(Data, 1);  // Lo = 5
Hi := UPPER_BOUND(Data, 1);  // Hi = 15
```

## 13. Error Conditions

### Runtime Errors

| Function | Error Condition |
|----------|-----------------|
| `SQRT` | Negative input |
| `LN`, `LOG` | Non-positive input |
| `DIV`, `MOD` | Division by zero |
| `ASIN`, `ACOS` | Input outside [-1, 1] |
| `STRING_TO_*` | Invalid string format |
| Array bound | Invalid dimension |

### Overflow

Numeric functions may overflow. Behavior is Implementer specific:
- Saturation to max/min value
- Wrap-around
- Error flag/exception

## Implementation Notes for trust-hir

### Function Resolution

1. Match function name (case-insensitive)
2. Check argument count (consider extensible functions)
3. Resolve overloaded variants by argument types
4. Apply implicit conversions if needed
5. Determine return type

### Type Inference for Overloaded Functions

```
// ADD is overloaded for all numeric types
A: INT;
B: INT;
C := ADD(A, B);  // C is INT

X: REAL;
Y: REAL;
Z := ADD(X, Y);  // Z is REAL
```

### Extensible Functions

These functions accept variable number of inputs:
- `ADD`, `MUL` (arithmetic)
- `AND`, `OR`, `XOR` (bitwise)
- `MAX`, `MIN` (selection)
- `GT`, `GE`, `EQ`, `LE`, `LT` (comparison)
- `CONCAT` (string)
- `MUX` (selection)

### Standard Library

The trust-hir should include definitions for all standard functions with:
- Name
- Parameter types (considering overloading)
- Return type
- Extensibility flag
- Built-in implementation or intrinsic marker

## Non-IEC Extensions (MP-014)

The following functions are non-IEC additions for the user-facing ST test framework:

| Function | Signature | Behavior |
|----------|-----------|----------|
| `ASSERT_TRUE` | `ASSERT_TRUE(IN: BOOL) : VOID` | Fails test if `IN` is not `TRUE` |
| `ASSERT_FALSE` | `ASSERT_FALSE(IN: BOOL) : VOID` | Fails test if `IN` is not `FALSE` |
| `ASSERT_EQUAL` | `ASSERT_EQUAL(EXPECTED: ANY_ELEMENTARY, ACTUAL: ANY_ELEMENTARY) : VOID` | Fails test when values are not equal |
| `ASSERT_NOT_EQUAL` | `ASSERT_NOT_EQUAL(EXPECTED: ANY_ELEMENTARY, ACTUAL: ANY_ELEMENTARY) : VOID` | Fails test when values are equal |
| `ASSERT_GREATER` | `ASSERT_GREATER(VALUE: ANY_ELEMENTARY, BOUND: ANY_ELEMENTARY) : VOID` | Fails test unless `VALUE > BOUND` |
| `ASSERT_LESS` | `ASSERT_LESS(VALUE: ANY_ELEMENTARY, BOUND: ANY_ELEMENTARY) : VOID` | Fails test unless `VALUE < BOUND` |
| `ASSERT_GREATER_OR_EQUAL` | `ASSERT_GREATER_OR_EQUAL(VALUE: ANY_ELEMENTARY, BOUND: ANY_ELEMENTARY) : VOID` | Fails test unless `VALUE >= BOUND` |
| `ASSERT_LESS_OR_EQUAL` | `ASSERT_LESS_OR_EQUAL(VALUE: ANY_ELEMENTARY, BOUND: ANY_ELEMENTARY) : VOID` | Fails test unless `VALUE <= BOUND` |
| `ASSERT_NEAR` | `ASSERT_NEAR(EXPECTED: ANY_NUM, ACTUAL: ANY_NUM, DELTA: ANY_NUM) : VOID` | Fails test when `ABS(EXPECTED-ACTUAL) > DELTA` |

Compatibility notes:
- These assertions are truST test-workflow extensions and are not part of IEC
  61131-3 Tables 22-36 or Table 39.
- They are intended for `TEST_PROGRAM` / `TEST_FUNCTION_BLOCK` execution paths.
- On success, every `ASSERT_*` runtime call returns `Value::Null`, the runtime
  representation of the IEC-facing `VOID` result.
- The reviewed mixed numeric comparisons are lossless `INT`/`DINT` equality
  and ordering, plus finite `REAL`/`LREAL` comparison for `ASSERT_NEAR`. Other
  mixed elementary-type pairs are not authorized by these representatives.
- A condition failure returns `RuntimeError::AssertionFailed`; it does not
  return a normal value.
- Failure messages use user-facing value text rather than internal `Value`
  debug forms. For the reviewed value families, integers use decimal text,
  integral `REAL` values retain `.0`, Boolean values use `TRUE`/`FALSE`, and
  `CHAR` values use single quotes.
- The stable message forms for the reviewed relational assertions are:
  - `ASSERT_EQUAL failed: expected {expected}, actual {actual}`
  - `ASSERT_NOT_EQUAL failed: values should differ, left {left}, right {right}`
  - `ASSERT_GREATER failed: value {value} is not greater than bound {bound}`
  - `ASSERT_LESS failed: value {value} is not less than bound {bound}`
  - `ASSERT_GREATER_OR_EQUAL failed: value {value} is not >= bound {bound}`
  - `ASSERT_LESS_OR_EQUAL failed: value {value} is not <= bound {bound}`
- An `ASSERT_NEAR` failure message identifies `ASSERT_NEAR` and includes its
  `delta` context. No exact full-string format is promised for that message.
