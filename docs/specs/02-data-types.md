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

### Partial Access to ANY_BIT Variables (Table 17, Section 6.6.1.3)

Variables of type `BYTE`, `WORD`, `DWORD`, and `LWORD` support partial
bit/byte/word/double-word access. The access suffix is appended to the variable
name with dot notation:

```
VAR
  b : BYTE := BYTE#16#00;
  w : WORD := WORD#16#1234;
  d : DWORD := DWORD#16#89ABCDEF;
  l : LWORD := LWORD#16#0123_4567_89AB_CDEF;
END_VAR

b.%X3 := TRUE;          // write bit 3 of b
b.7 := FALSE;           // %X may be omitted for bit access
w.%B0 := BYTE#16#FF;    // write byte 0 of w
d.%W1;                  // word 1 of d
l.%D1;                  // double word 1 of l
```

| Target Type | Bit Access | Byte Access | Word Access | DWord Access |
|-------------|------------|-------------|-------------|--------------|
| `BYTE` | `%X0`..`%X7` or `0`..`7` -> `BOOL` | - | - | - |
| `WORD` | `%X0`..`%X15` or `0`..`15` -> `BOOL` | `%B0`..`%B1` -> `BYTE` | - | - |
| `DWORD` | `%X0`..`%X31` or `0`..`31` -> `BOOL` | `%B0`..`%B3` -> `BYTE` | `%W0`..`%W1` -> `WORD` | - |
| `LWORD` | `%X0`..`%X63` or `0`..`63` -> `BOOL` | `%B0`..`%B7` -> `BYTE` | `%W0`..`%W3` -> `WORD` | `%D0`..`%D1` -> `DWORD` |

The lower numbered suffix addresses the lower value part independently of
target-platform endian layout; bit offset `0` addresses the rightmost bit of
the value. Partial writes require a value of the selected part type (`BOOL` for
bit access, `BYTE` for byte access, `WORD` for word access, `DWORD` for dword
access).

Partial access applies to ordinary variables of the supported `ANY_BIT` types,
including structure elements, function block instance fields, and properly
mapped `VAR_IN_OUT` references whose selected element has type `BYTE`, `WORD`,
`DWORD`, or `LWORD`. It is not valid on directly represented variables
themselves, for example `%IB10.%X0`.

Directly derived aliases are transparent for this rule: an alias whose
ultimate target is `BYTE`, `WORD`, `DWORD`, or `LWORD` has the same selector
set and result types as that target. `BOOL`, signed and unsigned integer types,
enumerations, strings, arrays, and aggregates do not acquire partial access
merely because they have a binary representation.

The selector index is a non-negative decimal integer written directly in the
suffix. Decimal digit separators are accepted. A sign, radix prefix,
identifier, or general constant expression is not a selector index. `%X`,
`%B`, `%W`, and `%D` are case-insensitive; the prefix may be omitted only for
bit access. The selector must be within the closed range in the table above,
and invalid selector kinds or indexes are compile-time errors.

A partial access is an expression and, when its base is assignable, an lvalue.
Reads have the selected result type. Writes must be assignment-compatible with
that exact result type and update only the selected low-order value part,
leaving every other bit unchanged. Constants, inputs, and other read-only
bases remain read-only through partial access. A partial access through a
structure field, array element, object field, dereference, or `VAR_IN_OUT`
alias retains the lifetime and write permissions of its complete base path.
The numeric part ordering is defined by value significance, not host byte
order: `%B0`, `%W0`, and `%D0` select the least-significant part.

Source compilation must preserve partial access as a typed projection through
runtime and bytecode lowering. It must not lower the suffix as an ordinary
structure/object field, silently read a default value, or replace a partial
write with a whole-value assignment.

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

1. Generic types are formal signature categories used by standard
   functions/function blocks and semantic compatibility checks. They are not
   concrete runtime data types.
2. `ANY` contains every concrete elementary or derived data type. It does not
   contain `VOID`, the internal unknown/error type, the `NULL` literal type, or
   another generic category.
3. `ANY_ELEMENTARY` contains the concrete elementary leaves shown in Figure 5.
   `ANY_DERIVED` contains arrays, structures, unions, enumerations, references,
   pointers, function blocks, classes, and interfaces.
