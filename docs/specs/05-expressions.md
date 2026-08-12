# Expressions

IEC 61131-3 Edition 3.0 (2013) - Section 7.3.2

This specification defines expression syntax and operator precedence for trust-syntax parser and trust-hir type checking.

## 1. Expression Overview

An expression is a construct which, when evaluated, yields a value corresponding to one of the data types.

### Expression Components

Expressions are composed of:
1. **Operands**: Literals, enumerated values, variables, function calls, method calls
2. **Operators**: Arithmetic, logical, comparison, etc.
3. **Parentheses**: For grouping and precedence control

### Maximum Length

The maximum allowed length of expressions is Implementer specific.

## 2. Operators (Table 71, Section 7.3.2)

### Complete Operator Table with Precedence

| Precedence | Operation | Symbol | Example | Associativity |
|------------|-----------|--------|---------|---------------|
| 11 (highest) | Parentheses | `(expr)` | `(A+B)/C` | N/A |
| 10 | Function/Method call | `name(args)` | `SIN(X)`, `obj.method(Y)` | Left-to-right |
| 9 | Dereference ([§4.4](#44-reference-operators)) | `^` | `ptr^` | Left-to-right |
| 8 | Unary operators ([§4.1](#41-arithmetic-operators), [§4.3](#43-logicalboolean-operators)) | `-`, `+`, `NOT` | `-A`, `+B`, `NOT C` | Right-to-left |
| 7 | Exponentiation ([§4.1](#41-arithmetic-operators)) | `**` | `A**B` | Left-to-right |
| 6 | Multiplicative ([§4.1](#41-arithmetic-operators)) | `*`, `/`, `MOD` | `A*B`, `A/B`, `A MOD B` | Left-to-right |
| 5 | Additive ([§4.1](#41-arithmetic-operators)) | `+`, `-` | `A+B`, `A-B` | Left-to-right |
| 4 | Comparison ([§4.2](#42-comparison-operators)) | `<`, `>`, `<=`, `>=`, `=`, `<>` | `A<B`, `A=B` | Left-to-right |
| 3 | Boolean AND ([§4.3](#43-logicalboolean-operators)) | `&`, `AND` | `A&B`, `A AND B` | Left-to-right |
| 2 | Boolean XOR ([§4.3](#43-logicalboolean-operators)) | `XOR` | `A XOR B` | Left-to-right |
| 1 (lowest) | Boolean OR ([§4.3](#43-logicalboolean-operators)) | `OR` | `A OR B` | Left-to-right |

## 3. Evaluation Rules

### Operator precedence

Operators with higher precedence are applied first.

```
A + B - C * ABS(D)
// Evaluated as: A + B - (C * ABS(D))
// With A=1, B=2, C=3, D=4: 1 + 2 - 12 = -9

(A + B - C) * ABS(D)
// Evaluated as: ((A + B) - C) * ABS(D)
// With A=1, B=2, C=3, D=4: (1 + 2 - 3) * 4 = 0
```

### Rule 2: Left-to-Right for Equal Precedence

```
A + B + C
// Evaluated as: (A + B) + C

A / B / C
// Evaluated as: (A / B) / C
```

### Rule 3: Left Operand First

When an operator has two operands, the leftmost operand is evaluated first.

```
SIN(A) * COS(B)
// 1. Evaluate SIN(A)
// 2. Evaluate COS(B)
// 3. Multiply results
```

### Rule 4: Short-Circuit Evaluation (closed truST choice)

IEC 61131-3 Ed.3 section 7.3.2 makes the extent of Boolean evaluation
implementer-specific. truST uses this closed policy:

```
(A > B) & (C < D)
// If A <= B, the result is FALSE and (C < D) is not evaluated
```

- `BOOL AND` and `BOOL &` stop after a false left operand.
- `BOOL OR` stops after a true left operand.
- `BOOL XOR` evaluates both operands.
- Bit-string `AND`, `&`, `OR`, and `XOR` evaluate both operands.
- A skipped operand produces no call, fault, read, write, or other side effect.

This implementer choice is recorded in `docs/IEC_DECISIONS.md`.

### Rule 5: Function/Method in Expressions

Functions and methods with return values can be elements of expressions.

```
Result := SIN(X) + COS(Y) * 2.0;
Distance := obj.GetLength() + offset;
```

### Rule 5a: Debug/Watch Expressions (IEC-Aligned)

Debugger expressions (watch conditions, breakpoint conditions, hover) are parsed using the standard
expression grammar and operator precedence in Table 71. Only expression forms are permitted; no
statement constructs are allowed, and evaluation must be side-effect free. (IEC 61131-3 Ed.3,
§7.3.2, Table 71)

**Rules**:
- Use the same operator precedence and associativity as Table 71. (IEC 61131-3 Ed.3, Table 71)
- Disallow assignments, control-flow statements, and function block/method invocations.
- Allow only the explicit whitelist of pure standard functions defined by the
  debugger-evaluation product contract.

### Rule 6: Type Conversion

When operands require conversion, implicit conversion rules apply.

```
// Accuracy-preserving implicit widening
RealVar := IntVar + 5;        // INT can widen exactly to REAL

// Explicit required for narrowing
IntVar := REAL_TO_INT(RealVar);
```

Typed operands do not promote by a total “widest type” ordering. They must be
identical or have one accuracy-preserving widening direction defined by the
matrix in `docs/IEC_DECISIONS.md`. Signed/unsigned cross-family arithmetic and
integer/real combinations that can lose accuracy require explicit conversion.
Representable untyped numeric literals are contextualized to the other typed
operand.

## 4. Operator Categories

### 4.1 Arithmetic Operators

| Operator | Symbol | Left Operand | Right Operand | Result | Notes |
|----------|--------|--------------|---------------|--------|-------|
| Add | `+` | ANY_NUM | ANY_NUM | ANY_NUM | Also the temporal combinations in section 6.1 |
| Subtract | `-` | ANY_NUM | ANY_NUM | ANY_NUM | Also the temporal combinations in section 6.1 |
| Multiply | `*` | ANY_NUM or duration | ANY_NUM | Numeric result or the left duration type |
| Divide | `/` | ANY_NUM or duration | ANY_NUM | Numeric result or the left duration type |
| Modulo | `MOD` | ANY_INT | ANY_INT | ANY_INT | |
| Exponent | `**` | ANY_REAL | ANY_NUM | ANY_REAL | See deviation note below |
| Negate | `-` | - | ANY_NUM | ANY_NUM | Unary |
| Plus | `+` | - | ANY_NUM | ANY_NUM | Unary (identity) |

IEC 61131-3 Ed.3 section 6.6.2.5.8 and Table 29 require the exponentiation
base to be `ANY_REAL`. The reviewed host evaluator additionally returns
`INT#8` for `INT#2 ** INT#3`; that exact integer-base form is an intentional
extension recorded in
[`IEC_DEVIATIONS.md`](../IEC_DEVIATIONS.md#2026-07-27---integer-base-exponentiation).
Other integer forms require separate specification behavior and proof.

### 4.2 Comparison Operators

| Operator | Symbol | Left Operand | Right Operand | Result |
|----------|--------|--------------|---------------|--------|
| Less than | `<` | ANY_ELEMENTARY | ANY_ELEMENTARY | BOOL |
| Greater than | `>` | ANY_ELEMENTARY | ANY_ELEMENTARY | BOOL |
| Less or equal | `<=` | ANY_ELEMENTARY | ANY_ELEMENTARY | BOOL |
| Greater or equal | `>=` | ANY_ELEMENTARY | ANY_ELEMENTARY | BOOL |
| Equal | `=` | ANY_ELEMENTARY | ANY_ELEMENTARY | BOOL |
| Not equal | `<>` | ANY_ELEMENTARY | ANY_ELEMENTARY | BOOL |

**Notes**:
- Operands must be identical or have one accuracy-preserving common type.
- String and wide-string comparison is lexicographic within the same string
  family.
- Each comparison produces `BOOL`. Consequently, a numeric chain such as
  `A < B < C` is rejected because its second comparison receives `BOOL` and a
  numeric operand; authors must write `(A < B) AND (B < C)`.

### 4.3 Logical/Boolean Operators

| Operator | Symbol | Left Operand | Right Operand | Result | Notes |
|----------|--------|--------------|---------------|--------|-------|
| AND | `AND`, `&` | BOOL | BOOL | BOOL | Bitwise for ANY_BIT |
| OR | `OR` | BOOL | BOOL | BOOL | Bitwise for ANY_BIT |
| XOR | `XOR` | BOOL | BOOL | BOOL | Bitwise for ANY_BIT |
| NOT | `NOT` | - | BOOL | BOOL | Bitwise for ANY_BIT |

**Bitwise Operations** (when applied to ANY_BIT types):

- `AND`, `&`, `OR`, and `XOR` operate on bit strings and produce the wider operand type when widths differ.
- `NOT` preserves the operand bit-string type.
- BOOL short-circuit and eager bit-string evaluation follow section 3.

```
// BYTE operations
B1 := 16#F0;
B2 := 16#0F;
Result := B1 AND B2;  // Result = 16#00
Result := B1 OR B2;   // Result = 16#FF
Result := B1 XOR B2;  // Result = 16#FF
Result := NOT B1;     // Result = 16#0F
```

### 4.4 Reference Operators

| Operator | Symbol | Operand | Result | Notes |
|----------|--------|---------|--------|-------|
| Dereference | `^` | REF_TO T | T | Access referenced value |
| Reference | `REF(x)` | T | REF_TO T | Function, get reference |

```
VAR
  myInt: INT := 42;
  pInt: REF_TO INT;
END_VAR

pInt := REF(myInt);    // Get reference
pInt^ := 100;          // Dereference and assign
```

## 5. Expression Types

### 5.1 Constant Expressions

Expressions that can be evaluated at compile time:

```
CONST_VAL := 3.14159 * 2.0;     // Compile-time constant
ARRAY_SIZE := 10 + 5;            // Used in declarations
```

### 5.2 Primary Expressions

Basic building blocks:

```
42                  // Integer literal
3.14                // Real literal
TRUE                // Boolean literal
'Hello'             // String literal
T#1s500ms           // Duration literal
MyVar               // Variable reference
MyEnum#Value        // Enumerated value
#LocalVar           // Siemens SCL local reference (vendor extension)
```

#### 5.2.1 Literal typing (implementer-specific)

- Untyped integer literals default to the **smallest integer type** that can represent the value.
  - Decimal literals prefer signed types (SINT → INT → DINT → LINT).
  - Based literals (`2#`, `8#`, `16#`) prefer unsigned types (USINT → UINT → UDINT → ULINT).
- Untyped real literals default to `LREAL`.
- Typed literal prefixes (e.g., `INT#`, `REAL#`, `WORD#`) always override.
- In assignments, returns, and call arguments, untyped numeric literals are coerced to the expected integer/real type when compatible.

IEC 61131-3 Ed.3 §6.3.3 and Tables 5–9 define literal forms but do not mandate a single default integer type; this project follows the smallest‑fit policy (see IEC‑DEC‑014).

#### 5.2.2 Siemens SCL local-reference prefix (extension)

`#identifier` is accepted as a `NameRef` in expression and statement contexts
as a documented Siemens SCL compatibility extension.

### 5.3 Postfix Expressions

```
arr[5]              // Array subscript
struct.field        // Member access
func(a, b)          // Function call
fb.method(x)        // Method call
ptr^                // Dereference
```

A dereference is a valid assignment target at parse time, and its right-hand
side may be any syntactically valid expression. In particular,
`ptr^ := INT#16#FF;` combines the postfix dereference form with a typed based
numeric literal and parses successfully. Type resolution, reference validity,
and assignment compatibility remain semantic checks.

#### 5.3.1 Parser classification for product call forms

An ordinary positional call such as `F(1, 2)` and a formal call such as
`F(A := 1, B := 2)` are both represented as a `CallExpr` with an `ArgList` and
distinct `Arg` children. Formal `:=` tokens remain argument connections; they
are not declaration aggregate initializers. Call binding and argument-type
validation remain semantic checks.

The parser recognizes `ADR(...)` and `SIZEOF(...)` as documented truST product
expression forms. `SIZEOF` has a deliberately wider syntactic operand surface
than its semantic acceptance surface:

- an explicit builtin or array type is represented as a type-reference
  operand;
- a name, field access, array index, dereference, or call is represented as an
  expression operand; and
- parsing an expression operand does not make it semantically valid.

The semantic `SIZEOF` contract in
`docs/specs/29-hir-sizeof-and-allocation.md` accepts only operands with the
documented statically known storage meaning and rejects call results and other
non-lvalue expressions. This parser/HIR split lets diagnostics operate on the
correct source shape without treating parse acceptance as semantic acceptance.

For source compatibility, the type keyword spelling `TIME()` is also retained
as a zero-argument call expression. This is a parser-shape contract only: name
binding and runtime meaning remain semantic concerns, and the parser does not
invent a callable declaration.

### 5.4 Parenthesized Expressions

```
(A + B) * C         // Grouping
((A > B) AND (C < D)) OR E
```

## 6. Type Checking Rules

### 6.1 Arithmetic Operations

| Left Type | Operator | Right Type | Result Type |
|-----------|----------|------------|-------------|
| Same integer type | +, -, *, / | Same integer type | That integer type |
| Widenable integer types | +, -, *, / | Widenable integer types | Accuracy-preserving common type |
| Same real type | +, -, *, / | Same real type | That real type |
| Widenable numeric types | +, -, *, / | Widenable numeric types | Accuracy-preserving common type |
| TIME | +, - | TIME | TIME |
| LTIME | +, - | LTIME | LTIME |
| TOD | + | TIME | TOD |
| LTOD | + | LTIME | LTOD |
| DT | + | TIME | DT |
| LDT | + | LTIME | LDT |
| DATE | - | DATE | TIME |
| LDATE | - | LDATE | LTIME |
| TOD | - | TIME | TOD |
| LTOD | - | LTIME | LTOD |
| TOD | - | TOD | TIME |
| LTOD | - | LTOD | LTIME |
| DT | - | TIME | DT |
| LDT | - | LTIME | LDT |
| DT | - | DT | TIME |
| LDT | - | LDT | LTIME |
| TIME | *, / | ANY_NUM | TIME |
| LTIME | *, / | ANY_NUM | LTIME |
| Same/widenable integer type | MOD | Same/widenable integer type | Accuracy-preserving common type |
| REAL/LREAL | ** | ANY_NUM | Base type after permitted conversion |

The temporal rows are the Structured Text operator forms of IEC 61131-3 Ed.3
section 6.6.2.5.12, Table 35. Operand order is part of the contract: duration
addition to `TOD`, `LTOD`, `DT`, or `LDT` uses the civil/date-time value on the
left, and multiplication or division uses `TIME` or `LTIME` on the left.
Short-family and long-family operands are not mixed implicitly. Swapped,
cross-family, date-addition, civil-time multiplication/division, and every
other unlisted temporal combination are compile-time type errors. Result-range
overflow remains the runtime error required by section 6.6.2.5.12.

The complete conversion matrix is the 2026-07-15 decision in
`docs/IEC_DECISIONS.md`. Integer division truncates toward zero as required by
IEC 61131-3 Ed.3 section 6.6.2.5.7. `MOD` has the corresponding signed
remainder, so `a = (a / b) * b + (a MOD b)` for nonzero `b`. Arithmetic and
unary negation are checked in the result type; overflow is a runtime error and
never wraps.

### 6.2 Comparison Operations

| Left Type | Right Type | Valid |
|-----------|------------|-------|
| ANY_NUM | ANY_NUM | Yes only with an accuracy-preserving common type |
| STRING | STRING | Yes (lexicographic) |
| TIME | TIME | Yes |
| DATE | DATE | Yes |
| BOOL | BOOL | Yes (`=` and `<>`; reviewed host extension: `TRUE >= FALSE` returns `TRUE`) |
| STRUCT | STRUCT | No (not an elementary type) |

### 6.3 Boolean Operations

| Operation | Operand Types | Result |
|-----------|---------------|--------|
| AND, OR, XOR | BOOL | BOOL |
| AND, OR, XOR | ANY_BIT | Wider of operands |
| NOT | BOOL | BOOL |
| NOT | ANY_BIT | Same bit width |

## 7. Error Conditions

### 7.1 Runtime Errors

1. **Division by zero**: Attempt to divide by zero
2. **Modulo by zero**: Attempt to apply `MOD` with a zero divisor
3. **Overflow**: Result or unary negation exceeds the operation's result type
4. **Invalid exponentiation domain**: Result cannot be represented by the
   declared numeric result contract
5. **Null dereference**: Dereferencing NULL reference

Operands are evaluated left first. A runtime expression error aborts the
containing assignment before its store, so the target retains its previous
value. The only skipped right operands are those covered by the BOOL
short-circuit policy in section 3.

### 7.2 Compile-time Errors

1. **Type mismatch**: Operands not compatible
2. **Invalid operand**: Wrong type for operator
3. **Undefined identifier**: Variable not declared
4. **Invalid call**: Function signature mismatch

## 8. Complex Expression Examples

### Arithmetic

```
// Quadratic formula discriminant
D := B * B - 4.0 * A * C;

// Distance calculation
Distance := SQRT(DX**2 + DY**2);

// Time calculation
TotalTime := BaseTime + T#1s * COUNT;
```

### Logical

```
// Complex condition
Valid := (Temp > MinTemp) AND (Temp < MaxTemp)
         AND NOT Error
         AND (Mode = Auto OR Override);

// Bit manipulation
Flags := (Flags AND NOT Mask) OR NewBits;
```

### Mixed

```
// Conditional with function calls
Result := SEL(Condition, ValueIfFalse, ValueIfTrue);

// Bounded value
Output := MIN(MAX(Input, LowLimit), HighLimit);

// String comparison
Match := (Name = 'ADMIN') OR (Name = 'ROOT');
```

## Implementation Notes

### Parser Requirements

1. Use precedence climbing or Pratt parsing for operator precedence.
2. Handle both symbols (`&`) and keywords (`AND`) for the same operator family.
3. Keep unary operators (`-`, `+`, `NOT`) right-associative.
4. Support chained comparisons such as `A < B < C` with left-to-right evaluation.
5. Treat function/method calls as primary expressions.

Parser support for a chained comparison preserves the left-associative source
shape; it is not semantic acceptance. Since the first comparison yields
`BOOL`, numeric `A < B < C` is rejected by type checking.

### AST Node Types

```
Expression
├── Literal (integer, real, string, bool, time, date)
├── Identifier
├── BinaryOp (operator, left: Expression, right: Expression)
├── UnaryOp (operator, operand: Expression)
├── FunctionCall (name, arguments: [Expression])
├── MethodCall (object: Expression, method, arguments: [Expression])
├── ArrayAccess (array: Expression, index: Expression)
├── FieldAccess (object: Expression, field)
├── Dereference (operand: Expression)
└── Parenthesized (inner: Expression)
```

### Type Checker Requirements

1. Determine the type of each operand.
2. Apply only the accuracy-preserving common-type matrix before evaluating
   operator compatibility.
3. Verify operator compatibility and determine the result type.
4. Report the specific operator, operand types, and precise source location.
