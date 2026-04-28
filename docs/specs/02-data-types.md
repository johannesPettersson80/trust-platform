# Data Types

IEC 61131-3 Edition 3.0 (2013) - Section 6.4

This specification defines the type system for trust-hir.

## 1. Elementary Data Types (Table 10, Section 6.4.2)

### Boolean

| No. | Keyword | Description | Default Value | Bits | Range |
|-----|---------|-------------|---------------|------|-------|
| 1 | `BOOL` | Boolean | `FALSE` or `0` | 1 | `0` (FALSE), `1` (TRUE) |

### Signed Integers

| No. | Keyword | Description | Default Value | Bits | Range |
|-----|---------|-------------|---------------|------|-------|
| 2 | `SINT` | Short integer | `0` | 8 | -128 to 127 |
| 3 | `INT` | Integer | `0` | 16 | -32,768 to 32,767 |
| 4 | `DINT` | Double integer | `0` | 32 | -2,147,483,648 to 2,147,483,647 |
| 5 | `LINT` | Long integer | `0` | 64 | -2^63 to 2^63-1 |

### Unsigned Integers

| No. | Keyword | Description | Default Value | Bits | Range |
|-----|---------|-------------|---------------|------|-------|
| 6 | `USINT` | Unsigned short integer | `0` | 8 | 0 to 255 |
| 7 | `UINT` | Unsigned integer | `0` | 16 | 0 to 65,535 |
| 8 | `UDINT` | Unsigned double integer | `0` | 32 | 0 to 4,294,967,295 |
| 9 | `ULINT` | Unsigned long integer | `0` | 64 | 0 to 2^64-1 |

### Real Numbers

| No. | Keyword | Description | Default Value | Bits | Precision |
|-----|---------|-------------|---------------|------|-----------|
| 10 | `REAL` | Real numbers | `0.0` | 32 | IEEE 754 single precision |
| 11 | `LREAL` | Long reals | `0.0` | 64 | IEEE 754 double precision |

### Duration

| No. | Keyword | Description | Default Value | Bits | Notes |
|-----|---------|-------------|---------------|------|-------|
| 12a | `TIME` | Duration | `T#0s` | Impl. | Implementer specific |
| 12b | `LTIME` | Long duration | `LTIME#0s` | 64 | Signed, unit: nanoseconds |

### Date and Time

| No. | Keyword | Description | Default Value | Bits | Notes |
|-----|---------|-------------|---------------|------|-------|
| 13a | `DATE` | Date only | Impl. | Impl. | Implementer specific |
| 13b | `LDATE` | Long date | `LDATE#1970-01-01` | 64 | Signed ns since 1970-01-01 |
| 14a | `TIME_OF_DAY` / `TOD` | Time of day | `TOD#00:00:00` | Impl. | Implementer specific |
| 14b | `LTIME_OF_DAY` / `LTOD` | Long time of day | `LTOD#00:00:00` | 64 | Signed ns since midnight |
| 15a | `DATE_AND_TIME` / `DT` | Date and time | Impl. | Impl. | Implementer specific |
| 15b | `LDATE_AND_TIME` / `LDT` | Long date and time | `LDT#1970-01-01-00:00:00` | 64 | Signed ns since 1970-01-01-00:00:00 |

### Strings

| No. | Keyword | Description | Default Value | Bits/Char | Notes |
|-----|---------|-------------|---------------|-----------|-------|
| 16a | `STRING` | Single-byte string | `''` (empty) | 8 | Variable length |
| 16b | `WSTRING` | Double-byte string | `""` (empty) | 16 | Variable length |
| 17a | `CHAR` | Single-byte character | `'$00'` | 8 | Single character |
| 17b | `WCHAR` | Double-byte character | `"$0000"` | 16 | Single character |

### Bit Strings

| No. | Keyword | Description | Default Value | Bits |
|-----|---------|-------------|---------------|------|
| 18 | `BYTE` | Bit string of 8 | `16#00` | 8 |
| 19 | `WORD` | Bit string of 16 | `16#0000` | 16 |
| 20 | `DWORD` | Bit string of 32 | `16#0000_0000` | 32 |
| 21 | `LWORD` | Bit string of 64 | `16#0000_0000_0000_0000` | 64 |

