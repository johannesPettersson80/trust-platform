# Runtime Semantics

## Status and scope
- Current runtime (production): bytecode-VM execution over STBC modules (`ExecutionBackend::BytecodeVm`).
- `run`/`play` accept `vm` only; `interpreter` is rejected in CLI/config startup selection.
- Helper evaluation remains only for const-folding, initializer/config evaluation, and debug expression/write flows.
- Runtime startup materializes TYPE defaults, struct/union member defaults,
  aggregate VAR initializers, VAR_CONFIG overrides, and legal FB instance
  member overrides through `harness::initializer` and the runtime
  `InitializerCatalog`.
- Debugger uses DAP plus the runtime control protocol; LSP/IDE technical spec is included below.
- Salsa incremental queries are used in `trust-hir` (analysis/LSP path), not in the deterministic runtime scan loop.
- IEC language specs remain in docs/specs/01-09-*.md.

## Runtime Execution Engine

IEC 61131-3 Edition 3.0 (2013) - Runtime Execution

This specification defines the `trust-runtime` execution engine for IEC 61131-3 Structured Text with cycle-based deterministic execution. Scheduled task/program execution is bytecode-VM only; helper evaluation exists only for bounded non-cycle flows.

### 1. Overview

#### 1.1 Design Goals

1. **VM-first execution**: Execute validated STBC bytecode in the runtime VM dispatch loop
2. **Cycle-based execution**: Execute programs in discrete cycles, not continuous loops
3. **Deterministic**: Same inputs produce same outputs, ordered iteration via IndexMap
4. **Testable**: First-class support for unit testing PLC logic, VM behavior-lock checks, and runtime vertical validation
5. **Zero unsafe**: Follows `unsafe_code = "forbid"` convention

#### 1.2 Architecture

```
crates/trust-runtime/
├── Cargo.toml
├── src/
│   ├── lib.rs            # Public API, Runtime struct
│   ├── bytecode/         # STBC encode/decode + metadata/debug maps
│   ├── eval/             # Shared model facade + test-only evaluator internals
│   ├── helper_eval/      # Storage-native helper evaluators for const/debug/config flows
│   ├── harness/initializer.rs # Runtime initializer materialization service
│   ├── program_model/    # Shared runtime/program AST + operator contracts
│   ├── runtime/          # Runtime core + VM dispatch/execution subsystems
│   ├── stdlib/           # Standard functions + FBs
│   ├── value/            # Value types + date/time profile
│   ├── io/               # I/O drivers
│   ├── control/          # Control protocol server
│   ├── debug/            # Debug hooks + state
│   ├── web/              # Browser UI server
│   ├── ui.rs             # TUI
│   ├── scheduler.rs      # Resource scheduling + clocks
│   ├── task.rs           # Task execution
│   ├── memory.rs         # Variable storage
│   └── ...               # Other runtime modules
└── tests/
```

> Historical note: older code snippets later in this document still show `EvalContext`-style conceptual APIs from the pre-VM migration era. Those snippets are background/reference material only and do not override the VM-only production contract above.

#### 1.2.1 Initializer and reference architecture contract

The source-level initializer path has one bounded ownership chain. HIR
collection retains member initializers in the Salsa-tracked symbol table and
exposes them through the initializer catalog. Runtime POU registration consumes
the HIR declaration catalog and fails visibly when a catalog entry cannot be
matched for body lowering; it does not rediscover programs, functions,
function blocks, classes, or interfaces by scanning raw syntax descendants.
Runtime initializer coercion is owned by the initializer service rather than
duplicated across callers, and runtime declaration lowering uses named
structural fields rather than positional tuples.

Elementary initializer coercion preserves the requested destination tag and
checks the complete destination domain. Signed integers, unsigned integers,
and bit strings accept integer source tags only and reject negative-to-unsigned
or width-overflowing values instead of wrapping. REAL and LREAL accept numeric
source tags only; a non-finite source or a value that becomes non-finite at the
destination width is rejected. BOOL accepts BOOL only. TIME and LTIME may
convert between their two runtime tags; DATE, LDATE, TOD, LTOD, DT, and LDT
require their exact runtime family and width.

STRING and WSTRING accept their own tag, the other string tag, and the
documented character inputs. CHAR and WCHAR accept their exact tag or an
exactly one-scalar STRING/WSTRING input; CHAR additionally requires an ASCII
scalar and WCHAR requires a scalar representable by `u16`. Bounded string
initializers truncate by Unicode scalar count, not UTF-8 byte count. These are
initializer-materialization rules and do not create additional source-level
implicit conversion operators.

Derived initializer coercion resolves aliases and subranges through their
target/base type. A partial fixed-array initializer fills leading elements in
row-major order and leaves omitted elements at their recursive type default.
After complete repetition expansion, excess rightmost elements are
constant-validated and ignored. Either cardinality mismatch emits the
preparation warning required by IEC 61131-3 Ed.3 §6.4.4.5.2; it is not a
runtime materialization error. A non-array input is rejected. Structure and
union input is field-named and case-insensitive, is stored with declaration
spelling and order, recursively coerces supplied members, defaults omitted
members, and rejects unknown members or a non-aggregate input.

Default materialization applies a registered type-level initializer before the
ordinary type default. A declared structure field or union variant initializer
then overrides that member's recursive type default. Type and member
initializers are evaluated with the supplied storage, current-instance,
date/time profile, and standard-library capabilities before being coerced
through the same destination boundary. A missing catalog record is an error;
it is never replaced with a silent type default.

Fixed arrays validate every inclusive dimension and the checked total element
count before allocating, then recursively materialize one default per element.
The wildcard sentinel retains its declared dimensions with no elements.
Structures and unions retain declared name, field spelling, and order.
Unresolved generic `ANY_INT` slots default to `NULL`. Unknown types, reversed or
overflowing array extents, invalid aggregate shapes, and initializer recursion
beyond 64 ownership steps fail with `TypeMismatch`; no partial aggregate is
published.

Subranges materialize their declared lower bound when they have no explicit
type initializer. Every initializer, assignment, parameter/result transfer,
reference write, and retained-value restoration into a subrange validates the
inclusive bounds after numeric normalization and before storage. A rejected
value leaves the complete destination unchanged.

#### 1.2.2 Textual ACTION fail-closed boundary

The production runtime has no textual-SFC action-control engine. A parsed
`ACTION name: ... END_ACTION` node is retained for front-end diagnostics only.
Before program-model lowering or bytecode emission, compilation must detect any
such node and fail with an unsupported textual ACTION/SFC diagnostic.

In particular:

- action statements must not be appended to the enclosing POU body;
- action statements must not be silently omitted while a runnable owner is
  emitted;
- declaration alone must fail even when the action body is empty or has no
  observable side effect; and
- multi-file compilation must fail when any participating source contains a
  textual action declaration.

Visual SFC remains a separate authoring surface. Its executable result is the
ordinary generated companion ST and runtime wrapper specified by
`17-visual-editors-runtime-unification.md`, not a textual ACTION node.

Fixed-array indices retain every declared lower and upper bound. Each
multidimensional index is checked against its corresponding dimension before
the storage offset is calculated; the checked row-major offset gives the
rightmost dimension unit stride. A failed index read produces no value, and a
failed index write leaves every element unchanged.

Ordinary enumeration runtime values retain their enum type identity and
selected declared literal. Values from different enum types never become
compatible through equal ordinals. Named-value integer types retain their
integer base representation and full base range. Whole-array and
whole-structure assignment copies the complete value; later mutation of one
ordinary aggregate does not mutate the other.

A truST union retains every declared variant as an independent logical member.
Variant defaults and aggregate overrides materialize recursively, and a write
to one variant leaves every other variant unchanged. Whole-union assignment
requires the same declared union type and copies every variant. `SIZEOF`
reserves the maximum storage size of any one variant; that storage-layout rule
does not introduce runtime aliasing. Shared backing bits require an IEC
`STRUCT OVERLAP` declaration.

Direct-I/O layout resolves aliases, subranges, and enumerations to their
storage type. BOOL occupies a bit; 8-, 16-, 32-, and 64-bit elementary
families map to byte, word, double-word, and long-word I/O widths respectively.
A bounded STRING occupies exactly its declared byte capacity. Unbounded
STRING, WSTRING, function-block, class, interface, pointer, reference, and
other non-materializable layouts are rejected.

An absolute field address has an `%I`, `%Q`, or `%M` area. A relative
structured-field address has `%X`, `%B`, `%W`, `%D`, or `%L` followed by a
decimal byte offset; `%X` optionally adds a decimal bit from zero through
seven. Relative offsets are added to the base binding with checked arithmetic.
Bit offsets carry into bytes, non-BOOL leaves reject a bit offset, and a
hierarchical address advances only its final path component while preserving
the root byte identity. No offset or stride may wrap.

The HIR layer must not depend on runtime `Value` types or raw CST nodes for
lowered initializer metadata. Syntax classification helpers delegate to the
central `trust-syntax` classifier. These are architectural dependency
contracts: their source-bound checks do not substitute for executable
initializer semantics.

The VM and register-IR paths preserve bounded allocation behavior:

- partial dynamic-reference indexing and field lookup borrow the existing
  `ValueRef` path rather than cloning the complete reference;
- function-block reference execution reads through the borrowed reference;
- register-IR decode stores instruction operands inline rather than allocating
  one `Vec` per decoded instruction; and
- VM local initialization populates VM frame slots without creating temporary
  runtime storage frames.

The reviewed initializer service modules remain at most 400 lines and reviewed
functions remain at most 60 lines. The checked-in initializer benchmark action,
include, fixture, VM backend selection, and authentication fixture value remain
present as the reproducible performance-test entrypoint. These caps and fixture
checks are maintainability and benchmark-contract gates; they do not establish
a general runtime performance result.

#### 1.3 Dependencies

```toml
[dependencies]
trust-syntax = { path = "../trust-syntax" }
trust-hir = { path = "../trust-hir" }
smol_str = "0.2"
rustc-hash = "1.1"
thiserror = "1.0"
indexmap = "2.0"  # Ordered maps for determinism
tracing = "0.1"
```

### 2. Value Representation

#### 2.1 Value Enum

Runtime value representation for all IEC 61131-3 types:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    // Boolean
    Bool(bool),

    // Signed integers
    SInt(i8),
    Int(i16),
    DInt(i32),
    LInt(i64),

    // Unsigned integers
    USInt(u8),
    UInt(u16),
    UDInt(u32),
    ULInt(u64),

    // Floating point
    Real(f32),
    LReal(f64),

    // Bit strings (stored as unsigned)
    Byte(u8),
    Word(u16),
    DWord(u32),
    LWord(u64),

    // Time types (IEC 61131-3 Ed.3 §6.4.2, Table 10)
    Time(Duration),
    LTime(Duration),
    Date(DateValue),
    LDate(LDateValue),
    Tod(TimeOfDayValue),
    LTod(LTimeOfDayValue),
    Dt(DateTimeValue),
    Ldt(LDateTimeValue),

    // Strings
    String(SmolStr),
    WString(String),
    Char(u8),
    WChar(u16),

    // Compound types
    Array(ArrayValue),
    Struct(StructValue),
    Enum(EnumValue),

    // Reference types (REF_TO)
    Reference(Option<ValueRef>),

    // Special
    Null,
    FbInstance(InstanceId),
    ClassInstance(InstanceId),
    InterfaceRef(Option<InstanceId>),
}
```

IEC `REF_TO` and the documented non-IEC `POINTER TO` extension share the same
runtime storage-handle representation but remain distinct source type
families. `POINTER TO` supports `ADR(...)`, dereference (`^`), `NULL`, and
same-family checked-copy `?=` as a typed vendor extension. No operation
implicitly converts `REF_TO` to `POINTER TO` or vice versa.
`Value::Null` remains the runtime sentinel for `NULL` literals and void-like
results, while uninitialized `REF_TO` / `POINTER TO` storage defaults to
`Value::Reference(None)` (IEC 61131-3 Ed.3 §6.4.4.10.2).

An ordinary compatible assignment copies the storage/instance identity. A
dynamic OOP assignment attempt evaluates its source once and then overwrites
the destination with that same identity when the runtime instance satisfies
the declared target class, function-block, or interface relation; otherwise it
overwrites the destination with `Value::Reference(None)`. Failure is an
ordinary NULL result, not a runtime error. An elementary/aggregate reference
or pointer checked-copy attempt is accepted only for its statically compatible
same-family target or NULL.

Dereference reads and writes traverse the retained storage identity. A null
dereference returns `RuntimeError::NullReference` before any load or store;
the destination of a failed read and the target storage of a failed write
remain unchanged.

#### 2.1.1 User-facing value presentation

User-facing runtime value text is Structured Text-oriented and must not expose
Rust enum debug constructors. Boolean values render as `TRUE` or `FALSE`,
integers and bit strings as their decimal value, integral `REAL`/`LREAL`
values retain a decimal component, strings use the Structured Text quoting and
`$` escaping rules, durations use the shortest exact `T#` unit, and an
instance reference renders as `Instance`. The reviewed formatter output must
not contain implementation names such as `Int(`, `Real(`, or `Instance(`.

The complete stable presentation contract is:

- all signed/unsigned integer and bit-string tags render as an unprefixed
  decimal number. Finite real values use Rust-independent numeric text, with
  `.0` appended when the representation has neither a decimal point nor an
  exponent;
- `TIME` and `LTIME` use `T#` and `LT#`. The formatter selects seconds,
  milliseconds, microseconds, then nanoseconds as the first unit that
  represents the signed nanosecond count exactly; zero therefore renders in
  seconds, and negative values preserve their sign;
- short `DATE`, `TOD`, and `DT` values use `D#`, `TOD#`, and `DT#` followed by
  their stored tick count. Long variants use `LD#`, `LTOD#`, and `LDT#`
  followed by their stored nanosecond count;
- `STRING` and `CHAR` use single quotes, doubling each IEC dollar escape and
  escaping an embedded quote as `$'`. `WSTRING` and `WCHAR` add the `W`
  prefix. A stored invalid wide-character scalar renders as `W'?'` rather than
  inserting invalid Unicode;
- arrays render as `[N]` using their current element count; structures render
  as `TypeName {...}`; enumerations render as `TypeName::VariantName`;
  populated and empty references render as `REF` and `NULL_REF`; an instance
  identity renders as `Instance`; and the void/null sentinel renders as
  `NULL`.

This text is diagnostic/debug presentation, not source serialization: it does
not promise round-trip reconstruction of aggregates, references, instances, or
calendar values.

#### 2.2 Compound Type Values

```rust
/// Reference to a value in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueRef {
    pub location: MemoryLocation,
    pub offset: usize,
}

/// Array value with bounds tracking.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayValue {
    elements: Vec<Value>,
    dimensions: Vec<(i64, i64)>, // (lower, upper) bounds
}

/// Struct value with named fields.
#[derive(Debug, Clone, PartialEq)]
pub struct StructValue {
    type_name: SmolStr,
    fields: IndexMap<SmolStr, Value>, // Ordered for determinism
}

/// Enum value storing canonical type identity plus display variant data.
/// Constructed through registry-backed helpers so aliases and case variants
/// canonicalize to the underlying enum type before comparison or retention.
#[derive(Debug, Clone)]
pub struct EnumValue {
    type_name: SmolStr,
    variant_name: SmolStr,
    numeric_value: i64,
}
```

Compound runtime values own their own invariants at construction time. Public
constructors resolve alias chains through `TypeRegistry`, canonicalize stored
type names and declared field casing, validate enum numeric/variant pairs,
validate struct field presence/order/value types, and validate array bounds,
element count, and element value types. Raw decode helpers may preserve untyped
wire/storage shape temporarily, but every entry point with declared type context
must validate before storing or executing the value; validation failure returns
a diagnostic error and never substitutes a default value.

The primitive Rust adapters preserve the exact runtime tag implied by their
source type: `bool` becomes `BOOL`, `i16` becomes `INT`, `i32` becomes `DINT`,
`i64` becomes `LINT`, `u8` becomes `USINT`, and `u16` becomes `UINT`. These
adapters do not widen, narrow, or infer a different PLC type.

#### 2.3 Declared-Type Materialization

IEC 61131-3 Ed.3 section 6.6.1.6 permits implicit conversion for assignments
and input/output parameter assignment, requires it to preserve value and
accuracy, and forbids it for `VAR_IN_OUT` assignment. Sections 6.4.4.1.2 and
6.5.1.3 permit compatible literal or constant-expression initializers.

The permitted typed widening matrix is closed:

- signed integer: `SINT -> INT -> DINT -> LINT`;
- unsigned integer: `USINT -> UINT -> UDINT -> ULINT`;
- bit string: `BYTE -> WORD -> DWORD -> LWORD`;
- exact integer-to-real: `SINT`/`INT -> REAL` and
  `SINT`/`INT`/`DINT -> LREAL`;
- real: `REAL -> LREAL`.

Typed `DINT -> REAL` and `LINT -> LREAL` require explicit conversion because
some source values cannot be represented without rounding. Signed/unsigned
cross-family, numeric/`BOOL`, and otherwise incompatible conversions are not
implicit. Contextual untyped literals remain allowed when representable by the
target.

Typed operands of an operator or overloaded standard function have a common
numeric type only when they are already identical or one operand can reach the
other operand's type through that same closed accuracy-preserving matrix. If
neither direction is permitted, semantic analysis and runtime evaluation
reject the operation with a type mismatch rather than choosing a type by a
total numeric rank. Thus `INT + REAL` may use `REAL`, while `DINT + REAL`,
`ULINT + REAL`, and signed/unsigned cross-family operations require an explicit
conversion.

The portable runtime normalization helpers enforce the same closed families.
Conversion to the signed `i64` execution domain accepts signed values and
unsigned values through `i64::MAX`, returning overflow above that boundary.
Conversion to the unsigned `u64` execution domain accepts every unsigned value
and non-negative signed values, while a negative signed value is a type
mismatch. Conversion to the floating execution domain accepts only the ten
numeric runtime tags. Bit strings, booleans, references, strings, and `NULL`
are not silently reinterpreted as numeric values.

