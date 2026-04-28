# Semantic Rules

IEC 61131-3 Edition 3.0 (2013) - Various Sections

This specification defines semantic rules and error conditions for trust-hir.

## 1. Scope Rules (Section 6.5.2.2)

### 1.1 Variable Scope

| Declaration | Scope | Visibility |
|-------------|-------|------------|
| VAR | Local to POU | Within declaring POU only |
| VAR_TEMP | Local to POU | Within declaring POU only, reinitialized each call |
| VAR_INPUT | Parameter | Read inside, written by caller |
| VAR_OUTPUT | Parameter | Written inside, read by caller |
| VAR_IN_OUT | Parameter | Read/write both sides |
| VAR_EXTERNAL | Reference | Access to VAR_GLOBAL (strict IEC form; optional in truST vendor-parity mode) |
| VAR_GLOBAL | Program/Configuration/Resource | Accessible via VAR_EXTERNAL or truST vendor-parity bare/qualified access |

### 1.2 Name Resolution Order

1. Local scope (VAR, VAR_TEMP, parameters)
2. Enclosing POU (for methods within FB/CLASS)
3. Global scope (via `VAR_EXTERNAL` in strict IEC form, or via truST vendor-parity bare/qualified global access)
4. Namespace-qualified names

### 1.3 Shadowing Rules

- Local names shadow global names
- No shadowing within same scope (error: duplicate declaration)
- Class members are accessed via THIS when shadowed

```
VAR_GLOBAL
  Value: INT := 100;
END_VAR

FUNCTION_BLOCK Example
VAR_EXTERNAL Value: INT; END_VAR  // References global
VAR
  Value: INT := 50;               // ERROR: duplicate declaration
END_VAR
END_FUNCTION_BLOCK
```

## 2. Assignment Rules

### 2.1 Valid Assignment Targets

| Target | Assignable |
|--------|------------|
| VAR | Yes |
| VAR_OUTPUT | Yes (inside POU) |
| VAR_IN_OUT | Yes |
| VAR_TEMP | Yes |
| VAR_INPUT | **No** (error) |
| CONSTANT | **No** (error) |
| VAR_EXTERNAL CONSTANT | **No** (error) |
| Function block output (external) | **No** (error) |

Notes:
- VAR_INPUT is externally supplied and not modifiable within the entity (IEC 61131-3 Ed.3 Figure 7).
- Assignment targets must resolve to assignable variables/parameters or properties with setters; assigning to functions, methods, `THIS`/`SUPER`, or read-only properties is invalid.

### 2.2 Type Compatibility

| Assignment | Rule |
|------------|------|
| Same type | Always valid |
| Integer widening | Valid (SINT→INT→DINT→LINT) |
| Unsigned widening | Valid (USINT→UINT→UDINT→ULINT) |
| Real widening | Valid (REAL→LREAL) |
| Integer to Real | Valid (implicit) |
| Real to Integer | **Error** (requires explicit conversion) |
| Different structures | **Error** (must be same type) |
| Different arrays | **Error** (must be same type and bounds) |

### 2.3 Error: Modifying Read-Only

```
FUNCTION_BLOCK Example
VAR_INPUT
  InputVal: INT;
END_VAR
  InputVal := 10;  // ERROR: Cannot modify VAR_INPUT
END_FUNCTION_BLOCK

VAR CONSTANT
  PI: REAL := 3.14159;
END_VAR
PI := 3.0;  // ERROR: Cannot modify CONSTANT
```

## 3. Type Mismatch Errors

### 3.1 Expression Type Errors

| Operation | Required Types | Error If |
|-----------|---------------|----------|
| +, -, *, / | ANY_NUM | Non-numeric operand |
| MOD | ANY_INT | Non-integer operand |
| ** | ANY_REAL base | Non-numeric operands |
| AND, OR, XOR | BOOL or ANY_BIT | Incompatible types |
| NOT | BOOL or ANY_BIT | Non-boolean/bit operand |
| <, >, <=, >= | ANY_ELEMENTARY | Incompatible comparison |
| =, <> | ANY_ELEMENTARY | Incompatible types |

### 3.2 Statement Type Errors

