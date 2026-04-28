# Variables

IEC 61131-3 Edition 3.0 (2013) - Section 6.5

This specification defines variable declarations and qualifiers for trust-hir symbols.

## 1. Variable Declaration (Tables 13-14, Section 6.5.1)

### Basic Declaration Syntax

```
VAR
  identifier_list : type_specification;
  identifier_list : type_specification := initial_value;
END_VAR
```

### Declaration Examples (Table 13)

| No. | Description | Example |
|-----|-------------|---------|
| 1 | Single variable | `A: INT;` |
| 2 | Multiple variables | `A, B, C: INT;` |
| 3 | Variable with initial value | `X: BOOL := TRUE;` |
| 4 | Array variable | `Arr: ARRAY[1..10] OF INT;` |
| 5 | String with length | `Name: STRING[50];` |
| 6 | Reference type | `pInt: REF_TO INT;` |

### Initialization (Table 14)

| No. | Description | Example |
|-----|-------------|---------|
| 1 | Elementary type | `X: INT := 42;` |
| 2 | Array initialization | `A: ARRAY[1..3] OF INT := [1, 2, 3];` |
| 3 | Partial array init | `B: ARRAY[1..5] OF INT := [1, 2];` |
| 4 | Repetition count | `C: ARRAY[1..6] OF INT := [3(1, 2)];` |
| 5 | Structure init | `S: MyStruct := (field1 := 1, field2 := 2);` |
| 6 | FB instance init | `Timer: TON := (PT := T#1s);` |

For multi-name declarations, the declared initializer is materialized into
independent storage for each declared name. Aggregate values rely on the runtime
`Value::Struct` copy-on-write contract for subsequent field mutation.

Function-block instance aggregate initializers may target `VAR_INPUT`,
`VAR_OUTPUT`, and explicitly `VAR PUBLIC` members. `VAR_IN_OUT`, `VAR_TEMP`,
`VAR_EXTERNAL`, and non-public members are rejected because in-out bindings are
caller-supplied references and temporary/external/private storage is not an
instance-initializer surface.

## 2. Variable Section Keywords (Figure 7, Section 6.5.2)

### Input/Output Variables

| Keyword | Description | Scope |
|---------|-------------|-------|
| `VAR_INPUT` | Input parameters | Read-only inside POU |
| `VAR_OUTPUT` | Output parameters | Write inside, read outside |
| `VAR_IN_OUT` | In-out parameters | Read/write both |

### Local Variables

| Keyword | Description | Scope |
|---------|-------------|-------|
| `VAR` | Local variables | Persistent in FBs; in functions/methods equivalent to VAR_TEMP |
| `VAR_TEMP` | Temporary variables | Non-persistent, fresh each call |

### Global Variables

| Keyword | Description | Scope |
|---------|-------------|-------|
| `VAR_GLOBAL` | Global declaration | Configuration/resource element or namespace scope |
| `VAR_EXTERNAL` | External reference | Access to VAR_GLOBAL |

### Special Variables

| Keyword | Description | Scope |
|---------|-------------|-------|
| `VAR_ACCESS` | Access paths | For communication services |
| `VAR_CONFIG` | Instance-specific | Configuration initialization |

**Rules**:
- `VAR_ACCESS` binds a symbolic name to an access path; the declared type must match the target access path type. (IEC 61131-3 Ed.3, Table 13, 6.5.2.2)
- `READ_ONLY` access paths are not assignable in ST code. (IEC 61131-3 Ed.3, 6.5.2.2)
- `VAR_CONFIG` entries shall use the same type as the target variable. (IEC 61131-3 Ed.3, 6.5.2.2)
- Instance-specific initialization in `VAR_CONFIG` is not allowed for `VAR_TEMP`, `VAR_EXTERNAL`, `VAR_IN_OUT`, or `VAR CONSTANT` targets. (IEC 61131-3 Ed.3, 6.5.2.2)
- trust-hir validates simple access paths only; cross-resource/program instance mapping is out-of-scope. (IEC 61131-3 Ed.3, Tables 13-16; DEV-003)

## 3. Variable Qualifiers

### Persistence Qualifiers

| Qualifier | Description | Behavior |
|-----------|-------------|----------|
| `RETAIN` | Retentive | Value retained on warm restart |
| `NON_RETAIN` | Non-retentive | Value initialized on warm restart |
| `PERSISTENT` | Persistent | Vendor extension; treated like RETAIN |
| (none) | Default | Implementer-specific |