A representable untyped numeric literal is contextualized to the other typed
operand or to the common typed argument of an overloaded standard function,
independently of argument order. Explicitly typed literals are never treated as
contextual and remain governed by the strict matrix.

When semantic analysis permits an implicit numeric widening, variable
initialization, assignment, function-result assignment, and POU input/output
parameter transfer materialize the value as the declared target type before it
is stored or used by the callee or caller. In particular, an `INT` value
assigned or passed to `REAL` storage is represented as `Value::Real`, and an
`INT` value assigned or passed to `DINT` storage is represented as
`Value::DInt`. The stack, register, and tier-1 execution paths must produce the
same declared runtime type and value.

Implicit conversion is never applied to `VAR_IN_OUT`: a binding that would
require it, including an `INT` actual bound to a `DINT` formal, fails
compilation with diagnostic category `E205`. A narrowing assignment that
cannot preserve the source value and accuracy is rejected unless the program
uses an explicit conversion;
incompatible assignment uses diagnostic category `E203`. Diagnostic prose is
not part of this contract. At the VM boundary, a value whose runtime tag cannot
materialize as the declared primitive is rejected with `TypeMismatch` before
storage and reports `runtime_type_mismatch`.

#### 2.3.1 Bounded String Runtime Writes

Every write into declared `STRING[n]` or `WSTRING[n]` storage applies the
receiving declaration's capacity in Unicode scalar values. Initializers,
ordinary assignment, function and function-block input copy-in, output
copy-back, and function-result assignment truncate only the excess suffix and
preserve the first `n` scalar values. An in-bound value is stored unchanged.

`VAR_IN_OUT` does not convert or resize its actual argument: the string family
and effective declared capacity must match exactly after alias resolution, and
a rejected binding cannot mutate caller state. `STRING` and `WSTRING` remain
separate families. Source-level cross-family assignment or binding is rejected
unless an explicit standard conversion is used; crafted VM input with the
wrong string-family runtime tag returns `TypeMismatch` before storage and
reports `runtime_type_mismatch`.

#### 2.3.2 Runtime String Elements

This product contract makes the runtime consequences of the reviewed
`DEV-017` IEC deviation explicit. It does not create a separate IEC deviation.
Both `STRING` and `WSTRING` runtime values use Unicode scalar values as their
element model:

- element positions are 1-based and element counts are Unicode-scalar counts,
  not UTF-8 byte or UTF-16 code-unit counts;
- `LEFT`, `RIGHT`, `MID`, `INSERT`, `DELETE`, `REPLACE`, and `FIND` use those
  scalar positions and counts, and `FIND` returns zero when no match exists;
- indexed `STRING` reads materialize `CHAR` when the selected scalar fits
  `u8`; indexed `WSTRING` reads materialize `WCHAR` when it fits `u16`;
- reads and writes apply the destination width even when the replacement
  arrives as a one-scalar `STRING` or `WSTRING`; a scalar that does not fit
  `CHAR` or `WCHAR` returns `RuntimeError::Overflow` without producing a
  replacement value;
- for an in-range index, indexed writes accept a `CHAR` or `WCHAR` encoding a
  valid Unicode scalar, or an exactly one-scalar `STRING` or `WSTRING`; an
  invalid scalar encoding, empty or multi-scalar string, or non-character
  replacement returns `RuntimeError::TypeMismatch`;
- an index below 1 returns `RuntimeError::IndexOutOfBounds` with lower bound 1
  and the runtime's unbounded-upper sentinel `i64::MAX`; an index above the
  current scalar count reports lower bound 1 and that count as the upper
  bound; and
- a successful indexed write replaces exactly one scalar and preserves all
  other scalar values.

Indexed writes validate the index before validating replacement width or
shape. An out-of-range write therefore returns `IndexOutOfBounds` even when
the supplied replacement is also invalid.

Runtime value paths and `VariableStorage` references apply the same indexed
read/write rules as the runtime-core helpers. Core helpers return the exact
`RuntimeError`; optional value-path reads translate a failure to `None`, and
boolean reference writes translate it to `false`. Direct evaluated indexing
preserves its `RuntimeError` in a runtime trap; VM reference materialization
and write routes preserve failure and nonmutation but expose the existing
null-reference trap after their `Option`/boolean translation. No failed
conversion, shape check, or bounds check may mutate the referenced string.

#### 2.4 Subrange Runtime Writes

Subrange lower and upper bounds are inclusive. Constant initializers outside
those bounds fail semantic analysis with an out-of-range diagnostic.
Assignment, POU parameter copy-in, and dynamic-reference writes into
subrange-typed storage validate the
incoming value before modifying the target. An out-of-range value produces a
visible runtime error and leaves the target at its previous value; the runtime
must not clamp, wrap, or partially store the rejected value. The same rule
applies at HMI/control and retain-reload write boundaries when declared
subrange type information is available. A wrong-base-type source fails
semantic analysis with `E203`; crafted VM input with the wrong runtime tag
returns `TypeMismatch` with `runtime_type_mismatch` and does not modify
storage. An out-of-range runtime value reports
`runtime_subrange_violation` at VM, retain-reload, and HMI/control boundaries.

This product rule implements the reviewed subrange decision in
`docs/IEC_DECISIONS.md`; it does not define that stable public error mapping.

#### 2.5 Time/Date Representation

IEC 61131-3 defines LTIME/LDATE/LTOD/LDT as signed 64-bit nanosecond counts with fixed
epochs, while TIME/DATE/TOD/DT have implementer-specific range and precision
(IEC 61131-3 Ed.3 §6.4.2, Table 10, footnotes b, m–q).

Custom Duration wrapper with nanosecond precision (no external time crate dependency):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Duration {
    nanos: i64, // Signed for subtraction results
}

impl Duration {
    pub const ZERO: Self = Self { nanos: 0 };

    pub fn from_nanos(nanos: i64) -> Self { Self { nanos } }
    pub fn from_micros(micros: i64) -> Self { Self { nanos: micros * 1_000 } }
    pub fn from_millis(millis: i64) -> Self { Self { nanos: millis * 1_000_000 } }
    pub fn from_secs(secs: i64) -> Self { Self { nanos: secs * 1_000_000_000 } }

    pub fn as_nanos(&self) -> i64 { self.nanos }
    pub fn as_millis(&self) -> i64 { self.nanos / 1_000_000 }
}
```

```rust
/// Implementer-specific profile for TIME/DATE/TOD/DT (IEC Table 10, footnote b).
#[derive(Debug, Clone, Copy)]
pub struct DateTimeProfile {
    /// Epoch for DATE/DT (default: 1970-01-01 for vendor compatibility).
    pub epoch: DateValue,
    /// Resolution for TIME/DATE/TOD/DT (default: 1 ms).
    pub resolution: Duration,
}

// For DATE/DT, a tick value of 0 corresponds to the profile epoch at midnight.
// For TOD, a tick value of 0 corresponds to midnight.

/// DATE value stored as ticks since epoch at midnight (ticks in profile resolution).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateValue {
    ticks: i64,
}

/// TIME_OF_DAY value stored as ticks since midnight (ticks in profile resolution).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeOfDayValue {
    ticks: i64,
}

/// DATE_AND_TIME value stored as ticks since epoch (ticks in profile resolution).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTimeValue {
    ticks: i64,
}

/// LDATE: signed 64-bit nanoseconds since 1970-01-01 (IEC Table 10, footnote n).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LDateValue {
    nanos: i64,
}

/// LTOD: signed 64-bit nanoseconds since midnight (IEC Table 10, footnote p).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LTimeOfDayValue {
    nanos: i64,
}

/// LDT: signed 64-bit nanoseconds since 1970-01-01-00:00:00 (IEC Table 10, footnote o).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LDateTimeValue {
    nanos: i64,
}
```

For TIME/DATE/TOD/DT, trust-runtime uses a configurable `DateTimeProfile` and treats values as
timezone-naive civil time (no timezone/DST metadata). The default profile targets common PLC
runtime behavior (CODESYS/TwinCAT-style):

- Epoch: `D#1970-01-01` (DATE) / `DT#1970-01-01-00:00:00` (DT)
- Resolution: 1 ms for TIME/DATE/TOD/DT
- Range: signed 64-bit ticks at the configured resolution

Conversions or arithmetic that exceed the configured range raise `RuntimeError::DateTimeOutOfRange`.

#### 2.6 Default Values

Per IEC 61131-3, default values for types (IEC 61131-3 Ed.3 §6.4.2, Table 10; §6.4.4.2; §6.4.4.10.2):

| Type | Default Value |
|------|---------------|
| BOOL | FALSE |
| Numeric (INT, REAL, etc.) | 0 |
| TIME | T#0s |
| LTIME | LTIME#0s |
| DATE | D#1970-01-01 (profile epoch) |
| LDATE | LDATE#1970-01-01 |
| TOD | TOD#00:00:00 |
| LTOD | LTOD#00:00:00 |
| DT | DT#1970-01-01-00:00:00 (profile epoch) |
| LDT | LDT#1970-01-01-00:00:00 |
| STRING/WSTRING | '' (empty) |
| CHAR/WCHAR | `'$00'` / `"$0000"` (numeric 0) |
| Array | Each element initialized to type default |
| Struct | Each field initialized to type default |
| Enum | First enumerator (unless explicitly initialized) |
| Reference (REF_TO) | NULL |

##### 2.6.1 Declaration Initializer Materialization

For the reviewed scalar and one-dimensional array declaration slice, runtime
startup materializes these exact outcomes:

- an `INT` declaration without an explicit initializer contains `0`;
- an `INT := 4` declaration contains `4`, and an `INT` initialized from the
  reviewed declared-constant expression `base + 1` with `base = 4` contains
  `5`;
- `ARRAY[1..3] OF INT := [1, 2, 3]` preserves the inclusive bounds and the
  three elements in source order;
- `ARRAY[1..5] OF INT := [1, 2]` preserves the supplied elements and fills the
  remaining three elements with the element-type default `0`; and
- `ARRAY[1..6] OF INT := [3(1, 2)]` expands to
  `[1, 2, 1, 2, 1, 2]` while preserving the declared inclusive bounds.

Successful materialization completes the reviewed scan without a runtime
cycle error. This bounded contract does not claim arbitrary dimensions,
element types, malformed initializers, or diagnostic behavior.

#### 2.7 Portable Runtime Value and Program-Model Contracts

The portable runtime core SHALL preserve these value and program-model rules:

- Calendar construction rejects an invalid month length or a non-leap-year
  February 29. Date/time ticks and long date/time values round-trip within
  their declared range; out-of-range tick conversion and timezone-bearing
  `DATE` plus `TOD` combination are rejected. Duration nanosecond and
  millisecond views describe the same stored duration.
- Civil-date conversion uses the proleptic Gregorian calendar and maps
  `1970-01-01` to day zero. Leap years are divisible by four except centuries
  not divisible by 400. Invalid months, invalid day-of-month values, and
  non-leap February 29 return `InvalidDate`; any intermediate or final
  calculation outside signed 64-bit range returns `Overflow` rather than
  panicking or wrapping.
- A short date/time profile resolution is valid only when it is positive, no
  larger than one day, and divides exactly into `86_400_000_000_000`
  nanoseconds. Otherwise profile conversion returns `InvalidResolution`.
  `ticks_per_day` is that exact quotient. `days_to_ticks` multiplies by the
  quotient and adds the configured epoch with checked signed arithmetic.
  Nanosecond conversion uses truncation toward zero for `DivisionMode::Trunc`
  and Euclidean division for `DivisionMode::Euclid`, including negative input.
  IEC 61131-3 Ed.3 section 6.4.2.1 and Table 10 define the DATE, TOD, and DT
  families while leaving the short-type starting date implementer-specific;
  the epoch, resolution, checked range, and division choices here are the truST
  runtime profile contract, not an IEC deviation.
- An L-value name is its own root and qualified name. Field and index nodes
  recursively preserve the root name of their target. Only an uninterrupted
  name/field chain has a qualified name, formed by joining its exact segments
  with `.`; an index or dereference anywhere in that chain makes the qualified
  name unavailable. `contains_index` is true for an index and for any field
  whose target contains an index. It is false for a name or dereference and
  does not inspect the expression behind a dereference.
- An initializer catalog starts empty. Each insert allocates the next
  consecutive `InitializerId`, beginning at zero, and preserves the exact
  expression under that identity. A missing initializer or missing type
  default returns no value. Setting a type default is last-write-wins for that
  type and does not remove either the old or new initializer record.
- Hidden property setter names and static-storage names preserve their
  canonical segment and prefix identity. Internal helper names SHALL NOT
  collide with user symbols.
- The numeric-kind classifier preserves the declared runtime tag for every
  supported numeric value. Numeric and Boolean operations use their checked
  runtime contract. Non-numeric comparisons are accepted only for the
  documented comparable value families; unsupported mixed families return a
  stable error. Unary negation rejects non-numeric operands with
  `RuntimeError::TypeMismatch` and rejects the unrepresentable negation of
  each signed integer minimum with `RuntimeError::Overflow`. The reviewed host
  evaluator returns `INT#8` for `INT#2 ** INT#3` and `LREAL#8.0` for
  `LREAL#2.0 ** REAL#3.0`. The integer-base case is an extension beyond IEC
  61131-3 Ed.3 section 6.6.2.5.8 and Table 29 and is recorded in
  `docs/IEC_DEVIATIONS.md#2026-07-27---integer-base-exponentiation`. Real
  and integer exponent boundary failures require separately cataloged
  behaviors. The reviewed Boolean result is `TRUE >= FALSE = TRUE`; it does
  not certify other Boolean operand/operator combinations.
- Default construction preserves the exact declared runtime tag for every
  elementary type. Fixed-size arrays preserve their declared inclusive
  dimensions and row-major element count, with every element recursively
  initialized to its element-type default; wildcard arrays start empty while
  retaining their wildcard dimensions. Invalid array bounds are rejected.
  Structures and unions preserve declared field order and recursively default
  every field. An enumeration selects its first declared enumerator and rejects
  an empty declaration. Aliases delegate to their target type. References and
  pointers default to an empty reference; `NULL` and interfaces default to
  `NULL`. An integer subrange defaults to its lower bound using the exact base
  integer tag and rejects a non-integer base. Unknown type IDs and runtime types
  without a portable value representation are rejected.
- Partial bit/byte/word reads and writes enforce bounds, and a write changes
  only the selected portion. Reference-path helpers preserve segment order and
  accept only the documented IEC partial-access suffixes.
- Borrowed reference-path traversal returns the original value for an empty
  path. A field segment borrows only an existing structure field. An index
  segment borrows only an existing array element selected by exact inclusive
  multidimensional bounds and row-major offset; traversal then continues
  recursively through the remaining segments. A missing field, wrong
  aggregate kind, wrong index arity, out-of-bounds index, or unrepresentable
  offset returns no value. Borrowed traversal does not synthesize a `CHAR` or
  `WCHAR` from string storage; string element paths use the materializing
  reference route. A string materialization/write path accepts exactly one
  index for its string segment.
- Array offsets use inclusive bounds and row-major order. Arity mismatches,
  out-of-bounds indices, checked-arithmetic failures, and offsets that cannot
  be represented by the target `usize` are rejected rather than wrapped.
- Portable `SIZEOF` type calculation resolves aliases, subranges, and
  enumerations to their storage type; sums structure fields; selects the
  largest union variant; multiplies fixed inclusive array dimensions by the
  element size; uses the declared bounded string capacity (twice that capacity
  for `WSTRING`); and uses the runtime reference-handle width for references
  and pointers. Unknown types, unbounded strings, wildcard or reversed array
  dimensions, and unsupported types are rejected. Any representable dimension
  product or byte-size overflow is rejected rather than wrapped.
- Portable runtime-value size calculation uses the exact elementary tag width,
  counts string content in Unicode scalar elements (`WSTRING` twice), sums
  structure fields, multiplies a validated array extent by its element size,
  resolves an enumeration through its declared base type, and uses the runtime
  reference-handle width. `NULL` has no storage size and is rejected.
- Stable runtime error identifiers use the exact lower-snake-case strings
  defined by section 10.2; a representative-family test proves only the named
  identifiers it asserts.
- Array construction resolves aliases and validates shape and element types,
  including arrays of structures. Clone/equality preserve shape and elements;
  mutation preserves shape. Structure construction resolves aliases and
  validates field types; clone/equality preserve field identity and mutation
  updates existing fields only.
- A serialized array value is accepted only when its complete ordered
  dimension list equals the resolved declaration, the element count equals the
  checked inclusive row-major extent, and every element recursively matches
  the declared element type. Reversed bounds and an extent not representable
  by `usize` are `InvalidArrayBounds`; a different dimension list, element
  count, or element type returns `ArrayDimensionsMismatch`,
  `ArrayElementCountMismatch`, or `ArrayElementTypeMismatch` respectively.
  Unknown type IDs, non-array types, and alias cycles are rejected without
  constructing a partial array.
- Serialized structure and union input resolves the type name and field names
  case-insensitively, then stores the declaration's canonical type name, field
  spelling, and field order. Missing, extra, or recursively mismatched fields
  are rejected. Unknown names or IDs, non-aggregate types, and alias cycles are
  rejected. After construction, field lookup and mutation use the stored
  canonical spelling; they do not perform another case-folded lookup.
- Enumeration construction resolves aliases, type names, and variant names
  case-insensitively, but stores the declaration's canonical type and variant
  spelling. Deserialized numeric data must equal the declared enumerator
  number. Unknown IDs or names, non-enum types, alias cycles, unknown variants,
  and numeric mismatches return their corresponding `EnumValueError`. Runtime
  enum equality is the canonical type name plus numeric value; the stored
  variant spelling is display data and does not independently determine
  equality.
- Declared-type matching is exact for every elementary runtime tag. Subranges
  require the declared integer base tag and inclusive bounds. Enumeration,
  structure/union, and array values additionally require canonical type
  identity and recursively matching members; arrays also require exact
  dimensions and checked length. References and pointers accept an empty or
  populated reference and `NULL`; interfaces accept an instance or `NULL`.
  Unknown, generic, and otherwise non-materializable types never match a
  runtime value.