## 2. Generic Data Types (Figure 5, Section 6.4.3)

Generic data types are used in standard function/function block specifications. They are identified by the `ANY` prefix.

```
ANY
├── ANY_DERIVED          (user-defined types)
└── ANY_ELEMENTARY
    ├── ANY_MAGNITUDE
    │   ├── ANY_NUM
    │   │   ├── ANY_REAL     → REAL, LREAL
    │   │   └── ANY_INT
    │   │       ├── ANY_UNSIGNED → USINT, UINT, UDINT, ULINT
    │   │       └── ANY_SIGNED   → SINT, INT, DINT, LINT
    │   └── ANY_DURATION     → TIME, LTIME
    ├── ANY_BIT              → BOOL, BYTE, WORD, DWORD, LWORD
    ├── ANY_CHARS
    │   ├── ANY_STRING       → STRING, WSTRING
    │   └── ANY_CHAR         → CHAR, WCHAR
    └── ANY_DATE             → DATE_AND_TIME, LDT, DATE,
                               TIME_OF_DAY, LTOD
```

### Generic Type Rules

1. The generic type of a directly derived type = generic type of the base elementary type
2. The generic type of a subrange type = `ANY_INT`
3. The generic type of all other derived types = `ANY_DERIVED`

## 3. User-Defined Data Types (Table 11, Section 6.4.4)

User-defined types are declared using `TYPE...END_TYPE`.

### 3.1 Enumerated Data Types (Section 6.4.4.2)

```
TYPE
  TrafficLight: (Red, Amber, Green);
  Colors: (Red, Yellow, Green, Blue) := Blue;  // With initialization
END_TYPE
```

**Rules**:
- First value is the default initial value (unless explicitly initialized)
- Different enums may use the same identifiers
- Qualified access: `TrafficLight#Red` resolves ambiguity
- Error if enumerated literal cannot be unambiguously determined

### 3.2 Data Types with Named Values (Section 6.4.4.3)

```
TYPE
  TrafficLight: INT (Red := 1, Amber := 2, Green := 3) := Green;
  Colors: DWORD (
    Red   := 16#00FF0000,
    Green := 16#0000FF00,
    Blue  := 16#000000FF,
    White := Red OR Green OR Blue
  ) := Green;
END_TYPE
```

**Rules**:
- Named values do NOT limit the value range
- Arithmetic operations are allowed on these types
- Values can be compared with numeric literals

### 3.3 Subrange Data Types (Section 6.4.4.4, Table 11)

```
TYPE
  AnalogData: INT(-4095 .. 4095) := 0;
END_TYPE
```

**Rules**:
- Base type shall be an integer type (generic type `ANY_INT`) (IEC 61131-3 Ed.3, 6.3, 6.4.4.4, Table 11)
- Default initial value is the lower limit (unless explicitly initialized) (IEC 61131-3 Ed.3, 6.4.4.4.2)
- Limits must be literals or constant expressions (IEC 61131-3 Ed.3, 6.4.4.4.1)
- Error if value goes outside the range (IEC 61131-3 Ed.3, 6.4.4.4.1)

### 3.4 Array Data Types (Section 6.4.4.5)

```
TYPE
  Analog16Input: ARRAY[1..16] OF INT;
  Matrix: ARRAY[1..10, 1..20] OF REAL;
  Timers: ARRAY[1..50] OF TON := [50(PT := T#100ms)];  // FB array
END_TYPE
```

**Initialization**:
```
ARRAY[0..5] OF INT := [2(1, 2, 3)]  // Results in: 1, 2, 3, 1, 2, 3
```

**Rules**:
- Array elements can be elementary types, user types, FBs, or classes
- Subscripts in ST must yield ANY_INT value (IEC 61131-3 Ed.3, Table 11)
- Error if subscript is outside declared range (IEC 61131-3 Ed.3, Table 11)
- Rightmost subscript varies most rapidly during initialization
- Excess initial values are ignored (with warning)
- Missing initial values use type defaults (with warning)

