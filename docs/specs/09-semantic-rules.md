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
- A field or index rooted in a value returned by a function or method call is
  not modifiable storage and is therefore not a valid assignment target.
- Variable-section qualifiers are validated before symbols are exposed.
  Exactly one `CONSTANT`, `RETAIN`, `NON_RETAIN`, or truST `PERSISTENT` token
  may qualify a section. Duplicate tokens and mixed combinations are
  `InvalidOperation` errors.
- `RETAIN`, `NON_RETAIN`, and `PERSISTENT` are legal only on the state-owning
  section/owner combinations in `03-variables.md`. A call-local function or
  method variable, temporary, in-out alias, external alias, access path, or
  configuration binding cannot acquire retention semantics by carrying a
  qualifier; the declaration is rejected rather than accepting and dropping
  the policy.
- `R_EDGE` and `F_EDGE` are declaration suffixes, not section modifiers. They
  require an uninitialized `BOOL` declaration in function-block/program
  `VAR_INPUT`, exactly one suffix, and one independent hidden trigger state per
  declared name. Wrong owner, section, type, initializer, duplicate/mixed
  suffix, function-block-method access, or use under `VAR_INPUT CONSTANT` is an
  `InvalidOperation` error.

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
| ** | ANY_REAL base; reviewed host extension `INT#2 ** INT#3` | Non-numeric operands |
| AND, OR, XOR | BOOL or ANY_BIT | Incompatible types |
| NOT | BOOL or ANY_BIT | Non-boolean/bit operand |
| <, >, <=, >= | ANY_ELEMENTARY | Incompatible comparison |
| =, <> | ANY_ELEMENTARY | Incompatible types |

