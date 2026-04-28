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

IEC `REF_TO` and the non-IEC `POINTER TO` extension share the same runtime
reference model. `POINTER TO` supports `ADR(...)`, dereference (`^`), `NULL`,
and `?=` as a typed vendor extension; see `docs/IEC_DEVIATIONS.md` (DEV-018).
`Value::Null` remains the runtime sentinel for `NULL` literals and void-like
results, while uninitialized `REF_TO` / `POINTER TO` storage defaults to
`Value::Reference(None)` (IEC 61131-3 Ed.3 §6.4.4.10.2).

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

#### 2.3 Time/Date Representation

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

#### 2.4 Default Values

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

#### 4.3 Task Scheduling (Periodic + Event)

Tasks are scheduled per IEC 61131-3 Ed.3 §6.8.2:

- **Event trigger (SINGLE)**: A task is scheduled on each rising edge of its `SINGLE` Boolean input.
- **Periodic trigger (INTERVAL)**: If `INTERVAL` is non-zero and `SINGLE` is FALSE, the task is scheduled
  periodically at the specified interval. If `INTERVAL` is zero (default), no periodic scheduling occurs.
- **Priority**: Lower numeric priority values run first (0 = highest).

trust-runtime uses **non-preemptive, deterministic scheduling**: due tasks are executed in priority order,
with declaration order as a tie-breaker for equal priorities. This is permitted by IEC 61131-3 (§6.8.2(c))
and makes execution reproducible for tests.

Event tasks are modeled by tracking the previous value of the SINGLE variable:

```
event_due = single_prev == FALSE && single_now == TRUE
periodic_due = interval > 0 && single_now == FALSE &&
               (current_time - last_run) >= interval
```

The SINGLE input must resolve to a BOOL variable; if it is missing or non-BOOL, task execution
fails with a runtime error.

Programs with no explicit task association are scheduled at the lowest priority. In this cycle-based
runtime, they execute once per `execute_cycle` (interpreting that call as the smallest scheduling
granularity). This preserves determinism while aligning with IEC's "reschedule after completion"
rule for background programs.

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
| RETURN | `ReturnStmt` | `RETURN;` or `RETURN expr;` |
| EXIT | `ExitStmt` | `EXIT;` (break innermost loop) |
| CONTINUE | `ContinueStmt` | `CONTINUE;` (next iteration) |
| Expression | `ExprStmt` | Function/FB calls as statements |
| Empty | `EmptyStmt` | `;` (no-op) |

#### 5.3 Control Flow Rules

**FOR Loop**:
- Control variable, initial, final, increment must be same integer type
- Control variable must NOT be modified in loop body
- Termination test at start: `var > final` (positive step) or `var < final` (negative step)
- Step of zero is a runtime error

**WHILE/REPEAT**:
- Condition must evaluate to BOOL
- WHILE tests before iteration; REPEAT tests after (executes at least once)

**CASE**:
- Selector must be elementary type
- Case labels must match selector type
- Duplicate/overlapping labels are errors
- ELSE branch optional

**EXIT/CONTINUE**:
- Must be inside a loop (FOR, WHILE, REPEAT)
- Affects innermost enclosing loop only

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
- Applying `REF` to temporary variables (VAR_TEMP or function-local temporaries) is not permitted.

**SIZEOF operator** (vendor extension, see `DEV-016`):
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

Per IEC 61131-3, short-circuit evaluation is implementer-specific. This implementation uses short-circuit:

- `AND`: Stop on first FALSE
- `OR`: Stop on first TRUE

This matches common programming languages and prevents unnecessary side effects from function calls in boolean expressions.

#### 6.4 Type Promotion

When operands have different types, implicit widening applies:

```
SINT → INT → DINT → LINT
USINT → UINT → UDINT → ULINT
REAL → LREAL
```

Narrowing conversions require explicit type conversion functions (e.g., `DINT_TO_INT`).

### 7. POU Execution

#### 7.1 FUNCTION

- **Stateless**: Variables re-initialized each call
- **Return value**: Via function name assignment or RETURN statement
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

#### 7.5 EN/ENO Mechanism

Standard enable/enable-out mechanism:

- `EN` (input): If FALSE, POU not executed, ENO set FALSE
- `ENO` (output): TRUE if execution succeeded

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

#### 10.2 Error Configuration

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

    /// Advances simulation time.
    pub fn advance_time(&mut self, duration: Duration);

    /// Gets the current simulation time.
    pub fn current_time(&self) -> Duration;

    /// Gets the cycle count.
    pub fn cycle_count(&self) -> u64;

    /// Asserts that a variable has a specific value.
    pub fn assert_eq(&self, name: &str, expected: impl Into<Value>);
}
```

#### 11.2 Example Tests

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
- REFERENCE types (REF_TO) + assignment attempt semantics (see `docs/IEC_DEVIATIONS.md`)
- `VAR_STAT` vendor-extension storage semantics (see `docs/IEC_DEVIATIONS.md`)
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
