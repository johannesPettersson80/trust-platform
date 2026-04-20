# Statements

IEC 61131-3 Edition 3.0 (2013) - Section 7.3.3

This specification defines statement syntax for trust-syntax parser.

## 1. Statement Overview (Table 72)

The ST language statements are:

| Category | Statements |
|----------|------------|
| Assignment | `:=`, `?=` |
| Call | Function, Function Block, Method |
| Control | `RETURN` |
| Selection | `IF...THEN...END_IF`, `CASE...OF...END_CASE` |
| Iteration | `FOR...END_FOR`, `WHILE...END_WHILE`, `REPEAT...END_REPEAT` |
| Jump | `EXIT`, `CONTINUE`, `JMP` |
| Empty | `;` |

**Deviation**:
- `JMP` statements are parsed and labels are resolved/validated. The current
  diagnostics pass already reports unreachable statements after terminators and
  constant-`IF` branches, but truST does not yet build a full control-flow
  graph for whole-body reachability analysis. (IEC 61131-3 Ed.3, Table 72;
  DEV-008)

### Statement Termination

All statements are terminated with a semicolon `;`.

### Maximum Length

The maximum allowed length of statements is Implementer specific.

Multiple statements form a sequence and execute in the order written:

```text
A := 1;
B := A + 2;
C := B * 3;
```

## 2. Assignment Statement (Section 7.3.3.2)

### Syntax

```
variable := expression;
```

### Examples (Table 72)

| No. | Description | Example |
|-----|-------------|---------|
| 1a | Elementary type | `A := B;` |
| 1b | With implicit conversion | `A_Real := B_Int;` |
| 1c | User-defined type | `A_Struct1 := B_Struct1;` |
| 1d | Array assignment | `C_Array1 := D_Array1;` |
| 1e | FB instance | `A_Instance1 := B_Instance1;` |
| 1f | With expression | `CV := CV + 1;`, `C := SIN(X);` |

### Rules

1. Left side must be a modifiable variable (not CONSTANT, not VAR_INPUT)
2. Right side expression type must be compatible with left side type
3. For structured types, both sides must be the same type
4. Implicit type conversion follows defined rules (e.g., INT to REAL)

### Assignment Attempt (Section 6.6.6.7)

```
interface1 ?= interface2;
```

Checks if assignment is valid (for interface references). If the instance implements the interface, assigns; otherwise, assigns NULL.

**Rules**:
- `?=` is only valid for `REF_TO` targets and sources; the source may be `NULL`. (IEC 61131-3 Ed.3, 6.6.6.7.2, Table 71)
- Assignment attempt may yield `NULL`; callers must check before dereference. (IEC 61131-3 Ed.3, 6.6.6.7.2, Table 52)
- trust-hir does not enforce inheritance/interface compatibility for `?=`. (DEV-006)

## 3. Call Statements (Section 7.3.3.2.4)

Call statements share one syntax skeleton and then refine the callable target:
function name, function-block instance, or method receiver.

### 3.1 Shared Call Syntax

```text
callable(arguments);
```

| Call style | Syntax | Example |
|------------|--------|---------|
| Formal | `param := value`, `param => target` | `ADD(IN1 := A, IN2 := B)` |
| Non-formal | positional | `ADD(A, B)` |

Shared rules:

- Formal calls assign inputs/in-outs with `:=` and outputs with `=>`; ordering
  is not significant. (IEC 61131-3 Ed.3, 6.6.1.4.2, Table 71)
- Formal calls may be incomplete; any unassigned parameters use their declared
  initial value or the type default. (IEC 61131-3 Ed.3, 6.6.1.4.2)
- Non-formal calls must provide all parameters in order, excluding execution
  control parameters `EN` and `ENO`. (IEC 61131-3 Ed.3, 6.6.1.4.2; Table 50)
- Do not mix formal and non-formal styles within the same call. (IEC 61131-3
  Ed.3, 6.6.1.4.2)
- `=>` is only valid for `VAR_OUTPUT` / `ENO` bindings and is invalid for
  non-output parameters. (IEC 61131-3 Ed.3, 6.6.1.2.2, Table 71)

### 3.2 Function Calls

Functions may appear as standalone call statements or inside expressions:

