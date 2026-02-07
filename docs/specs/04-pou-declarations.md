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

### Implementation Extension: Test POUs (DEV-033)

The following declarations are implemented as non-IEC extensions for MP-014:

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
2. **Return value**: Assigned via function name or RETURN statement
3. **VAR_IN_OUT and VAR_EXTERNAL**: May be modified inside the function; VAR_EXTERNAL CONSTANT shall not be modified
4. **CONSTANT restriction**: Function block instances shall not be declared in variable sections with CONSTANT qualifier

### Function Call (Section 6.6.1.7)

| No. | Call Type | Example |
|-----|-----------|---------|
| 1 | Formal call | `Y := Square(X := 5);` |
| 2 | Non-formal call | `Y := Square(5);` |
| 3 | Procedure call | `LogMessage('Hello');` |

Mixed calls are allowed when positional arguments precede formal arguments. (IEC 61131-3 Ed.3 §6.6.1.4.2; Table 50)

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
2. All member access is through methods or PUBLIC variables
3. Instantiation: `VAR MyMotor: Motor; END_VAR`
4. Cannot be associated with TASKs directly
5. EXTENDS must reference a CLASS type; FINAL classes cannot be extended (Table 48, IEC 61131-3 Ed.3 §6.6.5.5.4)

## 6. INTERFACE Declaration (Table 51, Section 6.6.6)

### Syntax

```
INTERFACE interface_name
  METHOD method_name
    // Parameter declarations only, no body
  END_METHOD
END_INTERFACE
```

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

1. Interfaces contain only method prototypes (no implementation) per IEC 61131-3 Ed.3 §6.6.6.1. Property signatures are accepted as an extension (see `IEC deviations log (internal)`).
2. All methods are implicitly PUBLIC
3. Classes implementing interface MUST implement all methods
4. Interfaces can extend other interfaces
5. A class can implement multiple interfaces
6. Interface variables are references and shall be assigned before use; they shall not be VAR_IN_OUT
7. Interface variables can be assigned NULL (default) and compared for equality
8. EXTENDS must reference INTERFACE types; cyclic interface inheritance is invalid (Table 51, IEC 61131-3 Ed.3 §6.6.6.3)

## 7. METHOD Declaration (Section 6.6.1.5)

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

| Specifier | Description |
|-----------|-------------|
| `PUBLIC` | Accessible from anywhere |
| `PROTECTED` | Accessible from class and derived classes (default) |
| `PRIVATE` | Accessible only within declaring class |
| `INTERNAL` | Accessible within same NAMESPACE |

### THIS and SUPER Keywords

```
METHOD PUBLIC Example
  THIS.Speed := 100;          // Access own member
  SUPER.Initialize();         // Call base class method
END_METHOD
```

## 8. ACTION Declaration (Table 56, Table 72; Section 6.7.4)

### Syntax

```
ACTION action_name
  // Statements
END_ACTION
```

### Rules

1. Actions may be declared inside PROGRAM or FUNCTION_BLOCK bodies; they share the enclosing POU scope (IEC 61131-3 Ed.3 §6.7.4, Table 56).
2. Action declarations are local to the enclosing POU.
3. Action bodies are type-checked like statement lists in the enclosing POU, including access to THIS/SUPER in function blocks.

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

### Implementation Notes for trust-hir

- USING directives are parsed and resolved for global, namespace, and POU scopes; only direct members of the imported namespace are made available. (IEC 61131-3 Ed.3, Section 6.9.4, Table 66)
- INTERNAL access specifier is enforced at namespace boundaries. (IEC 61131-3 Ed.3, Tables 64-66)

## 10. EN/ENO Mechanism (Section 6.6.1.6)

EN/ENO are optional inputs/outputs that may be provided in the POU declaration:
- `EN`: BOOL input, default TRUE (`BOOL := 1`) when declared
- `ENO`: BOOL output

### Behavior

| EN | Execution | ENO |
|----|-----------|-----|
| FALSE | POU not executed | FALSE (reset) |
| TRUE | POU executed normally | TRUE unless set FALSE by POU |
| TRUE | POU has error | FALSE (system resets) |

### Example

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

// Usage
Result := SafeDiv(EN := Cond, Num := A, Den := B, ENO => Valid);
```

## Implementation Notes for trust-hir

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
