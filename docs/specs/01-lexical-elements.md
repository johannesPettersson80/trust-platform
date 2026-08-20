# Lexical Elements

IEC 61131-3 Edition 3.0 (2013) - Section 6.1, 6.2, 6.3

This specification defines the lexical elements for the trust-syntax lexer.

## 1. Character Set (Table 1, Section 6.1.1)

The character set is based on ISO/IEC 10646:2012 (Unicode).

| No. | Description | Notes |
|-----|-------------|-------|
| 1 | ISO/IEC 10646 | Standard requires Unicode support; trust-lsp currently supports ASCII only (DEV-013) |
| 2a | Lower case characters | a-z |
| 2b | Number sign | `#` (used in typed literals; truST also accepts Siemens SCL `#identifier` local references as a vendor extension) |
| 2c | Dollar sign | `$` (used in string escapes) |

**Case Sensitivity Rule**: When lower-case letters are supported, the case of letters shall NOT be significant in language elements, except:
- Within comments
- Within string literals
- Within variables of type STRING and WSTRING

## 2. Identifiers (Table 2, Section 6.1.2)

An identifier is a string of letters, digits, and underscores which shall begin with a letter or underscore character.

### Rules

1. **Case insensitivity**: `abcd`, `ABCD`, and `aBCd` shall be interpreted identically
2. **Underscore significance**: `A_BCD` and `AB_CD` are different identifiers
3. **Multiple underscores forbidden**:
   - `__LIM_SW5` (leading double underscore) - INVALID
   - `LIM__SW5` (embedded double underscore) - INVALID
4. **Trailing underscores forbidden**: `LIM_SW5_` - INVALID
5. **Minimum uniqueness**: At least 6 characters of uniqueness shall be supported
6. **Maximum length**: Implementer specific

**Implementation note (DEV-013)**: trust-lsp currently validates identifiers using ASCII-only rules (A-Z, a-z, 0-9, `_`) with ASCII case-folding, and does not accept Unicode identifiers as allowed by IEC 61131-3 §6.1.1–6.1.2 (Table 1–2).

### Features

| No. | Description | Examples |
|-----|-------------|----------|
| 1 | Upper case letters and numbers | `IW215`, `QX75`, `IDENT` |
| 2 | Upper/lower case, numbers, embedded underscore | `LIM_SW_5`, `LimSw5`, `abcd`, `ab_Cd` |
| 3 | Upper/lower case, numbers, leading or embedded underscore | `_MAIN`, `_12V7` |

## 3. Keywords (Section 6.1.3)

Keywords are unique combinations of characters utilized as individual syntactic elements.

### Rules

1. Keywords shall not contain embedded spaces
2. Case of characters shall NOT be significant (e.g., `FOR` and `for` are equivalent)
3. Keywords shall not be used for other purposes (e.g., variable names)

### Complete Keyword List

#### Data Types
```
BOOL, SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT
REAL, LREAL
BYTE, WORD, DWORD, LWORD
STRING, WSTRING, CHAR, WCHAR
TIME, LTIME, DATE, LDATE, TIME_OF_DAY, TOD, LTIME_OF_DAY, LTOD
DATE_AND_TIME, DT, LDATE_AND_TIME, LDT
```

#### Generic Types
```
ANY, ANY_DERIVED, ANY_ELEMENTARY, ANY_MAGNITUDE, ANY_NUM
ANY_REAL, ANY_INT, ANY_UNSIGNED, ANY_SIGNED, ANY_DURATION
ANY_BIT, ANY_CHARS, ANY_STRING, ANY_CHAR, ANY_DATE
```

#### Variable Declarations
```
VAR, VAR_INPUT, VAR_OUTPUT, VAR_IN_OUT, VAR_TEMP
VAR_GLOBAL, VAR_EXTERNAL, VAR_ACCESS, VAR_CONFIG
END_VAR
CONSTANT, RETAIN, NON_RETAIN
AT
```

#### Type Declarations
```
TYPE, END_TYPE, STRUCT, END_STRUCT, OVERLAP, ARRAY, OF
```

