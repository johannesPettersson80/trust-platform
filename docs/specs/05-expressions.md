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

### Rule 1: Precedence

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

### Rule 4: Short-Circuit Evaluation (Implementer-Specific)

Boolean expressions MAY be evaluated only to the extent necessary.

```
(A > B) & (C < D)
// If A <= B, the result is FALSE
// Evaluation of (C < D) may be skipped (Implementer-specific)
```

**Note**: The extent of short-circuit evaluation is Implementer-specific.

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
- Allow only an explicit whitelist of pure standard functions for evaluation; see
  `docs/IEC_DEVIATIONS.md` for the permitted set and rationale.

### Rule 6: Type Conversion

When operands require conversion, implicit conversion rules apply.

```
// Implicit widening
RealVar := IntVar + 5;        // IntVar promoted to REAL

// Explicit required for narrowing
IntVar := REAL_TO_INT(RealVar);
```

## 4. Operator Categories

### 4.1 Arithmetic Operators

| Operator | Symbol | Left Operand | Right Operand | Result | Notes |
|----------|--------|--------------|---------------|--------|-------|
| Add | `+` | ANY_NUM | ANY_NUM | ANY_NUM | Also TIME+TIME |
| Subtract | `-` | ANY_NUM | ANY_NUM | ANY_NUM | Also TIME-TIME |
| Multiply | `*` | ANY_NUM | ANY_NUM | ANY_NUM | Also TIME*ANY_NUM |
| Divide | `/` | ANY_NUM | ANY_NUM | ANY_NUM | Also TIME/ANY_NUM |
| Modulo | `MOD` | ANY_INT | ANY_INT | ANY_INT | |
| Exponent | `**` | ANY_REAL | ANY_NUM | ANY_REAL | |
| Negate | `-` | - | ANY_NUM | ANY_NUM | Unary |
| Plus | `+` | - | ANY_NUM | ANY_NUM | Unary (identity) |

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
- Operands must be compatible types
- String comparison is lexicographic

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
#LocalVar           // Siemens SCL local reference (extension, DEV-034)
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
for Siemens SCL compatibility; see `docs/IEC_DEVIATIONS.md` DEV-034.

### 5.3 Postfix Expressions

```
arr[5]              // Array subscript
struct.field        // Member access
func(a, b)          // Function call
fb.method(x)        // Method call
ptr^                // Dereference
```

### 5.4 Parenthesized Expressions

```
(A + B) * C         // Grouping
((A > B) AND (C < D)) OR E
```

## 6. Type Checking Rules

### 6.1 Arithmetic Operations

| Left Type | Operator | Right Type | Result Type |
|-----------|----------|------------|-------------|
| ANY_INT | +, -, *, / | ANY_INT | Widest INT type |
| ANY_REAL | +, -, *, / | ANY_REAL | Widest REAL type |
| ANY_INT | +, -, *, / | ANY_REAL | REAL (promoted) |
| TIME | +, - | TIME | TIME |
| TIME | * | ANY_NUM | TIME |
| ANY_INT | MOD | ANY_INT | Widest INT type |
| ANY_REAL | ** | ANY_NUM | REAL/LREAL |

### 6.2 Comparison Operations

| Left Type | Right Type | Valid |
|-----------|------------|-------|
| ANY_NUM | ANY_NUM | Yes (with promotion) |
| STRING | STRING | Yes (lexicographic) |
| TIME | TIME | Yes |
| DATE | DATE | Yes |
| BOOL | BOOL | Yes (= and <> only) |
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
2. **Overflow**: Result exceeds type range
3. **Null dereference**: Dereferencing NULL reference

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
2. Apply promotion rules before evaluating operator compatibility.
3. Verify operator compatibility and determine the result type.
4. Report the specific operator, operand types, and precise source location.