```text
function_name(parameters);
variable := function_name(parameters);
```

Examples:

```
Y := SIN(X);
Z := MAX(A, B, C);
Distance := SQRT(X**2 + Y**2);
LogMessage('System started');
Result := SafeDivide(EN := Enabled, A := Num, B := Den, ENO => Success);
```

### 3.3 Function Block Calls

Function-block calls execute an instance and may update or read retained state:

```text
fb_instance(parameters);
```

Examples:

```
MyTimer(IN := Start, PT := T#5s);
Elapsed := MyTimer.ET;
TimerDone := MyTimer.Q;
MyFB(EN := Condition, Input := X, ENO => WasExecuted);
```

Input assignment may happen inline or through the instance fields before the
call:

```
MyTimer.IN := Start;
MyTimer.PT := T#5s;
MyTimer();

MyTimer(PT := T#10s);
```

### 3.4 Method Calls

Methods are invoked on an instance receiver:

```text
instance.method_name(parameters);
```

Examples:

```
Motor1.Start();
Motor1.SetSpeed(NewSpeed := 1000);
Status := Motor1.GetStatus();
```

## 4. RETURN Statement (Section 7.3.3.2.4)

### Syntax

```
RETURN;
// or
RETURN expression;  // For functions/methods with return value
```

### Examples

```
FUNCTION Max : INT
VAR_INPUT A, B: INT; END_VAR
  IF A > B THEN
    RETURN A;
  END_IF;
  RETURN B;
END_FUNCTION

// In FB/Program (early exit)
IF Error THEN
  RETURN;
END_IF;
```

### Rules

1. `RETURN expression;` is valid for functions and methods with a return value.
2. `RETURN;` is also valid for functions and methods when the implicit return variable has already been assigned on that control-flow path.
3. In programs, function blocks, and procedures, `RETURN;` performs an early exit.

## 5. IF Statement (Section 7.3.3.3.2)

### Syntax

```
IF condition THEN
  statements
END_IF;

IF condition THEN
  statements
ELSE
  statements
END_IF;

IF condition1 THEN
  statements
ELSIF condition2 THEN
  statements
ELSIF condition3 THEN
  statements
ELSE
  statements
END_IF;
```

### Examples

```
// Simple IF
IF Temperature > MaxTemp THEN
  Alarm := TRUE;
END_IF;

// IF-ELSE
IF Sensor THEN
  Output := TRUE;
ELSE
  Output := FALSE;
END_IF;

// IF-ELSIF-ELSE (quadratic formula)
D := B*B - 4.0*A*C;
IF D < 0.0 THEN
  NROOTS := 0;
ELSIF D = 0.0 THEN
  NROOTS := 1;
  X1 := -B / (2.0*A);
ELSE
  NROOTS := 2;
  X1 := (-B + SQRT(D)) / (2.0*A);
  X2 := (-B - SQRT(D)) / (2.0*A);
END_IF;
```

### Rules

1. Conditions must evaluate to BOOL
2. ELSIF can appear multiple times
3. ELSE is optional
4. First TRUE condition's block executes; rest skipped

## 6. CASE Statement (Section 7.3.3.3.3)

### Syntax

```
CASE selector OF
  value1: statements
  value2, value3: statements
  value4..value5: statements
  ELSE statements
END_CASE;
```

### Examples

```
// With integer selector
CASE Mode OF
  0: Output := 'Idle';
  1: Output := 'Running';
  2: Output := 'Paused';
  ELSE Output := 'Unknown';
END_CASE;

// Multiple values and ranges
TW := WORD_BCD_TO_INT(THUMBWHEEL);
TW_ERROR := 0;
CASE TW OF
  1, 5:     DISPLAY := OVEN_TEMP;
  2:        DISPLAY := MOTOR_SPEED;
  3:        DISPLAY := GROSS - TARE;
  4, 6..10: DISPLAY := STATUS(TW - 4);
  ELSE
    DISPLAY := 0;
    TW_ERROR := 1;
END_CASE;

// With enumeration
CASE TrafficLight OF
  Green:  AllowPass := TRUE;
  Amber:  AllowPass := FALSE; PrepareStop := TRUE;
  Red:    AllowPass := FALSE; PrepareStop := FALSE;
END_CASE;
```