4. The generic type of a directly derived alias is the generic type of its
   ultimate base type. This rule is transitive through alias chains and must
   fail closed for an unresolved or cyclic chain; an alias to `INT` is
   `ANY_SIGNED`/`ANY_INT`/`ANY_NUM`, not `ANY_DERIVED`.
5. The generic type of a subrange is inherited from its integer base type. A
   signed-base subrange is `ANY_SIGNED`; an unsigned-base subrange is
   `ANY_UNSIGNED`; both are `ANY_INT`, `ANY_NUM`, and `ANY_MAGNITUDE`.
6. An enumeration and every other non-alias derived type belongs to
   `ANY_DERIVED`, even when it has an integer representation. Enum membership
   does not make the enum an `ANY_INT`.
7. `ANY_MAGNITUDE` contains `ANY_NUM` and `ANY_DURATION`. Duration types do not
   thereby become numeric types.
8. `BOOL` belongs to `ANY_BIT`, not `ANY_INT`. `ANY_CHARS` contains
   `ANY_STRING` and `ANY_CHAR`. `ANY_DATE` contains the short and long
   date/time-of-day/date-and-time families, but not `TIME` or `LTIME`.
9. A bounded `STRING[n]` or `WSTRING[n]` retains the generic family of its
   unbounded elementary type.
10. Generic membership authorizes a formal-parameter match; it does not itself
    authorize an implicit conversion between two concrete members of that
    family.

### Source declaration boundary

The `ANY*` keywords are parsed as type references so malformed or unsupported
source receives a semantic diagnostic at the type site. User source may not
declare storage, a derived type, an array element, a pointer/reference target,
a POU parameter, or a POU result with a generic type. truST does not expose a
user-defined generic/overload declaration facility. No runtime value, default,
layout, retain image, I/O binding, or bytecode type may be created for an
`ANY*` category.

## 3. User-Defined Data Types (Table 11, Section 6.4.4)

User-defined types are declared using `TYPE...END_TYPE`.

User-defined type formation is transactional at the public semantic-registry
boundary. An invalid dependency, bound, member or variant declaration,
initializer repetition, or type-level default may be retained internally only
as diagnostic recovery state; it must not be returned by
`lookup_registered_type_name` or become usable by downstream type queries.
Valid sibling declarations in the same source remain published. This
fail-closed publication rule prevents an incomplete declaration from acquiring
a layout, initializer, assignment identity, runtime storage, or bytecode type.

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
- An explicit named value is an integer constant expression evaluated in the
  declaration site's complete constant graph and namespace/`USING` context.
  Visible forward or cross-file constants are accepted independently of source
  order.
- Every explicit value and every implicit successor must be representable by
  the declared integer base type. Successor overflow is rejected; it must not
  saturate, wrap, or reuse the preceding value.
- An undefined, mutable, ambiguous, cyclic, non-integer, or overflowing
  expression rejects the declaration before any partial enumeration is
  published.

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
- Constant bounds use the declaration site's complete constant dependency
  graph and namespace/`USING` context. They are independent of textual and
  project-source order, case-insensitive under normal identifier comparison,
  and must materialize as integer values representable by the runtime bound
  model.
- The lower bound must not exceed the upper bound. An undefined, mutable,
  ambiguous, cyclic, non-integer, overflowing, or reversed bound rejects the
  declaration rather than creating a partial or normalized subrange.
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
- Array limits may use integer literals or constant expressions whose resolved
  values are integers. This includes unique, unambiguous enumerated values:
  array limits may be constant expressions (IEC 61131-3 Ed.3 §6.4.4.5.1), and
  enumerated values are operands of constant expressions (IEC 61131-3 Ed.3
  §6.6.1.2.4).
- Every dimension uses the same declaration-site constant graph and namespace
  rules as subranges. Multidimensional bounds retain source dimension order,
  and project-source reordering cannot alter any resolved lower or upper bound.
- Each fixed dimension must have a lower bound no greater than its upper
  bound. Invalid dependencies, non-integer values, bound overflow, and reversed
  dimensions reject the type before array storage is allocated.