| Statement | Required Type | Error If |
|-----------|---------------|----------|
| IF condition | BOOL | Non-boolean condition |
| WHILE condition | BOOL | Non-boolean condition |
| REPEAT UNTIL | BOOL | Non-boolean condition |
| FOR control variable | ANY_INT | Non-integer control |
| FOR bounds/BY | Same integer type as control | Type mismatch |
| CASE selector | ANY_ELEMENTARY | Complex type selector |
| CASE label | Match selector | Label type mismatch |
| CASE label | Unique values | Duplicate case labels |

### 3.3 Call Type Errors

| Context | Error Condition |
|---------|-----------------|
| Function call | Argument type doesn't match parameter |
| FB call | Argument type doesn't match parameter |
| Method call | Argument type doesn't match parameter |
| Return value | Expression type doesn't match return type |

### 3.4 Call Binding Errors

IEC 61131-3 Ed.3 §6.6.1.4.1 requires VAR_IN_OUT parameters to be “properly mapped” in textual calls, and Table 50 distinguishes complete vs incomplete formal calls.

| Rule | Error Condition |
|------|-----------------|
| Formal calls | Unknown or duplicate parameter names |
| VAR_IN_OUT mapping | Missing binding for VAR_IN_OUT parameter |
| Non-formal calls | Positional argument count must match parameters (excluding EN/ENO) |
| Mixed calls | Positional arguments must precede formal arguments (IEC 61131-3 Ed.3 §6.6.1.4.2; Table 50) |

### 3.5 Aggregate Initializer Errors

Named aggregate initializers are validated at HIR collection/type-check time.

| Context | Error Condition | Diagnostic |
|---------|-----------------|------------|
| Struct/union/FB aggregate | Unknown field/member name | `E107 UndefinedField` |
| Struct/union/FB aggregate | Duplicate field/member name | `E108 DuplicateField` |
| Aggregate target | Non-aggregate target type | `E201 TypeMismatch` |
| FB aggregate | `VAR_IN_OUT`, temp, external, or non-public target | `E202 InvalidOperation` |
| String/WSTRING member default | Literal exceeds declared capacity | `E304 OutOfRange` |

### 3.5 Standard Function Call Errors

Standard functions and conversions (Tables 22–36) have fixed or extensible signatures with defined type categories. The type checker resolves overloads by argument types and reports errors when no valid overload matches. (IEC 61131-3 Ed.3, Tables 22–36)

| Rule | Error Condition |
|------|-----------------|
| Fixed-arity standard functions | Wrong number of arguments |
| Extensible standard functions (e.g., ADD, AND, CONCAT, MAX) | Fewer than the minimum required arguments |
| Typed conversions (`SRC_TO_DST`, `*_TRUNC_*`, `*_BCD_TO_*`) | Source type does not match the specified input type |
| Overloaded conversions (`TO_DST`, `TRUNC_DST`) | Source type not convertible to requested destination |
| Type-category mismatch | Arguments not in the required IEC generic category (ANY_INT/ANY_REAL/ANY_BIT/ANY_STRING/ANY_DATE) |

### 3.6 Standard Function Block Call Errors

Standard function blocks (Tables 43–46) have fixed or overloaded signatures. The type checker validates parameter names, directions, and types for standard FB calls, including counter/timer overloads. (IEC 61131-3 Ed.3, Tables 43–46)

| Rule | Error Condition |
|------|-----------------|
| Bistable/edge FBs (RS/SR, R_TRIG/F_TRIG) | Non-BOOL inputs/outputs |
| Counter FBs (CTU/CTD/CTUD) | PV/CV not INT/DINT/LINT/UDINT/ULINT |
| Timer FBs (TP/TON/TOF, TP_LTIME/TON_LTIME/TOF_LTIME) | PT/ET not TIME or LTIME |
| Output parameters | Non-assignable target or missing `=>` in formal call |

### 3.7 Array Index Rules

IEC 61131-3 Ed.3 §6.4.4.5.1 requires array subscripts to be ANY_INT expressions and within declared bounds; the number of subscripts matches the declared dimensions.

| Rule | Error Condition |
|------|-----------------|
| Index type | Subscript is not ANY_INT |
| Bounds | Constant index or subrange outside declared bounds |
| Dimensions | Subscript count doesn't match array dimensions |