### Rules

1. Selector must be an elementary data type. (IEC 61131-3 Ed.3, 7.3.3.3.3)
2. Case labels are literals, enumerated values, or subranges; label types must match the selector. Unqualified enum members and typed enum literals (`Type#Value`) are both accepted. (IEC 61131-3 Ed.3, 7.3.3.3.3; Table 72)
3. Ranges use `..` syntax (e.g., `1..10`); multiple values are comma-separated.
4. ELSE executes when the selector matches no label; otherwise no statements execute (ELSE optional). (IEC 61131-3 Ed.3, 7.3.3.3.3)
5. trust-hir warns when ELSE is omitted unless the selector is an enum and the labels cover all enum values.

## 7. FOR Statement (Section 7.3.3.4.2)

### Syntax

```
FOR control_var := initial TO final DO
  statements
END_FOR;

FOR control_var := initial TO final BY increment DO
  statements
END_FOR;
```

### Examples

```
// Simple FOR loop
FOR I := 1 TO 10 DO
  Sum := Sum + I;
END_FOR;

// With step value
FOR I := 0 TO 100 BY 10 DO
  Values[I / 10] := GetSample(I);
END_FOR;

// Counting down
FOR I := 10 TO 1 BY -1 DO
  Countdown[I] := I;
END_FOR;

// Search with EXIT
J := 101;
FOR I := 1 TO 100 BY 2 DO
  IF WORDS[I] = 'KEY' THEN
    J := I;
    EXIT;
  END_IF;
END_FOR;
```

### Rules

1. Control variable, initial, and final must be expressions of the same integer type (ANY_INT)
2. Increment must be an expression of the same integer type
3. If BY is omitted, increment defaults to 1
4. Control variable, initial, and final must NOT be modified in loop body
5. Test is performed at start of each iteration
6. Loop terminates when control variable exceeds final value
7. Value of control variable after loop completion is Implementer specific

### Termination Test

- Positive increment: terminates when `control_var > final`
- Negative increment: terminates when `control_var < final`

## 8. WHILE Statement (Section 7.3.3.4.3)

### Syntax

```
WHILE condition DO
  statements
END_WHILE;
```

### Examples

```
// Basic WHILE
J := 1;
WHILE J <= 100 AND WORDS[J] <> 'KEY' DO
  J := J + 2;
END_WHILE;

// Processing until complete
WHILE NOT ProcessComplete DO
  ProcessNextItem();
END_WHILE;
```

### Rules

1. Condition must evaluate to BOOL
2. Condition tested BEFORE each iteration
3. If condition is initially FALSE, body never executes
4. Error if termination cannot be guaranteed (infinite loop)
5. Should NOT be used for inter-process synchronization

Implementation note (trust-hir): termination-guarantee analysis is not implemented; see `docs/IEC_DEVIATIONS.md`.

## 9. REPEAT Statement (Section 7.3.3.4.4)

### Syntax

```
REPEAT
  statements
UNTIL condition
END_REPEAT;
```

### Examples

```
// Basic REPEAT
J := -1;
REPEAT
  J := J + 2;
UNTIL J = 101 OR WORDS[J] = 'KEY'
END_REPEAT;

// Read until valid
REPEAT
  Value := ReadInput();
UNTIL Value >= 0 AND Value <= 100
END_REPEAT;
```

### Rules

1. Condition must evaluate to BOOL
2. Condition tested AFTER each iteration
3. Body executes AT LEAST ONCE
4. Loop terminates when condition becomes TRUE
5. Error if termination cannot be guaranteed

Implementation note (trust-hir): termination-guarantee analysis is not implemented; see `docs/IEC_DEVIATIONS.md`.

## 10. EXIT Statement (Section 7.3.3.4.6)

### Syntax

```
EXIT;
```

### Behavior

Exits the innermost enclosing loop (FOR, WHILE, or REPEAT).

### Example

```
SUM := 0;
FOR I := 1 TO 3 DO
  FOR J := 1 TO 2 DO
    SUM := SUM + 1;
    IF FLAG THEN
      EXIT;           // Exits inner FOR loop only
    END_IF;
    SUM := SUM + 1;
  END_FOR;
  SUM := SUM + 1;
END_FOR;
// If FLAG=FALSE: SUM=15
// If FLAG=TRUE:  SUM=6
```

