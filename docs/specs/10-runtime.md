# truST Platform Runtime and Tooling Specification

## Status and scope
- Current runtime: tree-walking interpreter over a lowered eval AST (trust-syntax + trust-hir).
- Bytecode (STBC) is used for packaging and metadata; there is no bytecode VM in the runtime yet.
- Debugger uses DAP plus the runtime control protocol; LSP/IDE technical spec is included below.
- IEC language specs remain in docs/specs/01-09-*.md.

## Runtime Interpreter

IEC 61131-3 Edition 3.0 (2013) - Runtime Execution

This specification defines the trust-runtime crate, a tree-walking interpreter for IEC 61131-3 Structured Text with cycle-based, deterministic execution suitable for testing PLC code.

### 1. Overview

#### 1.1 Design Goals

1. **Tree-walking interpreter**: Execute a lowered eval AST (from trust-syntax + trust-hir) rather than a bytecode VM
2. **Cycle-based execution**: Execute programs in discrete cycles, not continuous loops
3. **Deterministic**: Same inputs produce same outputs, ordered iteration via IndexMap
4. **Testable**: First-class support for unit testing PLC logic
5. **Zero unsafe**: Follows `unsafe_code = "forbid"` convention

#### 1.2 Architecture

```
crates/trust-runtime/
├── Cargo.toml
├── src/
│   ├── lib.rs            # Public API, Runtime struct
│   ├── bytecode/         # STBC encode/decode + metadata
│   ├── eval/             # Lowered AST eval (expr/stmt)
│   ├── runtime/          # Runtime core + subsystems
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

Only IEC REF_TO references are modeled; POINTER extensions are not part of the runtime.
`Value::Null` is reserved for reference values (REF_TO) and is the default for uninitialized references
(IEC 61131-3 Ed.3 §6.4.4.10.2).

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
    pub elements: Vec<Value>,
    pub dimensions: Vec<(i64, i64)>, // (lower, upper) bounds
}

/// Struct value with named fields.
#[derive(Debug, Clone, PartialEq)]
pub struct StructValue {
    pub type_name: SmolStr,
    pub fields: IndexMap<SmolStr, Value>, // Ordered for determinism
}

/// Enum value storing both name and numeric value.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumValue {
    pub type_name: SmolStr,
    pub variant_name: SmolStr,
    pub numeric_value: i64,
}
```

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
| Sizeof | `SizeOfExpr` | `SIZEOF(type)` |

**REF operator** (IEC 61131-3 Ed.3 §6.4.4.10.3):
- `REF(var)` returns a reference to a declared variable or instance.
- Applying `REF` to temporary variables (VAR_TEMP or function-local temporaries) is not permitted.

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
| `%MD0` | Memory | DWord | Byte 0-3 |

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

#### Phase 1: Core Interpreter

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
- REFERENCE types (REF_TO) + assignment attempt semantics (see `IEC deviations log (internal)`)
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

## ST Runtime Implementation Specification

**Status:** Planned architecture. The current runtime is the interpreter described above; bytecode execution is not implemented yet.

### 1. Purpose

This document specifies the architecture for a portable Structured Text (ST) runtime capable of executing IEC 61131-3 compliant programs. The initial implementation targets desktop operating systems (Linux, Windows, macOS); embedded support is planned.

### 2. Design Goals

| Goal | Description |
|------|-------------|
| Portability | Single runtime codebase runs on desktop and embedded targets |
| Determinism | Predictable scan cycle execution suitable for automation |
| IEC Compliance | Align task scheduling and execution semantics with IEC 61131-3 Ed.3 |
| Simplicity | Minimal clock abstraction surface |
| Testability | Full runtime testable on desktop without hardware |

### 3. Architecture Overview

```
┌──────────────────────────────────────────────────┐
│               ST Program (Bytecode)              │
└──────────────────────┬───────────────────────────┘
                       ▼
┌──────────────────────────────────────────────────┐
│                 ST Runtime Core                  │
│  ┌────────────┐ ┌──────────────────┐ ┌──────────────┐  │
│  │  Executor  │ │Resource Scheduler│ │ Timer System │  │
│  └────────────┘ └──────────────────┘ └──────────────┘  │
│  ┌────────────────────────────────────────────┐  │
│  │            Process Image                   │  │
│  └────────────────────────────────────────────┘  │
└──────────────────────┬───────────────────────────┘
                       ▼
┌──────────────────────────────────────────────────┐
│                 Clock Trait                      │
└───────────┬──────────────────────┬───────────────┘
            ▼                      ▼
┌───────────────────┐    ┌───────────────────┐
│     StdClock      │    │   ManualClock     │
│ (Linux/Win/Mac)   │    │   (Tests/Sim)     │
└───────────────────┘    └───────────────────┘
```

### 4. Clock Abstraction Layer

#### 4.1 Rationale

The runtime requires only a monotonic clock and a way to sleep until a deadline. Rather than abstracting entire operating systems, we abstract only what the scheduler actually uses. This keeps the abstraction minimal and each clock implementation small.

#### 4.2 Clock Trait Definition

```rust
pub trait Clock: Send + Sync + 'static {
    /// Returns monotonic time for scheduling (nanosecond Duration).
    fn now(&self) -> Duration;

    /// Sleeps until a target time. Used only by real resource threads.
    fn sleep_until(&self, deadline: Duration);

    /// Wake any sleepers (best-effort).
    fn wake(&self) { /* optional */ }
}
```

The runtime scheduler uses a `Clock` for time and pacing. Thread creation and mutexing are handled by Rust’s standard library, keeping the abstraction surface minimal.

#### 4.3 Why Only a Clock

| Operation | Justification |
|-----------|---------------|
| `now` | Required for scan cycle timing and IEC timers (TON, TOF, TP) |
| `sleep_until` | Paces resource cycles in real threads |
| `wake` | Allows clean shutdown of resource threads |

Notably absent: file I/O (bytecode loaded at init), networking (handled separately via I/O abstraction), explicit mutex APIs (runtime uses `RwLock`/`Mutex` internally), dynamic allocation in hot path.

### 5. Clock Implementations

#### 5.1 StdClock (Desktop)

**Targets:** Linux, Windows, macOS

**Implementation:** Uses Rust standard library (`Instant`, `thread::sleep`).

```rust
pub struct StdClock {
    start: Instant,
}

impl Clock for StdClock {
    fn now(&self) -> Duration {
        let elapsed = self.start.elapsed();
        let nanos = i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX);
        Duration::from_nanos(nanos)
    }

    fn sleep_until(&self, deadline: Duration) {
        let now = self.now();
        let delta = deadline.as_nanos() - now.as_nanos();
        if delta <= 0 {
            return;
        }
        let delta = u64::try_from(delta).unwrap_or(u64::MAX);
        thread::sleep(std::time::Duration::from_nanos(delta));
    }
}
```

**Justification:** Rust’s standard library already abstracts Linux/Windows/macOS differences. Task priority is enforced by the runtime scheduler; OS thread priority is best-effort only and may be ignored.

#### 5.2 ManualClock (Tests)

Deterministic clock for unit tests and simulation. Time advances explicitly; no real sleeping occurs. Used by scheduler tests and trace reproducibility checks.

#### 5.3 Embedded Clock (Planned)

An RTOS-backed clock (e.g., FreeRTOS) is planned for embedded targets. The runtime core remains unchanged; only the `Clock` implementation differs.

### 6. Runtime Components

#### 6.1 Executor

Interprets compiled ST bytecode. Operates on the process image. Pure computation with no platform dependencies.

**Design decisions:**
- Stack-based bytecode VM (simpler than register-based)
- No heap allocation during execution (predictable timing)
- All state in process image (inspectable, serializable)

#### 6.2 Task Manager (Resource Scheduler)

Implements IEC 61131-3 task scheduling and program organization unit (POU) associations (IEC 61131-3 Ed.3, §6.8.2; Tables 62–63).

Each IEC **resource** runs inside a dedicated scheduler loop. The scheduler is executed on an OS thread started via `std::thread::spawn`; IEC tasks are *not* OS threads.

**IEC task model:**
- Tasks are periodic (INTERVAL) or event-driven (SINGLE rising edge). (IEC 61131-3 Ed.3, §6.8.2 a–b)
- If INTERVAL is non-zero, periodic scheduling occurs only while SINGLE is 0. (IEC 61131-3 Ed.3, §6.8.2 b)
- If INTERVAL is zero, no periodic scheduling occurs. (IEC 61131-3 Ed.3, §6.8.2 b)
- PRIORITY establishes scheduling order with 0 as highest priority and larger numbers as lower priority. (IEC 61131-3 Ed.3, §6.8.2 c; Table 63)
- A program with no task association executes once per resource cycle at the lowest priority. (IEC 61131-3 Ed.3, §6.8.2 d)
- A function block instance associated with a task executes only under that task, independent of program evaluation rules. (IEC 61131-3 Ed.3, §6.8.2 e)

**Scheduling policy (implementer choice permitted by IEC 61131-3, §6.8.2 c):**
- Deterministic, non-preemptive, fixed-priority scheduling per resource.
- Ready tasks at the same priority run in FIFO order by longest waiting time.
- Event tasks are edge-detected on the SINGLE input and enqueue one activation per rising edge.

```rust
pub struct TaskConfig {
    pub name: String,
    pub interval: Duration,      // INTERVAL; zero disables periodic scheduling
    pub single: Option<String>,  // SINGLE variable name (event + gating)
    pub priority: u32,           // 0 = highest priority per IEC 61131-3
    pub programs: Vec<ProgramId>,
    pub fb_instances: Vec<ValueRef>,
}

pub struct ResourceRunner<C: Clock + Clone> {
    runtime: Runtime,
    clock: C,
    cycle_interval: Duration,
}

impl<C: Clock + Clone> ResourceRunner<C> {
    pub fn tick(&mut self) -> Result<(), RuntimeError> {
        // single deterministic cycle (tests)
        Ok(())
    }

    pub fn spawn(self, name: &str) -> ResourceHandle<C> {
        // start dedicated OS thread
    }
}
```

**Implementation notes:**
- The SINGLE input is sampled from the current variable state; a transition 0 -> 1 enqueues exactly one activation.
- On task registration, the runtime initializes the previous SINGLE value to avoid a spurious edge on the first cycle.
- Periodic scheduling uses `Clock::now()` and the task interval (nanosecond Duration).
- Inputs are latched at the start of each scheduler cycle; outputs are committed after all ready tasks complete.
- The maximum number of tasks per resource and minimum interval resolution are implementer-specific and are reported by the runtime configuration.
- The resource loop maintains a `RUNNING/FAULT/STOPPED` state and halts on faults.

#### 6.3 Timer System

Implements IEC 61131-3 timers: TON (on-delay), TOF (off-delay), TP (pulse).

All timers use `Clock::now()` for elapsed time calculation. Timer instances are evaluated when their owning program or task-associated function block executes; no background threads or interrupts are required.

#### 6.4 Process Image

Memory-mapped area for inputs (%I), outputs (%Q), and markers (%M).

```rust
pub struct IoInterface {
    inputs: Vec<u8>,
    outputs: Vec<u8>,
    memory: Vec<u8>,
}
```

Sizes are derived from compiled program metadata at load time. On embedded targets, static sizing may be used, but the logical model remains the same.

The process image is owned by a single resource thread; no internal locking is required. Cross-resource data sharing is synchronized through the configuration-level shared globals lock (see 6.7). External I/O exchange (Modbus, etc.) reads/writes to this image at cycle boundaries.

#### 6.5 I/O Drivers

I/O exchange is explicit and deterministic: inputs are read into the input image at the start of each resource cycle, and outputs are written after all ready tasks complete.

```rust
pub trait IoDriver: Send {
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError>;
    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError>;
    fn health(&self) -> IoDriverHealth { IoDriverHealth::Ok }
}
```

Multiple drivers may be composed (e.g., fieldbus + simulated I/O). The resource scheduler owns the driver(s) and invokes them at cycle boundaries.

Driver error handling is configurable per driver:
- `fault`: return an error and fault the resource.
- `warn`: keep the resource running; driver health becomes **degraded**.
- `ignore`: keep the resource running; error is suppressed (health may still degrade).

Driver health is exposed via `ctl status` and the TUI.

**Built-in driver (v1): Modbus/TCP**
- Uses **input registers** (0x04) for input image.
- Uses **holding registers** (0x10) for output image.
- Register payloads are big‑endian (high byte first).
- Register quantity is derived from the process image size (`ceil(bytes / 2)`).

#### 6.6 Fault, Overrun, and Watchdog Handling

The runtime traps execution faults and reports them through a unified fault channel. By default, a fault transitions the resource into a **FAULT** state and halts further task execution until restarted.