#### Program Organization Units
```
FUNCTION, END_FUNCTION
FUNCTION_BLOCK, END_FUNCTION_BLOCK
PROGRAM, END_PROGRAM
CLASS, END_CLASS
INTERFACE, END_INTERFACE
METHOD, END_METHOD
PROPERTY, END_PROPERTY
NAMESPACE, END_NAMESPACE
USING
```
Property keywords are defined in IEC 61131-3 Ed.3 §6.6.5 (Table 50).

#### OOP Keywords
```
EXTENDS, IMPLEMENTS, OVERRIDE, FINAL, ABSTRACT
THIS, SUPER
PUBLIC, PRIVATE, PROTECTED, INTERNAL
```

#### Property Accessors
```
GET, END_GET
SET, END_SET
```
Accessor keywords are defined in IEC 61131-3 Ed.3 §6.6.5 (Table 50).

#### Control Flow
```
IF, THEN, ELSIF, ELSE, END_IF
CASE, OF, END_CASE
FOR, TO, BY, DO, END_FOR
WHILE, END_WHILE
REPEAT, UNTIL, END_REPEAT
EXIT, CONTINUE, RETURN
JMP
```
`JMP` is defined in IEC 61131-3 Ed.3 Table 72.

#### Operators
```
AND, OR, XOR, NOT, MOD
```

#### Boolean Literals
```
TRUE, FALSE
```

#### Reference Operations
```
REF, REF_TO, NULL
```

#### Configuration
```
CONFIGURATION, END_CONFIGURATION
RESOURCE, END_RESOURCE, ON
TASK, WITH
```

#### SFC Elements
```
STEP, END_STEP, INITIAL_STEP
TRANSITION, END_TRANSITION, FROM, TO
ACTION, END_ACTION
```

> **SFC profile note**: truST reserves the IEC SFC keywords and ships a visual
> SFC editor, but textual SFC body syntax is not specified in the Structured
> Text parser. Use the visual-editor documentation for current authoring scope;
> see `docs/public/develop/visual-editors/sfc.md`. This is a truST authoring
> profile boundary, not an IEC deviation in ST behavior.

#### Special
```
EN, ENO
R_EDGE, F_EDGE
READ_ONLY, READ_WRITE
```

#### Implementation Extensions (Reserved Keywords)

The following keywords are reserved by truST but are **not** part of the IEC
61131-3 keyword list. They are documented truST/vendor extensions, not IEC
deviations:

```
VAR_STAT
PERSISTENT
POINTER
UNION, END_UNION
NEW, __NEW, __DELETE
ADR, SIZEOF
TEST_PROGRAM, END_TEST_PROGRAM
TEST_FUNCTION_BLOCK, END_TEST_FUNCTION_BLOCK
```

`VAR_STAT` is a shipped vendor-extension keyword. Runtime semantics are defined
here and in the runtime specification: function statics persist across calls, method statics persist per
instance and per method, and `PROGRAM`/`FUNCTION_BLOCK`/`CLASS` `VAR_STAT` behaves as ordinary
instance storage.

## 4. White Space (Section 6.1.4)

White space characters (space, tab, newline, etc.) may be inserted anywhere except:
- Within keywords
- Within literals
- Within enumerated values
- Within identifiers
- Within directly represented variables
- Within delimiter combinations (e.g., `:=`, `(*`)

For the lossless truST token stream, each maximal run of space, horizontal tab,
carriage return, line feed, or form feed is emitted as `Whitespace`. These five
characters are parser trivia but remain part of the exact source partition and
therefore retain their original byte ranges.

## 5. Comments (Table 3, Section 6.1.5)

| No. | Type | Syntax | Example |
|-----|------|--------|---------|
| 1 | Single-line | `//...` | `X:= 13; // comment` |
| 2a | Multi-line | `(* ... *)` | `(* multi-line comment *)` |
| 2b | Multi-line (alt) | `/* ... */` | `/* multi-line comment */` |
| 3a | Nested | `(* ... (* ... *) ... *)` | `(* (* NESTED *) *)` |
| 3b | Nested (alt) | `/* ... /* ... */ ... */` | `/* /* NESTED */ */` |

### Rules

