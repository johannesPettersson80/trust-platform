# Program Organization Unit Declarations

IEC 61131-3 Edition 3.0 (2013) - Section 6.6

This specification defines POU declarations for trust-hir.

## 1. Overview

Program Organization Units (POUs) are the building blocks of IEC 61131-3 programs:

| POU Type | Keyword | Instances | State | Return Value |
|----------|---------|-----------|-------|--------------|
| Function | `FUNCTION` | N/A (call) | No | Optional |
| Function Block | `FUNCTION_BLOCK` | Yes | Yes | Via outputs |
| Program | `PROGRAM` | Yes | Yes | Via outputs |
| Class | `CLASS` | Yes | Yes | N/A |
| Interface | `INTERFACE` | N/A | N/A | N/A |
| Method | `METHOD` | N/A | No | Optional |

### Parser acceptance contract

The parser accepts an empty source buffer without parse diagnostics. This
editor-oriented acceptance does not claim a syntax-tree shape or that an empty
project is runnable.

The canonical minimal `PROGRAM name END_PROGRAM` form and a
`FUNCTION_BLOCK` containing a complete `VAR_INPUT ... END_VAR` section and no
body statements parse successfully when their required names and matching
terminators are present. Semantic declaration and project validation remain
separate.

For a POU with top-level executable statements, the parser preserves those
statements under one `StmtList` child of the owning POU. Nested methods,
properties, and actions own their own bodies; their statements are not folded
into the enclosing function-block statement list.

### Implementation Extension: Test POUs

truST accepts `TEST_PROGRAM` and `TEST_FUNCTION_BLOCK` as documented
test-oriented, non-IEC declaration forms.

```
TEST_PROGRAM name
  ...
END_TEST_PROGRAM

TEST_FUNCTION_BLOCK name
  ...
END_TEST_FUNCTION_BLOCK
```

Current parser/HIR behavior:
- `TEST_PROGRAM` is parsed with PROGRAM structure and collected as `SymbolKind::Program`.
- `TEST_FUNCTION_BLOCK` is parsed with FUNCTION_BLOCK structure and collected as `SymbolKind::FunctionBlock`.
- Mismatched end markers produce actionable diagnostics (`expected END_TEST_PROGRAM` / `expected END_TEST_FUNCTION_BLOCK`).
- End-of-file before the matching test-POU terminator produces the same
  actionable required-terminator diagnostic; an unterminated `TEST_PROGRAM`
  reports `expected END_TEST_PROGRAM`.

## 2. FUNCTION Declaration (Table 19, Section 6.6.2)

### Syntax

```
FUNCTION function_name : return_type
  // Variable declarations
  VAR_INPUT ... END_VAR
  VAR_OUTPUT ... END_VAR
  VAR_IN_OUT ... END_VAR
  VAR_EXTERNAL ... END_VAR
  VAR_EXTERNAL CONSTANT ... END_VAR
  VAR ... END_VAR
  VAR_TEMP ... END_VAR
  // Statements
END_FUNCTION
```

### Examples

```
// Function with return value
FUNCTION Square : INT
VAR_INPUT
  X: INT;
END_VAR
  Square := X * X;
END_FUNCTION

// Function without return value (procedure-like)
FUNCTION LogMessage
VAR_INPUT
  Message: STRING;
END_VAR
  // Implementation
END_FUNCTION
```

### Rules (Section 6.6.1.2)

1. **No state retention**: Variables in VAR/VAR_TEMP are re-initialized each call (VAR and VAR_TEMP are equivalent in functions/methods)
2. **Return value**: Assigned through the IEC function-name result variable or
   through the truST value-bearing RETURN extension recorded as `DEV-022`
3. **VAR_IN_OUT and VAR_EXTERNAL**: May be modified inside the function; VAR_EXTERNAL CONSTANT shall not be modified
4. **CONSTANT restriction**: Function block instances shall not be declared in variable sections with CONSTANT qualifier