### Rules

1. Must be inside a loop
2. Only exits innermost loop
3. If EXIT supported, it must work for all loop types (FOR, WHILE, REPEAT)

## 11. CONTINUE Statement (Section 7.3.3.4.5)

### Syntax

```
CONTINUE;
```

### Behavior

Jumps to the end of the current iteration, proceeding to the next iteration.

### Example

```
SUM := 0;
FOR I := 1 TO 3 DO
  FOR J := 1 TO 2 DO
    SUM := SUM + 1;
    IF FLAG THEN
      CONTINUE;       // Skip rest of inner loop body
    END_IF;
    SUM := SUM + 1;
  END_FOR;
  SUM := SUM + 1;
END_FOR;
// If FLAG=FALSE: SUM=15
// If FLAG=TRUE:  SUM=9
```

### Rules

1. Must be inside a loop
2. Affects only innermost loop
3. If CONTINUE supported, it must work for all loop types

## 12. Label Statement (Section 7.3.3, Table 72)

### Syntax

```
label: statement
```

### Examples

```
Start: X := 1;
JMP Start;
```

### Rules

1. Labels are identifiers and are case-insensitive
2. Labels are scoped to the enclosing POU or ACTION body
3. Labels must be unique within the same label scope
4. JMP targets must resolve to a label in the same scope (Table 72)

## 13. JMP Statement

### Syntax

```text
JMP label;
```

### Rules

1. `label` must resolve to a declared label in the same POU or ACTION body
2. Forward and backward jumps are both allowed after label resolution
3. Reachability diagnostics currently cover terminator-following statements and
   constant-branch dead code; full CFG-based jump analysis is still pending

### Example

```text
Start: X := 1;
JMP Start;
```

## 14. Empty Statement

### Syntax

```
;
```

### Use Case

Placeholder where statement is syntactically required but no action needed.

```
CASE Mode OF
  0: ;                    // Do nothing for mode 0
  1: ProcessMode1();
  2: ProcessMode2();
END_CASE;
```

## Implementation Notes for trust-syntax Parser

### AST Node Types

```
Statement
├── Assignment (variable: LValue, expression: Expression)
├── AssignmentAttempt (variable: LValue, expression: Expression)
├── FunctionCall (name: String, arguments: [Argument])
├── FBCall (instance: String, arguments: [Argument])
├── MethodCall (object: Expression, method: String, arguments: [Argument])
├── Return (expression: Option<Expression>)
├── If (condition: Expression, then_branch: [Statement],
│       elsif_branches: [(Expression, [Statement])], else_branch: Option<[Statement]>)
├── Case (selector: Expression, cases: [(CaseLabel, [Statement])], else_branch: Option<[Statement]>)
├── For (control_var: String, initial: Expression, final: Expression,
│        step: Option<Expression>, body: [Statement])
├── While (condition: Expression, body: [Statement])
├── Repeat (body: [Statement], condition: Expression)
├── Exit
├── Continue
├── Label (name: String, statement: Statement)
└── Empty
```

### Parsing Considerations

1. All statements end with `;` (except after END_xxx keywords)
2. IF/CASE/FOR/WHILE/REPEAT are block statements
3. Nested blocks must match correctly
4. CASE labels can be values, ranges, or comma-separated lists
5. FOR increment can be negative

### Semantic Analysis

1. Type check all expressions
2. Verify control variable constraints in FOR
3. Ensure EXIT/CONTINUE are inside loops
4. Check CASE label uniqueness and type compatibility
5. Verify assignment target is modifiable

## Error Conditions

### Compile-time Errors

1. Type mismatch in assignment
2. Assignment to CONSTANT or VAR_INPUT
3. EXIT/CONTINUE outside loop
4. Duplicate CASE labels
5. CASE label type mismatch
6. Non-boolean condition in IF/WHILE/REPEAT
7. FOR control variable not integer type
8. JMP target label not declared in scope
9. Duplicate label declaration

### Runtime Errors

1. CASE selector value not in any case (and no ELSE)
2. Division by zero in expression
3. Array index out of bounds
4. Null reference dereference