- `ValueConstructionError` and `EnumValueError` display text is stable
  diagnostic data. It identifies the rejected type ID or name, alias cycle,
  expected and actual dimensions/count/type, aggregate field, enum variant,
  and expected/actual numeric value as applicable. Converting an
  `EnumValueError` to `ValueConstructionError` preserves the original enum
  error and its diagnostic text.
- Interface values accept the explicit null representation and compatible
  instance references. Declared assignment materializes only the documented
  safe numeric widening and stores the destination runtime tag.
- Host expression access rejects an out-of-bounds array index with
  `RuntimeError::IndexOutOfBounds`, a null dereference with
  `RuntimeError::NullReference`, and a reference to absent storage with
  `RuntimeError::UndefinedVariable`. These access assertions establish the
  exact error variants, not post-state mutation behavior.

These are portable data-model guarantees. A test name, sampled helper, or
platform-width coincidence is not an oracle for a broader claim.

### 3. Memory Model

#### 3.1 Memory Locations

```rust
/// Memory location identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryLocation {
    /// Global variable area
    Global,
    /// Local variable area for a specific call frame
    Local(FrameId),
    /// FB/Class instance storage
    Instance(InstanceId),
    /// I/O area (direct addresses)
    Io(IoArea),
    /// Retain area (persistent across warm restart)
    Retain,
}

/// I/O area identifiers per IEC 61131-3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IoArea {
    Input,   // %I
    Output,  // %Q
    Memory,  // %M
}

/// Frame identifier for call stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameId(u32);

/// Instance identifier for FB/Class instances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstanceId(u32);
```

Memory identity equality is structural: the enum variant and its complete
payload determine equality, and equal identities produce equal hashes. Hash
values are in-process collection keys only and are not stable wire, storage,
or cross-version identifiers.

#### 3.2 Variable Storage

```rust
/// Storage for runtime variables.
#[derive(Debug, Default)]
pub struct VariableStorage {
    /// Global variables (VAR_GLOBAL)
    globals: IndexMap<SmolStr, Value>,

    /// Local variable frames (call stack)
    frames: Vec<LocalFrame>,

    /// FB/Class instances
    instances: FxHashMap<InstanceId, InstanceData>,

    /// Retain variables (persist across warm restart)
    retain: IndexMap<SmolStr, Value>,

    /// Next instance ID
    next_instance_id: u32,
}

/// A local variable frame for function/method calls.
#[derive(Debug)]
pub struct LocalFrame {
    pub id: FrameId,
    pub owner: SmolStr,        // POU name
    pub variables: IndexMap<SmolStr, Value>,
    pub return_value: Option<Value>,
}

/// Data for a single FB/Class instance.
#[derive(Debug)]
pub struct InstanceData {
    pub type_name: SmolStr,
    pub variables: IndexMap<SmolStr, Value>,
    pub parent: Option<InstanceId>,  // For inheritance
}
```

#### 3.2.1 Variable-storage reference and layout contract

`VariableStorage` exposes one logical storage model across global, local-frame,
and instance locations. The following behavior is part of the runtime product
contract:

- Cloning storage copies the logical values even if an internal lookup-cache
  lock was poisoned. Cache lock state is not observable storage state.
- Direct instance-field lookup is scoped by instance identity. A cached miss is
  invalidated when that instance gains the field. Recursive lookup prefers a
  field declared on the requested instance over a parent field and invalidates
  a cached inherited resolution when a child adds a shadowing field. A
  recursive parent-chain miss is not cached because a later parent insertion
  may make the same lookup valid.
- Declared-field offsets describe only fields declared by the instance's own
  type. Instances of the same declared layout use the same declared offsets;
  inherited fields are resolved through their owning parent instance rather
  than being inserted into the child's declared layout. Offset-based reads and
  writes address exactly the resolved field.
- Direct-slot and empty-reference-path helpers are equivalent for global,
  current-local-frame, and instance locations. Borrowed and owned `ValueRef`
  helpers observe and update the same slot.
- The reviewed host helper lvalue paths resolve an existing current-local name
  or global-root dereference, array-element, structure-field, or nested
  aggregate target and change only that selected target. If the root name
  cannot be resolved, the write returns `RuntimeError::UndefinedVariable` and
  creates no fallback global slot.
- A queued debug global or lvalue write uses the same existing-target
  authority. If its target is unresolved, the following cycle returns an error
  containing the missing identity and creates no fallback global. The reviewed
  lvalue route also emits a runtime fault event containing that identity.
- A nested reference write updates only the selected aggregate path. Shared
  structure values use copy-on-write isolation so a sibling slot that held the
  same pre-write value is unchanged. Array offset calculation is checked before
  conversion to a host index; inclusive bounds at signed extremes must not
  wrap, and a valid extreme-bound reference reads and writes the selected
  element exactly.

Lookup caches and lock implementations remain internal optimizations. Their
contents are evidence only where needed to prove the observable invalidation
rules above; cache capacity, eviction, and synchronization strategy are not
product semantics.

#### 3.3 Variable Lifetime Rules

Per IEC 61131-3:

| POU Type | VAR | VAR_TEMP | Behavior |
|----------|-----|----------|----------|
| FUNCTION | Re-init each call | Re-init each call | Stateless |
| FUNCTION_BLOCK | Persist across calls | Re-init each call | Stateful |
| PROGRAM | Persist across calls | Re-init each call | Stateful |
| METHOD | Re-init each call | Re-init each call | Uses instance state |

### 4. Execution Model

#### 4.1 Runtime Structure

```rust
/// The main runtime environment.
pub struct Runtime {
    /// Symbol table from semantic analysis
    symbols: Arc<SymbolTable>,

    /// Syntax trees for all loaded files
    syntax_trees: FxHashMap<FileId, SyntaxNode>,

    /// Variable storage
    storage: VariableStorage,

    /// I/O interface
    io: IoInterface,

    /// Current simulation time
    current_time: Duration,

    /// Profile for DATE/TOD/DT (implementer-specific per IEC Table 10)
    datetime_profile: DateTimeProfile,

    /// Cycle count
    cycle_count: u64,

    /// Task configurations
    tasks: Vec<TaskConfig>,

    /// Task scheduling state (last SINGLE value, last run time)
    task_state: IndexMap<SmolStr, TaskState>,

    /// Standard library
    stdlib: StandardLibrary,

    /// Execution trace (for debugging)
    trace: Option<ExecutionTrace>,
}

/// Configuration for a task (periodic and/or event-driven).
#[derive(Debug, Clone)]
pub struct TaskConfig {
    pub name: SmolStr,
    pub interval: Duration,     // INTERVAL input; 0 disables periodic scheduling
    pub single: Option<SmolStr>, // SINGLE input (event trigger)
    pub priority: u32,
    pub programs: Vec<SmolStr>, // Programs assigned to this task
    pub fb_instances: Vec<ValueRef>, // Task-associated FB instances
}

/// Scheduling state for a task (IEC 61131-3 Ed.3 §6.8.2).
#[derive(Debug, Clone)]
pub struct TaskState {
    pub last_single: bool,
    pub last_run: Duration,
    pub overrun_count: u64,
}
```

#### 4.2 Cycle Execution

```rust
/// Result of a single execution cycle.
#[derive(Debug)]
pub struct CycleResult {
    pub cycle_number: u64,
    pub elapsed_time: Duration,
    pub outputs_changed: Vec<(SmolStr, Value)>,
    pub errors: Vec<RuntimeError>,
}

impl Runtime {
    /// Creates a new runtime from analyzed source.
    pub fn new(symbols: Arc<SymbolTable>, trees: FxHashMap<FileId, SyntaxNode>) -> Self;

    /// Initializes the runtime (allocates instances, sets defaults).
    pub fn initialize(&mut self) -> Result<(), RuntimeError>;

    /// Executes a single scan cycle.
    pub fn execute_cycle(&mut self) -> CycleResult;

    /// Advances time by the given duration.
    pub fn advance_time(&mut self, delta: Duration);

    /// Executes cycles until a condition is met.
    pub fn run_until<F>(&mut self, condition: F) -> Vec<CycleResult>
    where
        F: Fn(&Runtime) -> bool;

    /// Executes a specific number of cycles.
    pub fn run_cycles(&mut self, count: u32) -> Vec<CycleResult>;
}
```

`Runtime::new` initializes the `DateTimeProfile` to its default (epoch 1970-01-01, 1 ms resolution).

#### 4.1.1 Runtime Core State and Control Boundaries

A newly constructed runtime has resource name `RESOURCE`, the bytecode-VM
execution backend, zero logical time and cycle count, empty user storage,
registries, task state, debug state, and protocol connections, and the default
date/time profile. Construction registers the reviewed standard function-block
definitions and their function-block types before user declarations are
accepted.

Runtime declaration registration preserves these lookup and lifecycle rules:

- functions, function blocks, classes, and interfaces are keyed
  case-insensitively by their uppercase declaration name;
- program registration creates its runtime instance, publishes that instance
  under the declared program name, and only then exposes the program
  definition;
- registering a function, function block, class, or program invalidates any VM
  local-initialization plan derived from the prior declaration set; and
- metadata snapshots own clones of the declaration, task, access, profile, and
  statement-location state. Later runtime mutation cannot retroactively alter a
  prior snapshot.

Task registration records the current logical time as its initial scheduling
baseline. If its `SINGLE` variable already contains `TRUE`, the saved sample is
initialized to `TRUE` so registration creates no artificial rising edge. Each
distinct task name receives one stable nonzero debugger thread identifier,
reused if that task name is registered again. When at least one registered
program is absent from every task's program list, one separate stable
background thread identifier is allocated on demand; otherwise no background
identifier is allocated. Resetting live task timing establishes the supplied
time as every task's new `last_run`, clears sampled `SINGLE` state and overrun
history, and preserves thread identities.

Logical time updates are deterministic. Setting time stores the exact signed
nanosecond value. Advancing time adds with signed saturation, so positive or
negative overflow clamps to `i64::MAX` or `i64::MIN`; it never wraps.

Execution and output-commit deadlines are optional absolute instants. Setting a
deadline records the debugger watchdog-pause total at that instant. The
effective deadline adds only pause time accumulated after that baseline, using
checked instant arithmetic; with no debugger pause it equals the configured
deadline exactly. Clearing a deadline yields no effective deadline.

#### 4.1.2 Debug, Evaluation, and VAR_ACCESS Boundaries

Runtime debug source state is keyed by numeric file identity. Statement
locations and source text replace the prior entry for the same file. A source
label resolves either through an explicitly registered exact label or through
the exact virtual form `file_<unsigned-decimal-id>`. Malformed, signed,
case-changed, or suffix-free virtual labels do not resolve. VM line and column
inputs are one-based and are projected to the same nearest-statement resolver
used by source breakpoints; missing labels, source text, or statement entries
return no location.

Debug expression, lvalue read, and lvalue write operations use the requested
runtime frame as the temporary current frame without changing persistent frame
order. The selected frame's instance supplies instance context. A missing frame
returns `runtime_invalid_frame` and performs no evaluation or write. With no
frame, the operation uses global runtime context.

`VAR_ACCESS` reads and writes resolve the binding's stored reference:

- a direct read returns the referenced value and a direct write replaces it;
- a partial binding applies the declared bit/byte/word/dword projection,
  preserves unselected bits, and maps bounds or type failures to the
  corresponding runtime error;
- an unknown access name reports `runtime_undefined_variable` on write and no
  value on read; and
- an unresolved stored reference reports `runtime_null_reference` on write and
  no value on read.

Failed partial or unresolved writes leave the referenced value unchanged.

Fault entrypoints clear queued debugger runtime mutations before applying fault
policy. They record the original fault once in runtime fault state and metrics,
emit its debug event when debugging is enabled, and return that original error
unless safe-state application itself fails. A safe-state failure returns
`runtime_safe_state_failed` while retaining the original fault as the recorded
root. Clearing fault state removes the recorded fault without changing runtime
declarations, time, or thread identities.

#### 4.1.3 Runtime Access, Diagnostics, and Protocol Projection Contract

Runtime accessors expose live runtime state; they do not return detached
declaration or process-image copies. Mutating variable storage, the type
registry, the initializer catalog, the process image, or the `VAR_ACCESS` map
through its mutable accessor is therefore immediately observable through the
corresponding immutable accessor. The combined registry/initializer and
instance-initialization accessors must borrow the one runtime-owned set of
objects, so callers cannot accidentally initialize against a stale registry,
declaration set, or standard library. Resource identity preserves the exact
configured spelling.

Function, function-block, class, and interface registration uses an uppercase
lookup key while preserving the declaration's spelling in its value. A later
case-only registration replaces the same logical entry instead of creating a
second entry. Function, function-block, and class registration invalidates
derived local-initialization plans. Read-only declaration accessors expose the
same registered definitions; the standard-library accessor exposes the
runtime's registered built-ins.

Execution-backend selection is one transaction: after success, both the runtime
selector and an attached metrics sink report the selected backend. Register-VM
profiling, register-IR lowering-cache reuse, and tier-1 specialization each
have independent enable controls and snapshots. Resetting one diagnostic
surface clears its cache/counters without changing its current enabled state or
configured capacity/threshold. With no bytecode module loaded, resolving any
VM POU identifier returns no name.

The process-image accessors share the one input, output, and memory image.
Driver health publication replaces the prior sink contents in driver
registration order, reports each driver's current `ok`, `degraded`, or
`faulted` state, and publishes an empty snapshot after all drivers are cleared.
Removing the health sink stops publication without modifying the last snapshot
owned by the caller.

Protocol lifecycle projections are fail-closed and side-effect bounded:

- an unconfigured ADS subsystem reports zero connections and `disabled`;
  shutdown of that empty subsystem succeeds;
- an unconfigured OPC UA client reports zero connections and `enabled = false`;
  reset of that empty subsystem succeeds;
- deployed configuration hashes round-trip through status projections; an
  OPC UA reset creates a fresh unconfigured subsystem and clears its prior
  deployed hash; and
- an active ADS device lookup returns no snapshot when no live connection
  matches the requested target.

`USING` resolution is tied to the selected frame. Function and program owners
resolve case-insensitively. A function-block or class instance frame resolves a
matching method case-insensitively; a non-empty method `USING` list overrides
the owning type's list, while an empty method list inherits the owning type's
list. A frame owned by the function block or class itself uses the type list.
Unknown or removed frame identities resolve to no list.

`VAR_ACCESS` names are exact external binding identities. The map retains the
declared name, stored reference, and optional projection. Bit projections
return/write `BOOL`; byte, word, and double-word projections return/write
`BYTE`, `WORD`, and `DWORD` respectively, with index zero denoting the
least-significant portion. Successful writes replace only the selected bits.
Projection bounds and value-type mismatches report the stable runtime bounds
or type error and are atomic: neither the selected value nor any unselected bit
may change.

#### 4.3 Task Scheduling (Periodic + Event)

Tasks are scheduled per IEC 61131-3 Ed.3 §6.8.2:

- **Event trigger (SINGLE)**: A task is scheduled on each rising edge of its `SINGLE` Boolean input.
- **Periodic trigger (INTERVAL)**: If `INTERVAL` is non-zero and `SINGLE` is FALSE, the task is scheduled
  periodically at the specified interval. If `INTERVAL` is zero (default), no periodic scheduling occurs.
- **Priority**: Lower numeric priority values run first (0 = highest).

trust-runtime uses **non-preemptive, deterministic scheduling**: ready tasks
are executed by lower numeric priority first, then by earlier due time, then by
stable declaration index. This ordering is permitted by IEC 61131-3
(§6.8.2(c)) and makes execution reproducible.

Event tasks are modeled by tracking the previous value of the SINGLE variable:

```
event_due = single_prev == FALSE && single_now == TRUE
periodic_due = interval > 0 && single_now == FALSE &&
               (current_time - last_run) >= interval
```

New task scheduling state records the supplied current logical time as
`last_run`, clears the saved `SINGLE` sample to `FALSE`, and starts missed
interval accounting at zero.

The SINGLE input must resolve to a BOOL variable; if it is missing or non-BOOL, task execution
fails with a runtime error.

Programs with no explicit task association are scheduled at the lowest priority. In this cycle-based
runtime, they execute once per `execute_cycle` (interpreting that call as the smallest scheduling
granularity). This preserves determinism while aligning with IEC's "reschedule after completion"
rule for background programs.

When a task with a `SINGLE` input is registered while that input is already
TRUE, the runtime initializes its saved sample to TRUE. Registration therefore
does not invent a rising edge. While the sampled value remains TRUE, periodic
readiness is suppressed; a later FALSE sample rearms event detection and
permits periodic readiness under the normal interval rules.

##### 4.3.1 Debugger Thread Mapping

Debugger threads map directly to IEC tasks. Each configured task (Table 63) is exposed as a distinct
debugger thread, ordered by task declaration, and the background program group (programs without
explicit task association) is exposed as a separate thread after the configured tasks. (IEC 61131-3
Ed.3, §6.8.2, Table 63)

#### 4.4 Cycle Execution Order

Per IEC 61131-3, within each **scheduled task** execution:

1. **Read Inputs**: Copy I/O inputs to variable images
2. **Execute Programs**: Execute assigned programs in declaration order
3. **Write Outputs**: Copy variable images to I/O outputs

`execute_cycle` determines due tasks (periodic/event) and invokes `execute_task` in scheduler order.

The bytecode executor applies the fixed resource limits in
`12-bytecode.md` section 4.6. In particular, one top-level VM invocation may
execute at most 1,000,000 original bytecode instructions. Nested calls share
that remaining budget, and every execution backend charges the same original
bytecode instruction count. Budget exhaustion is reported through the current
execution-timeout category before the invocation can complete; configured
deadline and watchdog checks remain independent.

```rust
impl Runtime {
    fn execute_task(&mut self, task: &TaskConfig) -> Result<(), RuntimeError> {
        // 1. Update input image from I/O
        self.io.read_inputs(&mut self.storage);

        // 2. Execute each program assigned to this task
        for program_name in &task.programs {
            self.execute_program(program_name)?;
        }

        // 3. Write output image to I/O
        self.io.write_outputs(&self.storage);

        Ok(())
    }
}
```