- Subscripts in ST must yield ANY_INT value (IEC 61131-3 Ed.3, Table 11)
- Error if subscript is outside declared range (IEC 61131-3 Ed.3, Table 11)
- The initializer list is fully expanded before its values are assigned.
  Values fill the declared array in row-major order with the rightmost
  subscript varying fastest.
- Excess rightmost initial values are ignored and produce a preparation
  warning. They are still parsed and constant-validated; an invalid excess
  expression is not hidden merely because its value would be ignored.
- Missing rightmost values use the recursive element-type default and produce
  a preparation warning.
- Repetition count zero contributes no values. A negative, non-integer,
  non-constant, cyclic, or overflowing repetition count is an error.

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
- Whole-structure assignment copies the complete value. Subsequent mutation of
  either ordinary structure variable does not mutate the other. Reference,
  pointer, class, and function-block identities stored in fields retain their
  identity according to their own value-family rules.
- Named aggregate initialization uses `field := value` entries. Missing fields
  are materialized from member defaults or type defaults; unknown and duplicate
  field names are diagnostics rather than runtime fallback.
- A type-level or member-level default that names a constant uses the constant
  visible at the declaration site. The dependency is resolved independently of
  source-unit and textual declaration order, while lexical namespace and
  `USING` visibility remain in force. Qualified names select the exact
  namespace member; an unqualified name is rejected when it is not imported or
  when multiple imports match.
- Runtime materialization preserves IEC initialization precedence: the
  underlying elementary or assigned data-type default is the base value, a
  structure member default overrides that member's type default, a type-level
  aggregate default overrides the listed members, a variable-specific
  initializer overrides the assigned type default, and an eligible
  instance-specific `VAR_CONFIG` initializer overrides the exact configured
  variable or component. Omitted aggregate members continue from the preceding
  applicable default rather than being reset to the elementary default.

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
- In a non-`OVERLAP` structure, two relative fields whose complete bit ranges
  intersect are a declaration error. Gaps are valid.
- In an `OVERLAP` structure, intersecting relative fields share the same
  backing bits. A write through one field is observed through every overlapping
  field according to its declared type and target byte order.
- Overlapped structures cannot have an explicit type, member, aggregate, or
  variable initializer. Their backing storage starts from zero/default storage
  only. (IEC 61131-3 Ed.3 §6.4.4.7.2)

### 3.7 UNION aggregate (truST extension)

```
TYPE
  Choice: UNION
    count: INT := 1;
    ready: BOOL := TRUE;
  END_UNION;
  DefaultChoice: Choice := (count := 7);
END_TYPE
```

`UNION ... END_UNION` is a truST aggregate extension. IEC 61131-3 Ed.3 does
not define this declaration form, so its behavior is a product contract rather
than an IEC deviation.

- Variants are ordered, named, independently addressable logical members.
  Declaring a `UNION` does not select one active variant and does not make
  variant writes alias one another.
- Every variant is materialized. Its recursive type default is overridden by a
  variant initializer when present.
- A named aggregate initializer may override zero or more variants. Omitted
  variants retain their preceding applicable variant or recursive type default.
  Names are matched case-insensitively; unknown or duplicate names are errors.
- Reading or writing one variant does not change, invalidate, or select another
  variant. Use an IEC `STRUCT OVERLAP` declaration when fields must share
  backing bits.
- Whole-value assignment requires the same declared union type and copies all
  variants independently.
- `SIZEOF` a union reserves the maximum storage size of any one variant even
  though the runtime value retains all logical variants.

### 3.8 Directly Derived Data Types (Section 6.4.4.1)

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
preserved for runtime materialization. A scalar or aggregate default may use a
visible constant expression under the dependency and namespace rules above.
Self-referential or cyclic constant dependencies, references to mutable
variables, and invalid constant operations reject the type/default declaration
before a runtime is returned.

IEC 61131-3 Ed.3 sections 6.4.4.1.2, 6.4.4.5.2, 6.4.4.6.2, and 6.4.4.9.2
define compatible type defaults and their precedence. Repository source order,
cross-file discovery, and vendor-style namespaced GVL constants are truST
project/compiler behavior rather than IEC deviations.

## 4. Reference Types (Section 6.4.4.10 and Table 12)

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
- Initial value of a reference is `NULL` (IEC 61131-3 Ed.3, 6.4.4.10.2).
- `REF` and dereference (`^`) are the standard reference operations (IEC
  61131-3 Ed.3, 6.4.4.10.3).