```
VAR RETAIN
  Counter: INT := 0;  // Retained across power cycles
END_VAR

VAR NON_RETAIN
  TempData: INT;      // Re-initialized on restart
END_VAR
```

**Rules**:
- RETAIN/NON_RETAIN apply to VAR, VAR_INPUT, VAR_OUTPUT, VAR_GLOBAL, and static VAR sections; not VAR_IN_OUT. (IEC 61131-3 Ed.3, 6.5.6.1-6.5.6.2)
- Only one of CONSTANT, RETAIN, NON_RETAIN, or PERSISTENT may appear per VAR section. (IEC 61131-3 Ed.3, Figure 7)
- PERSISTENT is accepted as a vendor extension and validated like RETAIN. (DEV-007)

### Constant Qualifier

| Qualifier | Description | Behavior |
|-----------|-------------|----------|
| `CONSTANT` | Named constant | Cannot be modified |

```
VAR CONSTANT
  PI: REAL := 3.14159;
  MaxCount: INT := 100;
END_VAR
```

**Rules**:
- `CONSTANT` may qualify any `VAR_*` section keyword listed in IEC Figure 7.
- Assignment to a `CONSTANT` declaration from within the enclosing entity is rejected.
- `VAR_INPUT CONSTANT` is valid and redundant: inputs are already read-only inside the entity.
- `VAR_IN_OUT CONSTANT` preserves normal `VAR_IN_OUT` call-site binding rules, but the aliased storage is read-only inside the entity.
- `VAR_OUTPUT CONSTANT` is valid and leaves the output unwritable from within the entity.
- `VAR_TEMP CONSTANT` is valid and behaves as a read-only temporary variable.
- Function block instances shall not be declared in `CONSTANT` sections. (IEC Figure 7 footnote `*`)
- Only `VAR CONSTANT`, `VAR_GLOBAL CONSTANT`, and `VAR_EXTERNAL CONSTANT` participate in named compile-time constant-expression evaluation; parameter-block and `VAR_TEMP CONSTANT` declarations are runtime storage with read-only semantics.

### Edge Detection Qualifiers (Table 14)

| Qualifier | Description | Use |
|-----------|-------------|-----|
| `R_EDGE` | Rising edge | VAR_INPUT only |
| `F_EDGE` | Falling edge | VAR_INPUT only |

```
FUNCTION_BLOCK MyFB
VAR_INPUT
  Trigger: BOOL R_EDGE;  // Rising edge detection
END_VAR
// Body sees Trigger=TRUE only on 0->1 transition
END_FUNCTION_BLOCK
```

## 4. Access Specifiers (Section 6.5.2.3)

For variables within CLASS and FUNCTION_BLOCK:

| Specifier | Description | Access |
|-----------|-------------|--------|
| `PUBLIC` | Public access | Anywhere class is visible |
| `PROTECTED` | Protected access | Own class and derived classes |
| `PRIVATE` | Private access | Own class only |
| `INTERNAL` | Internal access | Within same NAMESPACE only |

```
CLASS MyClass
  VAR PUBLIC
    PublicVar: INT;     // Accessible everywhere
  END_VAR
  VAR PRIVATE
    PrivateVar: INT;    // Only within MyClass
  END_VAR
  VAR PROTECTED
    ProtectedVar: INT;  // MyClass and derived classes
  END_VAR
END_CLASS
```

**Default Access**:
- PROTECTED is the default access specifier for variables
- `PROTECTED` members are visible to the declaring class and derived classes

## 5. Directly Represented Variables (Table 16, Section 6.5.5)

Directly represented variables map to physical I/O or memory locations.
See `10-runtime-semantics.md` §9 for the language/runtime I/O contract and
`11-runtime-engine.md` §6.4 for the process image/runtime engine view.

### Syntax

```
%<Location><Size><Address>
```

### Location Prefixes

| Prefix | Description |
|--------|-------------|
| `I` | Input location |
| `Q` | Output location |
| `M` | Memory location |

### Size Prefixes

| Prefix | Size | Type |
|--------|------|------|
| `X` | 1 bit | BOOL |
| `B` | 8 bits | BYTE |
| `W` | 16 bits | WORD |
| `D` | 32 bits | DWORD |
| `L` | 64 bits | LWORD |