#### 4.5 Evaluation Context

```rust
/// Context passed during evaluation.
#[derive(Debug)]
pub struct EvalContext<'a> {
    /// Current scope for name resolution
    pub scope_id: ScopeId,

    /// Current POU being executed
    pub current_pou: Option<SymbolId>,

    /// Current instance (for FB/Class methods)
    pub current_instance: Option<InstanceId>,

    /// THIS type (for method context)
    pub this_type: Option<TypeId>,

    /// SUPER type (for inheritance)
    pub super_type: Option<TypeId>,

    /// Reference to symbol table
    pub symbols: &'a SymbolTable,

    /// Current loop depth (for EXIT/CONTINUE)
    pub loop_depth: u32,
}
```

#### 4.6 Portable Bytecode, Scheduler, and VM Helper Contracts

The portable runtime core SHALL preserve these bytecode and execution-helper
contracts:

- Bytecode alignment adds only zero padding. Raw record discriminants are
  retained for validated decoding. Readers use little-endian field order,
  report end-of-input without over-reading, and perform case-insensitive
  resource lookup where the bytecode metadata contract names resources.
- Ready tasks are ordered by priority, due time, and original stable index.
  Retain/restart policy, resource lifecycle state, and periodic/event task
  metadata preserve their declared defaults and transition order.
- Frame stacks and operand stacks are LIFO, enforce call-depth/underflow
  boundaries, and preserve pair/swap ordering.
- Constant-pool decoding preserves primitive, enumeration, and alias runtime
  identity. Invalid payload widths, tags, and type shapes are rejected before
  a value is installed.
- VM dispatch helpers preserve opcode shape, operand decoding, stack/jump
  effects, and borrow materialization. `SIZEOF` resolves through the validated
  type table, and VM traps map to the corresponding stable runtime error.

These helper contracts constrain observable runtime results and deterministic
execution order; they do not promote internal implementation layout to a
public compatibility promise.

#### 4.7 Runtime Metadata, Cycle, I/O, and Metrics Transactions

Applying bytecode is fail-closed. Container validation, metadata extraction,
VM materialization, resource selection, task-reference validation, and
process-image limit validation must all succeed before the executable module is
committed. A failed application preserves the prior executable module, resource
identity, tasks, scheduling state, and process-image sizes. Applying metadata
without executable bytecode intentionally clears the executable VM and all
derived VM caches.

Named resource selection is case-insensitive and does not silently select the
primary resource when the requested name is absent. The sole compatibility
exception is one resource named exactly, ignoring case, `RESOURCE`; a caller
may assign that legacy placeholder its requested resource identity. Multiple
resources or any differently named resource do not qualify for this exception.

Program and task references are case-insensitive at every runtime boundary:
metadata validation, scheduled/background classification, task execution,
metadata snapshots, and `USING` resolution must agree on the same declaration.
A case-only spelling difference cannot make a program execute as both scheduled
and background, cannot pass validation and then fail execution, and cannot
hide a program's `USING` directives.

One successful scan cycle has this transaction order:

1. reject an already faulted resource;
2. apply queued debugger writes, read driver/process/protocol inputs, and
   reapply forced values;
3. collect and deterministically sort ready tasks, then execute their programs
   and function-block references;
4. execute each unscheduled program once in declaration order;
5. stage retain persistence when configured;
6. materialize process/protocol outputs, enforce the output-commit deadline,
   and write every registered driver;
7. publish OpenOT scan telemetry, refresh I/O health, record metrics and debug
   events, then saturating-increment the cycle number.

A failure before completion records one runtime fault and does not increment
the cycle number. Output commit is rollback-safe: if protocol capture, deadline
checking, or any driver output write fails, the process output image is
restored to its pre-commit bytes. An expired output deadline reports the
watchdog-timeout category. Negative logical time projects to protocol time
zero; non-negative milliseconds project exactly until the `u64` boundary.

I/O health snapshots replace, rather than append to, the previous sink content
and preserve driver registration order and names. Applying a non-empty safe
state first updates the complete output image, then asks every driver to write
that same image. The operation returns the first write or non-OK-health
failure, but still attempts later drivers and refreshes the health snapshot.
An empty safe state performs no output write and only refreshes health.

Metrics are optional and non-blocking. With no sink, timing acquisition and
recording are no-ops. With a sink, backend changes, cycles, tasks, profiled
calls, missed intervals, and faults update the same shared metrics object.
Overrun addition and fault counts saturate rather than wrap.

Mesh snapshots resolve requested global names case-insensitively, omit unknown
names, preserve request order, and emit the canonical stored declaration
spelling. Mesh updates resolve names case-insensitively, update only existing
globals, and never create storage for an unknown name.

#### 4.8 Retain, Restart, and Online-Change Transactions

A retain snapshot preserves insertion order. Replacing an existing key changes
its value in place and does not append a duplicate. Program-member retain keys
use the closed form `@program/<program-path>/<variable>`; parsing splits at the
last slash so a qualified program path is preserved, and rejects missing
prefix, program, separator, or variable components.

Only `RETAIN` and `PERSISTENT` declarations survive a warm restart.
`NON_RETAIN` and unspecified declarations are reinitialized. Scalars, strings,
dates, enums, and recursively retainable arrays/structures may be retained;
runtime references and instance identities are never retained, including when
nested in an aggregate.

Restart is prepared against isolated storage before commit. A preparation,
initializer, retained-value migration, or store-load failure leaves the live
storage, instances, cycle count, fault state, debug mutations, and executable
module unchanged. Commit atomically installs prepared storage, clears debug
runtime mutations and fault state, resets task timing and the cycle count, and
resets OpenOT scan-relative producer state. Warm restart restores eligible
values after reconstructing instances with stable identity order; cold restart
uses declaration initializers only.

Online change is one composed transaction: load the retain snapshot, prepare a
warm restart and retained-value migration, validate and materialize the new
bytecode, then commit restart and retained storage. It returns metadata from
the committed runtime. Any failure before commit leaves the old executable
runtime usable; protocol-server refresh belongs to the owning host after this
core swap succeeds.

### 5. Statement Execution

#### 5.1 Statement Result

```rust
/// Statement execution result.
#[derive(Debug)]
pub enum StmtResult {
    /// Normal completion
    Continue,
    /// RETURN statement executed
    Return(Option<Value>),
    /// EXIT from loop
    Exit,
    /// CONTINUE to next iteration
    LoopContinue,
}
```

#### 5.2 Supported Statements

| Statement | SyntaxKind | Description |
|-----------|------------|-------------|
| Assignment | `AssignStmt` | `x := expr;` |
| IF | `IfStmt` | `IF cond THEN ... ELSIF ... ELSE ... END_IF;` |
| CASE | `CaseStmt` | `CASE sel OF ... ELSE ... END_CASE;` |
| FOR | `ForStmt` | `FOR i := start TO end BY step DO ... END_FOR;` |
| WHILE | `WhileStmt` | `WHILE cond DO ... END_WHILE;` |
| REPEAT | `RepeatStmt` | `REPEAT ... UNTIL cond END_REPEAT;` |
| RETURN | `ReturnStmt` | IEC `RETURN;` or truST DEV-022 `RETURN expr;` |
| EXIT | `ExitStmt` | `EXIT;` (break innermost loop) |
| CONTINUE | `ContinueStmt` | `CONTINUE;` (next iteration) |
| Expression | `ExprStmt` | Function/FB calls as statements |
| Empty | `EmptyStmt` | `;` (no-op) |

#### 5.3 Control Flow Rules

**FOR Loop**:
- Control variable, initial, final, increment must be same integer type
- Initial, final, and increment are evaluated once in source order and captured
- Control variable and variables used by initial/final must NOT be modified in loop body
- Termination test at start: `var > final` (positive step) or `var < final` (negative step)
- Step of zero is a runtime error before control/body mutation
- Increment overflow is a runtime error before storing a wrapped value
- Normal completion leaves the first out-of-range value; zero iterations leave
  the initial value; EXIT leaves the current value
- CONTINUE performs the normal increment before the next test
- The production VM performs its internal zero-step and direction comparisons
  in the evaluated step value's integer type; it must not introduce a
  differently typed numeric sentinel that makes a valid signed or unsigned
  loop fail common-type validation.

**WHILE/REPEAT**:
- Condition must evaluate to BOOL
- WHILE tests before iteration; REPEAT tests after (executes at least once)

**CASE**:
- Selector must be elementary type
- Case labels must match selector type
- Duplicate/overlapping labels are errors
- Reversed ranges are errors and are not normalized
- Selector is evaluated exactly once; the first matching branch executes
- ELSE branch is optional; no match and no ELSE is a no-op

**EXIT/CONTINUE**:
- Must be inside a loop (FOR, WHILE, REPEAT)
- Affects innermost enclosing loop only

#### 5.4 Host-stage statement execution contract

The test-only host evaluator preserves the following bounded statement results
for the portable program model:

- assigning `INT#2` to the existing global `x` returns
  `StmtResult::Continue` and leaves `x` equal to `INT#2`;
- a numeric `CASE` selector equal to `INT#2` selects the reviewed inclusive
  `2..3` range branch and assigns `INT#9`, while a string selector equal to
  `"B"` selects the matching single-label branch and assigns `INT#9`;
- a true `IF` condition executes its `THEN` branch and assigns `INT#1`;
- the reviewed `FOR` loop skips `INT#1` through `CONTINUE`, exits at `INT#3`,
  and leaves its sum equal to `INT#2`; the reviewed `WHILE` and `REPEAT`
  loops each advance their target to `INT#2`; and
- `RETURN INT#4` produces `StmtResult::Return(Some(INT#4))`.

These are exact observed host-stage partitions, not proof of every statement,
selector type, branch, loop direction, loop bound, or error case described by
IEC 61131-3 Ed.3 section 7.3.3. Production execution remains the validated
STBC/VM path.

#### 5.5 Host-stage statement debug hook

In the default debug-enabled runtime build, executing the reviewed expression
statement invokes the supplied statement hook exactly once before the
statement completes. This contract does not establish debug-disabled behavior,
breakpoint, location, call-depth, or multi-statement ordering; those require
separate proof or their owning debug-control partitions.

#### 5.6 Source-to-runtime statement lowering

For source accepted by the parser and semantic analyzer, harness compilation
lowers statements in lexical order to the portable program model described by
IEC 61131-3 Ed.3 section 7.3.3. Empty statements emit no runtime operation.
Expression statements preserve their call expression. Assignments preserve
ordinary versus reference-assignment-attempt form, contextualize the right
side with the declared destination type, and route a property target through
its setter call. Bounded `STRING`/`WSTRING` destinations retain their declared
capacity at the assignment boundary.

`IF` lowers ordered `ELSIF` branches and one optional `ELSE`. `CASE` lowers
single labels, comma-separated labels, inclusive ranges, character/string
labels, enumeration labels, and compile-time integer constants without
changing source order. Its selector is evaluated once, and an unmatched
selector with no ELSE block completes without mutation. `FOR` preserves the
declared control variable and contextualizes start, end, and step with its
type; an omitted `BY` becomes the typed integer value one. Start, end, and step
are evaluated once in that order before the first test. Zero step and checked
increment failure occur before the mutation described in section 5.3.
`WHILE` and `REPEAT` preserve pre-test and post-test placement respectively.
`EXIT`, `CONTINUE`, `RETURN`, labels, and `JMP` retain
their explicit control-flow identity. Source locations, when available, are
registered in statement order and do not change execution semantics.
The current bytecode publication boundary rejects a program containing `JMP`
with the stable unsupported C5 edge-case diagnostic; retaining the lowered
identity does not claim executable jump support.

#### 5.6.1 Portable statement representation contract

The portable program model preserves the complete lowered statement identity
until bytecode publication:

- `Assign`, reference `AssignAttempt`, expression, `IF`, `CASE`, `FOR`,
  `WHILE`, `REPEAT`, label, `JMP`, `RETURN`, `EXIT`, and `CONTINUE` are distinct
  nodes. Every node stores its optional source location, and `Stmt::location`
  returns a reference to that exact location without synthesizing or
  normalizing one;
- `IF` retains ordered `ELSIF` pairs and a separate `ELSE` block. `CASE`
  retains ordered branch groups, each branch's ordered labels, and a separate
  `ELSE` block. A case label remains either one exact runtime value or one
  inclusive signed range; representation does not reorder, merge, normalize,
  or silently discard labels;
- loop nodes retain their control expression(s), body order, and—in the `FOR`
  case—the exact control name and explicit lowered step;
- a label retains its name and optional attached statement, while `JMP`
  retains its target name; and
- the host statement result channel distinguishes normal continuation, return
  with or without a value, loop exit, loop continue, and a named jump. Cloning
  a result preserves its value or jump target.

This representation contract supports IEC 61131-3 Ed.3 section 7.3.3 and
Table 72. Whether a represented form is accepted by a particular production
backend remains governed by that backend's validation contract.

### 6. Expression Evaluation

#### 6.1 Supported Expressions

| Expression | SyntaxKind | Description |
|------------|------------|-------------|
| Literal | `Literal` | All literal types |
| Name reference | `NameRef` | Variable lookup |
| Binary | `BinaryExpr` | `a + b`, `a AND b`, etc. |
| Unary | `UnaryExpr` | `NOT x`, `-x`, `+x` |
| Call | `CallExpr` | `func(args)` |
| Index | `IndexExpr` | `arr[i]` |
| Field | `FieldExpr` | `struct.field` |
| Dereference | `DerefExpr` | `ref^` (REF_TO) |
| Address-of | `AddrExpr` | `REF(var)` |
| Parentheses | `ParenExpr` | `(expr)` |
| This | `ThisExpr` | `THIS` |
| Super | `SuperExpr` | `SUPER` |
| Sizeof | `SizeOfExpr` | `SIZEOF(type | storage)` |

**REF operator** (IEC 61131-3 Ed.3 §6.4.4.10.3):
- `REF(var)` returns a reference to a declared variable or instance.
- Applying `REF` to temporary variables (VAR_TEMP, function-local temporaries,
  or the implicit result variable of a function or method) is not permitted.

**SIZEOF operator** (documented vendor extension):
- `SIZEOF(...)` accepts either an explicit type reference or a storage operand (`name`, field/index access, dereference, `THIS.field`).
- The operand is not evaluated; `SIZEOF(...)` resolves the operand's static type and returns a `DINT` byte count.
- Bare names resolve variables before types. Unsupported operands (for example calls or arithmetic expressions) and unsupported/unsized storage types are rejected during analysis.

#### 6.2 Operator Precedence

Per IEC 61131-3 (Table 71):

| Precedence | Operation | Symbol |
|------------|-----------|--------|
| 11 (highest) | Parentheses | `(expr)` |
| 10 | Function/Method call | `name(args)` |
| 9 | Dereference | `^` |
| 8 | Unary | `-`, `+`, `NOT` |
| 7 | Exponentiation | `**` |
| 6 | Multiply/Divide | `*`, `/`, `MOD` |
| 5 | Add/Subtract | `+`, `-` |
| 4 | Comparison | `<`, `>`, `<=`, `>=`, `=`, `<>` |
| 3 | Boolean AND | `AND`, `&` |
| 2 | Boolean XOR | `XOR` |
| 1 (lowest) | Boolean OR | `OR` |

#### 6.3 Short-Circuit Evaluation

Per IEC 61131-3, short-circuit evaluation is implementer-specific. truST uses
the closed choice recorded in `docs/IEC_DECISIONS.md`:

- `BOOL AND` and `BOOL &`: stop on a FALSE left operand
- `BOOL OR`: stop on a TRUE left operand
- `BOOL XOR`: evaluate both operands
- bit-string `AND`, `&`, `OR`, and `XOR`: evaluate both operands

A skipped Boolean operand produces no call, fault, read, write, or other side
effect. Every operand that is evaluated follows the IEC left-operand-first
rule.

#### 6.4 Type Promotion

When operands have different types, only accuracy-preserving implicit widening
applies:

```
SINT → INT → DINT → LINT
USINT → UINT → UDINT → ULINT
REAL → LREAL
```

Signed/unsigned cross-family combinations and integer-to-real combinations
that can lose accuracy have no implicit common type. Narrowing and
accuracy-losing conversions require explicit conversion functions (for
example `DINT_TO_INT` or `DINT_TO_REAL`). Representable untyped literals are
contextualized to the other typed operand.

Integer arithmetic, integer division (truncation toward zero), `MOD`, and unary
negation are checked in the result type. Division by zero reports
`RuntimeError::DivisionByZero`; modulo by zero reports
`RuntimeError::ModuloByZero`. Either fault, and arithmetic overflow, aborts the
containing assignment before storage so the destination retains its previous
value.

#### 6.5 REAL binary arithmetic overflow

For finite `REAL` operands, binary `+`, `-`, `*`, `/`, and `**` results are
accepted only when the result remains finite after representation at IEC basic
single width. Otherwise evaluation returns `RuntimeError::Overflow` before an
assignment store, leaving the target unchanged; the runtime does not clamp or
store infinity or NaN. This rule does not define `LREAL`, subnormal underflow,
signed zero, non-finite operands, explicit conversions, or named numerical
functions such as `EXPT` and `EXP`. (IEC 61131-3 Ed.3, §6.4.2.1, Table 10
footnote e; see
`docs/IEC_DECISIONS.md#2026-07-22---non-finite-real-result-and-explicit-conversion-policy`.)

#### 6.6 REAL named numerical-function overflow

For finite `REAL` operands, `EXP` and `EXPT` results are accepted only when the
result remains finite after representation at IEC basic single width.
Otherwise evaluation returns `RuntimeError::Overflow` before an assignment
store, leaving the target unchanged; the runtime does not clamp or store
infinity or NaN. This rule does not define `LREAL`, non-finite operands,
subnormal underflow, signed zero, explicit conversions, or domain behavior for
other numerical functions. (IEC 61131-3 Ed.3, Tables 28-29; §6.4.2.1,
Table 10 footnote e; see
`docs/IEC_DECISIONS.md#2026-07-22---non-finite-real-result-and-explicit-conversion-policy`.)

#### 6.7 Non-finite REAL conversion results

