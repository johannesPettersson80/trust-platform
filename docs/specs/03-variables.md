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

Under IEC 61131-3 Ed.3 Section 6.5.1.3, an explicit variable initial value
shall be a literal or a constant expression. Constant expressions may compose
literals, declared constant variables, enumerated values, and operators.
Referencing an ordinary mutable variable from another variable initializer is
rejected; declaring the referenced value in a `VAR CONSTANT` section makes the
same supported expression a valid initializer.

Reference variables have the IEC 61131-3 Ed.3 section 6.4.4.10.2 exception:
their initial value may be `NULL` or `REF(...)` naming an already-declared
eligible variable, function-block instance, or class instance. The referenced
storage is not treated as a constant-expression dependency; ordinary `REF()`
lvalue, type, lifetime, and visibility checks still apply. This exception is
specific to a reference-typed declaration and does not permit another mutable
value expression in an initializer.

Within one accepted declaration scope, textual declaration order is not
constant-dependency order. truST discovers the constants visible from that
scope, resolves their case-insensitive dependency graph, and evaluates an
initializer only after every constant on which it depends has a value. This
applies to root and namespace globals, POU-local constant sections, and
configuration/resource globals. A visible constant may therefore be declared
later in the same source unit or in a later project source unit. Lexical,
namespace/`USING`, configuration, and resource visibility still apply:
dependency ordering does not make a constant visible outside its scope.

A self-reference or multi-constant dependency cycle is a compile error.
Undefined names, references to mutable variables, division or remainder by
zero, integer overflow, and another invalid constant operation are rejected
before a runtime is returned. Each name in a multi-name constant declaration
receives the same evaluated declaration value without sharing mutable runtime
storage.

IEC defines which initial values and constant expressions are valid, but it
does not define repository source-unit order. The source-order-independent
dependency graph is the truST project-assembly contract specified further in
`10-runtime-semantics.md` section 7.10; it is not an IEC deviation.

For an explicit variable initializer that is not a literal, constant
expression, or the reference-initialization form above, including elementary
and aggregate initializers, truST reports category `E202` with the stable
message prefix:

```text
variable initializer must be a literal or constant expression
```

The category and message are a truST compiler diagnostic contract. They make
the IEC rejection observable but are not themselves IEC-defined wording.

For multi-name declarations, the declared initializer is materialized into
independent storage for each declared name. Aggregate values rely on the runtime
`Value::Struct` copy-on-write contract for subsequent field mutation.

Function-block instance aggregate initializers may target `VAR_INPUT`,
`VAR_OUTPUT`, and explicitly `VAR PUBLIC` members. `VAR_IN_OUT`, `VAR_TEMP`,
`VAR_EXTERNAL`, and non-public members are rejected because in-out bindings are
caller-supplied references and temporary/external/private storage is not an
instance-initializer surface.

### POU declaration initializer eligibility and resolution

IEC 61131-3 Ed.3 §6.5.1.3 permits a variable-specific literal or constant
expression after `:=`. The formal grammar in Annex A uses initialized variable
declarations for `VAR_INPUT` and `VAR_OUTPUT`, but uses the no-initializer
declaration form for `VAR_IN_OUT`; §6.5.1.3 separately forbids an initial value
on `VAR_EXTERNAL`. truST therefore applies this declaration contract:

- `VAR`, `VAR_TEMP`, `VAR_STAT`, `VAR_INPUT`, and `VAR_OUTPUT` may declare a
  compatible initial value;
- `VAR_IN_OUT` may not declare an initial value because its storage and initial
  value are supplied by the caller; and
- `VAR_EXTERNAL` may not declare an initial value because it denotes existing
  global storage.

An accepted POU initializer uses the complete constant graph visible at the
declaration site. Constants in a later variable section, a later source unit,
the enclosing namespace, or an unambiguous active `USING` namespace are
eligible even when they are textually later. Lookup is ASCII-case-insensitive
and retains lexical POU and namespace identity: same-leaf constants in two
POUs or namespaces do not alias, and an unimported namespace does not leak into
unqualified lookup.