## 4. Reference Errors

### 4.1 Undefined Reference

```
X := UndefinedVariable;  // ERROR: Undefined identifier 'UndefinedVariable'
```

### 4.2 Duplicate Declaration

```
VAR
  Count: INT;
  Count: REAL;  // ERROR: Duplicate declaration 'Count'
END_VAR
```

### 4.3 Invalid VAR_EXTERNAL

```
VAR_EXTERNAL
  NonExistentGlobal: INT;  // ERROR: No matching VAR_GLOBAL
END_VAR
```

### 4.4 Null Reference

```
VAR
  ptr: REF_TO INT := NULL;
END_VAR
X := ptr^;  // RUNTIME ERROR: Null dereference
```

### 4.5 Namespace Ambiguity (USING Conflicts)

```
USING LibA;
USING LibB;
X := Foo(); // ERROR: ambiguous reference to 'Foo'; qualify the name
```

Ambiguous identifiers caused by multiple USING directives must be qualified with the namespace path. (IEC 61131-3 Ed.3 §6.6.4; Tables 64-66)

## 5. OOP Rules (Sections 6.6.5-6.6.8)

### 5.1 Inheritance Rules

| Rule | Error Condition |
|------|-----------------|
| Single inheritance | CLASS cannot extend multiple classes |
| No circular inheritance | A→B→A is forbidden |
| FINAL class | Cannot extend a FINAL class |
| Abstract instantiation | Cannot instantiate ABSTRACT class |
| Abstract class | ABSTRACT class must declare at least one ABSTRACT method (IEC 61131-3 Ed.3 §6.6.5.8.2) |
| Inherited name conflict | Derived class declares a variable that conflicts with inherited variables (except PRIVATE) or a method with the name of an inherited variable (IEC 61131-3 Ed.3 §6.6.5.5.5) |

```
CLASS A EXTENDS B
END_CLASS

CLASS B EXTENDS A  // ERROR: Circular inheritance
END_CLASS

CLASS FINAL Sealed
END_CLASS

CLASS Derived EXTENDS Sealed  // ERROR: Cannot extend FINAL class
END_CLASS
```

### 5.2 Override Rules

| Rule | Error Condition |
|------|-----------------|
| OVERRIDE without base | OVERRIDE on method not in base class |
| FINAL method override | Cannot override FINAL method |
| Signature mismatch | Override must match base signature |
| Missing OVERRIDE | Method replaces base method without OVERRIDE (IEC 61131-3 Ed.3 §6.6.5.5.3) |
| Access specifier | Override must use the same access specifier as the base method (IEC 61131-3 Ed.3 §6.6.5.5.3) |
| ABSTRACT constraints | ABSTRACT methods require ABSTRACT class and cannot combine with OVERRIDE/FINAL (IEC 61131-3 Ed.3 §6.6.5.8.3) |

```
CLASS Base
  METHOD PUBLIC FINAL DoSomething
  END_METHOD

  METHOD PROTECTED Calculate: INT
  END_METHOD
END_CLASS

CLASS Derived EXTENDS Base
  METHOD PUBLIC OVERRIDE DoSomething  // ERROR: Cannot override FINAL
  END_METHOD

  METHOD PRIVATE OVERRIDE Calculate: INT  // ERROR: More restrictive access
  END_METHOD

  METHOD PUBLIC OVERRIDE NonExistent  // ERROR: No base method to override
  END_METHOD
END_CLASS
```

### 5.3 Interface Rules

IEC 61131-3 Ed.3 §6.6.6.4.2 defines the error conditions for interface implementation
(missing methods, signature mismatch, and access specifiers). Table 51 defines interface
declarations. The same checks are applied to function blocks that use `IMPLEMENTS`.

| Rule | Error Condition |
|------|-----------------|
| Method implementation | Class/FB must implement or declare all interface methods (IEC 61131-3 Ed.3 §6.6.6.4.1) |
| Signature match | Implementation must match interface signature (name, parameters, return type) |
| Access specifier | Implementation must be PUBLIC or INTERNAL |
| Property signatures (extension) | Interface PROPERTY signatures require matching type/accessors (see `docs/IEC_DEVIATIONS.md`) |