Faults include:
- Arithmetic errors (e.g., divide by zero)
- Out-of-bounds accesses
- Invalid type conversions
- FOR loops with a step expression that evaluates to 0 (guarded by bytecode and treated as a runtime fault)
- Task overruns (missed deadlines)

Overrun policy (default): if a periodic task misses its deadline, the missed activation is dropped, the overrun counter increments, and the task is eligible again on the next interval boundary.

**Watchdog policy (production):**
- A watchdog monitors cycle/task execution time.
- If the watchdog timeout elapses, the runtime raises a **FAULT** and halts the resource.
- Timeout thresholds and fault action are configured per resource (see §6.9) and are
  **implementer-specific** in IEC 61131-3 (recorded in `IEC deviations log (internal)`).
- Default action is **safe_halt**: outputs are set to configured safe values (if provided),
  then the resource halts. For **halt** and **safe_halt**, safe-state outputs are applied
  before halting.

#### 6.7 Retain Storage (IEC 61131-3 §6.5.6)

Retentive variables must follow IEC 61131-3 retentive variable rules (§6.5.6, Figure 9). At
startup:

- **Warm restart**: RETAIN variables restore their retained values; NON_RETAIN are initialized.
- **Cold restart**: RETAIN and NON_RETAIN variables are initialized.
- Unqualified variables follow the runtime's retain policy (see the internal IEC decisions log, ID IEC-DEC-009).

Retain storage is provided via a pluggable backend:

```rust
pub trait RetainStore: Send {
    fn load(&self) -> Result<RetainImage, RuntimeError>;
    fn store(&self, image: &RetainImage) -> Result<(), RuntimeError>;
}
```

The runtime loads retained values during resource startup and writes them on shutdown and
periodically (policy defined in the runtime configuration). The periodic cadence is
rate-limited and only writes when retained values have changed.

**Power-loss guidance:** retained values are only guaranteed to persist if the most recent
snapshot has been flushed to the retain store (i.e., at shutdown or after the save cadence).
Unflushed changes may be lost on sudden power loss (implementer-specific).

#### 6.8 Runtime Launcher & Deployment (Project Folder)

Production runtimes are started via the CLI (`trust-runtime run`) using a **project folder**
(runtime bundle format) directory. The launcher is responsible for:

- Loading the bytecode program (`program.stbc`).
- Loading runtime configuration (`runtime.toml`).
- Initializing I/O drivers (`io.toml` or system IO config).
- Initializing retain storage (if configured).
- Exposing a control endpoint for local attach/debug.
- Validating bundle version compatibility before execution (internal `bundle.version`).

The launcher **must** run on Linux, Windows, and macOS (desktop targets). Embedded targets may
replace the launcher with platform-specific init systems while preserving the same configuration
and control protocol.

If a project folder omits `io.toml`, the launcher loads the system IO config

This behavior is implementer-specific; IEC 61131-3 does not define
hardware driver selection or OS-level IO configuration (see the internal IEC deviations log, DEV-028).

Control endpoints are local by default (`unix://` on Unix-like platforms) and the Unix socket is
created with restrictive permissions (0600) to prevent accidental exposure.

#### 6.9 Debug Attach (Production)

Attach debugging is **optional** in production deployments but must be supported by the runtime
when enabled:

- Attach must not restart or reload the runtime.
- Attach must observe the current state (running/paused/faulted).
- Detach must not alter runtime execution.
- Debug hooks must be side-effect-free when disabled.
- Attach is gated by `runtime.control.debug_enabled`. When disabled, debug control requests are
  rejected. The default is **disabled** in production mode (see `runtime.control.mode`).
- `runtime.control.mode` defaults to `production` and can be set to `debug` for development
  workflows; `runtime.control.debug_enabled` overrides the mode when explicitly set.

#### 6.10 Configuration and Resources

IEC configurations may declare multiple resources. Each resource is scheduled independently in its own OS thread. (IEC 61131-3 Ed.3, §6.8.1; Table 62)

Cross-resource data exchange is limited to explicitly declared globals (e.g., `VAR_GLOBAL` in configuration scope). (IEC 61131-3 Ed.3, §6.8.1; Table 62) Shared globals are synchronized under a single configuration lock: each resource cycle copies shared values in, executes ready tasks, then writes back updates before releasing the lock. This preserves deterministic ordering while serializing shared-global access.

#### 6.11 Bytecode Format (Overview)

The executor consumes a stable bytecode container format emitted by the compiler. See the "ST Bytecode Format Specification" section in this document for details.
- Instruction encoding and versioning
- Program/function/function-block layouts
- Constant pools and type descriptors
- Resource, task, and POU metadata required by the runtime (process image sizing, task associations)

The runtime rejects unsupported major bytecode versions before configuring resources.

#### 6.12 Browser UI, Discovery, and Mesh (Operational UX)

Operational UX is **browser‑first** (no app). A built‑in web service exposes an
operational UI and discovery metadata. This is **implementer‑specific** and
outside IEC 61131‑3 scope.

Configuration (in `runtime.toml`):

```
[runtime.web]
enabled = true
listen = "0.0.0.0:8080"
auth = "local"              # local|token

[runtime.discovery]
enabled = true
service_name = "truST"
advertise = true
interfaces = ["eth0", "wlan0"]

[runtime.mesh]
enabled = false
listen = "0.0.0.0:5200"
auth_token = "change-me"
publish = ["Status.RunState", "Metrics.CycleMs", "TempA"]

[runtime.mesh.subscribe]
"Plant-1:TempA" = "RemoteTemp"
```

Rules:
- **Local‑only by default**. Remote access must be explicitly enabled.
- **Discovery uses mDNS/Bonjour** on the local LAN only.
- **Remote access** supports manual add and invite/QR pairing only.
- **Data sharing** is explicit (publish/subscribe mapping only).
- TOML remains the source of truth; offline edits are supported.

HMI customization (implementer-specific):
- `hmi.schema.get` returns `theme`, `pages`, and widget-level layout metadata (`page`, `group`, `order`, `unit`, bounds) in addition to stable widget IDs.
- Project-level `hmi.toml` supports:
  - `[theme]` (`style`, optional `accent`)
  - `[[pages]]` (`id`, `title`, `order`)
  - `[widgets.\"<path>\"]` overrides for label/unit/bounds/widget/page/group/order.
- ST-level `@hmi(...)` annotations on variable declarations support `label`, `unit`, `min`, `max`, `widget`, `page`, `group`, and `order`.
- Merge precedence is deterministic: defaults < ST annotations < `hmi.toml` overrides.
- Theme fallback is deterministic: unknown/missing theme values fall back to built-in `classic`.

Operational UX and pairing flow are documented internally.

#### 6.9 Debugging and Diagnostics

The runtime emits structured events for debugging and testing:
- Cycle start/end (with timestamp)
- Task start/end (with task name, priority)
- Breakpoint hit / step events (statement boundaries)
- Fault and overrun notifications

These events are consumed by the debugger (`trust-debug`) and test harnesses to validate behavior deterministically.

### 7. Build Configuration

#### 7.1 Feature Flags

```toml
[features]
default = ["debug"]
debug = []  # enable debug instrumentation and runtime events
```

#### 7.2 Conditional Compilation

Desktop builds use the standard library unconditionally. Embedded support will introduce additional `cfg` gates for alternative clock implementations.

### 8. Why Not Alternatives

| Alternative | Reason for rejection |
|-------------|---------------------|
| **Containers** | Cannot run on microcontrollers. Adds complexity without benefit for this use case. |
| **FreeRTOS POSIX simulator** | Adds unnecessary layer on desktop. Not production-grade. |
| **Embassy (async Rust)** | Cooperative scheduling unsuitable for deterministic PLC timing. |
| **WASM** | Adds complexity. Real-time I/O interaction awkward. Could be future target. |
| **Transpile to C** | Loses runtime flexibility. Debugging harder. |

### 9. Future Considerations

**WebAssembly target:** The runtime core (executor, timers) could compile to WASM for browser-based simulation. Would require a WASM-friendly `Clock` implementation.

**Remote I/O:** Process image exchange via Modbus TCP is architecturally separate from the clock layer. Networking abstraction would be added alongside the `Clock` trait, not replacing it.

**Retain variables:** Persistent storage across power cycles requires platform-specific implementation (filesystem on desktop, flash on embedded). This is orthogonal to the `Clock` trait and would be added as a separate storage interface if needed.

### 10. Summary

The thin clock abstraction approach provides:

1. **IEC-aligned task scheduling** (periodic and event tasks with defined priority rules)
2. **Minimal clock surface** - easy to maintain and verify
3. **Clear separation** - runtime logic vs clock primitives
4. **Deterministic behavior** - explicit scheduling and I/O latching rules
5. **Testability** - full runtime runs natively on development machine

The runtime is implemented in Rust, using the standard library for desktop targets initially. Embedded backends are planned with identical runtime logic and alternate clock implementations.

## ST Bytecode Format Specification (Draft)

**Status:** Implemented as a container + metadata format. The runtime currently applies metadata from STBC; it does not execute bytecode instructions.

### 1. Purpose

This document defines the bytecode format consumed by the ST runtime executor. It is intended to be stable, versioned, and easy to inspect for debugging and testing. The IEC 61131-3 standard does not define a bytecode format; this container is implementer-specific.

### 2. Goals

- Deterministic execution across platforms
- Compact, mostly fixed-width instruction encoding
- Explicit typing information for runtime checks
- Backward-compatible evolution via versioning
- KISS: one container, one section table, clear validation rules

### 3. Conventions

- Endianness: little-endian for all multi-byte integers.
- Integer sizes:
  - u8/u16/u32/u64: unsigned
  - i32/i64: signed two's complement
- Strings: UTF-8, encoded as `u32 length` followed by raw bytes (no trailing NUL).
- Arrays: `u32 count` followed by count entries.
- Offsets: `u32` byte offsets from the start of the file.
- Alignment: section offsets and lengths are 4-byte aligned; padding bytes are `0x00`.
- Jump offsets are `i32` byte deltas relative to the next instruction.

### 4. Container Layout

The bytecode is a single container with a fixed-size header and a section table.

#### 4.1 Header (Version 1.x)

```
struct Header {
  u8  magic[4];          // "STBC"
  u16 version_major;     // currently 1
  u16 version_minor;     // currently 1
  u32 flags;             // header flags (see below)
  u16 header_size;       // bytes, header only (currently 24)
  u16 section_count;     // number of section table entries
  u32 section_table_off; // offset to section table (currently 24)
  u32 checksum;          // CRC32 if flags&0x0001 != 0, else 0
}
```

Validation rules:
- `magic` must be `STBC`.
- `version_major` must be supported by the runtime.
- `header_size` and `section_table_off` must be >= 24 and 4-byte aligned.
- `section_table_off` + `section_count * 12` must fit within the file.
- If `flags & 0x0001` is set, `checksum` must be the CRC32 of the section table and all section payloads (bytes from `section_table_off` to end of file).

#### 4.2 Section Table Entry

```
struct SectionEntry {
  u16 id;        // section identifier
  u16 flags;     // 0 = none
  u32 offset;    // absolute offset in file
  u32 length;    // section length in bytes
}
```

Section table rules:
- Entries may appear in any order.
- Offsets must be 4-byte aligned.
- Sections must not overlap.
- Unknown section IDs are ignored unless marked required by the runtime configuration.

#### 4.3 Section Flags

- `0x0001` COMPRESSED_ZSTD (section payload is zstd compressed)
- All other bits are reserved and must be ignored if unknown.

#### 4.4 Header Flags

- `0x0001` CRC32 (header `checksum` is CRC32 of section table + section payloads)

### 5. Section IDs (Version 1.x)

| ID | Name | Required | Purpose |
|----|------|----------|---------|
| 0x0001 | STRING_TABLE | Yes | Interned UTF-8 strings |
| 0x0002 | TYPE_TABLE | Yes | Type declarations |
| 0x0003 | CONST_POOL | Yes | Constant literals |
| 0x0004 | REF_TABLE | Yes | Value reference table |
| 0x0005 | POU_INDEX | Yes | POU directory and signatures |
| 0x0006 | POU_BODIES | Yes | Bytecode bodies |
| 0x0007 | RESOURCE_META | Yes | Resources/tasks/process image |
| 0x0008 | IO_MAP | Yes | Direct I/O bindings |
| 0x0009 | DEBUG_MAP | No | Source mapping, breakpoints |
| 0x000A | DEBUG_STRING_TABLE | No | Debug-only strings (file paths) |
| 0x000B | VAR_META | No | Variable metadata (globals) |
| 0x000C | RETAIN_INIT | No | Retain initialization values |
| 0x8000-0xFFFF | VENDOR | No | Vendor/experimental |

### 6. Section Definitions

#### 6.1 STRING_TABLE (0x0001)