### POU initializer contract

The variable-initialization rules in IEC 61131-3 Ed.3 §6.5.1.3 and Annex A
apply independently of POU declaration order:

- function and method inputs, outputs, ordinary locals, temporary locals, and
  static locals may carry compatible literal or constant-expression
  initializers;
- function and method ordinary and temporary locals, plus outputs, are
  reinitialized for every call;
- function and method static locals are initialized once and persist, with
  method static storage isolated by receiver;
- function-block inputs, outputs, ordinary variables, and static variables are
  initialized with their instance, while temporary variables are initialized
  for each invocation;
- program inputs, outputs, ordinary variables, and static variables are
  initialized with their program instance, while temporary variables are
  initialized for each cycle; and
- in-out and external declarations do not accept initializers.

Every accepted initializer is resolved from the declaration site's complete
constant graph before POU storage is exposed. Textual section order and project
source order do not change the selected constant, but lexical POU scope,
namespace qualification, and `USING` ambiguity remain binding. Detailed
eligibility, omitted-input, output-copy, and failure rules are specified in
`03-variables.md`; runtime materialization is specified in
`10-runtime-semantics.md` §7.9.

### Variable-section legality

IEC 61131-3 Ed.3 Table 19 and the Annex A function production permit
`VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`, `VAR_EXTERNAL`, `VAR`, and
`VAR_TEMP` in a function. A method has the same variable-section set under the
Annex A method production. truST additionally accepts `VAR_STAT` in both as
the documented persistent-static extension.

Neither owner accepts `VAR_GLOBAL`, `VAR_ACCESS`, or `VAR_CONFIG`. Encountering
one of those sections rejects the complete function or method declaration;
the compiler must not silently discard the section and return a partial POU.

Variable-section ownership and qualifier placement are independent closed
checks. The matrix in `03-variables.md` permits `CONSTANT` on each legal
function/method storage section, but permits restart policy only on truST
`VAR_STAT`; ordinary variables and parameters are reinitialized per call and
cannot own `RETAIN`, `NON_RETAIN`, or `PERSISTENT`. A qualifier never makes an
otherwise forbidden section legal.

### Function Call (Section 6.6.1.7)

| No. | Call Type | Example |
|-----|-----------|---------|
| 1 | Formal call | `Y := Square(X := 5);` |
| 2 | Non-formal call | `Y := Square(5);` |
| 3 | Procedure call | `LogMessage('Hello');` |