### Examples (Table 16)

| No. | Variable | Description |
|-----|----------|-------------|
| 1 | `%IX1` | Input location 1, single bit |
| 2 | `%IW6` | Input word at location 6 |
| 3 | `%QB17` | Output byte at location 17 |
| 4 | `%MD48` | Memory double word at location 48 |
| 5 | `%QX7.5` | Output bit 5 of byte 7 |

### Hierarchical Addressing

```
%IX1.2.3.4     // Hierarchical address (leftmost = highest level)
%QW2.5.7.1    // Additional levels are implementer-specific
```

### Symbolic Mapping with AT

```
VAR
  StartButton AT %IX0.0: BOOL;     // Maps to physical input
  MotorSpeed  AT %QW10:  INT;      // Maps to physical output
END_VAR
```

### Incomplete Address Specification

```
VAR
  LocalAddr AT %I*: BOOL;  // Location determined by VAR_CONFIG
END_VAR
```

**Rules**:
- Incomplete direct addresses (`%I*`, `%Q*`, `%M*`) are not allowed in `VAR_INPUT` or `VAR_IN_OUT` sections. (IEC 61131-3 Ed.3, 6.5.5.4)
- Each incomplete direct address must be fully specified in a `VAR_CONFIG` entry using `AT` and a concrete address (no `*`). (IEC 61131-3 Ed.3, 6.5.5.4)

## 6. Variable-Length Arrays (Tables 15-16, Section 6.5.4.5)

### Declaration

Variable-length arrays are declared using `*` for bounds:

```
FUNCTION Sum: INT
VAR_INPUT
  Values: ARRAY[*] OF INT;  // Variable-length input array
END_VAR
VAR
  i: INT;
  result: INT := 0;
END_VAR
FOR i := LOWER_BOUND(Values, 1) TO UPPER_BOUND(Values, 1) DO
  result := result + Values[i];
END_FOR;
Sum := result;
END_FUNCTION
```

### Bound Functions

| Function | Description |
|----------|-------------|
| `LOWER_BOUND(arr, dim)` | Lower bound of dimension |
| `UPPER_BOUND(arr, dim)` | Upper bound of dimension |

**Rules**:
- Only allowed in VAR_INPUT, VAR_OUTPUT, VAR_IN_OUT
- Only usable in FUNCTIONs and METHODs
- Dimensions must match at call site
- Multiple dimensions: `ARRAY[*, *] OF INT`

## 7. Variable Scope Rules (Section 6.5.2.2)

### Scope Hierarchy

1. **Local variables** (VAR, VAR_TEMP) - Visible within declaring POU
2. **Parameters** (VAR_INPUT, VAR_OUTPUT, VAR_IN_OUT) - Part of POU interface
3. **Global variables** (VAR_GLOBAL) - Accessible via `VAR_EXTERNAL` in strict IEC form, or via vendor-parity bare/qualified access in truST

### Name Resolution

1. Local names take precedence over global names
2. Qualified names can disambiguate namespace-backed globals: `GVL.shared`
3. THIS.member for class/FB member access
4. Directly represented variables are globally unique

### Lifetime

| Section | Initialization | Persistence |
|---------|---------------|-------------|
| VAR (in FB) | Once at instantiation | Persists across calls |
| VAR (in Function) | Each call | Lost after return |
| VAR_TEMP | Each call | Lost after return |
| VAR_INPUT | Each call (from caller) | - |
| VAR_OUTPUT | Each call | - |
| VAR_IN_OUT | Each call (from caller) | - |

## 8. External Variable Declaration (Figure 8, Section 6.5.6)

### VAR_GLOBAL and VAR_EXTERNAL Relationship

```
// In CONFIGURATION or RESOURCE
VAR_GLOBAL
  GlobalCounter: INT := 0;
  GlobalTimer: TON;
END_VAR

// In PROGRAM, FUNCTION_BLOCK, or CLASS
VAR_EXTERNAL
  GlobalCounter: INT;    // Must match type exactly
  GlobalTimer: TON;
END_VAR
```

### VAR_EXTERNAL CONSTANT

```
VAR_GLOBAL CONSTANT
  MaxItems: INT := 100;
END_VAR

// Reference as constant
VAR_EXTERNAL CONSTANT
  MaxItems: INT;
END_VAR
```