Explicit numeric, text, and bit-transfer conversions whose destination is
`REAL` or `LREAL` accept a result only when it is finite in the destination
representation. Narrowing overflow, non-finite text, and IEEE bit patterns for
NaN or either infinity return `RuntimeError::Overflow` before the destination
is stored. Finite values, including signed zero and subnormal values, remain
valid. This reviewed policy is recorded in
`docs/IEC_DECISIONS.md#2026-07-22---non-finite-real-result-and-explicit-conversion-policy`;
IEC 61131-3 does not prescribe this runtime fault surface.

#### 6.8 Signed integer result materialization

When a runtime operation materializes an integer result as `SINT`, `INT`,
`DINT`, or `LINT`, a value inside the destination range is preserved exactly
and stored with that destination's runtime value tag. The closed destination
ranges are the signed integer ranges in IEC 61131-3 Ed.3 section 6.4.2.1,
Table 10.

A value below the destination minimum or above its maximum returns
`RuntimeError::Overflow` before any destination is stored. The runtime never
wraps, saturates, truncates, or substitutes a value. This fault mapping applies
both to signed integer arithmetic materialization and to internal numeric
coercion into a signed integer destination. IEC section 7.3.2 requires a
numerical result outside its result-type range to be treated as an error;
sections 6.6.2.5.2 and 6.6.2.5.3 leave conversion execution errors and an
out-of-range conversion result implementer-specific. The stable truST mapping
is specified by this section; it is not an IEC deviation.

The signed materialization boundary accepts only the four signed integer
destinations. Supplying `USINT`, `UINT`, `UDINT`, `ULINT`, `REAL`, or `LREAL`
as the destination returns `RuntimeError::TypeMismatch` without producing a
value. Callers must route those categories through their owning conversion
boundary.

#### 6.9 Integer runtime normalization to `i64`

The runtime's signed host-width operand boundary accepts only integer `Value`
tags. `SINT`, `INT`, `DINT`, and `LINT` values are returned as the same
mathematical value in `i64`. `USINT`, `UINT`, `UDINT`, and `ULINT` are also
preserved exactly when their value is no greater than `i64::MAX`.

A `ULINT` value above `i64::MAX` returns `RuntimeError::Overflow`; the boundary
does not wrap, truncate, saturate, or reinterpret the high bit as a sign.
Every non-integer runtime tag returns `RuntimeError::TypeMismatch` without a
substitute value. This is an internal operand-normalization contract, not an
additional source-level implicit conversion. IEC 61131-3 Ed.3 section 6.4.2.1,
Table 10 supplies the integer ranges; the fail-closed host representation is
the truST runtime contract specified by this section, not an IEC deviation.

#### 6.10 Unsigned integer result materialization

When a runtime operation materializes an integer result as `USINT`, `UINT`,
`UDINT`, or `ULINT`, a value from zero through the destination maximum is
preserved exactly and stored with that destination's runtime value tag. The
closed destination ranges are the unsigned integer ranges in IEC 61131-3 Ed.3
section 6.4.2.1, Table 10.

A value below zero or above the destination maximum returns
`RuntimeError::Overflow` before any destination is stored. The runtime never
wraps, saturates, truncates, or substitutes a value. This fault mapping applies
both to unsigned integer arithmetic materialization and to internal numeric
coercion into an unsigned integer destination. IEC section 7.3.2 requires a
numerical result outside its result-type range to be treated as an error;
sections 6.6.2.5.2 and 6.6.2.5.3 leave conversion execution errors and an
out-of-range conversion result implementer-specific. The stable truST mapping
is specified by this section; it is not an IEC deviation.

The unsigned materialization boundary accepts only the four unsigned integer
destinations. Supplying `SINT`, `INT`, `DINT`, `LINT`, `REAL`, or `LREAL` as
the destination returns `RuntimeError::TypeMismatch` without producing a
value. Callers must route those categories through their owning conversion
boundary.

#### 6.11 Host helper evaluation contract

The non-production host helpers used by constant, debugger, and configuration
flows evaluate only their explicitly supplied capabilities:

- nested constant arithmetic is evaluated recursively;
- a named constant is resolved only through the supplied resolver, while a
  resolver-less non-constant name returns `UnsupportedExpr`;
- an array repetition initializer expands the repeated sequence in source
  order and derives the resulting one-based shape from the expanded element
  count; for example, `[3(1, 2)]` produces the six-element sequence
  `1, 2, 1, 2, 1, 2` with bounds `(1..6)`, as required by IEC 61131-3 Ed.3
  section 6.4.4.5.2;
- with a supplied `StandardLibrary` capability, the reviewed
  `ABS(DINT#-1)` expression evaluates to `DINT#1`; without that capability the
  same expression returns `RuntimeError::TypeMismatch`. Other standard
  functions require separate specification behavior and proof.

This is a bounded host-helper contract. It does not make the old host
evaluator a production execution path and does not authorize function-block,
method, I/O, or other side-effecting calls.

##### 6.11.1 Constant-expression helper authority

The constant-expression helper follows the expression ordering and result
error requirements of IEC 61131-3 Ed.3 section 7.3.2 and Table 71 within this
closed, side-effect-free product boundary:

- literals are returned without coercion, and unary and binary expression
  trees are evaluated recursively with the portable operator implementation;
  division by zero, type mismatch, and overflow remain exact runtime errors
  rather than being replaced by a value;
- an unqualified name, or a field-only qualified chain such as
  `Constants.Limits.High`, is resolved as one exact name through the supplied
  resolver. An unresolved name, an indexed/dereferenced chain, `THIS`,
  `SUPER`, or a call outside the array-repeat grammar is not a constant
  expression;
- a `SIZEOF(type)` expression uses the supplied type registry, returns the byte
  count as `DINT`, maps an unknown or unsized type to `TypeMismatch`, and maps a
  size or `DINT` conversion overflow to `Overflow`;
- an ordinary array initializer evaluates its elements left-to-right and has
  the one-based helper shape `1..N`. A repetition group implements IEC
  61131-3 Ed.3 section 6.4.4.5.2: every positional expression in the group is
  evaluated for every repetition in source order. Zero repetitions contribute
  no elements; a negative count, a count not representable by the helper, a
  named argument, or a target that is not an integer literal is rejected
  without partial success.

The resolver is the helper's complete name authority. It does not fall through
to runtime storage, perform a call, or create a missing constant.

##### 6.11.2 Storage-expression helper authority

The bounded storage-expression helper evaluates IEC expression forms from
section 7.3.2 against an explicitly supplied storage snapshot. It is not the
cycle executor and has no implicit write capability.

- An ordinary name resolves in this exact order: current local frame, current
  instance including its nearest inherited field, global storage, then retained
  helper storage. A missing name returns `UndefinedVariable`.
- `THIS` returns the supplied current instance. `SUPER` returns its immediate
  parent. Missing current-instance context is `TypeMismatch`; a stale current
  instance is `NullReference`; and a current instance without a parent is
  `TypeMismatch`. These value forms support the reference concepts in IEC
  61131-3 Ed.3 sections 6.6.5.7.2 and 6.6.5.7.3 without authorizing a method
  call.
- A field-only name chain is first treated as one exact qualified storage name.
  Only when that exact name is absent is its target evaluated as a structure or
  instance. Structure lookup preserves the stored field contract, instance
  lookup includes inherited fields, and an absent field returns
  `UndefinedField`. `%X`, `%B`, `%W`, and `%D` partial-access suffixes use the
  portable bit-string access rules and preserve their exact type or bounds
  error.
- An untyped structure initializer rejects duplicate field names
  case-insensitively before returning the aggregate. Its values are otherwise
  evaluated left-to-right. Array initializer/repetition behavior is identical
  to section 6.11.1.
- An array index accepts the integer and bit-string index value tags supported
  by the portable offset helper, requires one index per declared dimension,
  honors every inclusive lower and upper bound, and returns the selected
  row-major element. `STRING` and `WSTRING` accept exactly one one-based
  character index and return `CHAR` and `WCHAR`, respectively. Wrong index
  arity or type is `TypeMismatch`; an index value outside the helper's signed
  host range is `Overflow`; and a representable, validly typed index outside
  its bounds is `IndexOutOfBounds`.
- Boolean `AND`/`&` with a false left operand and Boolean `OR` with a true left
  operand do not evaluate the right operand, following the closed truST choice
  for the implementer-specific evaluation extent in IEC section 7.3.2.
  `XOR`, bit-string operators, and non-short-circuited cases evaluate both
  operands left-to-right.
- `REF(lvalue)` implements the reference operation in IEC 61131-3 Ed.3 section
  6.4.4.10.3 only for an existing local, current-instance, or global root and
  validated field/index path. It never creates storage. Dereference returns the
  current referenced value; an empty or stale reference is `NullReference`, and
  a non-reference operand is `TypeMismatch`.
- `SIZEOF(type)` follows section 6.11.1. Calls are rejected unless a
  `StandardLibrary` capability is explicitly supplied. With that capability,
  only a name or field-only qualified call target is admitted; an unknown
  target is `UndefinedFunction`. Positional arguments retain source order.
  Named arguments are all-or-nothing, case-insensitive, unique, and are
  reordered to the registered fixed parameter order. Variadic names must form
  one contiguous registered sequence at or above its declared start and meet
  its minimum count. Missing, duplicate, mixed, unknown, or gapped names fail
  before the function is called.

`ArgValue::Target` reads the existing lvalue and supplies its current value to
a pure standard-library call. This does not turn the call into an output or
in-out write path.

##### 6.11.3 Storage-lvalue helper authority

The storage-lvalue helper applies the assignment-target concepts used by IEC
61131-3 Ed.3 sections 7.3.2 and 7.3.3 through a fail-closed host boundary:

- reads are the corresponding storage-expression read and therefore use the
  same name, aggregate, index, partial-access, and dereference semantics;
- a simple write updates the first existing target in this order: current local
  frame, current instance including the nearest inherited field, global
  storage, then retained helper storage. An unknown name returns
  `UndefinedVariable` and creates nothing;
- a field-only qualified lvalue first targets an existing exact qualified
  local, instance, or global storage name, matching expression and reference
  resolution. Only when that exact name is absent does the write rebuild a
  structure target or address an instance field;
- an index write first validates target shape, index arity, index type, and all
  inclusive bounds. It replaces only the selected element and commits the
  rebuilt aggregate through its owning lvalue. Any validation or recursive
  commit failure leaves the stored aggregate unchanged;
- a structure field write changes only an existing field. An instance field
  write changes the exact nearest resolved instance field. An absent field is
  `UndefinedField`, and neither path adds a field. Partial-access writes change
  only the selected bit/byte/word/double-word and preserve every other bit;
- a dereference write accepts either a `REF(lvalue)` expression or an existing
  non-empty reference value, validates its complete field/index path, and
  writes only that target. Empty or stale references return `NullReference`;
  non-reference values return `TypeMismatch`; and failed writes do not mutate
  storage.

These precedence and error mappings are truST host-runtime contracts. They do
not add source-language scoping rules or an IEC deviation, and they do not
authorize helper evaluation as production cyclic execution.

#### 6.12 Host-stage expression evaluation contract

The test-only host evaluator preserves these exact reviewed expression
partitions while constructing and exercising the portable program model:

- a literal `INT#42` evaluates to `INT#42`, and the name `x` resolves the
  existing global value `INT#7`;
- indexing `[INT#1, INT#2, INT#3]` at zero-based host-model index `1` returns
  `INT#2`, and selecting field `a` from the reviewed structure returns
  `INT#10`;
- nested index-then-field access returns `INT#20`, while nested
  field-then-index access returns `INT#4`;
- `FALSE AND (INT#1 / INT#0)` returns `FALSE` without evaluating the
  division;
- the reviewed mixed numeric operations produce, respectively,
  `INT#1 + DINT#2 = DINT#3`, reject `UINT#2 + INT#3` with
  `RuntimeError::TypeMismatch`,
  `LREAL#1.5 * REAL#2.0 = LREAL#3.0`,
  `REAL#5.0 / INT#2 = REAL#2.5`, `INT#2 < REAL#2.5 = TRUE`, and
  `INT#5 = DINT#5 = TRUE`;
- one composed source-and-harness path produces `sum = INT#5`,
  `neg = INT#-5`, `arr_val = INT#7`, `ref_out = INT#2`,
  `fb_out = INT#5`, and `size_t = DINT#2` without a cycle error;
- the reviewed time operations produce `TIME#1500ms` from
  `TIME#1000ms + TIME#500ms`, a time-of-day value at 1500 milliseconds from
  the reviewed `TOD + TIME`, `TIME#1500ms` from the reviewed `DT - DT`,
  `LTIME#6s` from `LTIME#2s * INT#3`, `TIME#500ms` from
  `TIME#1000ms / INT#2`, `TRUE` from the reviewed `DATE` comparison, and
  `LTIME#7ns` from the reviewed `LDT - LDT`;
- `REF(x)^` reads `INT#5`, and writing `INT#9` through that dereference
  changes the same existing global `x` to `INT#9`; and
- in a current child instance whose parent is the reviewed base instance,
  `SUPER` evaluates to that exact parent instance.

This is bounded host-stage product authority for the named observations. It
does not use one fixture to certify every expression form, numeric type
combination, temporal boundary, reference lifetime, inheritance graph, or
failure partition in IEC 61131-3 Ed.3 section 7.3.2. Production execution
remains the validated STBC/VM path.

#### 6.12.1 Source-to-runtime expression lowering

Harness lowering preserves the expression tree and contextual declared type
for the IEC 61131-3 Ed.3 section 7.3.2 expression forms. Parentheses affect
grouping but emit no extra runtime node. Unary and binary operators retain
their parsed operator and operand order. Names, field selection,
multidimensional indices, dereference, `REF`, `THIS`, `SUPER`, calls, named
arguments, array/aggregate initializers, and `SIZEOF` retain their semantic
target.

Literal lowering accepts decimal and based integers with separators, real
exponents, narrow and wide strings with IEC dollar escapes, the short and long
time/date families, typed bit strings, characters, and resolved enumeration
members. Context-free untyped integers use the reviewed default integer value;
an assignment or call-parameter context materializes the literal in the
declared destination type and rejects a value outside that type. Array
repetition expands in source order as specified by IEC 61131-3 Ed.3 section
6.4.4.5.2, while aggregate fields retain their written names for declared-type
materialization.

Malformed literals, unresolved types, invalid constant-only operands, invalid
assignment targets, calls with missing arguments, and unsupported syntax fail
compilation. Lowering never substitutes a default expression merely to keep a
malformed program executable.

#### 6.13 Portable date and time operator contract

IEC 61131-3 Ed.3 section 6.6.2.5.12 and Table 35 define the accepted
`TIME`/`DATE`/`TOD`/`DT` and long-family operator combinations and require an
error when the result exceeds the implementer-specific output range. The
portable runtime applies those operators as follows:

- `TIME` and `LTIME` addition or subtraction preserves width and uses checked
  signed nanoseconds.
- Short `DATE`, `TOD`, and `DT` arithmetic converts a duration to whole
  `DateTimeProfile` ticks by truncating toward zero. Same-family subtraction
  converts the signed tick difference back to a checked `TIME`.
- Long `LDATE`, `LTOD`, and `LDT` arithmetic operates directly in signed
  nanoseconds. Same-family subtraction returns `LTIME`.
- Duration addition to `TOD`, `LTOD`, `DT`, or `LDT` does not wrap at a day
  boundary; it returns the checked signed value in the same family.
- `TIME` and `LTIME` multiplication or division accepts signed and unsigned
  integer, `REAL`, and `LREAL` factors. Integer division and the final real
  result truncate toward zero. Zero division returns
  `RuntimeError::DivisionByZero`, a non-numeric factor returns
  `RuntimeError::TypeMismatch`, and a non-finite or out-of-range result returns
  `RuntimeError::Overflow`.
- Ordering compares stored ticks or nanoseconds only when both operands have
  the same runtime family. Cross-family ordering returns
  `RuntimeError::TypeMismatch`.

All rejected operations produce no substitute value or partial mutation.

### 7. POU Execution

#### 7.0 Portable POU-definition representation

The portable definition model preserves the semantic inputs needed by
initializer materialization and bytecode publication:

- every parameter retains its name, declared type, direction, optional direct
  address, and optional default expression;
- every variable retains its name, declared type, optional initializer,
  retain policy, static-storage, external, constant, and optional direct-address
  attributes. These attributes are independent; representation does not infer
  one flag from another;
- a function keeps its return type, ordered parameters, ordinary locals,
  static locals, ordered `USING` imports, and ordered body;
- a function block keeps an optional base classified distinctly as a function
  block or class, plus ordered parameters, instance variables, temporaries,
  imports, methods, and body;
- a method keeps its optional return type, ordered parameters, ordinary and
  static locals, imports, and body; and
- classes and interfaces keep their optional named base, ordered imports, and
  methods. Classes additionally keep ordered instance variables.

Names and import order are preserved exactly at this representation boundary;
case-insensitive resolution and duplicate/conflict rejection belong to the
owning assembly/registry boundary. The model corresponds to IEC 61131-3 Ed.3
Tables 19, 40, 47, 48, and 51, with truST product metadata fields governed by
this runtime specification rather than treated as IEC deviations.

#### 7.1 FUNCTION

- **Stateless**: Variables re-initialized each call
- **Return value**: Via IEC function-name assignment or the truST DEV-022
  value-bearing RETURN extension
- **Side effects**: VAR_IN_OUT and VAR_EXTERNAL may be modified
- **Default result**: If no assignment/RETURN occurs, the function result is the default initial value of its return type (IEC 61131-3 Ed.3 §6.4.2, Table 10).

```rust
fn call_function(
    &mut self,
    symbol_id: SymbolId,
    call_node: &SyntaxNode,
    ctx: &EvalContext,
) -> Result<Value, RuntimeError> {
    // 1. Create new frame
    let frame_id = self.storage.push_frame(symbol.name.clone());

    // 2. Bind arguments to parameters
    self.bind_arguments(symbol_id, call_node, ctx)?;

    // 3. Execute function body
    let result = self.eval_statement_list(&func_syntax, &func_ctx)?;

    // 4. Get return value
    let return_value = match result {
        StmtResult::Return(Some(v)) => v,
        _ => self.storage.current_frame()
            .and_then(|f| f.return_value.clone())
            .unwrap_or_else(|| self.default_value(func_return_type)),
    };

    // 5. Pop frame
    self.storage.pop_frame();

    Ok(return_value)
}
```