- Ordinary assignment accepts the same target type and the standard
  derived-to-base reference direction; it makes the destination reference the
  same storage or instance while retaining the destination's declared
  reference type (IEC 61131-3 Ed.3, 6.4.4.10.3).
- `REF_TO` and `POINTER TO` are distinct type families. `REF(...)` produces
  `REF_TO T`; `ADR(...)` produces `POINTER TO T`. Neither ordinary assignment
  nor assignment attempt implicitly converts one family to the other.
- Assignment attempt `target ?= source` performs the dynamic interface or
  downcast compatibility check. It overwrites `target` with the compatible
  reference on success and with `NULL` on failure, irrespective of the
  target's previous value. The result must be checked against `NULL` before
  dereference (IEC 61131-3 Ed.3, 6.6.6.7 and Table 52).
- For an elementary or aggregate `REF_TO` with no dynamic class/interface
  relation, `?=` is accepted only for the same resolved target type (including
  direct aliases) or `NULL`; it is a checked-copy extension and cannot be used
  to reinterpret storage.
- References are not valid `VAR_IN_OUT` variables or parameters (IEC
  61131-3 Ed.3, 6.4.4.10.3).
- Dereferencing `NULL` is a runtime error (IEC 61131-3 Ed.3, 6.4.4.10.3).

### 4.2 POINTER TO (Non-IEC Extension)

truST supports `POINTER TO` as a documented vendor-style extension alongside
IEC `REF_TO`.

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
- The truST pointer form of `?=` is a checked-copy extension: the source must
  be `NULL` or a `POINTER TO` the same compatible target type. It overwrites
  the destination with that value and is not conditional on the destination's
  previous value. An incompatible typed pointer is a compile-time error rather
  than a reinterpretation or dynamic cast.
- Pointer arithmetic is not supported
- Because `POINTER TO` accepts any supported type reference, the parser accepts
  compositions such as `POINTER TO ARRAY[*] OF BYTE`. This is a syntax
  composition rule for the truST pointer extension; the semantic legality of
  the variable-length array still follows the declaration-location rules in
  `03-variables.md`.

### 4.3 IEC reference operations and lifetime

IEC 61131-3 Ed.3 §6.4.4.10.3 defines `REF(variable-or-instance)` and forbids
applying `REF(...)` to temporary storage, explicitly including `VAR_TEMP` and
variables inside functions. It also specifies that a dereferenced reference is
used like the referenced variable and that dereferencing `NULL` is an error.

truST therefore rejects `REF(...)` for literals, calls, computed values,
`VAR_TEMP`, function-local automatic storage, and function result variables.
Method-result storage is also rejected because its lifetime ends with the
invocation; this method-specific lifetime rule is a truST product constraint
where the IEC text is not explicit.

truST additionally rejects `REF(...)` for CONSTANT-qualified variables. IEC
does not exclude those variables from `REF(...)`, so this stricter behavior is
recorded as
`docs/IEC_DEVIATIONS.md#2026-07-26---ref-rejects-constant-qualified-variables`.

### 4.4 truST pointer and indirect-write policy

The following rules define the separate truST `POINTER TO` and `ADR(...)`
extension boundary. They are product behavior, not IEC reference requirements:

- `ADR(...)` requires an lvalue with stable addressable storage; literals,
  calls, and computed values are rejected.
- `ADR(...)` and `REF(...)` reject `CONSTANT` storage. `REF(...)` additionally
  enforces the IEC temporary/function-local lifetime restrictions in section
  4.3. A pointer or reference to an array element or structure member retains
  that exact selected storage identity.
- Assignment to a `VAR_INPUT` pointer slot is rejected because the parameter
  itself is read-only.
- Dereferencing a valid `VAR_INPUT` pointer produces the pointed-to storage,
  which remains writable unless the pointed-to declaration is independently
  constant. This applies through array indexes, structure fields, and nested
  selections.
- For `VAR_IN_OUT CONSTANT PT : POINTER TO T`, the pointer slot is constant but
  `PT^` denotes separate target storage. Rebinding `PT` is rejected; writing
  `PT^` is accepted unless `T` itself is constant.