1. Single-line comments end at line feed, newline, form feed, or carriage return
2. In single-line comments, `(*`, `*)`, `/*`, `*/` have no special meaning
3. In multi-line comments, `//` has no special meaning
4. Comments are permitted anywhere spaces are allowed, except within string literals
5. Comments have no syntactic or semantic significance - treated as white space
6. Nested comments must use matching pairs

The truST lexer closes `//` immediately before the first carriage return, line
feed, or form feed; the terminator is emitted separately as `Whitespace`.
Pascal-style `(* ... *)` comments nest only on another `(*` and close only on
the matching `*)`. C-style `/* ... */` comments likewise nest only on `/*` and
close only on `*/`. Delimiters from the other block-comment family are ordinary
comment text and cannot change nesting depth. Comment markers inside a narrow
or wide string remain string content and never begin a comment.

## 6. Pragmas (Table 4, Section 6.2)

Pragmas are delimited by curly brackets `{` and `}`.

| No. | Description | Examples |
|-----|-------------|----------|
| 1 | Pragma | `{VERSION 2.0}`, `{AUTHOR JHC}`, `{x:= 256, y:= 384}` |

### Rules

1. Syntax and semantics of pragma contents are Implementer specific
2. Pragmas are permitted anywhere spaces are allowed, except within string literals

### truST lexer contract

A balanced brace-delimited pragma is emitted as one `Pragma` token whose source
text includes both delimiters and the complete pragma content. `Pragma` tokens
are classified as parser trivia: removing trivia removes the pragma but
preserves the exact tokenization of surrounding Structured Text. A brace
sequence inside a character string literal remains string content and is not
recognized as a pragma.

IEC 61131-3 Ed.3 section 6.2 and Table 4 specify the delimiters and placement
rules while leaving pragma content syntax and semantics implementer-specific.
The single lossless token and parser-trivia representation above is therefore a
truST product contract, not an IEC deviation.

## 6A. truST Emitted-Token Contract

The following table makes the lexer-facing product representation normative.
It does not change the IEC source syntax defined above; it fixes the internal
token boundary that parser and tooling consumers may rely on.

| Source form | Required emitted token sequence |
|-------------|---------------------------------|
| A supported keyword in any case spelling | The keyword's exact `Kw*` token; a neighboring non-keyword name remains `Ident` |
| `:= = <> < <= > >= + - * / ** & ;` | `Assign`, `Eq`, `Neq`, `Lt`, `LtEq`, `Gt`, `GtEq`, `Plus`, `Minus`, `Star`, `Slash`, `Power`, `Ampersand`, `Semicolon` |
| Siemens SCL `#identifier` extension | `Hash` followed by `Ident`; surrounding operators and delimiters remain separate tokens |
| Complete direct addresses such as `%IX0.0`, `%QW10`, `%MD100`, `%IB5` | One `DirectAddress` token |
| Incomplete direct addresses `%I*`, `%Q*`, `%M*` | One `DirectAddress` token; lexical recognition does not make the form contextually valid outside the uses permitted by `docs/specs/03-variables.md` section 5 |
| `//...` up to but excluding its line terminator, and `(* ... *)` through its matching terminator | One `LineComment` or `BlockComment` token, respectively; both are parser trivia |
| `1..5`, `1.0`, and malformed `1.` | `IntLiteral`, `DotDot`, `IntLiteral`; `RealLiteral`; and one `Error`, respectively |
| Integer, real, duration, narrow-string, and wide-string forms defined in sections 7 through 10 | The corresponding `IntLiteral`, `RealLiteral`, `TimeLiteral`, `StringLiteral`, or `WideStringLiteral` token |
| Typed numeric or Boolean forms such as `INT#-123` and `BOOL#TRUE` | A `TypedLiteralPrefix` followed by the separately emitted payload tokens: `Minus`, `IntLiteral` or `KwTrue`, respectively |
| Date, time-of-day, and combined date-and-time forms defined in section 11 | One `DateLiteral`, `TimeOfDayLiteral`, or `DateAndTimeLiteral` token, respectively |
| A narrow or wide string containing a malformed escape | One or more `Error` tokens and no `StringLiteral` or `WideStringLiteral` token for the malformed lexeme |
| A balanced pragma | One lossless `Pragma` token; it is parser trivia and does not change surrounding tokenization |