The compiler resolves every constant dependency before it materializes any
consuming parameter or local initializer. Undefined, mutable, ambiguous,
cyclic, non-constant, divide-by-zero, overflow, or incompatible operands reject
the POU before runtime metadata or storage is returned. Source permutation
does not change a valid initializer's resolved value.

`VAR_INPUT` and `VAR_OUTPUT` have different call behavior despite sharing the
initialized-declaration grammar:

- a supplied `VAR_INPUT` actual overrides the formal initializer; an omitted
  function or method input uses the initializer and otherwise the declared
  type default;
- the first omitted call of a function-block input uses its initializer and a
  later omitted call retains the instance's most recently stored input;
- a function or method `VAR_OUTPUT` begins every call with its declared
  initializer, or the declared type default when omitted, before the body can
  modify and copy it to a mapped caller target; and
- a function-block or program `VAR_OUTPUT` is initialized when its owning
  instance is created and then persists like the rest of that instance's
  state.

These rules govern declaration initialization. They are separate from a
function-block instance aggregate initializer, which overrides eligible member
defaults for one declared instance.

### Parser initializer classification and recovery

The lossless parser recognizes a parenthesized `InitializerList` only after
`:=` in a variable declaration or type-default position. A formal call
argument such as `F(Input := 1)` remains an `Arg` inside a `CallExpr`, and an
enumeration value such as `Running := 1` remains an `EnumValue`; neither is
reclassified as a declaration initializer.

Structure initializers use named members. A positional form such as `(1, 2)`
is rejected with:

```text
positional struct initializers are not supported; use named field initializers
```

An empty structure initializer is also rejected. Missing named-member values
diagnose `expected aggregate initializer value`. Recovery shall preserve the
following variable declaration and shall stop at `END_VAR` or end of file
rather than consuming an enclosing declaration. For generated positional
shapes containing otherwise balanced expressions, recovery emits no more than
two diagnostics and preserves the following declaration. These diagnostic and
recovery guarantees are truST parser contracts for malformed source; they do
not make the malformed forms valid IEC Structured Text.

For syntax-tree consumers, ordinary expression nodes and aggregate initializer
nodes are intentionally separate classifier sets. Ordinary expressions are
`Literal`, `NameRef`, `BinaryExpr`, `UnaryExpr`, `CallExpr`, `IndexExpr`,
`FieldExpr`, `DerefExpr`, `AddrExpr`, `ParenExpr`, `ThisExpr`, `SuperExpr`,
and `SizeOfExpr`. Aggregate initializers are `InitializerList` and
`ArrayInitializer`; neither is an ordinary expression. Initializer position
accepts the union of the ordinary-expression and aggregate-initializer sets.
Parser trivia, including `Pragma`, is neither an expression, statement, nor
initializer expression. This governs truST's internal syntax API and does not
expand the valid IEC initializer grammar.

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
- An omitted `VAR_ACCESS` direction defaults to `READ_ONLY`. `READ_ONLY`
  access paths are readable but cannot be modified through the external access
  boundary; a rejected write is atomic. `READ_WRITE` permits both operations
  only when the target itself is eligible for writes. (IEC 61131-3 Ed.3,
  §6.8.1, Table 62)
- A variable declared in `VAR_TEMP`, `VAR_EXTERNAL`, or `VAR_IN_OUT` cannot be
  exposed by `VAR_ACCESS`. A `CONSTANT` target and a function-block input that
  is externally connected to another variable remain read-only even if the
  access declaration requests `READ_WRITE`. (IEC 61131-3 Ed.3, §6.8.1,
  Table 62)
- A `VAR_ACCESS` symbolic name that collides with a global in the same effective
  namespace is a duplicate and leaves lookup ambiguous; resolution must not
  silently prefer the access declaration or the global.