```
struct StringTable {
  u32 count;
  StringEntry entries[count];
}

struct StringEntry {
  u32 length;
  u8  bytes[length];
}
```

String indices are zero-based. All identifiers in other sections refer to this table.
For version >= 1.1, each `StringEntry` is padded with `0x00` bytes to the next 4-byte boundary; the padding is not included in `length`.
The DEBUG_STRING_TABLE section uses the same encoding.

#### 6.2 TYPE_TABLE (0x0002)

```
struct TypeTable {
  u32 count;
  u32 offsets[count]; // byte offsets from TYPE_TABLE start (version >= 1.1)
  TypeEntry entries[count];
}

struct TypeEntry {
  u8  kind;       // see TypeKind
  u8  flags;      // reserved
  u16 reserved;
  u32 name_idx;   // 0xFFFFFFFF for anonymous
  // payload follows based on kind
}
```

For version 1.0, `offsets` is omitted and entries are stored back-to-back.

Type kinds (Version 1.x):
- 0 PRIMITIVE
- 1 ARRAY
- 2 STRUCT
- 3 ENUM
- 4 ALIAS
- 5 SUBRANGE
- 6 REFERENCE
- 7 UNION
- 8 FUNCTION_BLOCK
- 9 CLASS
- 10 INTERFACE

Primitive payload:
```
struct PrimitiveType {
  u16 prim_id;     // see PrimitiveId
  u16 max_length;  // for STRING/WSTRING; 0 means default/unspecified
}
```

Array payload:
```
struct ArrayType {
  u32 elem_type_id;
  u32 dim_count;
  Dim dims[dim_count];
}

struct Dim {
  i64 lower;
  i64 upper;
}
```

Struct payload:
```
struct StructType {
  u32 field_count;
  Field fields[field_count];
}

struct Field {
  u32 name_idx;
  u32 type_id;
}
```

Enum payload:
```
struct EnumType {
  u32 base_type_id; // integer type
  u32 variant_count;
  Variant variants[variant_count];
}

struct Variant {
  u32 name_idx;
  i64 value;
}
```

Alias payload:
```
struct AliasType {
  u32 target_type_id;
}
```

Subrange payload:
```
struct SubrangeType {
  u32 base_type_id; // signed/unsigned integer
  i64 lower;
  i64 upper;
}
```

Reference payload:
```
struct ReferenceType {
  u32 target_type_id;
}
```

Union payload:
```
struct UnionType {
  u32 field_count;
  Field fields[field_count];
}
```

POU type payload (FUNCTION_BLOCK / CLASS):
```
struct PouType {
  u32 pou_id; // POU_INDEX id
}
```

Interface payload:
```
struct InterfaceType {
  u32 method_count;
  InterfaceMethod methods[method_count];
}

struct InterfaceMethod {
  u32 name_idx;
  u32 slot; // interface method slot (0..method_count-1)
}
```

Primitive IDs (Version 1.x):
- 1 BOOL
- 2 BYTE
- 3 WORD
- 4 DWORD
- 5 LWORD
- 6 SINT
- 7 INT
- 8 DINT
- 9 LINT
- 10 USINT
- 11 UINT
- 12 UDINT
- 13 ULINT
- 14 REAL
- 15 LREAL
- 16 TIME
- 17 LTIME
- 18 DATE
- 19 LDATE
- 20 TOD
- 21 LTOD
- 22 DT
- 23 LDT
- 24 STRING
- 25 WSTRING
- 26 CHAR
- 27 WCHAR

#### 6.3 CONST_POOL (0x0003)

```
struct ConstPool {
  u32 count;
  ConstEntry entries[count];
}

struct ConstEntry {
  u32 type_id;
  u32 payload_len;
  u8  payload[payload_len];
}
```

Payload encoding follows the referenced type:
- Integer/boolean: little-endian, natural size of the primitive.
- REAL/LREAL: IEEE-754 binary32/binary64.
- STRING/WSTRING: `u32 string_idx` (string table reference).
- TIME/LTIME: `i64` nanoseconds.
- DATE/TOD/DT: `i64` ticks in the runtime `DateTimeProfile` resolution.
- LDATE/LTOD/LDT: `i64` nanoseconds.
- REFERENCE: `u32 ref_idx` or `0xFFFFFFFF` for NULL.
- ARRAY: `u32 elem_count` followed by `elem_count` element constant payloads.
- STRUCT/UNION: `u32 field_count` followed by `field_count` field constant payloads in field order.
- ENUM: `i64` numeric value.

#### 6.4 REF_TABLE (0x0004)

Static value references used by LOAD/STORE instructions and task FB associations.

```
struct RefTable {
  u32 count;
  RefEntry entries[count];
}

struct RefEntry {
  u8  location;     // see RefLocation
  u8  flags;        // reserved
  u16 reserved;
  u32 owner_id;     // frame/instance id; 0 for global/retain/io
  u32 offset;       // variable index within the owner scope
  u32 segment_count;
  RefSegment segments[segment_count];
}
```

Reference locations:
- 0 GLOBAL
- 1 LOCAL
- 2 INSTANCE
- 3 IO
- 4 RETAIN

Reference segments:
```
struct RefSegment {
  u8  kind; // 0 = INDEX, 1 = FIELD
  u8  reserved[3];
  union {
    IndexSegment index;
    FieldSegment field;
  };
}

struct IndexSegment {
  u32 count;
  i64 indices[count];
}

struct FieldSegment {
  u32 name_idx;
}
```

#### 6.5 POU_INDEX (0x0005)

```
struct PouIndex {
  u32 count;
  PouEntry entries[count];
}

struct PouEntry {
  u32 id;
  u32 name_idx;
  u8  kind;        // 0 PROGRAM, 1 FUNCTION_BLOCK, 2 FUNCTION, 3 CLASS, 4 METHOD
  u8  flags;       // reserved
  u16 reserved;
  u32 code_offset; // offset within POU_BODIES section
  u32 code_length; // byte length (0 if no body)
  u32 local_ref_start;
  u32 local_ref_count;
  u32 return_type_id; // 0xFFFFFFFF if no return
  u32 owner_pou_id;   // METHOD only; 0xFFFFFFFF otherwise
  u32 param_count;
  ParamEntry params[param_count];
  // if kind == FUNCTION_BLOCK or CLASS:
  u32 parent_pou_id; // 0xFFFFFFFF if no EXTENDS
  u32 interface_count;
  InterfaceImpl interfaces[interface_count];
  u32 method_count;
  MethodEntry methods[method_count];
}

struct ParamEntry {
  u32 name_idx;
  u32 type_id;
  u8  direction;   // 0 IN, 1 OUT, 2 IN_OUT
  u8  flags;       // reserved
  u16 reserved;
  u32 default_const_idx; // CONST_POOL index (0xFFFFFFFF if none; version >= 1.1)
}

For version 1.0, `default_const_idx` is omitted. Default values are only applied for `IN` parameters.

struct MethodEntry {
  u32 name_idx;
  u32 pou_id;      // method POU id
  u32 vtable_slot; // virtual dispatch slot
  u8  access;      // 0 PUBLIC, 1 PROTECTED, 2 PRIVATE
  u8  flags;       // 0x01 OVERRIDE, 0x02 FINAL, 0x04 ABSTRACT
  u16 reserved;
}

struct InterfaceImpl {
  u32 interface_type_id; // TYPE_TABLE index
  u32 method_count;
  u32 vtable_slots[method_count]; // map interface slot -> class vtable slot
}
```

#### 6.6 POU_BODIES (0x0006)

A raw bytecode blob that contains all POU instruction streams. Offsets are relative to the start of this section.

#### 6.7 RESOURCE_META (0x0007)

```
struct ResourceMeta {
  u32 resource_count;
  ResourceEntry resources[resource_count];
}

struct ResourceEntry {
  u32 name_idx;
  u32 inputs_size;
  u32 outputs_size;
  u32 memory_size;
  u32 task_count;
  TaskEntry tasks[task_count];
}

struct TaskEntry {
  u32 name_idx;
  u32 priority;        // 0 = highest priority
  i64 interval_nanos;  // 0 disables periodic scheduling
  u32 single_name_idx; // 0xFFFFFFFF means none
  u32 program_count;
  u32 program_name_idx[program_count];
  u32 fb_ref_count;
  u32 fb_ref_idx[fb_ref_count];
}
```

#### 6.8 IO_MAP (0x0008)

Direct I/O bindings between the process image and program variables.

```
struct IoMap {
  u32 binding_count;
  IoBinding bindings[binding_count];
}

struct IoBinding {
  u32 address_str_idx;  // IEC address string (e.g., "%IX0.0")
  u32 ref_idx;          // REF_TABLE entry
  u32 type_id;          // 0xFFFFFFFF if unspecified
}
```

#### 6.9 DEBUG_STRING_TABLE (0x000A, optional)

Same encoding as STRING_TABLE. Used for debug-only strings such as source file paths.

#### 6.10 DEBUG_MAP (0x0009, optional)

```
struct DebugMap {
  u32 entry_count;
  DebugEntry entries[entry_count];
}

struct DebugEntry {
  u32 pou_id;
  u32 code_offset;  // offset within POU_BODIES
  u32 file_idx;     // debug string table index (v1.1+)
  u32 line;         // 1-based
  u32 column;       // 1-based
  u8  kind;         // 0 statement, 1 breakpoint, 2 scope
  u8  reserved[3];
}
```

For version >= 1.1, `file_idx` refers to DEBUG_STRING_TABLE. For version 1.0, it refers to STRING_TABLE.

#### 6.11 VAR_META (0x000B, optional)

```
struct VarMeta {
  u32 entry_count;
  VarMetaEntry entries[entry_count];
}

struct VarMetaEntry {
  u32 name_idx;        // STRING_TABLE index
  u32 type_id;         // TYPE_TABLE index
  u32 ref_idx;         // REF_TABLE index
  u8  retain;          // 0=UNSPECIFIED, 1=RETAIN, 2=NON_RETAIN, 3=PERSISTENT
  u8  reserved;
  u16 reserved2;
  u32 init_const_idx;  // CONST_POOL index (0xFFFFFFFF if none)
}
```

VarMeta entries describe global variables and their retain policies.

#### 6.12 RETAIN_INIT (0x000C, optional)

```
struct RetainInit {
  u32 entry_count;
  RetainInitEntry entries[entry_count];
}

struct RetainInitEntry {
  u32 ref_idx;    // REF_TABLE index
  u32 const_idx;  // CONST_POOL index
}
```

RetainInit provides cold-start initialization values for retained variables; warm restarts restore retained state instead.

### 7. Instruction Encoding (Version 1.x)

#### 7.1 Encoding Rules

- Each instruction begins with a 1-byte opcode.
- Operands are encoded in little-endian, with sizes defined per opcode.
- Invalid opcodes or malformed operands cause a runtime fault.

#### 7.2 Operand Types

- `u32` indexes refer to STRING_TABLE, TYPE_TABLE, CONST_POOL, REF_TABLE, or POU_INDEX as documented.
- `i32` offsets are relative to the next instruction.
- Stack values are `Value` instances; references are pushed as `Value::Reference`.

#### 7.3 Baseline Instruction Set

Control flow:
- `0x00 NOP`
- `0x01 HALT`
- `0x02 JMP i32`
- `0x03 JMP_TRUE i32` (pop bool)
- `0x04 JMP_FALSE i32` (pop bool)
- `0x05 CALL u32` (POU id)
- `0x06 RET`
- `0x07 CALL_METHOD u32` (pop instance ref, call method by vtable slot)
- `0x08 CALL_VIRTUAL u32 u32` (interface_type_id, interface_method_slot)

Stack and constants:
- `0x10 CONST u32` (const pool index)
- `0x11 DUP`
- `0x12 POP`
- `0x13 SWAP`
- `0x14 OVER` (a b -- a b a)
- `0x15 ROT` (a b c -- b c a)
- `0x16 PICK u8` (copy nth item to top; 0 = top)

Static references:
- `0x20 LOAD_REF u32` (ref table index)
- `0x21 STORE_REF u32` (ref table index)
- `0x22 PUSH_REF u32` (push `Value::Reference`)
- `0x23 PUSH_SELF` (push `THIS`/`SELF` reference in a method)

Dynamic references:
- `0x30 REF_FIELD u32` (field name index; pop ref, push ref)
- `0x31 REF_INDEX` (pop index, pop ref, push ref)
- `0x32 LOAD` (pop ref, push value)
- `0x33 STORE` (pop value, pop ref)