Abstract classes may declare required interface methods as ABSTRACT (IEC 61131-3 Ed.3 §6.6.5.8.3).

```
INTERFACE IDevice
  METHOD Start
  END_METHOD
  METHOD Stop
  END_METHOD
END_INTERFACE

CLASS Motor IMPLEMENTS IDevice
  METHOD PUBLIC Start    // OK
  END_METHOD
  // ERROR: Missing implementation of 'Stop'
END_CLASS
```

### 5.4 Access Specifier Violations

| Specifier | Access From | Error If |
|-----------|-------------|----------|
| PUBLIC | Anywhere | Never |
| PROTECTED | Own class, derived | External access |
| PRIVATE | Own class only | Any other access |
| INTERNAL | Same namespace | Different namespace |

Access specifiers apply to class/FB member variables, methods, and properties (IEC 61131-3 Ed.3 §6.6.7.6, §6.6.7.7).

```
CLASS Example
  VAR PRIVATE
    secret: INT;
  END_VAR
END_CLASS

VAR
  obj: Example;
END_VAR
X := obj.secret;  // ERROR: Cannot access PRIVATE member
```

LSP diagnostics for access-specifier violations include IEC references (IEC 61131-3 Ed.3 §6.6.5; Table 50)
and related hint text suggesting valid access scopes or visibility adjustments.

### 5.5 THIS and SUPER Errors

```
CLASS Base
  METHOD DoWork
  END_METHOD
END_CLASS

CLASS Derived EXTENDS Base
  METHOD DoWork
    SUPER.DoWork();           // OK: calls Base.DoWork
    SUPER.SUPER.DoWork();     // ERROR: Cannot chain SUPER
  END_METHOD
END_CLASS

// Outside class context
THIS.Something();  // ERROR: THIS only valid inside class/FB
```

### 5.6 Property Accessors

- Reading a PROPERTY requires a GET accessor; writing a PROPERTY requires a SET accessor.
- A PROPERTY declaration must include at least one accessor (GET or SET).
- Methods without a result cannot be used in expressions; PROPERTY access follows the same read/write separation (IEC 61131-3 Ed.3 §6.6.5.4.5).

```
FUNCTION_BLOCK Example
  PROPERTY Value : INT
  SET
  END_SET
  END_PROPERTY

  METHOD Use
    Value := 1;  // OK: SET exists
    X := Value;  // ERROR: no GET accessor
  END_METHOD
END_FUNCTION_BLOCK
```

## 6. Control Flow Errors

### 6.1 EXIT/CONTINUE Outside Loop

```
IF Condition THEN
  EXIT;      // ERROR: EXIT not inside loop
  CONTINUE;  // ERROR: CONTINUE not inside loop
END_IF;
```

### 6.2 RETURN Value Mismatch

```
FUNCTION GetValue: INT
  // ERROR: No return value assigned
END_FUNCTION

FUNCTION GetValue: INT
  RETURN 'text';  // ERROR: Type mismatch (STRING vs INT)
END_FUNCTION
```

Missing return value in a function with a declared result is an error. (IEC 61131-3 Ed.3, Table 19)

### 6.3 CASE Label Errors

```
CASE Mode OF
  1: DoA();
  1: DoB();     // ERROR: Duplicate case label
  1..5: DoC();
  3..7: DoD();  // ERROR: Overlapping ranges (3-5)
END_CASE;
```

**Warning**:
- Missing ELSE in CASE may leave unmatched selector values without executed statements. (IEC 61131-3 Ed.3, 7.3.3.3.3)

### 6.4 FOR Loop Errors

```
FOR I := 1 TO 10 DO
  I := I + 2;  // ERROR: Modifying control variable
END_FOR;

VAR X: REAL; END_VAR
FOR X := 1.0 TO 10.0 DO  // ERROR: Control variable must be integer
END_FOR;
```

## 7. Array Errors

### 7.1 Index Out of Bounds

```
VAR
  Arr: ARRAY[1..10] OF INT;
END_VAR
Arr[0] := 5;   // ERROR: Index 0 out of bounds [1..10]
Arr[11] := 5;  // ERROR: Index 11 out of bounds [1..10]
```

### 7.2 Dimension Mismatch