- `VAR_ACCESS` identities are unique under the language's
  ASCII-case-insensitive declaration comparison. Runtime clients use the
  accepted declaration spelling as the external binding identity; a duplicate
  case spelling must not replace the first binding.
- `VAR_CONFIG` entries shall use the same type as the target variable. (IEC 61131-3 Ed.3, 6.5.2.2)
- Instance-specific initialization in `VAR_CONFIG` is not allowed for `VAR_TEMP`, `VAR_EXTERNAL`, `VAR_IN_OUT`, or `VAR CONSTANT` targets. (IEC 61131-3 Ed.3, 6.5.2.2)
- A valid instance-specific initializer may target a persistent program
  variable, a nested function-block member, or a structure component. It is
  applied after allocation and overrides the declaration/type default for that
  exact instance only. Rejection for type or target eligibility occurs before
  any target value is modified.
- Within the supported static project model, `VAR_CONFIG` resolves program
  instances after all project files are merged. It accepts a unique program
  instance declared directly in a configuration or inside a resource and may
  continue through nested function-block fields to the configured variable.
- For the supported resource subset, truST accepts that program instance by an
  unqualified path such as `P1.field` rather than requiring the complete
  resource-qualified IEC hierarchy. This simplified path is governed by the
  existing normative omission recorded as DEV-003.
- A program-instance name that resolves to more than one resource target is
  ambiguous and is rejected before target type or initializer checks.
- truST does not yet model every IEC communication-service topology or every
  cross-resource access-path form; that remaining normative omission is
  recorded as DEV-003.

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
- `RETAIN` and `NON_RETAIN` apply only to declarations that own state across
  calls or cycles: ordinary `VAR` in function blocks, programs, and classes;
  truST `VAR_STAT`; function-block/program `VAR_INPUT` and `VAR_OUTPUT`; and
  `VAR_GLOBAL`. Function and method `VAR`, `VAR_INPUT`, and `VAR_OUTPUT` are
  call-local and do not accept a retention policy. (IEC 61131-3 Ed.3,
  6.5.6.1-6.5.6.2)
- `VAR_IN_OUT`, `VAR_TEMP`, `VAR_EXTERNAL`, `VAR_ACCESS`, and `VAR_CONFIG`
  never own a retention policy.
- Exactly one occurrence of one of `CONSTANT`, `RETAIN`, `NON_RETAIN`, or
  `PERSISTENT` may appear per declaration section. Repeating a qualifier or
  combining two different qualifiers is an error. (IEC 61131-3 Ed.3,
  Figure 7)
- `PERSISTENT` is accepted as a documented vendor extension and has exactly
  the same placement rules as `RETAIN`.

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
- `CONSTANT` may qualify every legal section that directly declares storage:
  `VAR`, truST `VAR_STAT`, `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`,
  `VAR_TEMP`, `VAR_GLOBAL`, and `VAR_EXTERNAL`. `VAR_ACCESS` and `VAR_CONFIG`
  are binding/configuration sections rather than storage declarations and do
  not accept `CONSTANT`.
- Assignment to a `CONSTANT` declaration from within the enclosing entity is rejected.
- `VAR_INPUT CONSTANT` is valid and redundant: inputs are already read-only inside the entity.
- `VAR_IN_OUT CONSTANT` preserves normal `VAR_IN_OUT` call-site binding rules, but the aliased storage is read-only inside the entity.
- `VAR_OUTPUT CONSTANT` is valid and leaves the output unwritable from within the entity.
- `VAR_TEMP CONSTANT` is valid and behaves as a read-only temporary variable.
- Function block instances shall not be declared in `CONSTANT` sections. (IEC Figure 7 footnote `*`)
- Only `VAR CONSTANT`, `VAR_GLOBAL CONSTANT`, and `VAR_EXTERNAL CONSTANT` participate in named compile-time constant-expression evaluation; parameter-block and `VAR_TEMP CONSTANT` declarations are runtime storage with read-only semantics.
- The closed placement interpretation is recorded in
  `docs/IEC_DECISIONS.md`; accepting a qualifier never permits a variable
  section in an owner that rejects that section.

