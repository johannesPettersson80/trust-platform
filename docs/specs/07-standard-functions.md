# Standard Functions

IEC 61131-3 Edition 3.0 (2013) - Section 6.6.2.5

This specification defines standard functions for trust-hir.

## 1. Overview

Standard functions are predefined functions available in all IEC 61131-3 implementations.

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
| `LREAL_TO_REAL` | Narrowing, precision loss |

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

**Note**: Other conversions involving STRING/WSTRING (for example, numeric to string) are Implementer specific. When provided, they shall follow the external literal representation rules in 6.3.3.

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

```
// Examples
InOrder := GT(A, B, C);      // TRUE if A > B > C
AllEqual := EQ(X, Y, Z);     // TRUE if X = Y = Z
Different := NE(A, B);       // TRUE if A <> B
```

## 8. String Functions (Table 34)

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

## 10. Reference Functions

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

## 11. Array Bound Functions

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

## 12. Error Conditions

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
- These assertions are extension-only and not part of IEC 61131-3 Tables 22-36.
- They are intended for `TEST_PROGRAM` / `TEST_FUNCTION_BLOCK` execution paths.
- Runtime failures include assertion context (`expected` / `actual` and tolerance data for `ASSERT_NEAR`).