The exact reviewed integer-base `**` extension is recorded in
[`IEC_DEVIATIONS.md`](../IEC_DEVIATIONS.md#2026-07-27---integer-base-exponentiation).

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
| Direction marker | `:=` used for output/ENO or `=>` used for input/in-out/EN |
| Input mapping | Actual cannot use accuracy-preserving input conversion |
| Output mapping | Actual is not a writable, non-constant lvalue or cannot accept the output type |
| VAR_IN_OUT mapping | Missing binding, non-lvalue, constant target, temporary, reference-typed declaration, or non-exact type |
| Non-formal calls | Positional argument count must match parameters (excluding EN/ENO) |
| IEC call-form separation | IEC 61131-3 Ed.3 section 6.6.1.4.2 defines separate formal and non-formal lists |
| truST mixed-call extension | A positional prefix followed by formal assignments is accepted; a repeated occupied parameter or positional argument after a formal assignment is rejected ([`DEV-018`](../IEC_DEVIATIONS.md#2026-07-26---mixed-positional-prefix-and-formal-suffix-calls)) |
| Writable aliasing | Two output/in-out/ENO connections resolve to the same or overlapping caller storage |
| Execution control | EN/ENO supplied positionally, EN connected with `=>`, ENO connected with `:=`, or `REF(EN)` / `REF(ENO)` |

Formal names determine binding but not evaluation order. Actuals are evaluated
exactly once from left to right in source order, except that `EN` is evaluated
first and a false `EN` suppresses every other actual and writable-target
resolution. Output and in-out destinations are resolved once, validated as a
complete transfer set, and committed only after normal return. These semantic
requirements apply uniformly to functions, function blocks, and methods.

### 3.5 Aggregate Initializer Errors

Named aggregate initializers are validated at HIR collection/type-check time.

| Context | Error Condition | Diagnostic |
|---------|-----------------|------------|
| Struct/union/FB aggregate | Unknown field/member name | `E107 UndefinedField` |
| Struct/union/FB aggregate | Duplicate field/member name | `E108 DuplicateField` |
| Aggregate target | Non-aggregate target type | `E201 TypeMismatch` |
| FB aggregate | `VAR_IN_OUT`, temp, external, or non-public target | `E202 InvalidOperation` |
| String/WSTRING member default | Literal exceeds declared capacity | `E304 OutOfRange` |

#### 3.5.1 truST HIR initializer retention

Scalar, array, structure, union, alias, and function-block defaults are
retained as source-bound HIR initializer records so later compilation stages
can materialize the same reviewed value. Editing a default invalidates the
owning symbol and initializer catalog. Cross-file import translates both the
declared type identity and any retained initializer into identities valid in
the consuming symbol table. This paragraph defines truST HIR query and model
behavior; IEC 61131-3 does not prescribe these internal records or invalidation
mechanics.

#### 3.5.2 Default initialization and constant evaluation

IEC 61131-3 Ed.3 §6.4.4.1.2 and Figure 6 permit initialization by compatible
literals and constant expressions. Sections 6.4.4.4.1, 6.4.4.6.2,
6.4.4.9.2, and 6.4.4.10.2 further define subrange bounds, structure
initialization, derived-type initialization, and reference initialization.

Compile-time defaults obey these rules:

- named aggregate field matching is case-insensitive and independent of field
  order;
- unknown and duplicate fields are rejected;
- array repeat defaults validate the repeated element against the declared
  element type and expand the parenthesized sequence in source order
  (`[3(1, 2)]` becomes `1, 2, 1, 2, 1, 2`); an arbitrary call expression is
  not an array-repeat default (IEC 61131-3 Ed.3 §6.4.4.5.2);
- nested structure and union members are checked against their required type;
- `NULL` is a valid reference default;
- truST currently rejects a non-`NULL` reference type/member default,
  including the IEC-permitted `REF(target)` form; this known conflict with IEC
  61131-3 Ed.3 §6.4.4.10.2 is recorded as
  `docs/IEC_DEVIATIONS.md#2026-07-26---non-null-reference-defaults-on-type-and-aggregate-members`;
- integer, string, WSTRING, subrange, and field defaults enforce the inclusive
  bounds of their declared destination;
- POU `VAR`, `VAR_TEMP`, `VAR_STAT`, `VAR_INPUT`, and `VAR_OUTPUT`
  declarations may carry a compatible initializer, while `VAR_IN_OUT` and
  `VAR_EXTERNAL` declarations reject one (IEC 61131-3 Ed.3 §6.5.1.3 and Annex
  A initialized/no-initializer productions);
- a POU initializer resolves the complete visible constant graph before
  declaration storage is lowered, including later sections and later project
  sources, while preserving lexical POU, namespace, and `USING` identity;
- a constant may refer forward to another acyclic constant, including a
  resolved cross-file global constant;
- a type-level, structure-member, or union-member default resolves constants in
  the declaration's lexical namespace and active `USING` context; explicit
  qualification selects one namespace, a missing import does not leak a
  namespace-local constant, and multiple matching imports are ambiguous;
- integer constant operands that define array dimensions, subrange limits,
  bounded-string capacities, or explicit named enumeration values use that
  same dependency and namespace context, including forward and cross-file
  providers;
- fixed array and subrange lower bounds must not exceed their upper bounds,
  bounded-string capacities must be positive, and explicit enumeration values
  plus implicit successors must fit the declared integer base;
- a constant dependency cycle is rejected as cyclic;
- division by zero and arithmetic overflow are rejected as invalid constant
  operations; and
- a failed prerequisite produces its primary constant-evaluation diagnostic
  without parameter, local-storage, aggregate-field, or range cascades.

The retained HIR initializer record must preserve enough declaration context
for runtime materialization to resolve the same constant identity. Importing a
type into another file or namespace does not rebind its default expression to
the consumer's `USING` list. Source reordering likewise cannot change the
selected constant or the resulting default value. These context-retention and
cross-file identity rules are truST semantic-model behavior; IEC does not
prescribe HIR identities.

#### 3.5.3 truST union semantics

`UNION ... END_UNION` is a truST extension rather than an IEC 61131-3
construct. A union declaration publishes one ordered aggregate type whose
variants are independent logical members. All variants are present at once;
access does not select an active variant and a write to one variant does not
alias another. Named aggregate initialization uses the same case-insensitive
unknown/duplicate-name and required-type validation as a structure. Assignment
is permitted only between values of the same declared union type and copies
every variant. Physical shared-storage interpretation is expressed by IEC
`STRUCT OVERLAP`, not by this extension.

#### 3.5.4 POU variable-section ownership

Semantic collection validates the closed owner/section matrix in
`03-variables.md` before publishing the owning POU:

- functions and methods accept input, output, in-out, external, ordinary, and
  temporary sections;
- function blocks accept the same IEC section set;
- programs additionally accept program-local global and access sections;
- classes accept only ordinary and external sections; and
- interfaces accept no direct variable section.

The truST `VAR_STAT` extension is accepted exactly where ordinary `VAR` is
accepted. `VAR_CONFIG` is configuration-owned. An invalid section produces one
primary owner/section diagnostic and does not contribute parameters, fields,
locals, globals, access paths, or configuration overrides to a partial
declaration. These rules implement IEC 61131-3 Ed.3 Tables 19, 40, 47, 48, and
51 plus the corresponding Annex A productions; `VAR_STAT` is the documented
IEC-silent product extension, not an IEC deviation.

As a truST language extension, CASE labels may use integer constant
expressions in addition to the literals, enumerated values, and subranges
listed by IEC 61131-3 Ed.3 §7.3.3.3.3. They use the same constant evaluator as
initializers. Two labels whose expressions evaluate to the same value are
duplicates, including values reached through a POU-local constant scope chain.
Scalar/range collisions and overlapping ranges are also rejected after
constant evaluation. A range whose lower bound exceeds its upper bound is
invalid and is not normalized. The closed ordering and overlap policy is
recorded in `docs/IEC_DECISIONS.md`.

### 3.5 Standard Function Call Errors

Standard functions and conversions (Tables 22–36) have fixed or extensible signatures with defined type categories. The type checker resolves overloads by argument types and reports errors when no valid overload matches. (IEC 61131-3 Ed.3, Tables 22–36)

Generic `ANY*` categories are formal matching constraints, not source-level
storage types. Before matching a concrete actual against a generic formal, the
checker resolves directly derived aliases and subranges to the generic family
specified in `02-data-types.md`. Enumeration representation does not make an
enum an integer actual. A successful generic match never weakens the separate
concrete common-type, accuracy-preserving conversion, parameter-direction, or
result-type rules.

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
| Overloaded timer FBs (TP/TON/TOF) | PT/ET not one consistent TIME or LTIME family |
| Explicit TIME timer FBs (TP_TIME/TON_TIME/TOF_TIME) | PT/ET not TIME |
| Explicit LTIME timer FBs (TP_LTIME/TON_LTIME/TOF_LTIME) | PT/ET not LTIME |
| Output parameters | Non-assignable target or missing `=>` in formal call |

### 3.7 Array Index Rules

IEC 61131-3 Ed.3 §6.4.4.5.1 requires array subscripts to be ANY_INT expressions and within declared bounds; the number of subscripts matches the declared dimensions.

| Rule | Error Condition |
|------|-----------------|
| Index type | Subscript is not ANY_INT |
| Bounds | Constant index value outside declared bounds |
| Dimensions | Subscript count doesn't match array dimensions |

A nonconstant ANY_INT index is not rejected solely because its declared
integer or subrange domain extends beyond the array bounds. Static checking
rejects an out-of-bounds index when its value is a constant; a computed index
is checked against the array bounds at runtime. This follows IEC 61131-3 Ed.3
§6.4.4.5.1, which defines the error in terms of the subscript value and notes
that the error can only be detected at runtime for a computed index.

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

Project merge applies the same duplicate-name rule to globals imported from
different source files. An imported collision is reported as a duplicate; name
resolution must not silently select one declaration based on file order.

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

The semantic reference boundary also enforces:

- `REF(...)` and `ADR(...)` require one stable lvalue argument and reject
  literals, calls, computed values, and constant storage;
- `REF(...)` rejects `VAR_TEMP`, function-local storage, and function/method
  result storage under the lifetime rules in `02-data-types.md`;
- dereference requires `REF_TO` or `POINTER TO`;
- ordinary reference assignment permits the same resolved target or the
  standard derived-to-base direction, while dynamic downcasts require `?=`;
- `REF_TO` and `POINTER TO` never convert implicitly between families;
- assignment attempt requires a reference-like destination and an eligible
  same-family source or `NULL`; and
- reference arithmetic and reference ordering comparisons are invalid.

At runtime, a failed dynamic OOP assignment attempt stores `NULL`; it is not a
runtime fault. Dereferencing that result without the required null check is a
runtime fault and performs no partial read or write.

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
| Property signatures (extension) | Interface PROPERTY signatures require matching type/accessors as a documented truST extension |

Abstract classes may declare required interface methods as ABSTRACT (IEC 61131-3 Ed.3 §6.6.5.8.3).

IEC 61131-3 Ed.3 §6.6.6.5.1 defines an interface-typed variable as a reference
to an implementing class instance. Its initial value is `NULL` when no explicit
initializer is present. The variable must be assigned a valid implementing
instance before a method is invoked through it; code may compare the reference
with `NULL` before use.

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
| PUBLIC | Anywhere the complete containing path is accessible | A containing namespace/type is inaccessible |
| PROTECTED | Defining class/FB and derived POUs | Unrelated or external access |
| PRIVATE | Defining class/FB only | Any other POU, including a derived POU |
| INTERNAL | Exact declaring namespace | Global, parent, child, or sibling namespace |

Access specifiers apply to class/FB ordinary variables and methods (IEC
61131-3 Ed.3 §§6.6.5.9-6.6.5.10 and §§6.6.7.6-6.6.7.7). They also apply to
truST properties as a documented product extension.

Semantic analysis rejects all of the following with `InvalidOperation`:

- access outside the closed matrix above;
- an explicit access specifier on an interface method/property prototype;
- an explicit access specifier on function-block input, output, in-out,
  external, or temporary sections;
- more than one access specifier on one member declaration;
- an override whose visibility differs from the inherited method;
- `OVERRIDE` on a private method or on an internal method across a namespace
  boundary;
- external assignment to an FB output, even though the output is readable;
- member-style access to an FB in-out or temporary; and
- using the access token to broaden an inaccessible containing namespace or
  type.

For ordinary class/FB `VAR`, one legal storage qualifier and one access
specifier may appear in either order. Both orders have identical meaning.

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
- `PROPERTY`, `GET`, and `SET` are truST extensions rather than IEC
  61131-3 Ed.3 constructs. Methods without a result cannot be used in
  expressions; property access follows the same read/write separation as the
  documented product contract.
- A GET accessor has the declared property result type. A value returned from
  GET must be compatible with that type.
- A SET accessor has no result. Returning a value from SET is invalid.
- A property is checked against the method-style visibility matrix before GET
  or SET availability is considered. Interface property signatures are
  implicitly `PUBLIC` and cannot spell an access specifier.

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

For a function or value-returning method, the declaration name denotes the
implicit result variable inside its own body. The body may assign and
subsequently read that variable. A bare `RETURN` is valid only on a control-flow
path where the result has already been definitely assigned; `RETURN <expr>`
supplies the result directly. The value-bearing form is a truST grammar and
result-assignment extension recorded as `DEV-022`; IEC 61131-3 Ed.3 section
7.3.3.2.4 defines bare `RETURN`.

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

The control variable, initial, final, and explicit step must have one exact
integer type. The control variable and variables referenced by the initial and
final expressions cannot be assigned anywhere in the loop body, including
through nested statements. A variable referenced only by the step expression
may be assigned because the step is captured once before iteration. EXIT and
CONTINUE are valid only within a FOR, WHILE, or REPEAT body and affect the
innermost enclosing loop.

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

A statically known integer constant-expression outside a fixed dimension is an
`InvalidArrayIndex` preparation error. A computed integer index is checked at
runtime. A failed read or write reports the bounds error before producing a
value or mutating the array.

### 7.3 Array Initializer Cardinality

After recursively expanding repetition groups, initial values fill the array
in declaration order with the rightmost dimension varying fastest.

- Exact cardinality is accepted without a cardinality warning.
- Too few values are accepted, default-fill the remaining rightmost elements,
  and emit a preparation warning.
- Too many values are accepted, ignore the excess rightmost values, and emit a
  preparation warning.
- Every written initializer expression, including an excess expression, must
  still be a valid constant expression compatible with the element type.
- A repetition count must be a nonnegative integer constant that can be
  expanded within the compiler's checked resource limits.

### 7.4 Variable-Length Array Errors

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

An ordinary enumeration is a closed value set and accepts assignment only
from the same enumeration type or one of its resolved literals. A data type
with named integer values is not closed: it retains its declared integer base
range and supports integer constants and arithmetic under that base type.
Different ordinary enum types remain incompatible even when their literal
spelling or ordinal position is identical.

## 10. Subrange Errors

### 10.1 Value Out of Range

```
TYPE Percent: INT(0..100); END_TYPE
VAR P: Percent; END_VAR
P := 150;  // ERROR/WARNING: Value 150 outside range [0..100] (IEC 61131-3 Ed.3, 6.4.4.4.1)
```

truST treats a statically known out-of-range initializer or assignment as
`OutOfRange`. A dynamically computed out-of-range value reports
`RuntimeError::SubrangeViolation` before the destination is written. A subrange
without an explicit type-level initializer defaults to its lower bound, not to
the base integer type's zero.

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

## 12. Textual ACTION analysis

Textual action declarations use the bounded analysis profile defined by
`04-pou-declarations.md` and `sfc-profile.md`.

1. The semantic analyzer must visit every action body and use the direct
   enclosing `PROGRAM` or `FUNCTION_BLOCK` as its variable and receiver
   context.
2. An invalid statement in an action produces the same primary diagnostic it
   would produce in the corresponding owner body. Unsupported runtime execution
   is a later compilation-boundary error and must not suppress independent
   semantic errors.
3. Every action owns an independent label table. Duplicate labels are checked
   within that table, and unresolved jumps cannot bind to a label in the owner
   body or another action.
4. Action declarations are not ordinary callable symbols. A same-spelling
   `BOOL` variable remains an ordinary variable, while a call expression must
   still resolve to a function, method, or function-block instance.
5. Action declaration names are case-insensitively unique within one owner but
   do not conflict across different owners.

## 13. Diagnostic Ownership

Semantic validity rules stay in this document. Diagnostic code allocation,
severity guidance, LSP payload details, and editor refresh behavior are owned
by `14-lsp.md`.

Configuration/resource/task declarations and their IEC-aligned validation rules
are owned by `18-configurations-resources-tasks.md`.

### 13.1 Primary diagnostics and cascade suppression

Semantic analysis continues after an error to find independent problems, but a
failed prerequisite must not create wrong-reason secondary diagnostics:

- an unresolved source expression reports its resolution error; assignment,
  operator, unary, or index checks that require the missing type are suppressed;
- failed constant evaluation for an array bound, index, or subrange reports the
  evaluator's primary diagnostic, such as division by zero or an unresolved
  constant; dependent bounds and range diagnostics are suppressed;
- the same primary-diagnostic rule applies to bounded-string capacities and
  explicit/implicit enumeration values; a failed prerequisite must not produce
  a fabricated capacity, saturated successor, or secondary shape error;
- an ambiguous target reports ambiguity; the analyzer does not choose one
  candidate and then report type or operation errors for that arbitrary choice;
- a declaration that resolves to the wrong symbol kind reports the
  kind/operation error rather than degrading into an undefined-name error; and
- independent errors whose prerequisites resolved remain reportable.

This is a truST diagnostic-quality contract. IEC defines source validity, but
does not prescribe this editor-facing diagnostic cascade policy.

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