#### 7.2 FUNCTION_BLOCK

- **Stateful**: Internal VAR persists across calls
- **Instances**: Each instance has independent state
- **Call syntax**: `instance(inputs)` then access outputs via `instance.output`
- **Omitted `VAR_INPUT` arguments**: When a FUNCTION_BLOCK call leaves an input open, runtime reuses the instance's previously stored input value; on the first call it falls back to the parameter initializer or the IEC type default if no initializer exists.

```rust
fn call_fb(
    &mut self,
    type_id: SymbolId,
    instance_id: InstanceId,
    call_node: &SyntaxNode,
    ctx: &EvalContext,
) -> Result<(), RuntimeError> {
    // 1. Bind input arguments to instance
    self.bind_fb_inputs(instance_id, call_node, ctx)?;

    // 2. Execute FB body
    let fb_ctx = EvalContext {
        current_instance: Some(instance_id),
        this_type: Some(type_id),
        ..ctx
    };
    self.eval_statement_list(&fb_syntax, &fb_ctx)?;

    // 3. FB outputs accessed via instance after call
    Ok(())
}
```

#### 7.3 PROGRAM

- **Stateful**: Like FUNCTION_BLOCK
- **Task association**: Executed cyclically by assigned task
- **Instance-local variables**: PROGRAM variables are stored per program instance and accessed via that instance (IEC 61131-3 Ed.3 §6.8.2, Table 62; access paths to PROGRAM inputs/outputs/internal variables).
- **VAR_ACCESS**: Can expose variables for external access (IEC 61131-3 Ed.3 §6.8.2, Table 62).

#### 7.4 METHOD

- **Called on instance**: `obj.method(args)`
- **Access specifiers**: PUBLIC, PROTECTED, PRIVATE, INTERNAL
- **Inheritance**: Can OVERRIDE base implementation

Member visibility is a compile-time boundary and does not disappear during
lowering:

- an accepted `PUBLIC`, same-owner `PRIVATE`, derived `PROTECTED`, or
  same-namespace `INTERNAL` call lowers to the same method identity selected by
  semantic resolution;
- dynamic dispatch through a public interface or base reference reaches the
  selected concrete implementation without widening access to any other
  member;
- `SUPER.member` selects the inherited implementation but remains subject to
  the inherited member's visibility;
- an inaccessible member produces a compile error and no runnable
  program/class/function-block model; and
- runtime metadata must not synthesize callable entries for rejected access
  paths.

Class and function-block ordinary variables and truST properties follow the
same compile-time matrix. For function-block directional members, inputs and
outputs are externally readable, outputs are externally read-only, in-outs are
usable only for call binding and inside the FB body, and externals are
implicitly protected (IEC 61131-3 Ed.3 §6.6.7.7). A rejected external write or
member access performs no runtime storage change.

#### 7.5 EN/ENO Mechanism

Every function, function block, and method uses one deterministic call
transaction:

1. Resolve the callable and its declaration-order parameter metadata.
2. If a named `EN` actual exists, evaluate it exactly once before all other
   actuals regardless of its written position.
3. If `EN` is false, skip every other actual and writable-target resolution,
   do not enter the body, preserve function-block/receiver state and ordinary
   caller targets, copy only `FALSE` to a connected `ENO`, and return the
   declared type default for a value-producing callable.
4. Otherwise evaluate each actual exactly once from left to right in source
   order. Bind formal names without reordering evaluation. Snapshot inputs and
   in-outs; initialize function/method outputs per call and use stored
   function-block inputs/outputs.
5. Initialize `ENO` to `TRUE`, execute the body, and allow the body to set it
   `FALSE`.
6. On normal return, capture every result/output/in-out/ENO value, validate the
   complete destination set, then commit all connected transfers. `ENO =
   FALSE` set by a normally returning body does not cancel its other
   transfers.
7. On a runtime execution error, report the error, force the call's ENO
   disposition to false, and commit no result, output, in-out, or receiver /
   function-block instance mutation through the failed call boundary.