Arithmetic and logic:
- `0x40 ADD`
- `0x41 SUB`
- `0x42 MUL`
- `0x43 DIV` (fault on divide by zero)
- `0x44 MOD`
- `0x45 NEG`
- `0x46 AND`
- `0x47 OR`
- `0x48 XOR`
- `0x49 NOT`
- `0x4A SHL`
- `0x4B SHR`
- `0x4C EXPT`
- `0x4D ROL`
- `0x4E ROR`

Comparison:
- `0x50 EQ`
- `0x51 NE`
- `0x52 LT`
- `0x53 LE`
- `0x54 GT`
- `0x55 GE`

Type conversion:
- `0x60 CAST u32` (type id)

Standard library:
- `0x70 CALL_STD u32` (standard function id; resolved by the runtime stdlib)

Reserved opcode ranges:
- `0x80-0xEF` reserved for future core extensions.
- `0xF0-0xFF` vendor/experimental.

#### 7.4 Fault Semantics

The executor must fault on:
- Type mismatches (e.g., BOOL in arithmetic)
- Invalid references or out-of-bounds indexes
- Divide by zero
- FOR loop step expressions that evaluate to 0 (encoder emits a step==0 guard that executes `HALT` before loop entry)
- Invalid jump targets
- Method/interface dispatch on NULL or incompatible references

### 8. Versioning

- Major version changes are breaking and must be rejected by older runtimes.
- Minor version changes may be accepted if the runtime recognizes all required sections and opcodes.
- New sections and opcodes must be added in reserved ID/opcode ranges.

Version 1.1 additions:
- TYPE_TABLE offset index for O(1) lookup
- DEBUG_STRING_TABLE for debug-only strings
- VAR_META and RETAIN_INIT sections
- Param default values (`default_const_idx`)
- STRING_TABLE entry padding
- Header CRC32 flag (`flags & 0x0001`)

### 9. Metadata Integration Requirements

The loader must populate runtime metadata from:
- RESOURCE_META -> resources, tasks, process image sizes
- IO_MAP -> I/O bindings
- STRING_TABLE -> names for tasks/programs/resources
- REF_TABLE -> FB instance references
- POU_INDEX -> method tables, inheritance, interface dispatch mapping
- VAR_META / RETAIN_INIT -> global variable metadata and retain initialization (if present)

### 10. Debugging Data

The DEBUG_MAP section provides a deterministic mapping between bytecode offsets and source locations. Debug entries must refer to valid POU IDs and code offsets.
For version >= 1.1, file paths are stored in DEBUG_STRING_TABLE and referenced by `file_idx`.

### 11. Future Tasks (Deferred)

No deferred items at this time.

## Structured Text Debug Adapter Specification (DAP)

Status: Draft

### Scope

This specification defines the expected behavior of the Structured Text (ST) debug adapter and
runtime debug hooks for VS Code using the Debug Adapter Protocol (DAP). It covers breakpoints,
run control, stepping, source mapping, and multi-file navigation.

This document is implementation-agnostic but aligns with the DAP definitions in
`Debug Adapter Protocol specification` (see References).

### References (Normative)

- DAP base and request/response/event shapes: `Debug Adapter Protocol specification`
  - `Request`, `Response`, `Event`
  - `InitializeRequest`, `InitializedEvent`
  - `LaunchRequest`
  - `AttachRequest`
  - `SetBreakpointsRequest`, `Breakpoint`, `BreakpointLocationsRequest`
  - `ContinueRequest`, `PauseRequest`, `NextRequest`, `StepInRequest`, `StepOutRequest`
  - `StoppedEvent`
  - `StackTraceRequest`, `ScopesRequest`, `VariablesRequest`, `EvaluateRequest`
  - `DisconnectRequest`, `TerminateRequest`

### Terms

- **Adapter**: `trust-debug` process handling DAP requests.
- **Runtime**: `trust-runtime` process executing ST code.
- **Statement**: A single executable ST statement with a source location.
- **Location**: `(file_id, start_offset, end_offset)` in source text.
- **Task**: IEC task representing a cyclic execution unit.

### Source Mapping

1) Every executable statement **must** be assigned a location at the **first non-trivia token** in
   its syntax node. The location span covers the full statement text range.
2) Each source file loaded in a debug session has a unique `file_id` and is registered in the
   adapter with its path and full text.
3) The adapter converts runtime locations to `(line, column)` for DAP using 1-based coordinates
   when `linesStartAt1` / `columnsStartAt1` are true.

### Breakpoints

#### SetBreakpoints

- `SetBreakpointsRequest` replaces all breakpoints for the given source.
- Passing an empty list clears all breakpoints for that source in both adapter and runtime.
- Breakpoints are **statement-based** and resolved to the first statement whose location is at or
  after the requested `(line, column)`.
- Column snapping:
  - If the client omits a column, the adapter snaps to the first non-whitespace column on that line.
  - If a column is provided but points before the first non-whitespace column, the adapter snaps
    forward to that first column.

#### Breakpoint Locations

- `BreakpointLocationsRequest` returns the set of valid statement start positions in the requested
  range.

#### Cyclic Tasks

- In cyclic tasks, a breakpoint in a statement that executes every scan **will stop every scan**
  until the breakpoint is cleared or a hit condition/condition filters it.
- Users should use hit counts or conditional breakpoints for one-shot behavior.

### Run Control

#### Continue

- `ContinueRequest` resumes all threads.
- Any pending pause request is cleared.
- A `StoppedEvent` is emitted only if a breakpoint, step, or pause condition is hit after resuming.

#### Pause

- `PauseRequest` is honored only if execution is currently running.
- The adapter **must** respond to the request before emitting `StoppedEvent` with reason `pause`.
- If already paused, the adapter returns success and does not emit another pause event.

#### Stop on Entry

- `LaunchRequest` with `stopOnEntry=true` results in a pause as soon as the first statement boundary
  is reached.

#### Attach / Detach (Production)

- `AttachRequest` connects to a **running** runtime instance.
- Attach must **not** restart or reload the runtime.
- Attach must observe the existing execution state (running/paused/faulted).
- If attach occurs while the runtime is paused, the adapter should immediately emit a
  `StoppedEvent` reflecting the paused state.
- `DisconnectRequest` / `TerminateRequest` must not alter runtime execution unless the user
  explicitly requests termination.

Attach arguments (adapter-specific):
- `endpoint` (required): control endpoint, e.g. `unix:///tmp/trust-runtime.sock` or `tcp://127.0.0.1:9000`
- `authToken` (optional): control auth token (same value used by `trust-runtime ctl`)

Attach requires `runtime.control.debug_enabled=true`. If disabled, the adapter must report an
error and remain disconnected.

Current attach limitation: `setVariable` / `setExpression` are not supported in attach mode
(read-only variables).

### Stepping Semantics

The following are required semantics for DAP step requests:

1) **Step In** (`stepIn`):
   - Resume execution and stop at the **next executed statement**.
   - If the next statement is a call, stepping **enters** the callee and stops at the first statement
     inside the called function/method.

2) **Step Over** (`next`):
   - Resume execution and stop at the next statement in the **current frame**.
   - Calls are executed without entering the callee.

3) **Step Out** (`stepOut`):
   - Resume execution and stop at the next statement **after returning** to the caller.

Stepping is statement-granular, not instruction-granular.

### Stopped Events

- `StoppedEvent.reason` **must** match the cause:
  - `breakpoint` for active breakpoints,
  - `step` for stepping commands,
  - `pause` for explicit pause requests,
  - `entry` for stop-on-entry.

### Stack Trace and Navigation

1) `StackTraceRequest` returns stack frames for the current thread.
2) The **top frame** location is the current statement location.
3) For multi-file projects, when execution enters a function in another file, the top frame’s
   `source.path` must reflect that file, and the editor should navigate there.

### Variables / Evaluate

- `VariablesRequest` and `ScopesRequest` return locals, globals, retain, and instance scopes.
- `EvaluateRequest` in `hover` or `watch` context must not have side effects. Calls are rejected.
- `setVariable` and `setExpression` are allowed only when paused.

### Reload / Hot Reload

- `stReload` replaces runtime sources and revalidates breakpoints.
- If the session was paused before reload, it remains paused after reload.

#### Reload Trigger Policy (Required)

To avoid breaking step-in/step-out and multi-file navigation, reloads must follow these rules:

1) **No reload on editor focus**:
   - Opening a file or changing the active editor must **not** trigger `stReload`.
   - This includes stepping into a function in another file.

2) **Allowed reload triggers**:
   - Explicit user action (e.g., command: “Reload Runtime”).
   - Optional: save events for ST files (if enabled), but **never** on focus change.

3) **Program path correctness**:
   - The `program` argument of `stReload` must always reference the **configuration entry**
     file (the same one used in `LaunchRequest`), not the currently focused file.

4) **Reload must not override step stops**:
   - If a `stepIn/stepOver/stepOut` stop just occurred, reload must **not** emit a pause stop
     that replaces the step stop or changes the top frame.
   - If reload happens while paused, it must preserve the existing top frame until the user resumes.

### Required Improvements (Architecture + Behavior)

The following items are **required** to align the implementation with this specification and to
avoid the observed instability in multi-file debugging sessions. These requirements are derived
from the DAP references above and the current runtime/adapter architecture.

#### 1) Stop Reason Integrity

- The adapter **must not** emit `StoppedEvent{reason="breakpoint"}` if there are no active
  breakpoints at stop time.
- Pending stop reasons must be **cleared** on `continue`, `step*`, or breakpoint removal.

#### 2) Breakpoint Generation / Staleness Guard

- Breakpoint sets must be versioned. Each `SetBreakpointsRequest` increments a generation number
  and runtime stops must only be honored if they match the **current** generation.
- Clearing breakpoints (`SetBreakpointsRequest` with an empty list) must immediately invalidate
  any pending breakpoint stops.

#### 3) Reload Semantics

- `stReload` must preserve paused/running state explicitly:
  - If the session was running, it stays running after reload.
  - If the session was paused, it stays paused after reload with `StoppedEvent{reason="pause"}`.
- The state machine must include an explicit **Reloaded** transition to avoid ambiguity.

#### 4) Per‑Frame Source Mapping

- `StackTraceRequest` must report each frame with its **own** source location (file/line/column),
  not the top-of-stack location for all frames.
- When a function in another file is entered, the top frame must point to that file; caller frames
  must continue to show their original source locations.

#### 5) Pause/Continue Idempotency

- `PauseRequest` while already paused must be a no-op (no additional pause events).
- `ContinueRequest` must clear any adapter-side pause expectation and runtime pending pause.

#### 6) Stop-on-Entry Reason

- `stopOnEntry` must emit `StoppedEvent{reason="entry"}` (not `pause`) per DAP semantics.
- This reason must be distinct in logs and internal state to avoid confusion with manual pause.

#### 7) DAP Event Ordering

- For requests that cause a stop (pause/step), the adapter must **send the response first** and
  emit `StoppedEvent` **after** the response, matching DAP requirements.

#### 8) Multi‑Task Thread Model

- If multiple IEC tasks are configured, each must map to a distinct DAP thread ID.
- `step*` and `pause` must apply to the thread specified by the request.

#### 9) Cyclic Task Breakpoint Safety

- In cyclic tasks, a breakpoint hit must not starve `continue`:
  - If the breakpoint is cleared, the runtime must resume without re-triggering the old stop.
  - If the breakpoint remains, the adapter should support hit conditions to avoid infinite stops.

## Technical Specification: truST LSP

### Document Information

| Property | Value |
|----------|-------|
| Version | 0.1.0 |
| Status | Draft |
| Last Updated | 2026-01-30 |
| Author | truST Contributors |

### Table of Contents