```
VAR
  Arr2D: ARRAY[1..10, 1..5] OF INT;
END_VAR
X := Arr2D[5];      // ERROR: Missing dimension (expected 2)
X := Arr2D[1,2,3];  // ERROR: Too many dimensions (expected 2)
```

### 7.3 Variable-Length Array Errors

```
FUNCTION_BLOCK FB
VAR
  Data: ARRAY[*] OF INT;  // ERROR: wildcard only allowed in parameter positions
END_VAR
END_FUNCTION_BLOCK
```

## 8. Function/FB Call Errors

### 8.1 Argument Count

```
FUNCTION Add3 : INT
VAR_INPUT A, B, C: INT; END_VAR
  Add3 := A + B + C;
END_FUNCTION

X := Add3(1, 2);        // ERROR: Missing argument
X := Add3(1, 2, 3, 4);  // ERROR: Too many arguments
```

### 8.2 Named Parameter Errors

```
X := Add3(A := 1, D := 2, C := 3);  // ERROR: Unknown parameter 'D'
X := Add3(A := 1, A := 2, C := 3);  // ERROR: Duplicate parameter 'A'
```

### 8.3 VAR_IN_OUT Restrictions

```
FB(InOutParam := 5);        // ERROR: Must be variable, not literal
FB(InOutParam := A + B);    // ERROR: Must be variable, not expression
FB(InOutParam := MyVar);    // OK: Variable reference
```

## 9. Enumeration Errors

### 9.1 Ambiguous Enumerated Value

```
TYPE
  Color1: (Red, Green, Blue);
  Color2: (Red, Yellow, Purple);
END_TYPE

VAR
  C: Color1;
END_VAR
C := Red;        // ERROR: Ambiguous 'Red' (Color1 or Color2?)
C := Color1#Red; // OK: Qualified access
```

### 9.2 Invalid Enumeration Value

```
TYPE Status: (Idle, Running, Error); END_TYPE
VAR S: Status; END_VAR
S := 5;          // ERROR: Invalid enumeration value
S := Unknown;    // ERROR: 'Unknown' not in enumeration
```

## 10. Subrange Errors

### 10.1 Value Out of Range

```
TYPE Percent: INT(0..100); END_TYPE
VAR P: Percent; END_VAR
P := 150;  // ERROR/WARNING: Value 150 outside range [0..100] (IEC 61131-3 Ed.3, 6.4.4.4.1)
```

### 10.2 Range Definition Errors

```
TYPE
  Invalid1: INT(10..5);      // ERROR: Lower bound > upper bound
  Invalid2: INT(A..B);       // ERROR: Bounds must be constant (IEC 61131-3 Ed.3, 6.4.4.4.1)
  Invalid3: REAL(0.0..1.0);  // ERROR: Subrange base must be integer (IEC 61131-3 Ed.3, 6.3, 6.4.4.4, Table 11)
END_TYPE
```

## 11. Time/Date Errors

### 11.1 Invalid Literals

```
Duration := T#25h_70m;           // OK: Overflow allowed
Date := DATE#2024-13-01;         // ERROR: Invalid month 13
Time := TOD#25:00:00;            // ERROR: Invalid hour 25
DateTime := DT#2024-02-30-12:00; // ERROR: Feb 30 doesn't exist
```

## 12. Diagnostic Ownership

Semantic validity rules stay in this document. Diagnostic code allocation,
severity guidance, LSP payload details, and editor refresh behavior are owned
by `14-lsp.md`.

Configuration/resource/task declarations and their IEC-aligned validation rules
are owned by `18-configurations-resources-tasks.md`.

## Implementation Notes for trust-hir

### Semantic Analysis Phases

1. **Name Resolution**: Resolve all identifiers to their declarations
2. **Type Checking**: Verify type compatibility in all contexts
3. **Flow Analysis**: Check control flow (return paths, unreachable code)
4. **Constraint Checking**: Verify OOP rules, access specifiers

### Error Recovery

- Continue analysis after errors when possible
- Report multiple errors per compilation
- Avoid cascading errors from single mistake

### Error Message Quality

Good error messages should include:
1. Precise source location (file, line, column)
2. Clear description of the problem
3. Expected vs actual (for type mismatches)
4. Suggestions for fixing when possible