- A non-pointer `VAR_INPUT` function-block instance remains read-only through
  its fields; the writable-pointee rule does not turn an input aggregate into
  indirect storage.
- Dereference reads and writes follow the declared target type recursively.
  A `NULL` dereference fails before any read or write; a failed indirect write
  leaves all storage unchanged.

## 5. Type Conversion Rules (Figures 11-12, Section 6.4.2)

### Implicit Conversions

IEC 61131-3 Ed.3 section 6.6.1.6 requires implicit conversion to preserve both
value and accuracy. truST therefore permits only this closed widening matrix:

```
SINT → INT → DINT → LINT
USINT → UINT → UDINT → ULINT
BYTE → WORD → DWORD → LWORD
SINT, INT → REAL
SINT, INT, DINT → LREAL
REAL → LREAL
```

Typed `DINT -> REAL` and `LINT -> LREAL` are not implicit conversions because
not every source value is exactly representable by the floating target. They
require an explicit conversion function, as do signed/unsigned cross-family,
numeric/`BOOL`, and `STRING`/`WSTRING` cross-family conversions. Contextual
untyped numeric literals remain assignable when the literal is representable by
the target. No implicit conversion is applied to `VAR_IN_OUT`.

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

For vendor compatibility, truST also accepts a parenthesized spelling:

```
VAR
  s4: STRING(MaxLen + 2);
  s5: WSTRING(MaxLen);
END_VAR
```

**Rules**:
- `STRING[n]`/`WSTRING[n]` declare a maximum length of `n` characters; `n` must be a positive integer constant expression. (IEC 61131-3 Ed.3, Table 10)
- The capacity expression uses the declaration site's complete constant graph
  and namespace/`USING` context. Forward and cross-file constants are valid
  when visible; source order does not change the capacity.
- A zero, negative, non-integer, undefined, mutable, ambiguous, cyclic, or
  overflowing capacity is rejected before the bounded type is registered.
- `STRING(n)`/`WSTRING(n)` are truST vendor-compatible aliases of the bracketed
  form. The parser preserves the enclosed expression and semantic validation
  applies the same constant-integer, positivity, and implementation-bound
  requirements as the bracketed form. IEC Table 10 defines the bracketed form;
  the parenthesized spelling is product behavior, not an IEC requirement.
- Default initial value of `STRING`/`WSTRING` is the empty string (`''` / `""`). (IEC 61131-3 Ed.3, Table 10)
- String literals used for initialization must be compatible with `ANY_STRING` and shall not exceed the declared maximum length. (IEC 61131-3 Ed.3, Figure 6)
- Callable string-library functions (`LEN`, `LEFT`, `RIGHT`, `MID`, `CONCAT`, `INSERT`, `DELETE`, `REPLACE`, `FIND`) are specified in `07-standard-functions.md`.

### Assignment and parameter-binding bounds

IEC 61131-3 Ed.3 section 6.6.1.2.2 permits an implementation-specific result
when a source string is longer than its assignment target. For bounded
`STRING[n]` and `WSTRING[n]`, truST applies the following rules:

- Ordinary assignment truncates an overlong value to the target's declared
  character capacity.
- `VAR_INPUT` copy-in truncates to the formal parameter's declared capacity
  without modifying the caller.
- A function result is first bounded by its declared return capacity. Ordinary
  assignment of that result, and function or function-block `VAR_OUTPUT`
  copy-back, truncate to the receiving target's declared capacity.
- `VAR_IN_OUT` requires the actual and formal to have the same string family
  and the same effective capacity after alias and constant-expression
  resolution. A width mismatch, including bounded-to-unbounded binding, is
  rejected with invalid-argument diagnostic category `E205` instead of performing an
  implicit truncating copy-in/copy-back conversion.
- Literal bounds and truncation count Unicode scalar values and never split one
  scalar value.

These rules apply at function and function-block call boundaries. They prevent
call copy-back from storing a value longer than the receiving declaration and
prevent a no-op `VAR_IN_OUT` call from silently changing caller state.

`STRING` and `WSTRING` remain distinct assignment families. Use the explicit
standard conversion functions when crossing between them.