### Edge Detection Qualifiers (Tables 40 and 47)

| Qualifier | Description | Use |
|-----------|-------------|-----|
| `R_EDGE` | Rising edge | Function-block/program `VAR_INPUT` `BOOL` only |
| `F_EDGE` | Falling edge | Function-block/program `VAR_INPUT` `BOOL` only |

```
FUNCTION_BLOCK MyFB
VAR_INPUT
  Trigger: BOOL R_EDGE;  // Rising edge detection
END_VAR
// Body sees Trigger=TRUE only on 0->1 transition
END_FUNCTION_BLOCK
```

**Rules**:
- The closed declaration grammar is
  `variable_name (',' variable_name)* ':' BOOL ('R_EDGE' | 'F_EDGE') ';'`.
  An edge declaration has no initializer and accepts exactly one edge suffix.
  (IEC 61131-3 Ed.3 Annex A `Edge_Decl`)
- Edge declarations are legal only in `VAR_INPUT` owned by a function block or
  program. A function or method input, any non-input section, and every
  non-`BOOL` declaration are rejected. (IEC 61131-3 Ed.3 Tables 40 and 47)
- Each declared name owns an independent hidden trigger state. `R_EDGE` is
  equivalent to a private implicit `R_TRIG` instance and `F_EDGE` to a private
  implicit `F_TRIG` instance. The hidden identity is not a user symbol and
  cannot collide with or be referenced through a source declaration.
- The value visible to the owning function-block/program body is the trigger's
  `Q` pulse for the current executed invocation or cycle, not the raw external
  input level. The raw input is sampled once at that boundary.
- A function-block method cannot read or write an edge-qualified input; only
  the function-block body receives the transformed pulse. (IEC 61131-3 Ed.3
  6.6.7.2.3)
- `VAR_INPUT RETAIN`, `VAR_INPUT NON_RETAIN`, and truST
  `VAR_INPUT PERSISTENT` may contain edge declarations. The raw stored input
  and its hidden trigger phase share that section policy. This prevents a warm
  restart of retained state from fabricating an edge. An unqualified or
  non-retained hidden phase is reinitialized under the normal restart rules.
- `VAR_INPUT CONSTANT` cannot contain an edge declaration because the
  declaration implies a function-block instance and IEC Figure 7 forbids
  function-block instances in `CONSTANT` sections.

## 4. Member Access Specifiers (Sections 6.6.5.10, 6.6.7.7, and 6.9)

An access specifier controls use of a member after the containing class or
function-block type itself has been resolved. It never makes an inaccessible
type or an element of an `INTERNAL` containing namespace visible. IEC Figure
29 requires every containing namespace/type restriction and the member
restriction to be satisfied; an outer element can narrow access but cannot
broaden it.

For ordinary `VAR` members of a `CLASS` or object-oriented
`FUNCTION_BLOCK`:

| Specifier | Permitted access |
|-----------|------------------|
| `PUBLIC` | Anywhere the containing type is accessible |
| `PROTECTED` | The defining class/FB and every derived class/FB |
| `PRIVATE` | Only the defining class/FB; the member is not inherited |
| `INTERNAL` | Any POU in the exact declaring namespace; inherited only within that namespace |

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

### 4.1 Defaults and inheritance

- `PROTECTED` is the default for class ordinary `VAR` members (IEC Table 48
  and §6.6.5.10).
- truST applies the object-oriented function-block profile from IEC Table 53
  and §6.6.7.7, so `PROTECTED` is also the uniform default for function-block
  ordinary `VAR` members. The Table 40/Table 53 interpretation is recorded in
  `docs/IEC_DECISIONS.md`.