The operator spellings are aligned with IEC 61131-3 Ed.3 section 7.3.2,
Table 71; direct-address forms are aligned with section 6.5.5, Table 16. The
comment source forms, typed-literal source forms, and date/time source forms are
aligned with sections 6.1.5 and 6.3.2 through 6.3.5, Tables 3 and 5 through 9.
The `TokenKind` names, the Siemens SCL split, incomplete-address lexical
boundary, dot/range boundary, typed-literal token split, malformed-escape error
representation, comment trivia representation, and pragma trivia
representation are truST product contracts. They are not IEC deviations.

### Source-boundary contract

For every token emitted by `lex`, `Token::range` is the half-open UTF-8 byte
range of that token's exact source lexeme. Ranges include trivia tokens and
advance in source order without shifting the following token. For example,
lexing `abc := 123` yields ranges `0..3`, `3..4`, and `4..6` for `abc`, the
space, and `:=`.

For every pair emitted by `lex_with_text`, the returned text is exactly
`source[token.range]`. It is not normalized, case-folded, or reconstructed.
For example, the non-trivia text slices for `x := 42` are exactly `x`, `:=`,
and `42`.

IEC 61131-3 defines the external source forms but does not define truST's
internal byte-range or borrowed-source-slice API. These source-boundary rules
are therefore truST product contracts, not IEC decisions or deviations.

### Lexical rejection and value-validation boundary

The lexer fails closed for a contiguous candidate that uses the spelling of an
identifier or literal but violates the corresponding rules below:

- an identifier containing multiple leading or embedded underscores, or a
  trailing underscore, emits an `Error` spanning the complete candidate rather
  than a valid `Ident` prefix plus residual tokens;
- a numeric candidate with a leading, trailing, or repeated separator, a digit
  outside its declared base, an exponent without digits, or a sign embedded in
  a based number emits an `Error` and no valid numeric token for a fragment of
  that candidate;
- a duration candidate containing an exponent, or a fractional unit followed
  by another less-significant unit, emits an `Error` rather than a valid
  duration prefix;
- a date, time-of-day, or date-and-time candidate whose external field widths
  do not match section 11 emits an `Error`; a shape-valid candidate with an
  invalid calendar or clock value retains its temporal token for later value
  validation;
- a narrow string accepts only the narrow quote escape and exactly two
  hexadecimal digits, while a wide string accepts only the wide quote escape
  and exactly four hexadecimal digits; a wrong-family, short, unknown, or
  unterminated escape emits `Error` and no string token; and
- an unterminated block comment or pragma emits `Error` through the available
  candidate text.

This fail-closed token boundary is a truST diagnostic and tooling contract.
IEC 61131-3 Ed.3 sections 6.1.2 and 6.3.2 define the invalid spellings but do
not prescribe recovery-token boundaries.

Lexing recognizes the complete external *shape* of duration, date,
time-of-day, and date-and-time literals. Compilation then validates their
value:

- a date uses the proleptic Gregorian calendar, including century leap-year
  rules;
- time of day is within `00:00:00` through the last representable fraction
  before `24:00:00`;
- each date-and-time component satisfies both rules; and
- duration composition is accumulated in source order, allows overflow of the
  most significant written unit, permits a fraction only on the least
  significant written unit, and must fit the destination TIME/LTIME
  representation.

A shape-valid but value-invalid date/time token therefore remains a date/time
token and produces a compile-time value diagnostic; it is not retokenized as
identifiers and punctuation. A typed-literal prefix is likewise lexed
separately from its payload, while parsing and semantic analysis require a
supported elementary type and a compatible, in-range payload. These staged
validation rules preserve one source identity across lexer, parser, semantic
analysis, and runtime lowering.

### Parser handoff boundary

The parser consumes valid lexical tokens without reconstructing their source
spelling. Comments, pragmas, and whitespace remain lossless trivia and may
appear wherever the grammar permits whitespace. An `Error` token produced by
the fail-closed candidate rules above cannot be reinterpreted as a valid
identifier, literal, comment, pragma, or collection of valid token prefixes;
it causes a visible parse failure at the bounded containing construct. A
shape-valid temporal token remains syntactically accepted even when later
value validation rejects its calendar or clock fields. This handoff keeps
lexical shape, parser structure, and semantic value validation as distinct
proof boundaries.