IEC 61131-3 Ed.3 section 6.6.1.4.2 defines formal and non-formal parameter
lists as separate call forms. truST also accepts a mixed convenience form when
all positional arguments precede all formal arguments, for example
`Add(1, b := 2)`. This is the intentional extension recorded as
[`DEV-018`](https://github.com/johannesPettersson80/trust-platform/blob/main/docs/IEC_DEVIATIONS.md#2026-07-26---mixed-positional-prefix-and-formal-suffix-calls);
portable source should use one IEC call form throughout.

### EN/ENO Mechanism (Section 6.6.1.6)

`EN` and `ENO` are optional parameters on FUNCTION/FUNCTION_BLOCK declarations and calls:

- `EN`: `BOOL` input with an effective default of `TRUE`
- `ENO`: `BOOL` output initialized to `TRUE` for an enabled call

| EN | Execution | ENO |
|----|-----------|-----|
| FALSE | No other actual or writable target is evaluated; POU body is not executed | FALSE |
| TRUE | POU body executes normally | TRUE unless the POU sets it FALSE |
| TRUE | POU body encounters an execution error | FALSE; call transfer is not committed |

`EN` and `ENO` are execution-control parameters, not ordinary positional
parameters. They may be connected only by name. `EN` uses `:=`; `ENO` uses
`=>`. `EN` is evaluated before every other actual even if written later in the
formal list. A false value short-circuits evaluation and binding of every other
actual. The only caller write made by the skipped call is `FALSE` to a
connected `ENO` target. A skipped function or value-returning method produces
its declared type default; a skipped function block or void method preserves
its instance state.

For an enabled call, `ENO` is set to `TRUE` before the body and may be assigned
only inside that POU. A normally returning body commits its result and connected
outputs even when it explicitly leaves `ENO` false. A runtime execution error
reports the error and commits no result/output/in-out/instance-state transfer.
`REF(EN)` and `REF(ENO)` are invalid. These implementer-specific choices are
recorded in
[`IEC_DECISIONS.md`](https://github.com/johannesPettersson80/trust-platform/blob/main/docs/IEC_DECISIONS.md#2026-07-30---pou-call-evaluation-execution-control-and-output-transfer).

```
FUNCTION SafeDiv : REAL
VAR_INPUT
  EN: BOOL := TRUE;
  Num, Den: REAL;
END_VAR
VAR_OUTPUT
  ENO: BOOL;
END_VAR
  IF Den = 0.0 THEN
    ENO := FALSE;
    SafeDiv := 0.0;
  ELSE
    SafeDiv := Num / Den;
  END_IF;
END_FUNCTION

Result := SafeDiv(EN := Cond, Num := A, Den := B, ENO => Valid);
```

### Return Value (Table 20)

```
FUNCTION Max : INT
VAR_INPUT
  A, B: INT;
END_VAR
  IF A > B THEN
    Max := A;     // Assign to function name
  ELSE
    Max := B;
  END_IF;
END_FUNCTION
```

Or using RETURN:

```
FUNCTION Max : INT
VAR_INPUT
  A, B: INT;
END_VAR
  IF A > B THEN
    RETURN A;
  ELSE
    RETURN B;
  END_IF;
END_FUNCTION
```

## 3. FUNCTION_BLOCK Declaration (Table 40, Section 6.6.3)

### Syntax

```
FUNCTION_BLOCK fb_name
  // Variable declarations
  VAR_INPUT ... END_VAR
  VAR_OUTPUT ... END_VAR
  VAR_IN_OUT ... END_VAR
  VAR ... END_VAR
  VAR_TEMP ... END_VAR
  VAR_EXTERNAL ... END_VAR
  // Methods (optional)
  METHOD ... END_METHOD
  // Statements
END_FUNCTION_BLOCK
```

### Example

```
FUNCTION_BLOCK Counter
VAR_INPUT
  Reset: BOOL;
  CountUp: BOOL R_EDGE;
END_VAR
VAR_OUTPUT
  Count: INT;
  Overflow: BOOL;
END_VAR
VAR
  InternalCount: INT := 0;
END_VAR

IF Reset THEN
  InternalCount := 0;
ELSIF CountUp THEN
  IF InternalCount < 32767 THEN
    InternalCount := InternalCount + 1;
  ELSE
    Overflow := TRUE;
  END_IF;
END_IF;
Count := InternalCount;
END_FUNCTION_BLOCK
```

### Rules

1. **State retention**: Internal variables persist across calls
2. **Instantiation required**: Must be declared as instance to use
3. **Instance isolation**: Each instance has independent state
4. **Can contain methods**: OOP-style methods allowed
5. **Can inherit**: Using EXTENDS (if supported)
6. **EXTENDS targets**: Function blocks may EXTENDS a FUNCTION_BLOCK or CLASS; extending an INTERFACE is invalid (Table 40, IEC 61131-3 Ed.3 §6.6.3.4)
7. **Can implement interfaces**: A function block declaration may use
   `IMPLEMENTS` with one or more interface names; the parser preserves the
   complete ordered interface list for semantic validation (IEC 61131-3 Ed.3
   §6.6.7.1, Table 52).

Table 40 permits input, output, in-out, external, ordinary, and temporary
sections. truST also accepts `VAR_STAT` as ordinary persistent instance
storage. A function block rejects `VAR_GLOBAL`, `VAR_ACCESS`, and
`VAR_CONFIG`; those owners belong to program/configuration assembly and cannot
be ignored inside the function-block declaration.

Function-block ordinary/static variables, inputs, and outputs may own one
restart policy. In-outs, temporaries, and external aliases may not. The same
sections may be `CONSTANT` where allowed by `03-variables.md`, but a constant
section cannot also declare a restart policy.

Table 40 edge-qualified `BOOL` inputs are a function-block-body surface, not
ordinary parameters visible to methods. Each `R_EDGE` or `F_EDGE` declaration
creates independent hidden trigger state and exposes its one-invocation pulse
to the body. The complete declaration, qualifier, restart, and rejection
contract is in `03-variables.md`.

### Function Block Instance Declaration (Table 41)

```
VAR
  MyCounter: Counter;                           // Simple instance
  Timers: ARRAY[1..10] OF TON;                  // Array of instances
  HeaterPID: PID := (Kp := 2.5, Ti := T#10s);  // With initialization
END_VAR
```

### Function Block Call (Table 42)

| No. | Call Type | Example |
|-----|-----------|---------|
| 1 | Complete formal | `MyCounter(Reset := FALSE, CountUp := TRUE);` |
| 2 | Incomplete formal | `MyCounter(CountUp := Trigger);` |
| 3 | Output access | `Value := MyCounter.Count;` |
| 4 | With EN/ENO | `MyFB(EN := Cond, ENO => Success);` |

## 4. PROGRAM Declaration (Table 47, Section 6.6.4)

### Syntax

```
PROGRAM program_name
  // Variable declarations
  VAR_INPUT ... END_VAR
  VAR_OUTPUT ... END_VAR
  VAR ... END_VAR
  VAR_EXTERNAL ... END_VAR
  VAR_TEMP ... END_VAR
  VAR_ACCESS ... END_VAR
  // Statements
END_PROGRAM
```

### Example

```
PROGRAM MainControl
VAR_INPUT
  EmergencyStop: BOOL;
END_VAR
VAR_OUTPUT
  SystemRunning: BOOL;
END_VAR
VAR
  StartupSequence: INT := 0;
  ProcessTimer: TON;
END_VAR
VAR_EXTERNAL
  GlobalConfig: Configuration;
END_VAR

IF EmergencyStop THEN
  SystemRunning := FALSE;
  StartupSequence := 0;
ELSE
  // Main control logic
END_IF;
END_PROGRAM
```

### Rules

1. Similar to FUNCTION_BLOCK but with additional capabilities
2. Can be associated with TASKs
3. Can have VAR_ACCESS declarations
4. Typically represents a complete control application
5. Instantiated in CONFIGURATION/RESOURCE

IEC Table 47 permits the function-block-like input, output, in-out, external,
ordinary, and temporary sections and additionally permits program-local
`VAR_GLOBAL` and `VAR_ACCESS`. truST accepts `VAR_STAT` as ordinary persistent
program-instance storage. `VAR_CONFIG` remains configuration-owned and is
rejected inside a program declaration.

Program ordinary/static variables, inputs, outputs, and globals may own one
restart policy. In-outs, temporaries, external aliases, and access
declarations may not. Qualifier conflicts and duplicate qualifier tokens reject
the complete program declaration.

Table 47 adopts the Table 40 textual edge-input form for programs. A program
samples each edge-qualified `BOOL` input once per executed program cycle and
the program body observes only the resulting one-cycle pulse.

## 5. CLASS Declaration (Table 48, Section 6.6.5)

### Syntax

```
CLASS class_name
  // Variable declarations
  VAR ... END_VAR
  // Methods
  METHOD ... END_METHOD
END_CLASS
```

### With Inheritance and Interface

```
CLASS class_name EXTENDS base_class IMPLEMENTS interface1, interface2
  // ...
END_CLASS
```

### Example

```
CLASS Motor
VAR PUBLIC
  Speed: INT;
  Running: BOOL;
END_VAR
VAR PRIVATE
  InternalState: INT;
END_VAR

METHOD PUBLIC Start
  Running := TRUE;
  InternalState := 1;
END_METHOD

METHOD PUBLIC Stop
  Running := FALSE;
  Speed := 0;
  InternalState := 0;
END_METHOD

METHOD PUBLIC SetSpeed
VAR_INPUT
  NewSpeed: INT;
END_VAR
  IF Running THEN
    Speed := NewSpeed;
  END_IF;
END_METHOD
END_CLASS
```

### Class Modifiers

| Modifier | Description |
|----------|-------------|
| `FINAL` | Cannot be extended |
| `ABSTRACT` | Cannot be instantiated, must be extended |

```
CLASS ABSTRACT BaseController
  METHOD PUBLIC ABSTRACT Execute;
END_CLASS

CLASS FINAL SpecificController EXTENDS BaseController
  METHOD PUBLIC OVERRIDE Execute
    // Implementation
  END_METHOD
END_CLASS
```

### Rules

1. Classes cannot have VAR_INPUT, VAR_OUTPUT, VAR_IN_OUT
2. External member access is through methods, truST properties, or `PUBLIC`
   variables; `PROTECTED`, `PRIVATE`, and `INTERNAL` access follows the closed
   matrix in `03-variables.md`
3. Instantiation: `VAR MyMotor: Motor; END_VAR`
4. Cannot be associated with TASKs directly
5. EXTENDS must reference a CLASS type; FINAL classes cannot be extended (Table 48, IEC 61131-3 Ed.3 §6.6.5.5.4)

Table 48 and §6.6.5.2 permit ordinary `VAR` and `VAR_EXTERNAL` class
declarations and explicitly forbid `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`,
and `VAR_TEMP`. truST accepts `VAR_STAT` only as its documented ordinary
instance-storage extension. Classes also reject `VAR_GLOBAL`, `VAR_ACCESS`,
and `VAR_CONFIG`. Rejection occurs before a class type or field set is exposed
to runtime assembly.

Class `VAR` and truST `VAR_STAT` may own one restart policy; class
`VAR_EXTERNAL` may be `CONSTANT` but cannot own one. A qualifier does not
expand the class section set.

Class ordinary variables, methods, and truST properties default to
`PROTECTED`. `PRIVATE` members are not inherited. `INTERNAL` members are
inherited only within the exact declaring namespace. An overriding method must
be accessible to the derived class and repeat the inherited method's exact
access specifier (IEC §6.6.5.5.2-6.6.5.5.3).

## 6. INTERFACE Declaration (Table 51, Section 6.6.6)

### Syntax

```
INTERFACE interface_name
  METHOD method_name
    // Parameter declarations only, no body
  END_METHOD
END_INTERFACE
```

An interface declaration owns method prototypes, not storage. No `VAR`,
`VAR_STAT`, `VAR_TEMP`, directional parameter, external, global, access, or
configuration variable section is legal directly under `INTERFACE`. The
input/output/in-out declarations of each method prototype remain children of
that prototype and follow the method signature grammar.

### Example

```
INTERFACE IControllable
  METHOD Start
  END_METHOD

  METHOD Stop
  END_METHOD

  METHOD GetStatus : INT
  END_METHOD
END_INTERFACE

CLASS Pump IMPLEMENTS IControllable
VAR PRIVATE
  IsRunning: BOOL;
END_VAR

METHOD PUBLIC Start
  IsRunning := TRUE;
END_METHOD

METHOD PUBLIC Stop
  IsRunning := FALSE;
END_METHOD

METHOD PUBLIC GetStatus : INT
  IF IsRunning THEN
    GetStatus := 1;
  ELSE
    GetStatus := 0;
  END_IF;
END_METHOD
END_CLASS
```

### Interface Inheritance

```
INTERFACE IAdvancedControl EXTENDS IControllable
  METHOD Pause
  END_METHOD

  METHOD Resume
  END_METHOD
END_INTERFACE
```

### Interface as Variable Type

```
VAR
  MyPump: Pump;
  Controller: IControllable;   // Reference to any implementing class
END_VAR

Controller := MyPump;          // Assign implementing instance
Controller.Start();            // Call through interface
```

### Rules

1. Interfaces contain only method prototypes (no implementation) per IEC 61131-3 Ed.3 §6.6.6.1. Property signatures are accepted as a documented truST extension.
2. Every method prototype is implicitly `PUBLIC`; writing an explicit access
   specifier on a prototype is an error (IEC §6.6.5.9 and §6.6.6.3)
3. Classes implementing interface MUST implement all methods
4. Interfaces can extend other interfaces
5. A class can implement multiple interfaces
6. Interface variables are references and shall be assigned before use; they shall not be VAR_IN_OUT
7. Interface variables can be assigned NULL (default) and compared for equality
   (IEC 61131-3 Ed.3 §6.6.6.5.1)
8. EXTENDS must reference INTERFACE types; cyclic interface inheritance is invalid (Table 51, IEC 61131-3 Ed.3 §6.6.6.3)

## 7. METHOD Declaration (Sections 6.6.5.4 and 6.6.7.2)

### Syntax

```
METHOD access_specifier method_name : return_type
  VAR_INPUT ... END_VAR
  VAR_OUTPUT ... END_VAR
  VAR_IN_OUT ... END_VAR
  VAR ... END_VAR
  VAR_TEMP ... END_VAR
  // Statements
END_METHOD
```

### Example

```
METHOD PUBLIC Calculate : REAL
VAR_INPUT
  A, B: REAL;
END_VAR
VAR_TEMP
  Temp: REAL;
END_VAR
  Temp := A * A + B * B;
  Calculate := SQRT(Temp);
END_METHOD
```

### Method Modifiers

| Modifier | Description |
|----------|-------------|
| `OVERRIDE` | Overrides base class method |
| `FINAL` | Cannot be overridden in derived classes |
| `ABSTRACT` | No implementation, must be overridden |

```
CLASS Base
  METHOD PUBLIC Process
    // Default implementation
  END_METHOD
END_CLASS

CLASS Derived EXTENDS Base
  METHOD PUBLIC OVERRIDE Process
    SUPER.Process();  // Call base implementation
    // Additional processing
  END_METHOD
END_CLASS
```

### Access Specifiers for Methods

| Specifier | Permitted call sites |
|-----------|----------------------|
| `PUBLIC` | Anywhere the containing type is accessible |
| `PROTECTED` | The defining class/FB and derived classes/FBs (default) |
| `PRIVATE` | The defining class/FB only; not inherited |
| `INTERNAL` | POUs in the exact declaring namespace |

The matrix is normative for class methods under IEC §6.6.5.9 and is adopted
for function-block methods by §§6.6.7.2.5 and 6.6.7.6. Improper use is an
error. The following inheritance rules are part of the same contract:

- a derived POU may override only an inherited `PUBLIC`, `PROTECTED`, or
  same-namespace `INTERNAL` method;
- an override repeats the exact access specifier of the inherited method;
- `PRIVATE` methods are not inherited, so a same-named derived method is a new
  method and must not carry `OVERRIDE`;
- an `INTERNAL` method is neither inherited nor overrideable across a
  namespace boundary; and
- a containing `INTERNAL` namespace or type restriction may prevent an
  otherwise `PUBLIC` call.

### THIS and SUPER Keywords

```
METHOD PUBLIC Example
  THIS.Speed := 100;          // Access own member
  SUPER.Initialize();         // Call base class method
END_METHOD
```

### truST PROPERTY access extension

`PROPERTY`, `GET`, and `SET` are a documented truST language extension; IEC
61131-3 Ed.3 does not define those keywords. A class/FB property uses the same
`PUBLIC`/`PROTECTED`/`PRIVATE`/`INTERNAL` access matrix and `PROTECTED`
default as a method. The access check occurs before the getter/setter
capability check: an inaccessible getter or setter remains inaccessible even
when that accessor exists.

An interface property signature is implicitly `PUBLIC`, matching an IEC
interface method prototype. An explicit access token on an interface property
signature is rejected. An implementing property must be `PUBLIC` or
`INTERNAL`, must match the declared type, and must provide every accessor
required by the interface signature.

## 8. ACTION Declaration (Table 56, Table 72; Section 6.7.4)

truST's current `ACTION` support is a **front-end analysis profile**, not an
executable textual-SFC profile. The lexer, parser, and semantic layer retain
textual ST action declarations so source can be inspected and diagnosed, but
the runtime compiler does not implement IEC step/action association or action
control. See `sfc-profile.md`.

### Syntax

```
ACTION action_name:
  // Statements
END_ACTION
```

### Rules

1. The colon after `action_name` is required by the IEC grammar for `Action`
   (IEC 61131-3 Ed.3 Annex A, Tables 54-64).
2. A textual action declaration may be a direct child of a `PROGRAM` or
   `FUNCTION_BLOCK`. It is rejected at file/namespace scope and inside a
   `FUNCTION`, `CLASS`, `METHOD`, `PROPERTY`, another `ACTION`, or any statement
   block.
3. An action name is an identifier and is case-insensitive. Two textual action
   declarations in the same owner may not have the same name. Actions with the
   same name in different owners are independent.
4. IEC permits a Boolean variable to act as an action name. Consequently, a
   textual action declaration does not reserve the ordinary variable namespace:
   an enclosing `BOOL` variable may have the same spelling. That variable and
   the textual action declaration remain distinct semantic facts.
5. An action body is an ST statement list. It does not declare its own
   `VAR...END_VAR` sections and it cannot contain another action declaration.
6. The action body shares the enclosing POU's variable, type, function, and
   function-block visibility. An action in a function block also has the
   enclosing receiver context, including valid `THIS` and `SUPER` access.
7. Semantic analysis checks every action body even though the current runtime
   profile cannot execute it. Invalid assignments, unresolved names, invalid
   calls, invalid loop control, and other statement errors must not be hidden
   merely because runtime compilation will later reject the unsupported
   construct.
8. Labels and `JMP` targets form a scope for each action body. A jump cannot
   enter or leave an action body, matching IEC 61131-3 Ed.3 §8.1.6.
9. A bare `RETURN;` in an action body has the containing program or function
   block as its IEC control context. A return value is invalid because neither
   kind of owner returns a value.
10. A textual action name is not a callable ST function, method, or function
    block instance. `action_name()` does not invoke the action. Action
    association and qualifiers belong to the unsupported textual-SFC profile.

### Parser acceptance and recovery boundary

The parser recognizes `ACTION`, `END_ACTION`, and the required punctuation
case-insensitively. It requires exactly one identifier followed by exactly one
colon, accepts an empty body or a complete ST statement list, and retains each
action body in its own syntax node even when one owner declares several
actions. Ordinary owner statements before and after an action remain in the
owner's statement list. An ordinary owner variable may use the same spelling
as an action, and the same action spelling may be used independently by two
different owners.

The parser emits a visible error for a missing or reserved-keyword name, a
missing, doubled, or semicolon-substituted colon, a missing `END_ACTION`, a
local `VAR...END_VAR` section, a nested action, or an action in an owner/scope
not permitted by rule 2. Textual `STEP` and `TRANSITION` forms are not accepted
as ST statements inside an action body under the current front-end analysis
profile. These failures may retain a bounded partial syntax tree for tooling;
they do not create a valid action declaration or authorize execution.

### Compilation boundary

1. Any source unit containing a textual `ACTION...END_ACTION` declaration must
   be rejected by runtime/bytecode compilation with a diagnostic that identifies
   textual ACTION/SFC execution as unsupported.
2. The compiler must not omit an action body, execute it as part of the owner's
   ordinary ST body, or emit a runnable program that contains only the owner's
   non-action statements.
3. No action-local state, scheduling, qualifier, step association, activation,
   or scan-cycle semantics are defined by the current profile.
4. Visual-SFC artifacts remain executable through their generated companion
   function block and wrapper `PROGRAM` described by
   `17-visual-editors-runtime-unification.md`; they do not gain execution
   semantics by embedding textual action declarations.

## 9. NAMESPACE Declaration (Tables 64-66, Section 6.9)

### Syntax

```
NAMESPACE namespace_name
  // Type declarations
  // POU declarations
END_NAMESPACE
```

### Nested Namespaces

```
NAMESPACE Company
  NAMESPACE Project
    NAMESPACE Module
      FUNCTION_BLOCK MyFB
        // ...
      END_FUNCTION_BLOCK
    END_NAMESPACE
  END_NAMESPACE
END_NAMESPACE
```

### USING Directive

```
USING Company.Project.Module;
USING Standard.Timers, Standard.Counters;

VAR
  FB1: MyFB;  // Can use without full qualification
END_VAR
```

### Qualified Access

```
VAR
  FB1: Company.Project.Module.MyFB;  // Full qualification
END_VAR
```

### Rules

1. Namespaces can be nested
2. USING may appear in the global namespace, inside a namespace, or immediately after a POU header (IEC 61131-3 Ed.3, Section 6.9.4, Table 66)
3. USING brings names from the referenced namespace into scope (direct members only)
4. Qualified names can always be used
5. Name conflicts resolved by qualification
6. INTERNAL access specifier limits scope to namespace

### Namespace implementation notes

- USING directives are parsed and resolved for global, namespace, and POU scopes; only direct members of the imported namespace are made available. (IEC 61131-3 Ed.3, Section 6.9.4, Table 66)
- INTERNAL access specifier is enforced at namespace boundaries. (IEC 61131-3 Ed.3, Tables 64-66)

## Implementation Notes for trust-hir

### Syntax-tree POU semantic owners

The POU-like semantic-owner classifier contains `Program`, `Function`,
`FunctionBlock`, `Class`, `Method`, `Property`, and `Interface`.
`PropertyGet`, `PropertySet`, `ProgramConfig`, `Namespace`, `Configuration`,
and `Resource` are not independent owners in this classifier. The listed order
is the canonical iteration order of the shared classifier constant. This is an
internal truST syntax/HIR ownership boundary, and “POU-like” here does not
redefine the IEC POU taxonomy.

### POU Symbol Requirements

1. **Name**: Unique identifier
2. **Kind**: Function, FB, Program, Class, Interface, Method
3. **Parameters**: Input, output, in-out lists
4. **Return type**: For functions and methods
5. **Body**: Statement list
6. **Scope**: Containing namespace/POU
7. **Modifiers**: FINAL, ABSTRACT, access specifiers

### Semantic Checks

1. **Duplicate definition**: Same POU name in scope
2. **Missing implementation**: ABSTRACT method not overridden
3. **Interface compliance**: All methods implemented
4. **Override without base**: OVERRIDE on non-virtual method
5. **FINAL violation**: Extending FINAL class/overriding FINAL method
6. **Access violation**: Calling PRIVATE/PROTECTED inappropriately
7. **Return value**: Function/method must assign return value
8. **Parameter matching**: Call arguments match declaration

### Error Conditions

1. Undefined POU reference
2. Missing return value assignment
3. Type mismatch in call
4. Invalid inheritance (circular, FINAL violation)
5. Interface method not implemented
6. Abstract class instantiation
7. Invalid use of THIS/SUPER
8. OVERRIDE without matching base method