- A `PRIVATE` member is usable only by the POU that declares it. It is not
  inherited and a same-spelled declaration in a derived POU is a new member,
  not an override.
- A `PROTECTED` member is inherited and usable from the declaring POU and any
  depth of derived POU, including through `THIS` or `SUPER`.
- An `INTERNAL` member is usable and inheritable only when the access site and
  declaring POU have the same fully qualified namespace path. A parent,
  child, or sibling namespace is not the same namespace.

### 4.2 Function-block directional members

IEC §6.6.7.7 defines direction-specific access independently of the ordinary
`VAR` matrix:

| Section | Access contract |
|---------|-----------------|
| `VAR_INPUT` | Implicitly `PUBLIC`; supplied through the FB call and read-only in the FB |
| `VAR_OUTPUT` | Implicitly `PUBLIC`; writable in the FB and externally read-only |
| `VAR_IN_OUT` | Usable only in the FB body and the call statement; it is not a generally accessible member |
| `VAR_EXTERNAL` | Implicitly `PROTECTED` |
| `VAR_TEMP` | Call-local to the FB body; never an externally accessible member |

No explicit `PUBLIC`, `PROTECTED`, `PRIVATE`, or `INTERNAL` token is permitted
on those function-block sections. An explicit token is rejected instead of
being silently ignored.

### 4.3 Access and storage qualifiers

For an ordinary class/FB `VAR` section, exactly one access specifier may be
combined with one legal storage qualifier. IEC §6.6.5.10 permits the access
and storage specifiers in either order, for example `VAR PUBLIC RETAIN` and
`VAR RETAIN PUBLIC`. Duplicate or conflicting access specifiers are invalid.
An access specifier changes visibility only; it does not alter constant,
retention, initialization, or ownership semantics.

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

The semantic symbol record preserves the complete direct-address spelling from
the declaration, including location, size, hierarchy separators, and numeric
components. Later lowering may normalize the address into process-image
coordinates, but it must not silently change the source-level binding retained
by HIR.

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
- Semantically allowed as `VAR_INPUT`, `VAR_OUTPUT`, or `VAR_IN_OUT` in
  functions and methods, and as `VAR_IN_OUT` in function blocks. (IEC
  61131-3 Ed.3, 6.5.3)
- The lossless parser accepts the `ARRAY[*]` type shape in any syntactically
  valid variable type-reference position, including function-block
  `VAR_INPUT` and the target type of the truST `POINTER TO` extension. This
  permissive parse preserves source for tooling; semantic validation still
  rejects declaration locations outside the allowed set above.
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

| Owner and section | Initialization | Persistence |
|-------------------|---------------|-------------|
| Function/method `VAR` | Explicit initializer or type default at each call | Lost after return |
| Function/method `VAR_TEMP` | Explicit initializer or type default at each call | Lost after return |
| Function/method `VAR_STAT` | Explicit initializer or type default at runtime/instance creation | Persists across calls; method state is receiver-local |
| Function/method `VAR_INPUT` | Supplied actual, otherwise formal initializer or type default | Call-local |
| Function/method `VAR_OUTPUT` | Formal initializer or type default at each call | Copied to a mapped actual on return |
| Function block `VAR`/`VAR_STAT`/`VAR_OUTPUT` | Explicit initializer or type default at instance creation | Persists across calls |
| Function block `VAR_TEMP` | Explicit initializer or type default at each invocation | Lost after return |
| Function block `VAR_INPUT` | Supplied actual; first omission uses formal initializer/type default; later omission reuses stored input | Instance state |
| Program `VAR`/`VAR_STAT`/`VAR_INPUT`/`VAR_OUTPUT` | Explicit initializer or type default at instance creation | Persists across cycles |
| Program `VAR_TEMP` | Explicit initializer or type default at each cycle | Lost after the cycle |
| Any `VAR_IN_OUT` | Caller-supplied referenced storage; no declaration initializer | Caller-owned |

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
- In addition to the IEC-declared locations below, truST accepts top-level
  file-scope `VAR_GLOBAL ... END_VAR` and namespaced `VAR_GLOBAL` blocks as
  documented vendor-style GVL extensions. Because IEC does not define those
  forms, they are not IEC deviations.