This focused contract covers ordinary assignment, literal bounds, and direct
function/function-block parameter boundaries. Cross-family conversion and
storage-specific HMI, retain, I/O, and reference-write policies remain governed
by their own contracts and gaps.

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
- Character indexing takes exactly one `ANY_INT` subscript.
- Indexing a string yields or updates a compatible `CHAR`/`WCHAR` element.
- String library calls are specified in `07-standard-functions.md` §8.
- A source expression supplies exactly one signed or unsigned integer index;
  REAL, BOOL, bit-string, character, and multi-index forms are rejected.
- A statically known index below 1, or above a bounded declaration's capacity,
  is a compile-time range error. An index within declared capacity may still be
  beyond the value's current length and is checked at runtime.
- Position 1 is the first character. Reads and writes beyond the current
  string length fail with `runtime_index_out_of_bounds`; a failed write leaves
  the complete string and unrelated state unchanged.
- A character write replaces exactly one existing element. It does not extend
  the string, shrink it, or change its declared capacity.
- `STRING` indexing reads/writes `CHAR`; `WSTRING` indexing reads/writes
  `WCHAR`. Mixing the narrow and wide families is a compile-time type error.
- Aliases retain the indexed family and declared capacity. Indexed lvalues
  through structure fields, array elements, dereferences, and `VAR_IN_OUT`
  aliases retain the lifetime and write permissions of the complete base path.
  Constants, inputs, and other read-only bases remain read-only.
- Indexed `STRING` / `WSTRING` access follows the documented runtime element
  model in `DEV-017`; both VM reference access and the shipped string stdlib
  select Unicode scalar elements rather than raw UTF-8 bytes or raw 16-bit
  code units.

## 7. Parser acceptance and recovery boundaries

The following rules close the syntax-only boundary shared by the type and
selector forms above. Parser acceptance preserves a lossless syntax shape for
semantic analysis; it does not authorize a generic storage type, prove type
compatibility, establish reference lifetime, or validate a constant value.

- Every `ANY*` keyword is case-insensitive and is retained as a generic type
  reference wherever a type reference can occur, including derived-type shapes
  and POU signature positions. An `ANY*` keyword cannot be used as a
  declaration name. `ARRAY ... OF` and `POINTER TO` still require their element
  or target type after a generic context.
- A partial-access suffix accepts case-insensitive `%X`, `%B`, `%W`, and `%D`
  prefixes with a non-negative decimal index whose digits may contain legal
  separators; only `%X` may omit the prefix. The base may be a selected,
  indexed, or dereferenced expression and may remain an assignment target.
  A missing, signed, radix-prefixed, identifier, parenthesized, or otherwise
  general-expression selector is a syntax error rather than an ordinary field.
- `REF_TO T` and `POINTER TO T` require a complete target type. `REF(expr)` and
  `ADR(expr)` require a complete parenthesized argument; an empty `REF()` is
  retained as a complete call node for the semantic missing-argument
  diagnostic. Dereference is postfix (`expr^`) and composes with further field,
  index, read, and assignment syntax; prefix `^expr` is rejected.
  Assignment-attempt syntax requires both target and source. `NULL` remains a
  primary expression usable in assignment and equality/inequality syntax,
  with compatibility deferred to semantic analysis.
- String indexing retains exactly one complete expression between `[` and `]`
  and composes with selected, indexed, and dereferenced bases in both read and
  assignment-target positions. A missing index, empty second index, trailing
  comma, or missing closing bracket is rejected at the bounded postfix
  boundary.
- Enumerations and integer named-value types retain members, explicit values,
  and optional defaults; subranges retain signed or constant-expression bounds;
  arrays retain every dimension and nested/repetition initializer; structures,
  relative structures, overlap structures, and unions retain members and
  defaults. A missing array upper or element type, empty or unclosed
  enumeration, missing subrange upper bound, repetition without a value,
  missing relative-field type, or missing structure/union terminator produces
  a visible bounded parse failure.

These parser recovery choices are truST product behavior. The accepted source
forms cite IEC 61131-3 Ed.3 Tables 10-12 and 17 and sections 6.4.3, 6.4.4, and
6.6.1.3; IEC does not prescribe the lossless-node or bounded-recovery
representation.

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