## 7. Numeric Literals (Table 5, Section 6.3.2)

| No. | Type | Examples | Notes |
|-----|------|----------|-------|
| 1 | Integer | `-12`, `0`, `123_4`, `+986` | Decimal |
| 2 | Real | `0.0`, `0.4560`, `3.14159_26` | With decimal point |
| 3 | Real with exponent | `-1.34E-12`, `1.0E+6`, `1.234e6` | Scientific notation |
| 4 | Binary | `2#1111_1111`, `2#1110_0000` | Base 2 |
| 5 | Octal | `8#377`, `8#340` | Base 8 (DEPRECATED) |
| 6 | Hexadecimal | `16#FF`, `16#ff`, `16#E0` | Base 16 |
| 7 | Boolean (numeric) | `0`, `1` | |
| 8 | Boolean (keyword) | `FALSE`, `TRUE` | |
| 9 | Typed literal | `INT#-123`, `WORD#16#AFF`, `BOOL#TRUE` | Type prefix with `#` |

### Rules

1. Underscores `_` between digits are not significant and can be used as separators
2. No other use of underscores in numeric literals is allowed
3. Real literals are distinguished by presence of a decimal point
4. Exponents indicate power of ten
5. Based numbers (2#, 8#, 16#) shall NOT contain leading sign `+` or `-`
6. Based numbers are interpreted as bit string literals
7. For base 16, letters A-F (or a-f) represent decimal 10-15
8. Octal literals (8#) are DEPRECATED

## 8. Character String Literals (Table 6, Section 6.3.3)

### Single-byte Strings (using single quotes)

| No. | Description | Example |
|-----|-------------|---------|
| 1a | Empty string | `''` |
| 1b | Single character (CHAR) | `'A'` |
| 1c | Space character | `' '` |
| 1d | Single quote in string | `'$''` |
| 1e | Double quote in string | `'"'` |
| 1f | Escape sequences | `'$R$L'` |
| 1g | Hex character (2 digits) | `'$0A'` |

### Double-byte Strings (using double quotes)

| No. | Description | Example |
|-----|-------------|---------|
| 2a | Empty string | `""` |
| 2b | Single character (WCHAR) | `"A"` |
| 2c | Space character | `" "` |
| 2d | Single quote in string | `"'"` |
| 2e | Double quote in string | `"$""` |
| 2f | Escape sequences | `"$R$L"` |
| 2h | Hex character (4 digits) | `"$00C4"` |

### Typed String Literals

| No. | Description | Example |
|-----|-------------|---------|
| 3a | Typed string | `STRING#'OK'` |
| 3b | Typed character | `CHAR#'X'` |
| 4a | Typed double-byte string | `WSTRING#"OK"` |
| 4b | Typed double-byte character | `WCHAR#"X"` |
| 4c | Typed double-byte string (single quotes) | `WSTRING#'OK'` |
| 4d | Typed double-byte character (single quotes) | `WCHAR#'X'` |

## 9. Escape Sequences (Table 7, Section 6.3.3)

Two-character combinations beginning with dollar sign `$`:

| No. | Meaning | Combination |
|-----|---------|-------------|
| 1 | Dollar sign | `$$` |
| 2 | Single quote | `$'` |
| 3 | Line feed | `$L` or `$l` |
| 4 | Newline | `$N` or `$n` |
| 5 | Form feed (page) | `$P` or `$p` |
| 6 | Carriage return | `$R` or `$r` |
| 7 | Tab | `$T` or `$t` |
| 8 | Double quote | `$"` |

**Notes**:
- `$'` is only valid inside single-quoted strings
- `$"` is only valid inside double-quoted strings
- `$N` (newline) provides implementation-independent line ending

## 10. Duration Literals (Table 8, Section 6.3.4)

### Time Unit Abbreviations

| Abbrev. | Meaning |
|---------|---------|
| d | Day |
| h | Hour |
| m | Minute |
| s | Second |
| ms | Millisecond |
| us | Microsecond (no μ available) |
| ns | Nanosecond |

### Prefixes

| Short | Long |
|-------|------|
| `T#` | `TIME#` |
| `LT#` | `LTIME#` |

### Examples

| No. | Description | Examples |
|-----|-------------|----------|
| 2a | Without underscore (short) | `T#14ms`, `T#-14ms`, `T#14.7h`, `t#14.7d` |
| 2b | Without underscore (long) | `TIME#14ms`, `TIME#-14ms` |
| 3a | With underscore (short) | `t#25h_15m`, `t#5d_14h_12m_18s_3.5ms` |
| 3b | With underscore (long) | `TIME#25h_15m`, `LTIME#5d_14h_12m_18s_3.5ms` |

### Rules

1. Duration literals are delimited by `T#`, `TIME#`, `LT#`, or `LTIME#`
2. Units can be in upper or lower case
3. Underscore separators can be used between units
4. The least significant unit can use real notation (e.g., `3.5ms`)
5. "Overflow" of most significant unit is permitted (e.g., `T#25h_15m`)
6. Both positive and negative values are allowed

## 11. Date and Time Literals (Table 9, Section 6.3.5)

### Date Literals

| No. | Type | Prefix | Example |
|-----|------|--------|---------|
| 1a | Date (long) | `DATE#` or `date#` | `DATE#1984-06-25` |
| 1b | Date (short) | `D#` | `D#1984-06-25` |
| 2a | Long date (long) | `LDATE#` | `LDATE#2012-02-29` |
| 2b | Long date (short) | `LD#` | `LD#1984-06-25` |

### Time of Day Literals

| No. | Type | Prefix | Example |
|-----|------|--------|---------|
| 3a | Time of day (long) | `TIME_OF_DAY#` | `TIME_OF_DAY#15:36:55.36` |
| 3b | Time of day (short) | `TOD#` | `TOD#15:36:55.36` |
| 4a | Long time of day (short) | `LTOD#` | `LTOD#15:36:55.36` |
| 4b | Long time of day (long) | `LTIME_OF_DAY#` | `LTIME_OF_DAY#15:36:55.36` |

### Date and Time Literals

| No. | Type | Prefix | Example |
|-----|------|--------|---------|
| 5a | Date and time (long) | `DATE_AND_TIME#` | `DATE_AND_TIME#1984-06-25-15:36:55.360227400` |
| 5b | Date and time (short) | `DT#` | `DT#1984-06-25-15:36:55.360_227_400` |
| 6a | Long date and time (long) | `LDATE_AND_TIME#` | `LDATE_AND_TIME#1984-06-25-15:36:55.360_227_400` |
| 6b | Long date and time (short) | `LDT#` | `LDT#1984-06-25-15:36:55.360_227_400` |

### Format

- Date format: `YYYY-MM-DD`
- Time format: `HH:MM:SS[.fraction]`
- Combined format: `YYYY-MM-DD-HH:MM:SS[.fraction]`
- Underscores can separate fraction digits

## Implementation Notes for trust-syntax

### Token Categories

1. **Keywords**: All reserved words (case-insensitive)
2. **Identifiers**: User-defined names
3. **Literals**: Numbers, strings, durations, dates
4. **Operators**: Symbols and operator keywords
5. **Delimiters**: Punctuation (`;`, `,`, `.`, etc.)
6. **Comments**: Preserved as `LineComment` or `BlockComment` trivia tokens
7. **Pragmas**: For implementer-specific processing
8. **Whitespace**: Separators (can be discarded)

### SyntaxKind token and trivia representation

Every lexer `TokenKind` converts to the same-named `SyntaxKind`. `Eof` is the
final token-kind discriminant; kinds through `Eof` are tokens and later kinds
are composite syntax nodes. `is_node()` is the inverse of `is_token()`.

The parser-trivia classifier contains `Whitespace`, `LineComment`,
`BlockComment`, and `Pragma`. Ordinary identifiers are not trivia. These are
truST concrete-syntax representation rules, not IEC source-language
additions.

### Lexer Error Conditions

1. Invalid identifier (double underscore, trailing underscore)
2. Unterminated string literal
3. Unterminated comment
4. Invalid escape sequence in string
5. Invalid numeric literal format
6. Invalid duration/date format