**Rules**:
- VAR_EXTERNAL creates a reference to VAR_GLOBAL declared in the associated program, configuration, or resource. (IEC 61131-3 Ed.3, 6.5.2.2, Figure 8)
- Type must exactly match the VAR_GLOBAL declaration. (IEC 61131-3 Ed.3, 6.5.2.2)
- VAR_EXTERNAL cannot declare an initial value. (IEC 61131-3 Ed.3, 6.5.1.3)
- VAR_EXTERNAL CONSTANT is required when the referenced VAR_GLOBAL is CONSTANT. (IEC 61131-3 Ed.3, Figure 8)
- Error if VAR_GLOBAL doesn't exist. (IEC 61131-3 Ed.3, 6.5.2.2)
- Modification of VAR_EXTERNAL CONSTANT is an error. (IEC 61131-3 Ed.3, 6.5.2.2)

truST vendor-parity note:
- `VAR_EXTERNAL` remains supported and type-checked, but it is optional in truST for vendor-parity global access paths. See `docs/IEC_DEVIATIONS.md`.
- `NAMESPACE ... VAR_GLOBAL ... END_NAMESPACE` is accepted as a vendor-style namespaced GVL extension, and qualified access such as `GVL.shared` resolves directly.

## 9. Declaration Rules Summary

truST vendor-parity note:
- In addition to the IEC-declared locations below, truST accepts top-level file-scope `VAR_GLOBAL ... END_VAR` and namespaced `VAR_GLOBAL` blocks as vendor-style GVL extensions. See `docs/IEC_DEVIATIONS.md`.

### What Can Be Declared Where

| Section | Function | FB | Program | Class | Config |
|---------|----------|-----|---------|-------|--------|
| VAR | Yes | Yes | Yes | Yes | - |
| VAR_TEMP | Yes | Yes | Yes | Yes | - |
| VAR_INPUT | Yes | Yes | Yes | - | - |
| VAR_OUTPUT | Yes | Yes | Yes | - | - |
| VAR_IN_OUT | Yes | Yes | Yes | - | - |
| VAR_EXTERNAL | Yes | Yes | Yes | Yes | - |
| VAR_GLOBAL | - | - | Yes | - | Yes |
| VAR_ACCESS | - | - | - | - | Yes |
| VAR_CONFIG | - | - | - | - | Yes |

### Qualifier Combinations

| Qualifier | VAR | VAR_INPUT | VAR_OUTPUT | VAR_GLOBAL |
|-----------|-----|-----------|------------|------------|
| CONSTANT | Yes | - | - | Yes |
| RETAIN | Yes | Yes | Yes | Yes |
| NON_RETAIN | Yes | Yes | Yes | Yes |
| PERSISTENT (DEV-007) | Yes | Yes | Yes | Yes |
| R_EDGE | - | Yes | - | - |
| F_EDGE | - | Yes | - | - |

## Implementation Notes for trust-hir

### Symbol Table Requirements

1. Track variable name, type, and location
2. Record scope (local, parameter, global)
3. Store qualifiers (CONSTANT, RETAIN, NON_RETAIN, PERSISTENT, access specifier)
4. Maintain reference to initial value expression
5. For AT: store direct address mapping (IEC 61131-3 Ed.3, Table 16)

### Semantic Checks

1. **Undefined variable**: Reference to undeclared identifier
2. **Duplicate declaration**: Same name in same scope
3. **Type mismatch**: Initial value type vs declared type
4. **Constant assignment**: Attempt to modify CONSTANT
5. **Input modification**: Attempt to modify VAR_INPUT
6. **Missing VAR_GLOBAL**: VAR_EXTERNAL without corresponding global
7. **Invalid qualifier**: Wrong qualifier for variable section
8. **Scope violation**: Access specifier violation
9. **Missing VAR_CONFIG mapping**: Incomplete AT address without a concrete VAR_CONFIG entry

### Error Conditions

1. Variable used before declaration
2. Variable declared multiple times in same scope
3. Assignment to CONSTANT or VAR_INPUT
4. Type mismatch in initialization
5. Invalid direct address format
6. VAR_EXTERNAL without matching VAR_GLOBAL
7. Access specifier violation (PRIVATE member access)
8. Array bounds out of range