### What Can Be Declared Where

The matrix is closed: a section not marked `Yes` is rejected in that owner
before a POU or configuration model is returned.

| Section | Function | Method | FB | Program | Class | Interface | Configuration/resource |
|---------|----------|--------|----|---------|-------|-----------|------------------------|
| `VAR` | Yes | Yes | Yes | Yes | Yes | - | - |
| `VAR_STAT` (truST extension) | Yes | Yes | Yes | Yes | Yes | - | - |
| `VAR_TEMP` | Yes | Yes | Yes | Yes | - | - | - |
| `VAR_INPUT` | Yes | Yes | Yes | Yes | - | - | - |
| `VAR_OUTPUT` | Yes | Yes | Yes | Yes | - | - | - |
| `VAR_IN_OUT` | Yes | Yes | Yes | Yes | - | - | - |
| `VAR_EXTERNAL` | Yes | Yes | Yes | Yes | Yes | - | - |
| `VAR_GLOBAL` | - | - | - | Yes | - | - | Yes |
| `VAR_ACCESS` | - | - | - | Yes | - | - | Yes |
| `VAR_CONFIG` | - | - | - | - | - | - | Configuration only |

This follows IEC 61131-3 Ed.3 Tables 19, 40, 47, 48, and 51 plus the Annex A
POU productions. In particular, Table 48 permits only ordinary and external
class variables and explicitly forbids class input, output, in-out, and
temporary sections. Table 47 adds program-local `VAR_GLOBAL` and `VAR_ACCESS`
to the function-block-like section set. Interfaces own method prototypes and
do not own variable sections.

`VAR_STAT` is not an IEC keyword. truST accepts it in every owner that accepts
ordinary `VAR`; it has the storage behavior specified in
`01-lexical-elements.md` and `10-runtime-semantics.md`. Top-level and namespace
`VAR_GLOBAL` remain the separately documented vendor-style GVL extensions.

### Qualifier Combinations

The matrix is closed. `Yes` means that the qualifier is accepted only when the
owner also accepts the section in the preceding ownership matrix. A blank cell
is rejected before semantic or runtime metadata is returned.

| Section and owner | `CONSTANT` | `RETAIN` / `NON_RETAIN` / `PERSISTENT` |
|-------------------|------------|-----------------------------------------|
| Function/method `VAR` | Yes | - |
| Function-block/program/class `VAR` | Yes | Yes |
| `VAR_STAT` in any legal owner (truST extension) | Yes | Yes |
| Function/method `VAR_INPUT` | Yes | - |
| Function-block/program `VAR_INPUT` | Yes | Yes |
| Function/method `VAR_OUTPUT` | Yes | - |
| Function-block/program `VAR_OUTPUT` | Yes | Yes |
| `VAR_IN_OUT` | Yes | - |
| `VAR_TEMP` | Yes | - |
| `VAR_EXTERNAL` | Yes | - |
| `VAR_GLOBAL` | Yes | Yes |
| `VAR_ACCESS` | - | - |
| `VAR_CONFIG` | - | - |

`R_EDGE` and `F_EDGE` are per-variable suffixes rather than section
qualifiers. They are accepted only on `BOOL` declarations in
function-block/program `VAR_INPUT`; they are rejected in functions, methods,
classes, and every other variable section. (IEC 61131-3 Ed.3 Tables 40 and 47)

## Implementation Notes for trust-hir

### Symbol Table Requirements

1. Track variable name, type, and location
2. Record scope (local, parameter, global)
3. Store section qualifiers (CONSTANT, RETAIN, NON_RETAIN, PERSISTENT, access
   specifier) and the per-declaration R_EDGE/F_EDGE suffix
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