A call cannot map the same or overlapping caller storage to two writable
formals. Rejecting this before execution makes transfer independent of
parameter declaration order. An input may snapshot storage that is also the
single target of one output or in-out. These implementer-specific rules are
recorded in
[`IEC_DECISIONS.md`](https://github.com/johannesPettersson80/trust-platform/blob/main/docs/IEC_DECISIONS.md#2026-07-30---pou-call-evaluation-execution-control-and-output-transfer).

#### 7.6 Host-stage POU evaluation contract

The test-only host evaluator retains a stage-accurate compatibility contract
for constructing and executing the portable POU model before bytecode
lowering:

- the reviewed function call binds the supplied named input, executes the
  body, and returns the assigned function result;
- omitted interface inputs use their declaration initializer or declared type
  default; uninitialized call-local interface variables and an unassigned
  interface return use their declared type default;
- function-block instance variables persist across calls;
- an omitted function-block `VAR_INPUT` uses its declared default on the first
  call and reuses the last explicitly stored input on later calls;
- a read-only `VAR_INPUT` pointer slot does not make its referenced storage
  read-only: dereference/index writes update only the selected caller element;
- the reviewed `ARRAY[*]` `VAR_IN_OUT` and pointer callers preserve the
  asserted whole-element sequences and write through to only the selected
  element. These assertions do not establish array bounds independently;
- the reviewed fixed standard-library named-argument binder rejects an unnamed
  argument as `RuntimeError::InvalidArgumentName("<unnamed>")`;
- once the reviewed split binder has accepted a named argument, a following
  unnamed argument is rejected with that same error rather than being assigned
  by position;
- a function call with `EN := FALSE` does not execute its reviewed body, leaves
  the existing global counter at `INT#0`, and copies `FALSE` to the caller's
  `ENO` target; `EN` is evaluated first and suppresses every other actual and
  writable-target resolution;
- the reviewed function binding copies `VAR_INPUT a = INT#2` into the call,
  copies `VAR_OUTPUT b = INT#5` and `VAR_IN_OUT c = INT#4` back to their
  caller targets, and returns `INT#2`; and
- creating the reviewed function-block instance materializes its declared
  `VAR_INPUT inc := INT#5` initializer in instance storage.

These rules specify the host-stage model exercised by focused unit tests.
Production program execution remains the STBC/VM path.

#### 7.7 Source-to-runtime POU assembly

Harness compilation predeclares all functions, function blocks, classes, and
interfaces from the semantic declaration catalog before lowering bodies. This
permits forward and cross-file type references while still rejecting
case-insensitive duplicate POU names. Namespace qualification and each POU's
local `USING` list are preserved in its runtime definition.

Function parameters retain declaration order, direction, type, accepted input
or output initializer, and reviewed direct address. Inputs use their
initializer only when omitted. Outputs materialize their initializer or type
default at the start of every call before body execution and optional
copy-back. Ordinary and temporary locals are likewise per-call storage.
`VAR_STAT` uses function-qualified persistent storage and is initialized once
per runtime construction. `VAR_IN_OUT` and `VAR_EXTERNAL` carry no declaration
initializer; the former aliases caller storage and the latter refers to an
existing global without creating a local slot. Function return assignment and
`RETURN` use the declared return type. A function that declares a result type
but provides no result assignment is rejected as a missing return value.

Function-block parameters retain the same ordered parameter contract.
`VAR`/`VAR_STAT` are instance state, `VAR_TEMP` is call-temporary state, and
separate instances never share those fields. A function block may extend a
function block or class; another base kind is rejected. Program `VAR`,
`VAR_INPUT`, `VAR_OUTPUT`, and `VAR_IN_OUT` declarations become instance state,
`VAR_TEMP` is cycle-temporary, program `VAR_GLOBAL` is lifted into assembled
global storage, and `VAR_EXTERNAL` creates no duplicate.

Class fields, base identity, methods, and synthesized property getter/setter
methods are retained. Function blocks retain their methods and properties as
well. Interface declarations retain their base and method/property signatures;
they allocate no concrete instance storage by themselves. Method parameters,
locals, `VAR_STAT`, return type, and local `USING` context follow the function
rules, with method-static storage isolated by owning instance.

Wildcard direct addresses are not accepted on `VAR_INPUT` or `VAR_IN_OUT`.
Unsupported variable-block kinds, invalid direct addresses, unknown types or
bases, incompatible inheritance, and missing required names/types fail
compilation rather than producing a partial POU definition. These rules
implement the runtime assembly boundary for IEC 61131-3 Ed.3 sections 6.4.2,
6.5, and 6.6.

#### 7.8 Source-to-runtime type assembly

Harness compilation materializes the supported IEC derived-type declarations
into one project type registry before it lowers POU variables and bodies. A
declared type retains its source spelling as its canonical name and is found
case-insensitively. Enclosing namespaces qualify the canonical name. An
unqualified type reference may resolve through the declaration's active
`USING` list; an explicitly qualified reference is resolved as written.

The runtime registry represents the following declaration contracts:

- a directly derived type is a named alias to its resolved target;
- an integer subrange retains its resolved integer base and inclusive lower
  and upper bounds;
- an array retains its element type and every declared dimension in source
  order;
- a structure retains field order, all names from a multi-name declaration,
  the resolved field type, an optional relative/direct address, and an
  optional field-default initializer;
- a union retains the same ordered member metadata as variants;
- an enumeration retains its declared base type, source-ordered named values,
  implicit values beginning at zero, and checked continuation from the
  preceding explicit value;
- `REF_TO` retains its target type according to IEC Table 12;
- the truST `POINTER TO` extension retains its target type under the separate
  pointer policy in `02-data-types.md`; and
- bounded `STRING[n]` and `WSTRING[n]` retain their character capacity in the
  resolved target type.

Every integer constant operand that defines a subrange bound, array dimension,
bounded-string capacity, or explicit enumeration value is imported from the
semantic declaration result with its resolved constant identity and value.
Runtime type lowering must not reevaluate it against an empty or source-order
partial constant map. The selected value remains stable across source
permutations and retains declaration-site namespace/`USING` resolution.

Fixed bounds are inclusive and require `lower <= upper`. String capacities are
positive. Enumeration explicit values and implicit successors fit the
declared integer base; continuation uses checked arithmetic and never
saturates or wraps. Undefined, mutable, ambiguous, cyclic, non-integer,
out-of-domain, or otherwise invalid operands fail compilation before the type
registry exposes a partial declaration.

A structure may contain a `REF_TO` field targeting its own declared type. The
registry reserves that owning identity before lowering its fields so the
self-reference resolves to the completed structure rather than to an
unrelated or unknown type.

A type-level default initializer is lowered once into the runtime initializer
catalog and bound to the declared type. A default on a structure or union
member is likewise bound to that exact member. Multiple names in one member
declaration retain independent member identities while sharing the same
declared default expression. Each retained initializer also preserves the
declaration namespace and active `USING` context needed to select the same
constant identity chosen by semantic analysis. Importing or instantiating the
type from another source does not rebind that default to the consumer's
namespace context.

Type-level and member-level constant dependencies are evaluated from the
complete accepted project constant graph before a value of that type is
materialized. A later-declared constant therefore cannot be observed through
its temporary elementary default. Reordering source units does not change the
result. Explicit qualification is exact; unqualified lookup uses the
declaration's lexical namespace and `USING` list and fails closed on a missing
or ambiguous match.

Default materialization follows this precedence chain:

1. elementary or recursively assigned data-type default;
2. structure/union member-specific default;
3. listed type-level aggregate override;
4. variable-specific initializer; and
5. eligible exact-instance `VAR_CONFIG` override.

An omitted aggregate member inherits the preceding applicable default. A
failed constant dependency, coercion, or aggregate override returns no
partially initialized runtime.

Function-block, class, and interface declarations are predeclared as type
identities before their members or dependent POU bodies are lowered. Derived
data types and POU types occupy one case-insensitive project type namespace:
duplicate names, including case-only or cross-kind collisions, are rejected.
Unknown targets, invalid bounds or capacities, and malformed member
declarations fail compilation rather than leaving a reserved or partial type
available to the returned runtime.

These rules are the runtime-assembly projection of IEC 61131-3 Ed.3 section
6.4.4 and Tables 11-12. Namespace lookup and `POINTER TO` are the documented
truST product behaviors described in `02-data-types.md`; they do not alter the
IEC type rules.

#### 7.9 Source-to-runtime variable assembly

After resolving each declaration's type, harness compilation projects IEC
variable sections into the runtime POU and global metadata model without
changing declaration order or source spelling.

For functions and methods, `VAR_INPUT`, `VAR_OUTPUT`, and `VAR_IN_OUT` become
ordered parameters with their IEC direction. Inputs retain an optional
omission default. Outputs retain an optional per-call initializer. `VAR` and
`VAR_TEMP` become automatic locals and are reinitialized for each call from
their explicit initializer or declared type default. `VAR_STAT` becomes
function- or method-qualified static storage and is initialized once; method
static storage is receiver-local. `VAR_IN_OUT` aliases caller storage and
cannot declare an initializer. `VAR_EXTERNAL` refers to existing global
storage, cannot declare an initializer, and creates no local variable record.

For function blocks, `VAR_INPUT`, `VAR_OUTPUT`, and `VAR_IN_OUT` become ordered
parameters. Input and output declaration initializers are materialized when
the instance is created; a supplied input replaces its stored value, while an
omitted input reuses it. `VAR` and `VAR_STAT` become persistent instance
variables initialized with the instance, while `VAR_TEMP` becomes
call-temporary storage reinitialized on every invocation. `VAR_IN_OUT` aliases
caller storage and has no declaration initializer. For programs, the input,
output, ordinary, and static sections all become initialized persistent
program-instance variables, while `VAR_TEMP` becomes cycle-temporary storage
reinitialized on every cycle. Program `VAR_IN_OUT` remains externally supplied
storage and cannot declare an initializer. Program
`VAR_GLOBAL` declarations are lifted into the project global table, and
`VAR_EXTERNAL` creates no duplicate program field. Class `VAR` and `VAR_STAT`
declarations become class fields; a class `VAR_EXTERNAL` creates no field.

Every projected variable retains:

- each declared name in source order, including every name in a multi-name
  declaration;
- its resolved type identity;
- its lowered optional initializer;
- its `CONSTANT` state where the runtime record has such a field;
- its effective `RETAIN`, `NON_RETAIN`, unspecified, or truST `PERSISTENT`
  policy;
- whether it uses static storage; and
- its parsed concrete direct address, including area, size, byte/bit
  coordinates, and complete hierarchy path.

Qualifier validation precedes projection. Exactly one occurrence of
`CONSTANT`, `RETAIN`, `NON_RETAIN`, or truST `PERSISTENT` may qualify a
declaration section. A restart policy is projected only for ordinary
function-block/program/class storage, truST static storage,
function-block/program inputs and outputs, and globals. Function/method
call-local storage, in-out aliases, temporaries, external aliases, access
paths, and configuration bindings reject a restart qualifier. The compiler
must not accept a qualifier and replace it with `Unspecified`. `CONSTANT`
remains orthogonal read-only metadata on the accepted storage-declaration
sections in `03-variables.md`, but cannot be combined with a restart policy.

An accepted function-block/program `VAR_INPUT` edge declaration lowers to one
raw input slot plus one hidden trigger state per declared name. At each
executed function-block invocation or program cycle, the runtime samples the
raw input exactly once, advances the Table 44 `R_TRIG` or `F_TRIG` state
machine, and supplies `Q` as the input value visible to the owning body.
Holding a level cannot repeat a pulse, multiple names never share phase, and
an unexecuted function block does not sample or infer an intervening edge. The
hidden state is not a source-visible field and cannot collide with a user
declaration.

Cold restart uses the Table 44 initial phase: an initially high `R_EDGE` and
an initially low `F_EDGE` each pulse on the first execution. On warm restart,
the raw input and hidden phase follow the same effective input-section
`RETAIN`, `NON_RETAIN`, unspecified, or truST `PERSISTENT` policy. Retained
phase therefore suppresses a fabricated pulse at an unchanged retained level;
reinitialized phase follows the cold initial-state rule. Function-block
methods cannot resolve the transformed input.

A constant declaration is available to every constant-expression initializer
that can see it in the same accepted lexical, namespace, configuration, or
resource scope, independent of textual declaration order. The compiler
evaluates the scope's case-insensitive constant-dependency graph before
materializing consuming input, output, local, temporary, static, instance, or
global initializers. A later POU-local constant is therefore available to an
earlier parameter or local declaration without leaking into another POU with
the same leaf name. Each name in a multi-name declaration receives its own
variable record and independent storage while retaining the same declared
initializer expression and address metadata. A cyclic, undefined, mutable,
ambiguous, incompatible, or otherwise invalid dependency fails compilation
before a runtime is returned.

Root, namespace, program, configuration, and resource `VAR_GLOBAL`
declarations become global metadata and initialized storage. Namespace-owned
globals retain their qualified names. A global declaration's retain policy and
resolved type remain attached to its restart metadata; its initializer is
evaluated only after the owning declaration has been accepted.

Wildcard direct addresses remain governed by IEC 61131-3 Ed.3 section
6.5.5.4: they are rejected for `VAR_INPUT` and `VAR_IN_OUT`, and an accepted
program or function-block wildcard requires a matching `VAR_CONFIG` binding.
Malformed concrete addresses, unknown types, missing names/types, and variable
sections not supported by the owning POU fail compilation rather than
producing partial runtime metadata.

Runtime assembly enforces the complete variable-section ownership matrix
before registering the POU. A function, method, or function block containing
`VAR_GLOBAL`, `VAR_ACCESS`, or `VAR_CONFIG` is rejected. A program accepts its
IEC `VAR_GLOBAL` and `VAR_ACCESS` additions but rejects `VAR_CONFIG`. A class
accepts ordinary/static-extension and external fields only; directional,
temporary, global, access, and configuration sections are rejected. An
interface with a direct variable section is rejected, while parameter sections
inside an interface method prototype remain signature metadata. No rejected
section is silently ignored, lifted to another scope, or used to construct a
partial runtime type, POU, global, access path, or initializer.

These rules project IEC 61131-3 Ed.3 sections 6.5.1-6.5.6, Figures 7-9, and
Tables 13-16. `PERSISTENT` remains the documented truST extension specified in
`03-variables.md`; this runtime projection does not classify it as IEC
behavior.

#### 7.10 Project-wide source assembly and declaration resolution

Harness compilation treats the complete `CompileSession` source list as one
project declaration graph. Every source is parsed and entered into the
semantic project before runtime declarations are materialized. Source-vector
order and optional source paths are not visibility rules: reordering otherwise
identical source units, or adding diagnostic path metadata, must not change
which valid declarations resolve or the canonical runtime identities they
produce.

Before lowering dependent variables or bodies, the assembler establishes
project-wide identities for derived data types, function blocks, classes,
interfaces, functions, programs, and visible constant declarations.
Derived-type definitions and constant expressions are resolved in dependency
order rather than source-file order. Consequently:

- a type alias, array element, structure field, variable, or POU member may
  name a compatible declaration in a later source unit;
- a function, function block, class, interface, or program may be referenced
  from an earlier source unit;
- a root or namespace global initializer may use a visible constant declared
  in a later source unit, and a POU/configuration/resource initializer may use
  a visible constant declared later in its accepted declaration scope;
- a configuration may occur before the program and task-related declarations
  it binds; and
- the permitted self-reference remains a structure field whose type is
  `REF_TO` the owning structure. A by-value or alias dependency cycle is
  rejected instead of leaving a reserved placeholder in the returned runtime.

Namespace membership, explicit qualified names, and unqualified `USING`
resolution follow IEC 61131-3 Ed.3 section 6.9.4 and Tables 64-66 across the
complete source set. An explicit qualified reference is resolved exactly as
written and never falls back to a same-leaf declaration imported through
`USING`. Multiple imported matches are ambiguous and fail closed.

The derived-type and POU-type identities occupy the one case-insensitive
project type namespace defined in section 7.8. Duplicate identities are
rejected across source units as well as within one unit, including case-only
duplicates and data-type/POU-type collisions. Function and program identities
are likewise compared case-insensitively across the project. Exactly one
`CONFIGURATION` may survive whole-project assembly; declarations in separate
files do not create separate runtime configurations.

Runtime configuration lowering starts only after the complete program and
constant declaration sets are available. Qualified program types and visible
constant references therefore resolve independently of whether the provider
source precedes or follows the consumer source. Constant lookup remains
case-insensitive and scope-correct; project discovery does not leak a
POU-local, namespace-local, or resource-local constant into an unrelated
scope. Unknown program types, unknown explicit qualified names, ambiguous
imports, duplicate declarations, and type or constant dependency cycles
return a compile error without registering a partial program, type,
configuration, or global.

Within an accepted declaration, member, parameter, variable, enum-value, and
dimension order remains source order. Where a diagnostic reports multiple
independent missing declarations, the original `CompileSession` order is the
stable reporting order. With labeled errors enabled, an explicit `SourceFile`
path is the diagnostic label; otherwise the stable zero-based virtual source
index is used. Paths also feed debug source identity, but they do not alter
semantic lookup or executable declaration identity.

IEC 61131-3 defines the namespace and configuration semantics but does not
define repository files or `CompileSession` ordering. Project-wide discovery,
dependency ordering, path labels, and order-independent runtime assembly are
truST product behavior, not IEC deviations.

### 8. Standard Library

#### 8.1 Standard Functions

##### Numeric Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| ABS | ANY_NUM → ANY_NUM | Absolute value |
| SQRT | ANY_REAL → ANY_REAL | Square root |
| SIN | ANY_REAL → ANY_REAL | Sine (radians) |
| COS | ANY_REAL → ANY_REAL | Cosine (radians) |
| TAN | ANY_REAL → ANY_REAL | Tangent (radians) |
| ASIN | ANY_REAL → ANY_REAL | Arc sine |
| ACOS | ANY_REAL → ANY_REAL | Arc cosine |
| ATAN | ANY_REAL → ANY_REAL | Arc tangent |
| LOG | ANY_REAL → ANY_REAL | Base-10 logarithm |
| LN | ANY_REAL → ANY_REAL | Natural logarithm |
| EXP | ANY_REAL → ANY_REAL | e^x |
| EXPT | (ANY_REAL, ANY_NUM) → ANY_REAL | x^y |

##### String Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| LEN | STRING → INT | String length |
| CONCAT | (STRING, ...) → STRING | Concatenate strings |
| LEFT | (STRING, INT) → STRING | Left substring |
| RIGHT | (STRING, INT) → STRING | Right substring |
| MID | (STRING, INT, INT) → STRING | Middle substring |
| FIND | (STRING, STRING) → INT | Find position |
| REPLACE | (STRING, STRING, INT, INT) → STRING | Replace substring |

##### Selection Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| SEL | (BOOL, T, T) → T | Select based on condition |
| MAX | (T, T, ...) → T | Maximum value |
| MIN | (T, T, ...) → T | Minimum value |
| LIMIT | (T, T, T) → T | Clamp to range |
| MUX | (INT, T, ...) → T | Multiplexer |

#### 8.2 Standard Function Blocks

##### Timers

| FB | Inputs | Outputs | Description |
|----|--------|---------|-------------|
| TON | IN: BOOL, PT: TIME | Q: BOOL, ET: TIME | On-delay timer |
| TOF | IN: BOOL, PT: TIME | Q: BOOL, ET: TIME | Off-delay timer |
| TP | IN: BOOL, PT: TIME | Q: BOOL, ET: TIME | Pulse timer |

**TON Behavior**:
```
      IN: _____|‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾|_____
      Q:  _____|     |‾‾‾‾‾‾‾‾‾‾‾|_____
      ET: _____|////|‾‾‾‾‾‾‾‾‾‾‾|_____
             |<-PT->|
```

##### Counters

| FB | Inputs | Outputs | Description |
|----|--------|---------|-------------|
| CTU | CU: BOOL, R: BOOL, PV: INT | Q: BOOL, CV: INT | Up counter |
| CTD | CD: BOOL, LD: BOOL, PV: INT | Q: BOOL, CV: INT | Down counter |
| CTUD | CU, CD, R, LD: BOOL, PV: INT | QU, QD: BOOL, CV: INT | Up/down counter |

##### Edge Detection

| FB | Inputs | Outputs | Description |
|----|--------|---------|-------------|
| R_TRIG | CLK: BOOL | Q: BOOL | Rising edge (TRUE for one cycle) |
| F_TRIG | CLK: BOOL | Q: BOOL | Falling edge (TRUE for one cycle) |

##### Bistable

| FB | Inputs | Outputs | Description |
|----|--------|---------|-------------|
| SR | S1: BOOL, R: BOOL | Q1: BOOL | Set-dominant latch |
| RS | S: BOOL, R1: BOOL | Q1: BOOL | Reset-dominant latch |

#### 8.3 Type Conversion Functions

Pattern: `<SOURCE>_TO_<TARGET>`

Examples:
- `INT_TO_REAL`, `REAL_TO_INT`
- `DINT_TO_STRING`, `STRING_TO_DINT`
- `TIME_TO_LTIME`, `LTIME_TO_TIME`

Truncation functions for reals:
- `TRUNC`: Truncate toward zero
- `REAL_TRUNC_DINT`: Combined conversion

### 9. I/O Interface

#### 9.1 Direct Address Mapping

```rust
/// I/O interface for direct addresses (%I, %Q, %M).
pub struct IoInterface {
    /// Input area (%I)
    inputs: IoArea,
    /// Output area (%Q)
    outputs: IoArea,
    /// Memory area (%M)
    memory: IoArea,
}

/// A single I/O area.
#[derive(Debug, Default)]
pub struct IoArea {
    /// Byte-addressable storage
    bytes: Vec<u8>,
}
```

#### 9.2 Direct Address Format

```rust
/// Parsed direct address (%IX0.1, %QW4, etc.).
#[derive(Debug, Clone)]
pub struct DirectAddress {
    pub area: AddressArea,
    pub size: AddressSize,
    pub byte_offset: usize,
    pub bit_offset: Option<u8>,
}

#[derive(Debug, Clone, Copy)]
pub enum AddressArea {
    Input,  // I
    Output, // Q
    Memory, // M
}

#[derive(Debug, Clone, Copy)]
pub enum AddressSize {
    Bit,    // X or none
    Byte,   // B
    Word,   // W
    DWord,  // D
    LWord,  // L
}
```

The runtime parser implements IEC 61131-3 Ed.3 section 6.5.5.1 and Table 16 as
`%` followed by area `I`, `Q`, or `M`, an optional size `X`, `B`, `W`, `D`, or
`L`, and one or more dot-separated unsigned address components. Components use
ASCII decimal digits without a sign and must fit in `u32`. For a bit address
with at least two components, the final component is the bit index and must be
in `0..=7`; the preceding components form the location path. A one-component
implicit- or `X`-sized bit address uses that component as its location and
defaults the bit index to zero. For every concrete address, `IoAddress::byte`
mirrors the first location-path component. For a non-bit address, every
component remains in `IoAddress::path` and `IoAddress::bit` is zero. The parser
does not impose an additional fixed
hierarchy-level limit beyond those component and input-size bounds. This
location correspondence, returned representation, and hierarchy limit are
truST's implementer-specific choices under sections 6.5.5.1 and 6.5.5.2.

Partly specified addresses follow section 6.5.5.4: after surrounding
whitespace is removed, only `%I*`, `%Q*`, and `%M*` are wildcard forms. The
asterisk replaces both the size prefix and all unsigned address components, so
sized wildcards, trailing wildcard text, and interior whitespace are invalid.
An accepted wildcard sets the matching area and `wildcard = true`; because it
does not name a concrete location, its representation uses `IoSize::Bit`, zero
byte and bit fields, and an empty path as an unresolved sentinel. Declaration
and `VAR_CONFIG` resolution of that sentinel is a separate downstream contract.
Malformed input returns `RuntimeError::InvalidIoAddress` without producing a
partial address. This parsing contract is IEC-conforming product behavior, not
an IEC decision or deviation.

#### 9.3 Address Examples

| Address | Area | Size | Offset |
|---------|------|------|--------|
| `%IX1.2` | Input | Bit | Byte 1, Bit 2 |
| `%IW4` | Input | Word | Byte 4-5 |
| `%QD10` | Output | DWord | Byte 10-13 |
| `%MX0.7` | Memory | Bit | Byte 0, Bit 7 |
| `%MB12` | Memory | Byte | Byte 12 |
| `%MW50` | Memory | Word | Byte 50-51 |
| `%MD0` | Memory | DWord | Byte 0-3 |
| `%ML8` | Memory | LWord | Byte 8-15 |

#### 9.4 I/O Provider Interface

```rust
/// Trait for external I/O providers (for testing or simulation).
pub trait IoProvider: Send + Sync {
    /// Called at the start of each cycle to update inputs.
    fn read_inputs(&self, io: &mut IoInterface);

    /// Called at the end of each cycle after outputs are written.
    fn write_outputs(&self, io: &IoInterface);
}

/// Default provider that does nothing (for unit testing).
pub struct NullIoProvider;
```

### 10. Error Handling

#### 10.1 Runtime Errors

```rust
#[derive(Debug, Clone, thiserror::Error)]
pub enum RuntimeError {
    // Name resolution
    #[error("undefined variable '{0}'")]
    UndefinedVariable(SmolStr),

    #[error("undefined function '{0}'")]
    UndefinedFunction(SmolStr),

    #[error("undefined program '{0}'")]
    UndefinedProgram(SmolStr),

    #[error("'{0}' is not callable")]
    NotCallable(SmolStr),

    // Type errors
    #[error("type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("cannot coerce {from} to {to}")]
    CoercionFailed { from: String, to: String },

    // Arithmetic errors
    #[error("division by zero")]
    DivisionByZero,

    #[error("integer overflow")]
    IntegerOverflow,

    #[error("domain error: {0}")]
    DomainError(&'static str),

    // Date/time errors
    #[error("date/time value out of range")]
    DateTimeOutOfRange,

    // Array/reference errors
    #[error("array index {index} out of bounds [{lower}..{upper}]")]
    IndexOutOfBounds { index: i64, lower: i64, upper: i64 },

    #[error("null reference dereference")]
    NullReferenceDereference,

    // Control flow errors
    #[error("FOR loop step cannot be zero")]
    ForStepZero,

    #[error("infinite loop detected (cycle limit exceeded)")]
    InfiniteLoop,

    // I/O errors
    #[error("direct address out of range")]
    AddressOutOfRange,

    // Subrange errors
    #[error("value {value} out of subrange [{lower}..{upper}]")]
    SubrangeViolation { value: i64, lower: i64, upper: i64 },
}
```

#### 10.2 Stable Machine Error Identifiers

`RuntimeError::stable_code()` returns a `StableErrorCode` whose
lower-snake-case string is the machine contract. Existing `RuntimeError`
variants map to `runtime_<variant-name>` in lower snake case, including
`runtime_type_mismatch`, `runtime_division_by_zero`,
`runtime_modulo_by_zero`, `runtime_overflow`,
`runtime_index_out_of_bounds`, `runtime_subrange_violation`,
`runtime_null_reference`, `runtime_for_step_zero`,
`runtime_condition_not_bool`, `runtime_watchdog_timeout`, and
`runtime_execution_timeout`. Bytecode and VM structural errors retain the
more specific `bytecode_*` or `vm_*` code specified in
`docs/specs/12-bytecode.md` rather than collapsing to
`runtime_invalid_bytecode`.

The mapping is exhaustive for the committed `RuntimeError` enum. Adding or
renaming a variant requires an explicit stable-code mapping and review.
Human-readable `Display` text is diagnostic context and may become more
specific without changing the code. Machine consumers and verification cases
must compare the code, not message substrings.

Conversion boundaries preserve the committed error identity:

- converting a `BytecodeError` produces `RuntimeError::Bytecode`, preserving
  the source bytecode stable code and its rendered diagnostic detail;
- converting either `DateTimeError` value produces
  `RuntimeError::DateTimeRange` with the original value and
  `runtime_date_time_range`;
- converting `DateTimeCalcError::InvalidDate`, `InvalidResolution`, or
  `Overflow` normalizes to `RuntimeError::Overflow` and
  `runtime_overflow`.

The HMI admission boundary uses the same runtime codes for type and subrange
failures. It additionally reports `runtime_string_capacity_exceeded` for a
rejected bounded-string request and `runtime_non_finite_value` for a rejected
NaN, infinity, or width-overflowing floating-point request. A failed control
response carries the stable identifier in `error_code` and retains the
diagnostic message in `error`.

These identifiers are truST product API choices. They do not interpret,
extend, or deviate from IEC 61131-3 semantics.

#### 10.3 Error Configuration

```rust
/// Configuration for error handling behavior.
#[derive(Debug, Clone)]
pub struct ErrorConfig {
    /// Continue execution after non-fatal errors
    pub continue_on_error: bool,

    /// Maximum errors before halting
    pub max_errors: usize,

    /// Behavior for division by zero
    pub div_zero_behavior: DivZeroBehavior,

    /// Behavior for integer overflow
    pub overflow_behavior: OverflowBehavior,
}

#[derive(Debug, Clone, Copy)]
pub enum DivZeroBehavior {
    Error,      // Raise error
    MaxValue,   // Return type's max value
    Zero,       // Return zero
}

#[derive(Debug, Clone, Copy)]
pub enum OverflowBehavior {
    Error,      // Raise error
    Saturate,   // Clamp to min/max
    Wrap,       // Wrap around
}
```

### 11. Testing API

#### 11.1 Test Harness

##### Simulation clock overflow

Explicit simulation-time advancement saturates at the signed nanosecond bounds.
It must not panic, wrap, or make the test clock move backward. This policy is a
deterministic harness contract and does not alter production monotonic-clock
behavior.

```rust
/// Test harness for PLC code unit testing.
pub struct TestHarness {
    runtime: Runtime,
}

impl TestHarness {
    /// Creates a new test harness from source code.
    pub fn from_source(source: &str) -> Result<Self, CompileError>;

    /// Sets an input value.
    pub fn set_input(&mut self, name: &str, value: impl Into<Value>);

    /// Gets an output value.
    pub fn get_output(&self, name: &str) -> Option<Value>;

    /// Sets a direct input address.
    pub fn set_direct_input(&mut self, address: &str, value: impl Into<Value>);

    /// Gets a direct output address.
    pub fn get_direct_output(&self, address: &str) -> Value;

    /// Runs one cycle.
    pub fn cycle(&mut self) -> CycleResult;

    /// Runs multiple cycles.
    pub fn run_cycles(&mut self, count: u32) -> Vec<CycleResult>;

    /// Runs until a condition is met.
    pub fn run_until<F>(&mut self, condition: F) -> Vec<CycleResult>
    where
        F: Fn(&Runtime) -> bool;

    /// Advances simulation time, saturating at the signed Duration bounds.
    pub fn advance_time(&mut self, duration: Duration);

    /// Gets the current simulation time.
    pub fn current_time(&self) -> Duration;

    /// Gets the cycle count.
    pub fn cycle_count(&self) -> u64;

    /// Asserts that a variable has a specific value.
    pub fn assert_eq(&self, name: &str, expected: impl Into<Value>);
}
```

`from_source` constructs a fresh initialized runtime before executing any
cycle; its simulated time and completed-cycle count both begin at zero.
Symbolic input/output and explicitly bound direct-address input/output values
cross the same harness cycle boundary. `run_cycles(n)` executes exactly `n`
cycles without advancing simulated time by itself. `run_until` evaluates its
predicate against the current runtime before the first cycle, returns only the
cycles it executed, and leaves the completed-cycle count equal to the total
cycles run by that harness. `run_until_max` uses the same pre-cycle predicate
order and, after exhausting the reviewed bound without a match, panics with
`run_until exceeded <N> cycles`.

These rules specify the in-process Rust testing API. They do not establish the
JSON-line transport contract, physical I/O behavior, or wall-clock timing.

`SourceFile::new` records virtual source text without a path;
`SourceFile::with_path` preserves the caller's exact path label.
`CompileSession::from_source` owns one source and leaves diagnostics unlabeled
by default. `from_sources` owns all sources in caller order and enables
per-source labels when more than one source is present; an explicit
`label_errors` selection overrides that default. Runtime, bytecode-module, and
encoded-byte builds first complete source instrumentation and then compile the
same ordered source set. A source/path helper requires equal list lengths.
Parse, semantic, lowering, instrumentation, and bytecode-encoding failures are
returned as `CompileError`, never as a partial runtime or module.

`TestHarness::from_source` and `from_sources` build the host runtime and apply a
runtime-aligned bytecode module before publishing the harness. Read-only,
mutable, and consuming runtime accessors expose that same runtime. Every call
to `cycle` increments the completed-cycle count exactly once, even when the
cycle returns a runtime error, and reports the post-cycle virtual time.
`run_cycles(0)` is passive. Direct-address parsing failures retain their runtime
error at direct I/O methods and are translated to a boundary error at
`bind_direct`.

Reload is transactional. It compiles and bytecode-initializes a replacement,
applies the prior retain snapshot, preserves debug-control ownership, virtual
time, and completed-cycle count, and only then replaces the live harness.
Compilation or retain migration failure leaves the entire old harness usable.
Single- and multi-source reloads have the same state rule.

##### 11.1.1 Debug expression and assignment-target parsing

Debug input is trimmed and may contain one trailing semicolon. Empty or
syntactically invalid input is a compile error. Watch expressions accept
side-effect-free syntax and calls only to the reviewed pure standard-function,
conversion, and time-split families. User calls, function-block/method calls,
and dynamically targeted calls are rejected before lowering. Assignment
targets accept names, fields, indices, and dereferences that lower to an
`LValue`; a call anywhere in the target is rejected. Caller-provided `USING`
names and the mutable type registry are the only external lowering context.

The in-process symbolic boundary resolves a unique program variable by its
unqualified name and resolves indexed array elements and dotted
function-block fields beneath that variable. If more than one program supplies
the same unqualified variable name, the read returns `AmbiguousName` with the
complete reviewed candidate set. A missing name returns `UnresolvedName`; an
out-of-range reviewed array path returns the stable `wrong_kind` boundary code.
Neither case is converted to a null-like value. A declared `REF_TO` value that
is not initialized is different: it is a successful read of
`Value::Reference(None)`.

`HarnessAutomation::set_input` uses the same symbolic authority. A misspelled
target returns the stable `unresolved_name` code, creates no fallback global,
and leaves the declared target readable in the following snapshot while the
misspelled watch remains an individual unresolved-name result.
`TestHarness::bind_direct` rejects a misspelled declaration with the stable
`undeclared_binding` code and does not create a binding.

##### 11.1.2 Symbolic boundary resolution

The in-process boundary is fail-closed. Reads and writes first resolve an exact
declared global name, then an exact variable name that occurs in exactly one
bound program instance. A global therefore takes precedence over same-named
program variables. Multiple matching programs produce `ambiguous_name` with
candidate paths in program registration order. A missing simple read or write
produces `unresolved_name`; a missing simple direct-binding target produces
`undeclared_binding`. Failed writes and binds do not create fallback globals,
aliases, or I/O bindings.

Composite reads and writes accept only assignment-path syntax rooted at a
declared global or uniquely resolved program variable: structure and instance
fields, constant array indices, partial bit/byte/word/dword selections, and
reference dereference. Arbitrary expressions, calls, and rootless paths are
`unsupported_path_syntax`. Out-of-range indices, null dereferences, and
incompatible runtime values are `wrong_kind`; unresolved fields remain
`unresolved_name`. Boundary writes preserve the supplied runtime `Value`
identity and do not perform an implicit numeric conversion.

Direct binding accepts only a declared scalar simple name. A global name is
bound by name; a unique program variable is bound by its storage reference.
Composite paths are rejected as unsupported, ambiguous simple names retain
their candidate list, and an invalid direct-address string is translated to
`internal_failure` at the harness boundary.

`BoundaryError` exposes a stable snake-case code, an optional affected path,
and candidates only for ambiguity. Its display text contains the complete
human-readable diagnosis without changing those machine fields. Runtime
undefined-variable, undefined-program, undefined-field, type-mismatch,
null-reference, and array-bound failures have explicit boundary mappings;
other runtime failures are redacted to `internal_failure`.

Each `BoundaryEntry` is internally consistent: `ok(value)` has status `Ok`, one
value, no error, and `is_ok() == true`; `error(error)` has status `Error`, no
value, one error, and `is_ok() == false`.

##### 11.1.3 Configuration assembly and access binding

Harness compilation assembles exactly one runtime configuration from the
complete ordered source set. Without a `CONFIGURATION`, at least one `PROGRAM`
declaration is required and every declared program is instantiated once under
its declared name as an untasked background program. With a
`CONFIGURATION`, every declared program type must be bound by a configured
instance or by an explicitly requested test-builder extra instance. Unbound
types are a compile error that names every missing declaration in declaration
order. Extra-instance names are case-insensitive for matching and
deduplication, do not replace configured instances, and must name an existing
program declaration.

Only one `CONFIGURATION` may be assembled. A `RESOURCE` supplies the runtime
resource identity. Program instance names, task names, and program type
resolution are ASCII case-insensitive; caller spelling remains the stored
display name. Duplicate instance names, a second instance of one program type,
unknown program types, unknown tasks, and unbound program declarations are
compile errors. A namespaced program type is selected with its qualified name;
`USING` is not a configuration-body declaration.

One harness build materializes exactly one runtime resource. With no explicit
`RESOURCE`, configuration-level tasks and programs belong to the implicit
resource named `RESOURCE`. With one explicit resource, its exact instance name
becomes runtime identity; the implementer-specific identifier after `ON` does
not replace it or select the host backend. Multiple explicit resources, or an
explicit resource mixed with configuration-level task/program declarations,
fail before global allocation, program registration, or task attachment.

`TASK` initialization requires one unsigned integer literal `PRIORITY` in the
runtime `u32` range and accepts at most one non-negative `TIME` literal
`INTERVAL` and one named `SINGLE` trigger. `SINGLE` must resolve at assembly
time to a visible `BOOL` global; literals, missing names, and non-Boolean
variables are rejected. Unknown or repeated task-initializer fields are
rejected. The omitted interval and trigger use zero/no-trigger defaults. Task
names are case-insensitively unique in their configuration/resource scope and
accepted tasks retain declaration order and source spelling. A configured
program is attached only to its named task; a program without `WITH` remains a
background program. Function-block task bindings must resolve beneath the
configured program instance to an actual function-block instance and cannot
use a direct address.

Program-level `RETAIN` or `NON_RETAIN` configuration qualifiers apply to
variables whose declaration policy is unspecified. They do not replace an
explicit variable-level policy. Conflicting program-level policies for the
same program type are rejected before instances are registered.

All root, program-declared global, configuration, and resource globals are
allocated before their accepted literal or constant-expression initializers
are materialized. Constant-expression values are resolved from the accepted
scope dependency graph, not by reading the temporary default value of a
later-declared global. Function-block globals are created as instances and
accept reviewed aggregate initialization of externally initializable members.
Class globals are created as instances but reject aggregate initializers at
the semantic boundary. Interface globals begin as null references. Each global
records its type, initial value or instance recipe, and retain policy for
restart.

The single-resource runtime stores root/vendor GVL, program-global,
configuration-global, and resource-global declarations in one effective
case-insensitive global namespace while preserving accepted spelling.
Cross-host duplicates are rejected before allocation. A resource
`VAR_EXTERNAL` links to the matching global without allocating or registering
another global. Configuration/resource `VAR_GLOBAL CONSTANT` values may feed
any accepted initializer that can see them, independent of declaration order;
configuration constants are visible to the contained resource, while resource
constants do not escape that resource. Sections other than `VAR_GLOBAL`,
resource `VAR_EXTERNAL`, `VAR_ACCESS`, and `VAR_CONFIG` are rejected in these
scopes rather than being reinterpreted as globals.

Access paths are resolved case-insensitively from a global root or from a
field that is unique across configured program instances, followed by
structure fields, instance fields, constant array indices, and at most one
terminal bit/byte/word/dword partial selection. Missing roots, ambiguous
unqualified program fields, invalid indices, invalid fields, and malformed
partial paths are compile errors. A `VAR_ACCESS` variable path registers a
symbolic read/write alias. A direct `VAR_ACCESS` declaration instead creates a
direct-address global and must not create a symbolic alias.

A wildcard direct declaration remains unresolved until one matching
`VAR_CONFIG ... AT` entry supplies a fully specified address in the same I/O
area. The runtime wildcard guard reports its outstanding names sorted and
deduplicated; the semantic boundary may reject an individual unmapped
declaration earlier. `VAR_CONFIG` targets must be variable access paths:
direct-address and partial-selection targets are rejected. `VAR_CONFIG AT`
also rejects wildcard addresses and area mismatches. Its optional initializer
is evaluated after allocation and written to the resolved variable.
Successful bindings resize each process-image area to include the complete byte
span of its highest scalar binding; sparse addresses are valid and do not
alias another area.

Each symbolic `VAR_ACCESS` binding also retains its declared access direction.
An omitted direction is `READ_ONLY`. Runtime reads remain available for both
directions, but a write through a read-only binding fails without changing the
whole value or a partial projection. `READ_WRITE` does not override IEC target
eligibility: constants, temporary/external/in-out variables, and externally
connected function-block inputs cannot become writable through the access map.
The declared access type must equal the resolved target type before a binding
is registered.

Instance-specific `VAR_CONFIG` initialization accepts persistent program
variables, nested function-block members, and structure components. It rejects
type mismatches and constant, temporary, external, or in-out targets before
evaluation or mutation. An accepted initializer is applied after allocation
to the exact configured instance and overrides its declaration/type default.

#### 11.2 Harness automation JSON-line contract

`trust-harness` exposes the deterministic test harness as newline-delimited
JSON. Each non-empty input line produces exactly one response line. Malformed
JSON and command failures are isolated to that request; the process continues
with the same session. Protocol version 2 is the default, version 1 preserves
the legacy watch-value shape, and every response includes the selected version.

Source selection and replacement are transactional:

- `sources` is authoritative when both `source` and `sources` are present;
- an explicitly empty `sources` list is a semantic `invalid_argument` error,
  while omitting both source fields is a structural `invalid_request` error;
- successful `load` builds a fresh harness and completes its initial cycle
  before replacing the session;
- failed compilation or initial execution leaves the previously loaded session
  unchanged, or leaves the session unloaded when there was no previous load;
- `reload` requires a loaded session and preserves supported retained values,
  virtual time, and cycle count; compilation or retained-state migration
  failure leaves the complete old harness unchanged.

Cycle/time commands use deterministic, explicit boundaries. `cycle` defaults to
one cycle, zero time advancement, and no watched values. Each requested time
increment occurs immediately before its corresponding cycle. `advance_time`
executes no cycle; `duration_ms` is authoritative over its legacy `dt_ms` alias.
`snapshot` executes no cycle and advances no time. `run_until` checks the
current value before its first cycle, so an already-matching value succeeds with
`cycles_ran = 0`; otherwise it executes at most `max_cycles` and reports
`run_until_timeout` after that budget. Prior completed cycles and time advances
remain committed when a later cycle or bounded run reports an error.

Missing command fields, unsupported commands, and unsupported restart modes are
structural `invalid_request` errors. Typed-value, source-list, and non-negative
duration validation failures are `invalid_argument`. Compile, runtime,
runtime-cycle, timeout, and boundary failures retain their stable protocol error
kinds and data. Version 2 represents each watch failure independently; version
1 promotes a watch failure to the complete request because its legacy value map
cannot encode per-entry errors.

The detailed wire examples and stable error vocabulary are in
`docs/guides/TRUST_HARNESS_PROTOCOL.md`.

##### 11.2.1 Automation state and typed-value codec

`HarnessAutomation` is initially unloaded. Every operation that requires a
runtime, including reload, cycle, symbolic or direct I/O, time advancement,
restart, snapshot, and bounded execution, returns `not_loaded` until a load
succeeds. An empty source list is invalid. A load publishes its replacement
harness only after compilation and the mandatory initial cycle both succeed,
so a failed replacement cannot destroy an older usable session.

The automation clock is represented by the runtime's signed nanosecond
`Duration`. Millisecond inputs are therefore accepted only from zero through
`floor(i64::MAX / 1_000_000)`. This limit applies equally to `cycle`,
`advance_time`, and `run_until`; a larger input is `invalid_argument` and must
not panic or mutate cycle count, time, or values. A zero-count cycle performs
no time advance. Snapshot preserves cycle and time and reports every distinct
requested name in deterministic lexical order; an unresolved name is an
entry-local boundary error.

`run_until` reads the target before consuming its budget. A current match
returns zero cycles. Otherwise each iteration advances the requested virtual
time, runs one cycle, and then rechecks. Timeout occurs after exactly
`max_cycles`; those completed cycles and time advances remain committed. A
cycle failure also retains work committed by earlier iterations.

The typed JSON codec is closed over BOOL; signed and unsigned integers; REAL
and LREAL; bit strings; TIME, LTIME, DATE, LDATE, TOD, LTOD, DT, and LDT;
STRING, WSTRING, CHAR, and WCHAR; ARRAY, STRUCT, ENUM, and NULL. Type names are
ASCII case-insensitive. Required payload fields have exact JSON kinds, integer
widths are checked, REAL rejects finite values outside the `f32` domain, and
CHAR requires exactly one ASCII scalar while WCHAR requires exactly one scalar
representable by `u16`. Unsupported type names and malformed typed objects are
`invalid_argument`.

Untyped JSON booleans, strings, nulls, arrays, and numbers decode
deterministically. Integer numbers select the smallest signed IEC container
through `LINT`; JSON integers above `i64::MAX` select the smallest unsigned
container through `ULINT`; fractional numbers select `LREAL`. Non-empty
untyped arrays use a zero-based one-dimensional bound and recursively decode
their elements; an empty untyped array is invalid because it supplies no IEC
array bound. Typed arrays require `[lower, upper]` integer pairs and an element
count that matches their complete declared shape. Typed structures require a
type name and object-valued fields, and typed enumerations require a type name,
variant, and signed numeric value.

Encoding emits the canonical uppercase type tag and the canonical field for
each runtime value. Every codec-supported finite value whose character payload
is a valid Unicode scalar must survive encode-then-decode without changing its
runtime value, dimensions, structure field values, or enumeration identity.
References and instance handles are observable-only encodings and are not
accepted as incoming typed values.

#### 11.3 Example Tests

```rust
#[test]
fn test_counter() {
    let source = r#"
        PROGRAM TestCounter
        VAR
            count: INT := 0;
            increment: BOOL;
        END_VAR

        IF increment THEN
            count := count + 1;
        END_IF;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();

    // Initial state
    harness.assert_eq("count", 0i16);

    // Cycle without increment
    harness.set_input("increment", false);
    harness.cycle();
    harness.assert_eq("count", 0i16);

    // Cycle with increment
    harness.set_input("increment", true);
    harness.cycle();
    harness.assert_eq("count", 1i16);

    // Multiple increments
    harness.run_cycles(5);
    harness.assert_eq("count", 6i16);
}

#[test]
fn test_timer() {
    let source = r#"
        PROGRAM TestTimer
        VAR
            start: BOOL;
            delay: TON;
            done: BOOL;
        END_VAR

        delay(IN := start, PT := T#100ms);
        done := delay.Q;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();

    // Start timer
    harness.set_input("start", true);
    harness.cycle();
    harness.assert_eq("done", false);

    // Advance time less than PT
    harness.advance_time(Duration::from_millis(50));
    harness.cycle();
    harness.assert_eq("done", false);

    // Advance time past PT
    harness.advance_time(Duration::from_millis(60));
    harness.cycle();
    harness.assert_eq("done", true);
}
```

### 12. Implementation Phases

#### Phase 1: Core Runtime (legacy interpreter-first milestone)

- Value enum with elementary types
- Variable storage (globals, local frames)
- Expression evaluation (arithmetic, comparison, logical with short-circuit)
- Control flow (IF, FOR, WHILE, CASE, REPEAT)
- Assignment statements
- Basic test harness

#### Phase 2: POU Support

- FUNCTION implementation
- FUNCTION_BLOCK instances and state
- PROGRAM execution with cycles
- VAR_INPUT/VAR_OUTPUT/VAR_IN_OUT binding

#### Phase 3: Standard Library

- Numeric functions (ABS, SQRT, SIN, etc.)
- String functions (LEN, CONCAT, etc.)
- Type conversions
- Timer FBs (TON, TOF, TP)
- Counter FBs (CTU, CTD)
- Edge detection (R_TRIG, F_TRIG)

#### Phase 4: Advanced Features (Implemented)

- CLASS/INTERFACE/METHOD/PROPERTY support
- Inheritance (EXTENDS) + interface conformance (IMPLEMENTS)
- REFERENCE types (`REF_TO`) plus IEC dynamic assignment-attempt semantics and
  the separate typed `POINTER TO` product extension
- `VAR_STAT` documented vendor-extension storage semantics
- Direct address I/O (%I, %Q, %M)

#### Phase 5: Debugging (Implemented)

- Execution tracing
- Debugger interface (step, breakpoints)
- Coverage tracking (future)

### 13. Verification

#### 13.1 Unit Tests

Each module has inline tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_default() { ... }

    #[test]
    fn test_arithmetic_ops() { ... }
}
```

#### 13.2 Integration Tests

`tests/` directory with complete ST programs:

- Control flow tests
- Expression evaluation tests
- POU interaction tests
- Standard library tests

#### 13.3 Snapshot Tests

Use `insta` for complex outputs:

```rust
#[test]
fn test_execution_trace() {
    let trace = run_program("...");
    insta::assert_debug_snapshot!(trace);
}
```

#### 13.4 Compliance Tests

Test against IEC 61131-3 examples from specification.