1. [Overview](#1-overview)
2. [Architecture](#2-architecture)
3. [Lexer Specification](#3-lexer-specification)
4. [Parser Specification](#4-parser-specification)
5. [Semantic Analysis](#5-semantic-analysis)
6. [IDE Features](#6-ide-features)
7. [LSP Protocol](#7-lsp-protocol)
8. [Runtime & Debugger](#8-runtime--debugger)
9. [Error Handling](#9-error-handling)
10. [Performance Requirements](#10-performance-requirements)
11. [Testing Strategy](#11-testing-strategy)
12. [Current Implementation Status](#12-current-implementation-status)

---

### 1. Overview

#### 1.1 Purpose

truST LSP is a Language Server Protocol implementation for IEC 61131-3 Structured Text (ST). It provides IDE features including diagnostics, completion, navigation, and refactoring for ST source code. The workspace also contains the
ST runtime, bytecode format, and debug adapter used for execution and testing.

#### 1.2 Scope

This specification covers:
- Lexical analysis of ST source code
- Syntactic analysis and CST construction
- Semantic analysis including type checking
- IDE feature implementations
- LSP protocol integration
- Runtime execution and bytecode decoding
- Debug adapter behavior and control protocols

#### 1.3 Target Standard

Primary: IEC 61131-3 Edition 3.0 (2013)

With extensions for:
- CODESYS v3.5
- Beckhoff TwinCAT 3
- Siemens TIA Portal (partial)

#### 1.4 Design Goals

1. **Correctness** - Accurately parse and analyze valid ST code
2. **Error Tolerance** - Provide useful feedback even for invalid code
3. **Performance** - Sub-100ms response times for interactive features
4. **Incrementality** - Re-analyze only what changed
5. **Extensibility** - Support vendor-specific extensions

---

### 2. Architecture

#### 2.1 Crate Structure

```
trust-platform (workspace)
├── trust-syntax      # Lexing and parsing
├── trust-hir         # High-level IR and semantic analysis
├── trust-ide         # IDE feature implementations
├── trust-lsp         # LSP protocol layer
├── trust-runtime     # Runtime execution engine + bytecode
└── trust-debug       # Debug adapter (DAP)
```

#### 2.2 Data Flow

```
Source Text
    │
    ▼
┌─────────┐
│  Lexer  │  → Token Stream
└─────────┘
    │
    ▼
┌─────────┐
│ Parser  │  → Concrete Syntax Tree (CST)
└─────────┘
    │
    ▼
┌─────────┐
│   HIR   │  → High-level IR + Symbol Table
└─────────┘
    │
    ▼
┌─────────┐
│   IDE   │  → Completions, Diagnostics, etc.
└─────────┘
    │
    ▼
┌─────────┐
│   LSP   │  → JSON-RPC Responses
└─────────┘
```

Runtime and debugger behavior are specified in:
- `docs/specs/10-runtime.md`

#### 2.3 Key Dependencies

| Crate | Purpose | Version |
|-------|---------|---------|
| logos | Lexer generation | 0.14 |
| rowan | Lossless syntax trees | 0.15 |
| salsa | Incremental computation | 0.18 |
| tower-lsp | LSP framework | 0.20 |

#### 2.4 Concurrency Model

- Single-threaded analysis (salsa handles caching)
- Async I/O for LSP communication (tokio)
- Document store protected by RwLock

---

### 3. Lexer Specification

#### 3.1 Token Categories

##### 3.1.1 Trivia Tokens

Tokens preserved in CST but not semantically significant:

| Token | Pattern | Example |
|-------|---------|---------|
| Whitespace | `[ \t\r\n]+` | ` `, `\n` |
| LineComment | `//[^\r\n]*` | `// comment` |
| BlockComment | `\(\*([^*]\|\*[^)])*\*\)` | `(* comment *)` |
| Pragma | `\{[^}]*\}` | `{VERSION 2.0}` |

##### 3.1.2 Punctuation

| Token | Lexeme |
|-------|--------|
| Semicolon | `;` |
| Colon | `:` |
| Comma | `,` |
| Dot | `.` |
| DotDot | `..` |
| LParen | `(` |
| RParen | `)` |
| LBracket | `[` |
| RBracket | `]` |
| Hash | `#` |
| Caret | `^` |
| At | `@` |

##### 3.1.3 Operators

| Token | Lexeme | Precedence |
|-------|--------|------------|
| Assign | `:=` | - |
| Arrow | `=>` | - |
| RefAssign | `?=` | - |
| Eq | `=` | 4 |
| Neq | `<>` | 4 |
| Lt | `<` | 5 |
| LtEq | `<=` | 5 |
| Gt | `>` | 5 |
| GtEq | `>=` | 5 |
| Plus | `+` | 6 |
| Minus | `-` | 6 |
| Star | `*` | 7 |
| Slash | `/` | 7 |
| Power | `**` | 8 |

##### 3.1.4 Keywords

All keywords are case-insensitive.

**POU Keywords:**
- `PROGRAM`, `END_PROGRAM`
- `FUNCTION`, `END_FUNCTION`
- `FUNCTION_BLOCK`, `END_FUNCTION_BLOCK`
- `CLASS`, `END_CLASS`
- `METHOD`, `END_METHOD`
- `PROPERTY`, `END_PROPERTY`
- `INTERFACE`, `END_INTERFACE`
- `NAMESPACE`, `END_NAMESPACE`
- `USING`
- `ACTION`, `END_ACTION`

**Variable Keywords:**
- `VAR`, `END_VAR`
- `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`
- `VAR_TEMP`, `VAR_GLOBAL`, `VAR_EXTERNAL`
- `VAR_ACCESS`, `VAR_CONFIG`, `VAR_STAT`
- `CONSTANT`, `RETAIN`, `NON_RETAIN`, `PERSISTENT`

**Access Modifiers:**
- `PUBLIC`, `PRIVATE`, `PROTECTED`, `INTERNAL`
- `FINAL`, `ABSTRACT`, `OVERRIDE`

**Type Keywords:**
- `TYPE`, `END_TYPE`
- `STRUCT`, `END_STRUCT`
- `UNION`, `END_UNION`
- `ARRAY`, `OF`
- `STRING`, `WSTRING`
- `POINTER`, `REF`, `REF_TO`, `TO`

**Control Flow:**
- `IF`, `THEN`, `ELSIF`, `ELSE`, `END_IF`
- `CASE`, `END_CASE`
- `FOR`, `BY`, `DO`, `END_FOR`
- `WHILE`, `END_WHILE`
- `REPEAT`, `UNTIL`, `END_REPEAT`
- `RETURN`, `EXIT`, `CONTINUE`, `JMP`

**Logical Operators:**
- `AND`, `OR`, `XOR`, `NOT`, `MOD`

**OOP Keywords:**
- `EXTENDS`, `IMPLEMENTS`
- `THIS`, `SUPER`
- `NEW`, `__NEW`, `__DELETE`

**Elementary Types:**
- Boolean: `BOOL`
- Signed: `SINT`, `INT`, `DINT`, `LINT`
- Unsigned: `USINT`, `UINT`, `UDINT`, `ULINT`
- Float: `REAL`, `LREAL`
- Bit: `BYTE`, `WORD`, `DWORD`, `LWORD`
- Time: `TIME`, `LTIME`, `DATE`, `TOD`, `DT`, etc.
- Generic: `ANY`, `ANY_INT`, `ANY_REAL`, etc.

**Boolean Literals:**
- `TRUE`, `FALSE`

**Configuration Keywords:**
- `CONFIGURATION`, `END_CONFIGURATION`
- `RESOURCE`, `END_RESOURCE`, `ON`
- `READ_ONLY`, `READ_WRITE`
- `TASK`, `WITH`

##### 3.1.5 Literals

| Token | Pattern | Examples |
|-------|---------|----------|
| IntLiteral | `[0-9][0-9_]*` | `123`, `1_000` |
| IntLiteral (hex) | `16#[0-9A-Fa-f_]+` | `16#FF`, `16#DEAD_BEEF` |
| IntLiteral (binary) | `2#[01_]+` | `2#1010`, `2#1111_0000` |
| IntLiteral (octal) | `8#[0-7_]+` | `8#77` |
| RealLiteral | `[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?` | `3.14`, `1.0E10` |
| TimeLiteral | `T#...` or `TIME#...` | `T#1h30m`, `TIME#5s` |
| DateLiteral | `D#YYYY-MM-DD` | `D#2024-01-15` |
| TimeOfDayLiteral | `TOD#HH:MM:SS` | `TOD#14:30:00` |
| DateTimeLiteral | `DT#...` | `DT#2024-01-15-14:30:00` |
| StringLiteral | `'...'` | `'hello'` |
| WideStringLiteral | `"..."` | `"hello"` |
| TypedLiteralPrefix | `IDENT#` | `INT#`, `REAL#` |

##### 3.1.6 Direct Addresses

| Pattern | Description | Examples |
|---------|-------------|----------|
| `%I[XBWDL]?addr` | Input | `%IX0.0`, `%IW10` |
| `%Q[XBWDL]?addr` | Output | `%QX1.0`, `%QD100` |
| `%M[XBWDL]?addr` | Memory | `%MW50`, `%MD200` |
| `%I*`, `%Q*`, `%M*` | Partially located | `%I*` |

##### 3.1.7 Identifiers

Pattern: `[A-Za-z_][A-Za-z0-9_]*`

Must not match any keyword (case-insensitive comparison).

#### 3.2 Lexer Error Handling

- Unrecognized characters produce `Error` tokens
- Lexing always succeeds (never panics)
- Error tokens preserve the invalid text for error reporting

---

### 4. Parser Specification

#### 4.1 Parser Type

Hand-written recursive descent parser with:
- Pratt parsing for expressions
- Error recovery via synchronization tokens
- Lossless CST construction using rowan

#### 4.2 Syntax Node Kinds

Representative subset; see `SyntaxKind` for the full list.

```rust
enum SyntaxKind {
    // Roots
    SourceFile,
    
    // POUs
    Program,
    Function,
    FunctionBlock,
    Method,
    Property,
    Interface,
    Namespace,
    
    // Declarations
    VarBlock,
    VarDecl,
    TypeDecl,
    StructDef,
    UnionDef,
    EnumDef,
    ArrayType,
    
    // Statements
    AssignStmt,
    IfStmt,
    CaseStmt,
    ForStmt,
    WhileStmt,
    RepeatStmt,
    ReturnStmt,
    ExitStmt,
    ContinueStmt,
    ExprStmt,
    
    // Expressions
    BinaryExpr,
    UnaryExpr,
    CallExpr,
    IndexExpr,
    FieldExpr,
    DerefExpr,
    ParenExpr,
    AddrExpr,
    SizeOfExpr,
    Literal,
    NameRef,
    
    // Other
    Name,
    TypeRef,
    ParamList,
    ArgList,
    CaseBranch,
    ElsifBranch,
    
    // Trivia (from lexer)
    Whitespace,
    LineComment,
    BlockComment,
    
    // Special
    Error,
}
```

#### 4.3 Grammar (EBNF-like)

```ebnf
(* Top Level *)
source_file = { pou_declaration | type_declaration } ;

pou_declaration = program | function | function_block | interface | namespace ;

program = 'PROGRAM' name { var_block } { statement } 'END_PROGRAM' ;

function = 'FUNCTION' name ':' type_ref { var_block } { statement } 'END_FUNCTION' ;

function_block = 'FUNCTION_BLOCK' name [ extends ] [ implements ]
                 { var_block } { method | property } { statement }
                 'END_FUNCTION_BLOCK' ;

interface = 'INTERFACE' name [ extends ] { method_signature | property_signature } 'END_INTERFACE' ;

namespace = 'NAMESPACE' name { pou_declaration | type_declaration | namespace } 'END_NAMESPACE' ;

method = [ access_modifier ] 'METHOD' name [ ':' type_ref ]
         { var_block } { statement } 'END_METHOD' ;

property = [ access_modifier ] 'PROPERTY' name ':' type_ref
           [ 'GET' { statement } 'END_GET' ]
           [ 'SET' { statement } 'END_SET' ]
           'END_PROPERTY' ;

(* Variable Declarations *)
var_block = var_kind { var_decl } 'END_VAR' ;

var_kind = 'VAR' | 'VAR_INPUT' | 'VAR_OUTPUT' | 'VAR_IN_OUT'
         | 'VAR_TEMP' | 'VAR_GLOBAL' | 'VAR_EXTERNAL' ;

var_decl = name { ',' name } ':' type_ref [ ':=' expression ] ';' ;

(* Type Declarations *)
type_declaration = 'TYPE' name ':' type_def ';' 'END_TYPE' ;

type_def = struct_def | enum_def | array_def | alias_def ;

struct_def = 'STRUCT' { var_decl } 'END_STRUCT' ;

enum_def = '(' enum_value { ',' enum_value } ')' [ type_ref ] ;

array_def = 'ARRAY' '[' range { ',' range } ']' 'OF' type_ref ;

(* Statements *)
statement = assign_stmt | expr_stmt | if_stmt | case_stmt | for_stmt
          | while_stmt | repeat_stmt | return_stmt | exit_stmt
          | continue_stmt | ';' ;

assign_stmt = variable ':=' expression ';' ;

expr_stmt = expression ';' ;

if_stmt = 'IF' expression 'THEN' { statement }
          { 'ELSIF' expression 'THEN' { statement } }
          [ 'ELSE' { statement } ]
          'END_IF' ;

case_stmt = 'CASE' expression 'OF'
            { case_branch }
            [ 'ELSE' { statement } ]
            'END_CASE' ;

case_branch = case_label { ',' case_label } ':' { statement } ;

for_stmt = 'FOR' name ':=' expression 'TO' expression [ 'BY' expression ]
           'DO' { statement } 'END_FOR' ;

while_stmt = 'WHILE' expression 'DO' { statement } 'END_WHILE' ;

repeat_stmt = 'REPEAT' { statement } 'UNTIL' expression 'END_REPEAT' ;

return_stmt = 'RETURN' [ expression ] ';' ;

exit_stmt = 'EXIT' ';' ;

continue_stmt = 'CONTINUE' ';' ;

(* Expressions - Pratt Parsing *)
expression = unary_expr | binary_expr | primary_expr ;

binary_expr = expression binop expression ;

unary_expr = unop expression ;

primary_expr = literal | name_ref | paren_expr | call_expr | index_expr | field_expr
             | addr_expr | sizeof_expr ;

paren_expr = '(' expression ')' ;

call_expr = name_ref '(' [ arg_list ] ')' ;

index_expr = expression '[' expression { ',' expression } ']' ;

field_expr = expression '.' name ;

addr_expr = 'ADR' '(' expression ')' ;

sizeof_expr = 'SIZEOF' '(' (type_ref | expression) ')' ;

(* Operator Precedence - Pratt Parser *)
(* 1. OR (lowest)
   2. XOR
   3. AND, &
   4. =, <>
   5. <, >, <=, >=
   6. +, -
   7. *, /, MOD
   8. ** (power, right associative)
   9. NOT, unary -, unary + (highest) *)
```

Typed literal prefixes (`IDENT#`) are parsed as literals, for example `INT#16#FF`,
`BOOL#TRUE`, and `MyEnum#Value`. Interfaces and namespaces accept empty bodies,
for example `INTERFACE I END_INTERFACE` or `NAMESPACE N END_NAMESPACE`.

#### 4.4 Error Recovery Strategy

1. **Synchronization Tokens**: On error, skip to next synchronization point
   - Statement level: `;`, `END_IF`, `END_FOR`, etc.
   - Declaration level: `VAR`, `END_VAR`, `FUNCTION_BLOCK`, etc.

2. **Missing Token Insertion**: Insert expected tokens when unambiguous
   - Missing `;` when the next token starts a new statement, closes a block
     (`ELSIF`, `ELSE`, `UNTIL`, `END_*`), or begins a CASE label
   - Missing `END_*` when the next token is a synchronization point or EOF

3. **Error Node Creation**: Wrap invalid sequences in `Error` nodes

#### 4.5 CST Properties

- **Lossless**: All source text recoverable from CST
- **Immutable**: Trees are immutable, edits create new trees
- **Green/Red Pattern**: Shared green nodes, position-aware red nodes

---

### 5. Semantic Analysis

#### 5.1 Symbol Table Structure

```rust
struct SymbolTable {
    scopes: Vec<Scope>,
    symbols: HashMap<SymbolId, Symbol>,
}

struct Scope {
    parent: Option<ScopeId>,
    symbols: HashMap<Name, SymbolId>,
    kind: ScopeKind,
}

enum ScopeKind {
    Global,
    Namespace(Name),
    Program(Name),
    Function(Name),
    FunctionBlock(Name),
    Method(Name),
    Property(Name),
    Block,  // IF, FOR, etc.
}

struct Symbol {
    name: Name,
    kind: SymbolKind,
    type_: TypeId,
    visibility: Visibility,
    location: TextRange,
}

enum SymbolKind {
    Variable { qualifier: VarQualifier },
    Constant,
    Function { params: Vec<ParamId>, return_type: TypeId },
    FunctionBlock { members: Vec<SymbolId> },
    Method { params: Vec<ParamId>, return_type: Option<TypeId> },
    Property { get: bool, set: bool },
    Type,
    EnumValue { value: i64 },
}
```

#### 5.2 Type System

```rust
enum Type {
    // Elementary
    Bool,
    SInt, Int, DInt, LInt,
    USInt, UInt, UDInt, ULInt,
    Real, LReal,
    Byte, Word, DWord, LWord,
    String { max_len: Option<u32> },
    WString { max_len: Option<u32> },
    Time, LTime, Date, Tod, LTod, Dt, Ldt,
    
    // Compound
    Array { element: TypeId, dimensions: Vec<Range> },
    Struct { fields: Vec<(Name, TypeId)> },
    Enum { base: TypeId, values: Vec<(Name, i64)> },
    Pointer { target: TypeId },
    Reference { target: TypeId },
    
    // User-defined
    Alias { name: Name, target: TypeId },
    FunctionBlock { name: Name },
    Interface { name: Name },
    
    // Generic
    Any, AnyInt, AnyReal, AnyNum, AnyBit, AnyString, AnyDate,
    
    // Special
    Void,
    Error,
}
```

#### 5.3 Type Checking Rules

##### 5.3.1 Assignment Compatibility

| Target | Source | Compatible |
|--------|--------|------------|
| INT | SINT | ✅ (widening) |
| SINT | INT | ⚠️ (warning) |
| REAL | INT | ✅ (promotion) |
| INT | REAL | ❌ |
| BOOL | INT | ❌ |
| STRING | WSTRING | ❌ |
| ARRAY[1..10] | ARRAY[1..5] | ❌ |

##### 5.3.2 Operator Type Rules

| Operator | Operand Types | Result Type |
|----------|---------------|-------------|
| `+`, `-`, `*`, `/` | ANY_NUM | Wider of operands |
| `MOD` | ANY_INT | Wider of operands |
| `**` | ANY_REAL | LREAL |
| `=`, `<>` | Compatible | BOOL |
| `<`, `>`, `<=`, `>=` | ANY_NUM, STRING | BOOL |
| `AND`, `OR`, `XOR` | ANY_BIT, BOOL | Wider of operands |
| `NOT` | ANY_BIT, BOOL | Same as operand |

#### 5.4 Name Resolution

1. Check current scope
2. Check parent scopes (lexical)
3. Check imported namespaces
4. Check global scope

#### 5.5 Salsa Queries

```rust
#[salsa::query_group(SemanticDatabaseStorage)]
trait SemanticDatabase: SyntaxDatabase {
    fn symbols(&self, file: FileId) -> Arc<SymbolTable>;
    fn resolve_name(&self, file: FileId, pos: TextSize) -> Option<SymbolId>;
    fn type_of(&self, file: FileId, expr: ExprId) -> TypeId;
    fn diagnostics(&self, file: FileId) -> Arc<Vec<Diagnostic>>;
}
```

---

### 6. IDE Features

#### 6.1 Completion

##### 6.1.1 Trigger Points

- After `.` - member completion
- After `:` - type completion
- After `(` / inside call arguments - parameter-name completions for formal calls (`name :=` / `name =>`) with direction-aware binding (IEC 61131-3 Ed.3, 6.6.1.4.2; Table 50/71)
- After typed literal prefixes (`T#`, `DATE#`, `TOD#`, `DT#`, etc.) - range-aware typed literal snippets with format hints (IEC 61131-3 Ed.3, 6.1.5; Tables 5-9)
- Start of line - statement/keyword completion
- After `VAR` etc. - variable name suggestions

##### 6.1.2 Completion Kinds

| Context | Suggestions |
|---------|-------------|
| After `.` on FB | Properties, methods |
| After `.` on STRUCT | Fields |
| After `:` | Types in scope |
| Call arguments | Formal parameter names + in-scope expressions (IEC 61131-3 Ed.3, 6.6.1.4.2; Table 50/71) |
| Statement start | Keywords, variables, standard functions (IEC 61131-3 Ed.3, Tables 22-36) |
| Expression | Variables, literals, standard functions/FBs with IEC docs (IEC 61131-3 Ed.3, Tables 22-36, 43-46) |

#### 6.2 Diagnostics

Diagnostics are delivered via both push (`textDocument/publishDiagnostics`) and pull (`textDocument/diagnostic`, `workspace/diagnostic`) APIs. Pull diagnostics return stable `resultId` values derived from content + diagnostic hashes, allowing unchanged responses when the client supplies the previous ID. On configuration/profile changes or workspace file updates, the server requests a refresh (`workspace/diagnostic/refresh`) when supported.
Warning diagnostics can be filtered via `[diagnostics]` configuration; rule packs can preconfigure defaults and severity overrides can promote warning codes to errors. Vendor profiles may adjust defaults to mirror tooling expectations (e.g., CASE/implicit conversion warnings per IEC 61131-3 Ed.3 §7.3.3.3.3 and §6.4.2).
Project configuration diagnostics are reported for `trust-lsp.toml` to flag library dependency issues (missing libraries or version mismatches).
External diagnostics can be merged from `[diagnostics].external_paths` JSON files, and optional fix payloads are exposed as quick-fix code actions.

##### 6.2.1 Syntax Errors

- Missing tokens
- Unexpected tokens
- Unclosed blocks

##### 6.2.2 Semantic Errors

- Undefined variable
- Type mismatch
- Duplicate declaration
- Invalid assignment target
- Missing return statement
- Task configuration errors (missing/invalid PRIORITY) and unknown task references in PROGRAM configs (IEC 61131-3 Ed.3 §6.2; §6.8.2; Table 62)

##### 6.2.3 Warnings

- Unused variable
- Unused parameter
- Missing ELSE in CASE (IEC 61131-3 Ed.3, 7.3.3.3.3)
- Implicit type conversion (IEC 61131-3 Ed.3, 6.4.2)
- Non-determinism checks for time/date usage and direct I/O bindings (tooling lint; IEC 61131-3 Ed.3 §6.4.2 Table 10; §6.5.5 Table 16)
- Shared global access across tasks with writes (tooling lint; IEC 61131-3 Ed.3 §6.5.2.2 Tables 13–16; §6.2/§6.8.2 Table 62)

##### 6.2.4 Diagnostic Explainability

When a diagnostic is mapped to an IEC reference, the LSP payload includes:
- `codeDescription.href` → file URL to the relevant `docs/specs/*.md` (when present in the workspace)
- `data.explain` → `{ iec: "...", spec: "docs/specs/..." }`

Initial explainer coverage:

| Codes | IEC reference | Spec doc |
|------|---------------|----------|
| E001–E003 | IEC 61131-3 Ed.3 §7.3 | `docs/specs/06-statements.md` |
| E101/E104/E105/W001/W002/W006 | IEC 61131-3 Ed.3 §6.5.2.2 | `docs/specs/09-semantic-rules.md` |
| E102 | IEC 61131-3 Ed.3 §6.2 | `docs/specs/02-data-types.md` |
| E103/E204/E205/E206/E207 | IEC 61131-3 Ed.3 §6.6.1 | `docs/specs/04-pou-declarations.md` |
| E106 | IEC 61131-3 Ed.3 §6.1.2 | `docs/specs/01-lexical-elements.md` |
| E201/E202/E203 | IEC 61131-3 Ed.3 §7.3.2 | `docs/specs/05-expressions.md` |
| E301/E302 | IEC 61131-3 Ed.3 §7.3.1 | `docs/specs/09-semantic-rules.md` |
| E303/E304 | IEC 61131-3 Ed.3 §6.2.6 | `docs/specs/02-data-types.md` |
| W004 | IEC 61131-3 Ed.3 §7.3.3.3.3 | `docs/specs/06-statements.md` |
| W005 | IEC 61131-3 Ed.3 §6.4.2 | `docs/specs/02-data-types.md` |
| W008/W009 | Tooling quality lint (non-IEC) | `docs/specs/09-semantic-rules.md` |
| W010 | Tooling lint; TIME/DATE types per IEC 61131-3 Ed.3 §6.4.2 (Table 10) | `docs/specs/09-semantic-rules.md` |
| W011 | Tooling lint; Direct variables per IEC 61131-3 Ed.3 §6.5.5 (Table 16) | `docs/specs/09-semantic-rules.md` |
| W012 | Tooling lint; shared global access across tasks (IEC 61131-3 Ed.3 §6.5.2.2 Tables 13–16; §6.2/§6.8.2 Table 62) | `docs/specs/09-semantic-rules.md` |
| L001–L003 | Tooling config lint (non-IEC) | `docs/specs/10-runtime.md` |

For access-specifier violations reported under E202 (e.g., PRIVATE/PROTECTED/INTERNAL access),
the explainer is mapped to IEC 61131-3 Ed.3 §6.6.5 (Table 50) in `docs/specs/09-semantic-rules.md`.

Diagnostics without a mapping return only `code` + `message` until their IEC references are added.

#### 6.3 Navigation

##### 6.3.1 Go to Definition

- Variables → declaration
- `VAR_EXTERNAL` resolves to the matching `VAR_GLOBAL` across the workspace (IEC 61131-3 Ed.3, §6.5.2.2; Tables 13–16)
- Types → type definition
- Methods → method definition
- Properties → property definition

##### 6.3.2 Find References

- All usages of a symbol
- Include/exclude declaration
- Filter by read/write

##### 6.3.3 Document Symbols

- Flat list of declarations

#### 6.4 Refactoring

##### 6.4.1 Rename

- All references updated (workspace-wide)
- Preview changes
- Namespace path moves via dotted rename or refactor action (updates namespace declarations, `USING`, qualified names, and namespace-qualified field access; relocation across files moves the namespace block to a derived target file and removes the source file when empty; default target path maps `Namespace.Path` → `<workspace>/Namespace/Path.st` unless an explicit URI is provided) (IEC 61131-3 Ed.3, 6.6.4; Tables 64-66)
- VS Code surfaces namespace relocation via `Structured Text: Move Namespace`, prompting for the new path and optional target file (invokes `trust-lsp.moveNamespace`) (IEC 61131-3 Ed.3, 6.6.4; Tables 64-66)

##### 6.4.2 Code Actions / Quick Fixes

- Create missing VAR declarations for undefined identifiers (IEC 61131-3 Ed.3, 6.5.3; Tables 13-14)
- Create missing TYPE definitions for undefined types (IEC 61131-3 Ed.3, 6.5.2; Table 11)
- Insert missing END_* blocks (IEC 61131-3 Ed.3, 7.3; Table 72)
- Insert missing RETURN in FUNCTION (IEC 61131-3 Ed.3, 7.3.3.3.2; Table 72)
- Convert formal ↔ positional call style (IEC 61131-3 Ed.3, 6.6.1.4.2; Table 50)
- Reorder mixed calls to positional-first argument order (IEC 61131-3 Ed.3, 6.6.1.4.2; Table 50)
- Move namespace path (refactor action invoking rename UI) (IEC 61131-3 Ed.3, 6.6.4; Tables 64-66)
- Move namespace path (execute command; relocates declarations across files) (IEC 61131-3 Ed.3, 6.6.4; Tables 64-66)
- Move namespace quick fix (VS Code lightbulb on `NAMESPACE`/`USING` lines; invokes `trust-lsp.moveNamespace` via UI command) (IEC 61131-3 Ed.3, 6.6.4; Tables 64-66)
- Qualify ambiguous namespace references when multiple USING directives apply (IEC 61131-3 Ed.3, 6.6.4; Tables 64-66)
- Fix VAR_OUTPUT binding operators / add missing OUT bindings (IEC 61131-3 Ed.3, 6.6.1.2.2; Table 71)
- Wrap implicit conversions using standard conversion functions (IEC 61131-3 Ed.3, Tables 22–27)
- Generate stub implementations for missing interface methods/properties from IMPLEMENTS clauses (IEC 61131-3 Ed.3, 6.6.5–6.6.6; Tables 50–51)
- Inline variable/constant with safety checks (const-expression analysis, no writes, cross-file constants when safe) (IEC 61131-3 Ed.3, 6.5.1–6.5.2; Tables 13–14)
- Extract method/property/function from a selection (method/property in CLASS/FB, function in POU body) with inferred VAR_INPUT/VAR_IN_OUT parameters; expression selections extract a FUNCTION returning the inferred expression type (IEC 61131-3 Ed.3, 6.6.5; Table 50 for methods/properties; 6.6.2.2; Table 19 for functions)
- Convert FUNCTION ↔ FUNCTION_BLOCK with safe call-site updates (supports qualified names and assignment/return expression sites; no recursive calls; FUNCTION→FB requires no existing VAR_OUTPUT when a return type is present; FB→FUNCTION requires a single VAR_OUTPUT and no type references/instances) (IEC 61131-3 Ed.3, 6.6.2.2; Table 19 and 6.6.3.2; Table 40)
- Remove unused variables/parameters

##### 6.4.3 Future

- Change signature

#### 6.5 Hover Information

Hover content includes:
- Symbol signature + visibility/modifiers (IEC 61131-3 Ed.3, 6.6.5; Table 50)
- Standard function/FB documentation (IEC 61131-3 Ed.3, Tables 22–36, 43–46)
- Namespace/USING resolution details (IEC 61131-3 Ed.3, 6.6.4; Tables 64–66)
- Typed literal guidance for TIME/DATE/TOD/DT prefixes (IEC 61131-3 Ed.3, 6.1.5; Tables 5–9)
- Configuration/Resource/Task declarations show task scheduling inputs and program bindings (IEC 61131-3 Ed.3 §6.2; §6.8.2; Table 62)

```
motorSpeed : REAL
───────────────────
Variable (VAR_INPUT)
Declared in: FB_Motor

The target speed for the motor in RPM.
Range: 0.0 to 3000.0
```

---

### 7. LSP Protocol

#### 7.1 Supported Capabilities

| Capability | Method | Status | Notes |
|------------|--------|--------|-------|
| Text Sync | `textDocument/didOpen`, etc. | ✅ | Incremental sync with full-change fallback |
| Diagnostics | `textDocument/publishDiagnostics` | ✅ | Parse + semantic diagnostics (undefined names, type mismatch, invalid assignments) |
| Pull Diagnostics | `textDocument/diagnostic` | ✅ | Per-file result IDs; unchanged when `previousResultId` matches |
| Workspace Diagnostics | `workspace/diagnostic` | ✅ | Full/unchanged reports per document across indexed workspace |
| Diagnostics Refresh | `workspace/diagnostic/refresh` | ✅ | Server requests refresh on config/profile or workspace changes (client-supported) |
| Completion | `textDocument/completion` | ✅ | Scope-aware + member access + parameter-name completion + standard docs |
| Hover | `textDocument/hover` | ✅ | Shows type + qualifiers |
| Signature Help | `textDocument/signatureHelp` | ✅ | Call signatures with active parameter |
| Definition | `textDocument/definition` | ✅ | Project-wide (workspace indexed; file watching updates) |
| Declaration | `textDocument/declaration` | ✅ | Same target as definition |
| Type Definition | `textDocument/typeDefinition` | ✅ | Type/alias definition lookup |
| Implementation | `textDocument/implementation` | ✅ | Interface implementers (project-wide) |
| References | `textDocument/references` | ✅ | Symbol-aware (workspace indexed; no text fallback); work-done progress + partial results when client provides tokens |
| Document Highlight | `textDocument/documentHighlight` | ✅ | Highlight reads/writes in current document |
| Symbols | `textDocument/documentSymbol` | ✅ | Flat list |
| Workspace Symbols | `workspace/symbol` | ✅ | Multi-root symbol federation with per-root priority/visibility; work-done progress + partial results when client provides tokens |
| File Rename | `workspace/willRenameFiles` | ✅ | Renames single top-level POU/namespace when file stem changes; updates references and USING directives for that namespace (IEC 61131-3 Ed.3, 6.1.2; 6.6.4; Tables 64-66) |
| Rename | `textDocument/rename` | ✅ | Symbol-aware; workspace edits; renames the declaring file when renaming the single primary POU whose identifier matches the file stem (IEC 61131-3 Ed.3, 6.1.2) |
| Semantic Tokens | `textDocument/semanticTokens` | ✅ | Full + range + delta; classified by symbol kind/modifiers |
| Semantic Tokens Refresh | `workspace/semanticTokens/refresh` | ✅ | Server requests refresh on config/profile changes (client-supported) |
| Folding Range | `textDocument/foldingRange` | ✅ | CST-based region folding |
| Selection Range | `textDocument/selectionRange` | ✅ | CST-based hierarchical selection ranges |
| Linked Editing | `textDocument/linkedEditingRange` | ✅ | Identifier-linked ranges in document (IEC 61131-3 Ed.3, 6.1 identifiers) |
| Document Link | `textDocument/documentLink` | ✅ | Links for `USING` directives and `trust-lsp.toml` path entries (IEC 61131-3 Ed.3, 6.6.4; Tables 64-66) |
| Inlay Hints | `textDocument/inlayHint` | ✅ | Parameter-name hints for positional calls (IEC 61131-3 Ed.3, 6.6.1.2.2; Table 71) |
| Inline Values | `textDocument/inlineValue` | ✅ | Constant/enum references show initializer text; runtime values surfaced via debug control for locals/globals/retain when configured (IEC 61131-3 Ed.3, 6.5.1–6.5.2; Tables 13–14) |
| Code Lens | `textDocument/codeLens` | ✅ | Reference count lenses for POU declarations |
| Call Hierarchy | `textDocument/prepareCallHierarchy` | ✅ | Incoming/outgoing call graph for POU declarations |
| Type Hierarchy | `textDocument/prepareTypeHierarchy` | ✅ | Class/FB/interface supertypes + subtypes (IEC 61131-3 Ed.3, 6.6.5) |
| Formatting | `textDocument/formatting` | ✅ | Indentation + spacing + alignment + wrapping (configurable) |
| Range/On-Type Formatting | `textDocument/rangeFormatting`, `textDocument/onTypeFormatting` | ✅ | Line-based formatting using document formatter |
| Configuration | `workspace/didChangeConfiguration` | ✅ | Settings stored (formatting/indexing); project config file is separate |
| Code Actions | `textDocument/codeAction` | ✅ | Quick fixes for unused symbols, missing END_* / RETURN, call style conversion, namespace disambiguation, implicit conversion, etc. |
| Execute Command | `workspace/executeCommand` | ✅ | `trust-lsp.moveNamespace` for namespace relocation across files (IEC 61131-3 Ed.3, 6.6.4; Tables 64-66); `trust-lsp.projectInfo` surfaces build flags, targets, and library dependency graph |

#### 7.2 Document Synchronization

- Incremental sync using `TextDocumentContentChangeEvent` ranges.
- Full-document replacement is supported when the change range is omitted.

#### 7.3 Semantic Token Types

| Token Type | Usage |
|------------|-------|
| keyword | All ST keywords |
| type | Type names |
| variable | Variable names |
| property | Property names |
| method | Method names |
| function | Function names |
| parameter | Parameter names |
| number | Numeric literals |
| string | String literals |
| comment | Comments |
| operator | Operators |

#### 7.4 Semantic Token Modifiers

| Modifier | Usage |
|----------|-------|
| declaration | At declaration site |
| definition | At definition site |
| readonly | CONSTANT variables |
| static | VAR_STAT variables |
| modification | Write to variable |

#### 7.5 Formatting

- Indentation and token-based spacing normalization (operators/separators).
- VAR block `:` alignment across declarations.
- Assignment alignment for `:=` and `=>` within aligned blocks (range formatting expands to align pasted statement lists).
- Keyword casing (upper/lower/preserve), spacing style (spaced/compact), end keyword indentation (aligned/indented), and max line length are configurable; `vendor_profile` presets default indent width, spacing, and end keyword style for common IDEs.
- Block comment lines are left unchanged; line comments and pragma lines preserve inline spacing.
- String literal and pragma lines are excluded from assignment alignment and wrapping to preserve lexical content (IEC 61131-3 Ed.3, 6.1; Tables 4–7).
- Line endings are preserved (LF vs CRLF).
- Line-wrapping at commas honors `maxLineLength` and avoids comment/pragma/string lines (IEC 61131-3 Ed.3, 6.1; Tables 4–7).
- Range formatting expands to the nearest syntactic block (e.g., VAR blocks, IF/CASE loops, POU/method/property bodies) to avoid partial-block drift.
- VAR alignment respects manual grouping: blank lines or comment/pragma lines split alignment groups to preserve intentional spacing and comment anchors.
- Formatting config keys: `indentWidth`, `insertSpaces`, `keywordCase`, `spacingStyle`, `endKeywordStyle`, `alignVarDecls`, `alignAssignments`, `maxLineLength`.
- Vendor preset defaults (overrideable via config): `codesys`/`beckhoff`/`twincat` use 4-space indents with spaced operators; `siemens` uses 2-space indents with compact operator spacing; all align `END_*` keywords by default.

#### 7.6 Project Configuration & Workspace Indexing

- Per-root project config file: `trust-lsp.toml`, `.trust-lsp.toml`, or `trustlsp.toml`.
- `[project]` supports `include_paths`, `library_paths`, `vendor_profile` (dialect + formatting presets), and `stdlib` selection.
- `stdlib` profiles: `full` (default), `iec` (IEC standard functions/FBs only; Tables 22–36, 43–46), `none` (no standard library completions/hover), or an explicit allow-list array.
- When `vendor_profile` is set and no explicit stdlib allow-list/profile is provided, the server defaults to the IEC profile for completions/hover.
- `[[libraries]]` entries include `name`, `path`, and optional `version` for external library indexing.
- `[dependencies]` supports local package references (`Name = "path"` or `Name = { path = "...", version? = "..." }`) with transitive resolution for indexing and runtime build input assembly.
- `[[libraries]]` can declare `dependencies` (array of `{ name, version? }`) to model library graphs; missing dependencies or version mismatches are reported as config diagnostics.
- Library/dependency graphs report missing references (L001), version mismatches (L002), conflicting declarations (L003), and dependency cycles (L004).
- `[[libraries]]` can declare `docs` (array of markdown files) to attach vendor library documentation to hover/completion. Each file uses `# SymbolName` headings followed by doc text.
- `[workspace]` controls multi-root federation: `priority` orders root results for workspace symbol search, and `visibility` (`public`, `private`, `hidden`) filters which roots participate when querying (private roots only appear for non-empty queries) (tooling behavior, non-IEC).
- `[build]` exposes project compile flags (`flags`), `defines`, and optional `target`/`profile` defaults.
- `[[targets]]` describes target profiles (`name`, `profile`, `flags`, `defines`) surfaced to LSP clients for toolchain selection.
- `[indexing]` budgets (`max_files`, `max_ms`) bound large workspace indexing.
- `[indexing]` cache options: `cache` (default true) enables persistent index caching across sessions; `cache_dir` overrides the cache location. Cache reuse checks file metadata and stored content hashes.
- `[indexing]` memory budget controls: `memory_budget_mb` caps closed-document index memory (MB) and `evict_to_percent` defines the LRU eviction target; evicted documents are reloaded on demand when accessed.
- `[indexing]` adaptive throttling: `throttle_idle_ms`, `throttle_active_ms`, `throttle_max_ms`, and `throttle_active_window_ms` pace background indexing based on recent editor activity and observed per-file work.
- `[runtime]` supports `control_endpoint` and optional `control_auth_token` for debug-assisted inline values.
- `[diagnostics]` toggles warning categories (`warn_unused`, `warn_unreachable`, `warn_missing_else`, `warn_implicit_conversion`, `warn_shadowed`, `warn_deprecated`, `warn_complexity`, `warn_nondeterminism`) for vendor-dialect alignment (IEC 61131-3 Ed.3 §6.4.2; §7.3.3.3.3). Cyclomatic complexity warnings (W008) use a default threshold of 15; unused warnings (W001/W002/W009) cover variables, parameters, and top-level POUs.
- `[diagnostics].rule_pack` presets safety-focused defaults (e.g., `iec-safety`, `siemens-safety`, `codesys-safety`, `beckhoff-safety`, `twincat-safety`); explicit `warn_*` keys override pack defaults. `[diagnostics].severity_overrides` can promote specific warning codes to error severity (W004 missing ELSE per IEC 61131-3 Ed.3 §7.3.3.3.3; W005 implicit conversion per §6.4.2; W010 TIME/DATE nondeterminism per §6.4.2; W011 direct variables per §6.5.5).
- `[diagnostics].external_paths` lists JSON diagnostics payloads from external linters (optional per-diagnostic fix data yields quick-fix actions).
- Vendor diagnostic defaults: `siemens` disables Missing ELSE (W004) and implicit conversion (W005); `codesys`, `beckhoff`, and `twincat` keep all warning categories enabled unless overridden in `[diagnostics]`.
- `[telemetry]` (opt-in) records aggregated feature usage + latency to JSONL (`enabled`, `path`, `flush_every`); payloads include event names and durations only (tooling behavior, non-IEC).
- Indexing progress is reported via `window/workDoneProgress` when supported by the client.
- Workspace indexing runs in the background; adaptive throttling yields between files to keep interactive edits responsive (tooling behavior, non-IEC).
- Stdlib selection currently filters standard function/FB docs and completions (IEC 61131-3 Ed.3, Tables 22–36, 43–46).

---

### 8. Runtime & Debugger

The workspace includes a runtime and debug adapter used for executing and testing ST programs.
The authoritative specifications for these components are:

- `docs/specs/10-runtime.md`

#### 8.1 Runtime

- Runtime execution is defined by the ST runtime specification, including task scheduling,
  process image semantics, retain behavior, and fault handling.
- Production runtimes are started via the CLI using the project folder (runtime bundle format)
  format (`trust-runtime` or `trust-runtime run --project`). Project folders can be generated by
  `trust-runtime build` (preferred) or CI tooling that emits STBC.

#### 8.2 Debugger

- Debug adapter behavior follows DAP and the ST debugger specification.
- Breakpoints and stepping are statement-based and use source locations from the compiler.

---

### 9. Error Handling

#### 9.1 Error Categories

```rust
enum DiagnosticSeverity {
    Error,      // Prevents compilation
    Warning,    // Potential issue
    Info,       // Informational
    Hint,       // Style suggestion
}

struct Diagnostic {
    range: TextRange,
    severity: DiagnosticSeverity,
    code: DiagnosticCode,
    message: String,
    related: Vec<RelatedInfo>,
}
```

#### 9.2 Error Codes

| Code | Category | Description |
|------|----------|-------------|
| E001 | Syntax | Unexpected token |
| E002 | Syntax | Missing token |
| E003 | Syntax | Unclosed block |
| E101 | Name | Undefined variable |
| E102 | Name | Duplicate declaration |
| E103 | Name | Cannot resolve type |
| E201 | Type | Type mismatch |
| E202 | Type | Invalid operation |
| E203 | Type | Incompatible assignment |
| W001 | Warning | Unused variable |
| W002 | Warning | Unreachable code |
| W003 | Warning | Implicit conversion |

---

### 10. Performance Requirements

#### 10.1 Latency Targets

| Operation | Target | Maximum |
|-----------|--------|---------|
| Keystroke response | < 16ms | 50ms |
| Completion list | < 50ms | 200ms |
| Go to definition | < 20ms | 100ms |
| Find references | < 100ms | 500ms |
| Full file diagnostics | < 200ms | 1000ms |

#### 10.2 Memory Targets

| Metric | Target |
|--------|--------|
| Per-file overhead | < 10x source size |
| Idle memory | < 100MB |
| Large project (100 files) | < 500MB |

#### 10.3 Optimization Strategies

1. **Incremental parsing** - Salsa invalidation on changed files (file-level granularity)
2. **Cross-file dependency tracking** - Salsa queries recompute only affected dependents
3. **Indexing budgets** - `max_files` / `max_ms` limits for large workspaces
4. **Expression-level type cache** - Cache `type_of` results by expression hash + scope, invalidated when symbol tables change
5. **Adaptive background indexing** - Per-file throttling based on recent editor activity and observed indexing cost
6. **Memory budgets** - Closed-document eviction with on-demand reload when over memory budget
7. **Progress reporting** - Work-done progress notifications during indexing
8. **Request prioritization** - Background workspace scans (`workspace/symbol`, `workspace/diagnostic`, cross-file references) are concurrency-limited to keep interactive requests responsive

---

### 11. Testing Strategy

#### 11.1 Test Categories

##### 11.1.1 Unit Tests

- Lexer token output
- Parser tree structure
- Type checker rules
- Symbol resolution

##### 11.1.2 Integration Tests

- Full file parsing
- Cross-file references
- LSP protocol compliance + golden handler responses
- Performance harness (ignored by default): hover/completion/rename budgets + large workspace indexing (Section 10 targets)
- VS Code extension integration tests for completion, formatting, and code actions (IEC 61131-3 Ed.3 §6.1-6.3; Tables 4-9; §6.5.2.2)
- Stdlib coverage check: ensures all IEC standard function/FB names appear in `docs/specs/coverage/standard-functions-coverage.md` (IEC 61131-3 Ed.3, Tables 22–36, 43–46)

##### 11.1.3 Snapshot Tests

- Parser output (insta)
- Diagnostic output
- Completion / signature / formatting results

#### 11.2 Test Corpus

```
tests/corpus/
├── declarations/
│   ├── variables.st
│   ├── types.st
│   └── functions.st
├── expressions/
│   ├── arithmetic.st
│   ├── logical.st
│   └── comparison.st
├── statements/
│   ├── if.st
│   ├── case.st
│   ├── for.st
│   └── while.st
├── function_blocks/
│   ├── basic.st
│   ├── inheritance.st
│   └── interfaces.st
└── errors/
    ├── syntax_errors.st
    └── type_errors.st
```

#### 11.3 Fuzzing

- AFL/libFuzzer for parser robustness
- Grammar-aware fuzzing for valid-ish input

#### 11.4 Benchmarks

- Large file parsing (10K+ lines)
- Completion response time
- Memory usage under load

---

### 12. Current Implementation Status

#### 12.1 What's Implemented

- **Lexer**: Complete token set for IEC 61131-3 ST
- **Parser**: ST constructs parsed per specs (including ACTION blocks and AT addresses)
- **Symbol Table**: Scope-aware with namespaces and cross-file resolution
- **Type Registry**: Elementary, generic, and user-defined types (STRUCT/UNION/ENUM/ARRAY/STRING[n])
- **Hover**: Full implementation with type and qualifier display
- **Go to Definition**: Project-wide navigation (workspace indexed)
- **Document Symbols**: Flat list of declarations
- **Debugger (DAP)**: Core DAP adapter with breakpoints, stepping, scopes, variables, evaluate, logpoints

#### 12.2 Known Limitations

1. **Workspace**
   - Workspace indexing runs on initialize; on-disk changes are tracked via file watching when supported by the client (otherwise require reload)

2. **LSP**
   - Formatting does not wrap/reflow lines beyond operator spacing + VAR alignment
   - Folding ranges are coarse (node-based regions)
3. **Debugger**
   - VS Code extension wiring exists, but the manual test plan is still pending
   - Stepping is statement-level only; expressions are not single-stepped
   - Debug evaluation is restricted to side-effect-free expressions and a small pure stdlib whitelist
   - Hot reload is implemented via a custom request and is limited to single-file reloads with retained globals only (see DEV-024/DEV-025)
   - I/O simulation panel is implemented but supports inputs only (no output forcing)


---

### Appendix A: IEC 61131-3 Operator Precedence

| Precedence | Operators | Associativity |
|------------|-----------|---------------|
| 1 (lowest) | OR | Left |
| 2 | XOR | Left |
| 3 | AND, & | Left |
| 4 | =, <> | Left |
| 5 | <, >, <=, >= | Left |
| 6 | +, - | Left |
| 7 | *, /, MOD | Left |
| 8 | ** | Right |
| 9 (highest) | NOT, -(unary), +(unary) | Right |

---

### Appendix B: Type Hierarchy

```
ANY
├── ANY_DERIVED
│   ├── ANY_ELEMENTARY
│   │   ├── ANY_MAGNITUDE
│   │   │   ├── ANY_NUM
│   │   │   │   ├── ANY_REAL
│   │   │   │   │   ├── REAL
│   │   │   │   │   └── LREAL
│   │   │   │   └── ANY_INT
│   │   │   │       ├── ANY_SIGNED
│   │   │   │       │   ├── SINT
│   │   │   │       │   ├── INT
│   │   │   │       │   ├── DINT
│   │   │   │       │   └── LINT
│   │   │   │       └── ANY_UNSIGNED
│   │   │   │           ├── USINT
│   │   │   │           ├── UINT
│   │   │   │           ├── UDINT
│   │   │   │           └── ULINT
│   │   │   └── ANY_DURATION
│   │   │       ├── TIME
│   │   │       └── LTIME
│   │   ├── ANY_BIT
│   │   │   ├── BOOL
│   │   │   ├── BYTE
│   │   │   ├── WORD
│   │   │   ├── DWORD
│   │   │   └── LWORD
│   │   ├── ANY_STRING
│   │   │   ├── STRING
│   │   │   └── WSTRING
│   │   ├── ANY_DATE
│   │   │   ├── DATE
│   │   │   └── LDATE
│   │   └── ANY_DATE_AND_TIME
│   │       ├── DT
│   │       └── LDT
│   └── USER_DEFINED
│       ├── STRUCT
│       ├── ENUM
│       ├── ARRAY
│       └── FUNCTION_BLOCK
└── ANY_POINTER
    ├── POINTER TO ...
    └── REF_TO ...
```

---

### Appendix C: PLCopen XML Interchange (Strict Subset)

Runtime exposes a strict PLCopen XML profile through `trust-runtime plcopen`:

- `trust-runtime plcopen profile` prints the supported profile contract.
- `trust-runtime plcopen export` exports ST POUs to PLCopen XML.
- `trust-runtime plcopen import` imports supported PLCopen POUs into `sources/`.

Current strict subset contract:

- Namespace: `http://www.plcopen.org/xml/tc6_0200`
- Profile: `trust-st-strict-v1`
- Supported POU body: `ST` text bodies for `PROGRAM`, `FUNCTION`, `FUNCTION_BLOCK`
- Source mapping: embedded `addData` payload + sidecar `*.source-map.json`
- Unsupported nodes: reported as warnings and preserved via vendor extension hooks

### Appendix D: References

1. IEC 61131-3:2013 - Programmable controllers - Part 3: Programming languages
2. PLCopen - Technical Committee 6 (XML)
3. CODESYS Online Help - https://help.codesys.com
4. Beckhoff InfoSys - https://infosys.beckhoff.com
5. rust-analyzer Architecture - https://github.com/rust-lang/rust-analyzer/blob/master/docs/dev/architecture.md