### 3.5 Structured Data Types (Section 6.4.4.6)

```
TYPE
  AnalogChannel: STRUCT
    Range:     AnalogSignalRange;
    MinScale:  AnalogData := -4095;
    MaxScale:  AnalogData := 4095;
  END_STRUCT;
END_TYPE
```

**Initialization**:
```
VAR
  Config: AnalogChannel := (Range := Bipolar, MinScale := 0);
END_VAR
```

**Rules**:
- Elements accessed with dot notation: `Config.MinScale`
- FBs and classes can be structure elements
- Two structured variables are assignment-compatible only if same type
- Named aggregate initialization uses `field := value` entries. Missing fields
  are materialized from member defaults or type defaults; unknown and duplicate
  field names are diagnostics rather than runtime fallback.

### 3.6 Structures with Relative Addressing (Section 6.4.4.7)

```
TYPE
  ComData: STRUCT
    head   AT %B0:  INT;
    length AT %B2:  USINT := 26;
    flag1  AT %X3.0: BOOL;
    end    AT %B25: BYTE;
  END_STRUCT;
END_TYPE
```

**With Overlap**:
```
TYPE
  UnionLike: STRUCT OVERLAP
    data1 AT %B0: BYTE;
    data2 AT %B0: REAL;  // Overlaps with data1
  END_STRUCT;
END_TYPE
```

**Rules**:
- `%B<n>` = byte offset n
- `%X<n>.<m>` = byte n, bit m (0-7)
- Components shall not overlap unless `OVERLAP` keyword is used
- Overlapped structures cannot be explicitly initialized

### 3.7 Directly Derived Data Types (Section 6.4.4.1)

```
TYPE
  Counter: UINT;
  Frequency: REAL := 50.0;
  MyAnalog: AnalogChannel := (MinScale := 0, MaxScale := 4000);
  Channels: ARRAY[1..2] OF AnalogChannel := [
    (Range := Bipolar, MinScale := 0),
    (Range := Bipolar, MaxScale := 1023)
  ];
END_TYPE
```

Directly derived TYPE-level defaults use the same initializer grammar as VAR
declarations: scalar defaults, array defaults, and named aggregate defaults are
preserved for runtime materialization.

## 4. Reference Types (Table 12, Section 6.4.4.6.2)

### REF_TO Declaration

```
TYPE
  RefInt: REF_TO INT;
  RefFB:  REF_TO TON;
END_TYPE
```

### Reference Operations

| No. | Operation | Syntax | Description |
|-----|-----------|--------|-------------|
| 1 | Reference | `REF(variable)` | Get reference to variable |
| 2 | Dereference | `ref^` | Access referenced value |
| 3 | Null check | `ref = NULL` | Check if reference is null |
| 4 | Assignment | `ref := REF(var)` | Assign reference |
| 5 | Assignment attempt | `ref ?= other_ref` | Attempt to assign reference; result may be `NULL` |

**Example**:
```
VAR
  myInt: INT := 42;
  refInt: REF_TO INT;
END_VAR

refInt := REF(myInt);
refInt^ := 100;  // myInt is now 100
```

**Rules**:
- Initial value of a reference is `NULL` (IEC 61131-3 Ed.3, Table 12)
- `REF` and dereference (`^`) are the standard reference operations (IEC 61131-3 Ed.3, Table 12)
- `ref := other_ref` requires equal reference types (IEC 61131-3 Ed.3, Table 12)
- Assignment attempt with `?=` may yield `NULL`; callers must check for `NULL` before use (IEC 61131-3 Ed.3, 6.6.6.7.2, Table 52)
- Dereferencing `NULL` is a runtime error (IEC 61131-3 Ed.3, Table 12)

### 4.2 POINTER TO (Non-IEC Extension)

truST supports `POINTER TO` as a vendor-style extension alongside IEC
`REF_TO`; see `docs/IEC_DEVIATIONS.md` (DEV-018).

```text
VAR
  ValuePtr : POINTER TO INT;
END_VAR

ValuePtr := ADR(SomeInt);
ValuePtr^ := 42;
IF ValuePtr = NULL THEN
  ValuePtr ?= ADR(FallbackInt);
END_IF;
```

**Rules**:

- `ADR(...)` produces a typed `POINTER TO <target>`
- Dereference (`^`) is a valid lvalue/rvalue on compatible pointer targets
- `NULL` is allowed for `POINTER TO` and `REF_TO`
- `?=` may be used to initialize a pointer/reference only when the target is
  currently `NULL`
- Pointer arithmetic is not supported

## 5. Type Conversion Rules (Figures 11-12, Section 6.4.2)

### Implicit Conversions

Implicit conversions are allowed from smaller to larger types within the same category:

```
SINT → INT → DINT → LINT
USINT → UINT → UDINT → ULINT
REAL → LREAL
```

### Explicit Conversions

Use `<TYPE>_TO_<TYPE>` functions:
- `INT_TO_REAL(x)`
- `REAL_TO_INT(x)`
- `DINT_TO_STRING(x)`
- etc.

### Conversion Categories

1. **Numeric to Numeric**: Truncation/rounding may occur
2. **Bit to Numeric**: Binary transfer
3. **Numeric to Bit**: Binary transfer
4. **Date/Time conversions**: Various standard functions
5. **String conversions**: Various standard functions

## 6. String Types and Character Access

### String Length Declaration

```
VAR
  s1: STRING[10] := 'ABCD';      // Max 10 chars, initial length 4
  s2: STRING;                    // Implementer-specific max length
END_VAR
```

The declared length may also be a compile-time constant expression:

```
VAR_GLOBAL CONSTANT
  MaxLen: INT := 12;
END_VAR

VAR
  s3: STRING[MaxLen + 2];
END_VAR
```

**Rules**:
- `STRING[n]`/`WSTRING[n]` declare a maximum length of `n` characters; `n` must be a positive integer constant expression. (IEC 61131-3 Ed.3, Table 10)
- Default initial value of `STRING`/`WSTRING` is the empty string (`''` / `""`). (IEC 61131-3 Ed.3, Table 10)
- String literals used for initialization must be compatible with `ANY_STRING` and shall not exceed the declared maximum length. (IEC 61131-3 Ed.3, Figure 6)
- Callable string-library functions (`LEN`, `LEFT`, `RIGHT`, `MID`, `CONCAT`, `INSERT`, `DELETE`, `REPLACE`, `FIND`) are specified in `07-standard-functions.md`.

### Character Access

```
VAR
  str: STRING[10] := 'ABCD';
  ch: CHAR;
END_VAR

ch := str[2];      // ch = 'B' (1-indexed)
str[3] := 'X';     // str = 'ABXD'
```

**Rules**:
- Character indexing is 1-based.
- Indexing a string yields or updates a compatible `CHAR`/`WCHAR` element.
- String library calls are specified in `07-standard-functions.md` §8.

**Rules**:
- Position 1 is the first character
- Error if accessing beyond string length
- Error if mixing CHAR/STRING with WCHAR/WSTRING
- truST runtime note: indexed `STRING` / `WSTRING` access is 1-based and follows the documented runtime element model in `DEV-018`; both VM ref access and the shipped string stdlib operate on Unicode scalar elements rather than raw UTF-8 bytes / raw 16-bit code units.

## Implementation Notes for trust-hir

### Type Representation

Each type needs:
1. **Size**: Number of bits/bytes
2. **Default value**: For initialization
3. **Operations**: Valid operators for this type
4. **Compatibility**: Which types can be converted to/from

### Type Checking Requirements

1. Assignment compatibility
2. Operator type requirements
3. Function parameter matching
4. Implicit conversion detection
5. Range validation for subranges
6. Array bounds checking
7. Reference validity

### Error Conditions

1. Type mismatch in assignment
2. Type mismatch in operation
3. Range violation (subrange)
4. Array index out of bounds
5. Null pointer dereference
6. Invalid type conversion
7. Ambiguous enumerated value
