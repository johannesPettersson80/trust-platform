//! Token definitions for IEC 61131-3 Structured Text.
//!
//! This module defines all lexical tokens that can appear in ST source code.
//! The token kinds are designed to work with both the `logos` lexer generator
//! and the `rowan` lossless syntax tree library.

use logos::Logos;

fn lex_block_comment_pascal(lex: &mut logos::Lexer<TokenKind>) -> bool {
    lex_nested_comment(lex, b"(*", b"*)")
}

fn lex_block_comment_c(lex: &mut logos::Lexer<TokenKind>) -> bool {
    lex_nested_comment(lex, b"/*", b"*/")
}

fn lex_nested_comment(lex: &mut logos::Lexer<TokenKind>, open: &[u8], close: &[u8]) -> bool {
    let mut depth = 1usize;
    let bytes = lex.remainder().as_bytes();
    let mut i = 0usize;

    while i + 1 < bytes.len() {
        if bytes[i] == open[0] && bytes[i + 1] == open[1] {
            depth += 1;
            i += 2;
            continue;
        }
        if bytes[i] == close[0] && bytes[i + 1] == close[1] {
            depth -= 1;
            i += 2;
            if depth == 0 {
                lex.bump(i);
                return true;
            }
            continue;
        }
        i += 1;
    }

    lex.bump(bytes.len());
    false
}

/// All token kinds in IEC 61131-3 Structured Text.
///
/// Token kinds are divided into categories:
/// - Trivia (whitespace, comments) - preserved but not semantically significant
/// - Punctuation and operators
/// - Keywords (reserved words)
/// - Literals (numbers, strings, etc.)
/// - Identifiers
/// - Special tokens (errors, EOF)
#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
#[derive(Default)]
pub enum TokenKind {
    // =========================================================================
    // TRIVIA
    // =========================================================================
    /// Whitespace (spaces, tabs, newlines)
    #[regex(r"[ \t\r\n]+")]
    Whitespace,

    /// Single-line comment: // ...
    #[regex(r"//[^\r\n]*")]
    LineComment,

    /// Block comment: (* ... *) or /* ... */ (supports nesting).
    #[token("(*", lex_block_comment_pascal)]
    #[token("/*", lex_block_comment_c)]
    BlockComment,

    /// Pragma: { ... }
    /// IEC 61131-3 Section 6.2, Table 4
    /// Pragma contents are implementer-specific; treated as trivia
    #[regex(r"\{[^}]*\}")]
    Pragma,

    // =========================================================================
    // PUNCTUATION
    // =========================================================================
    /// `;`
    #[token(";")]
    Semicolon,

    /// `:`
    #[token(":")]
    Colon,

    /// `,`
    #[token(",")]
    Comma,

    /// `.`
    #[token(".")]
    Dot,

    /// `..`
    #[token("..")]
    DotDot,

    /// `(`
    #[token("(")]
    LParen,

    /// `)`
    #[token(")")]
    RParen,

    /// `[`
    #[token("[")]
    LBracket,

    /// `]`
    #[token("]")]
    RBracket,

    /// `#`
    #[token("#")]
    Hash,

    /// `^`
    #[token("^")]
    Caret,

    /// `@`
    #[token("@")]
    At,

    // =========================================================================
    // OPERATORS - Assignment
    // =========================================================================
    /// `:=`
    #[token(":=")]
    Assign,

    /// `=>`
    #[token("=>")]
    Arrow,

    /// `?=`
    #[token("?=")]
    RefAssign,

    // =========================================================================
    // OPERATORS - Comparison
    // =========================================================================
    /// `=`
    #[token("=")]
    Eq,

    /// `<>`
    #[token("<>")]
    Neq,

    /// `<`
    #[token("<")]
    Lt,

    /// `<=`
    #[token("<=")]
    LtEq,

    /// `>`
    #[token(">")]
    Gt,

    /// `>=`
    #[token(">=")]
    GtEq,

    // =========================================================================
    // OPERATORS - Arithmetic
    // =========================================================================
    /// `+`
    #[token("+")]
    Plus,

    /// `-`
    #[token("-")]
    Minus,

    /// `*`
    #[token("*")]
    Star,

    /// `/`
    #[token("/")]
    Slash,

    /// `**`
    #[token("**")]
    Power,

    /// `&`
    #[token("&")]
    Ampersand,

    // =========================================================================
    // KEYWORDS - Program Organization Units
    // =========================================================================
    /// `PROGRAM`
    #[token("PROGRAM", ignore(ascii_case))]
    KwProgram,

    /// `END_PROGRAM`
    #[token("END_PROGRAM", ignore(ascii_case))]
    KwEndProgram,

    /// `TEST_PROGRAM` (non-IEC extension for user tests)
    #[token("TEST_PROGRAM", ignore(ascii_case))]
    KwTestProgram,

    /// `END_TEST_PROGRAM`
    #[token("END_TEST_PROGRAM", ignore(ascii_case))]
    KwEndTestProgram,

    /// `FUNCTION`
    #[token("FUNCTION", ignore(ascii_case))]
    KwFunction,

    /// `END_FUNCTION`
    #[token("END_FUNCTION", ignore(ascii_case))]
    KwEndFunction,

    /// `FUNCTION_BLOCK`
    #[token("FUNCTION_BLOCK", ignore(ascii_case))]
    KwFunctionBlock,

    /// `END_FUNCTION_BLOCK`
    #[token("END_FUNCTION_BLOCK", ignore(ascii_case))]
    KwEndFunctionBlock,

    /// `TEST_FUNCTION_BLOCK` (non-IEC extension for user tests)
    #[token("TEST_FUNCTION_BLOCK", ignore(ascii_case))]
    KwTestFunctionBlock,

    /// `END_TEST_FUNCTION_BLOCK`
    #[token("END_TEST_FUNCTION_BLOCK", ignore(ascii_case))]
    KwEndTestFunctionBlock,

    /// `CLASS`
    #[token("CLASS", ignore(ascii_case))]
    KwClass,

    /// `END_CLASS`
    #[token("END_CLASS", ignore(ascii_case))]
    KwEndClass,

    /// `METHOD`
    #[token("METHOD", ignore(ascii_case))]
    KwMethod,

    /// `END_METHOD`
    #[token("END_METHOD", ignore(ascii_case))]
    KwEndMethod,

    /// `PROPERTY`
    #[token("PROPERTY", ignore(ascii_case))]
    KwProperty,

    /// `END_PROPERTY`
    #[token("END_PROPERTY", ignore(ascii_case))]
    KwEndProperty,

    /// `INTERFACE`
    #[token("INTERFACE", ignore(ascii_case))]
    KwInterface,

    /// `END_INTERFACE`
    #[token("END_INTERFACE", ignore(ascii_case))]
    KwEndInterface,

    /// `NAMESPACE`
    #[token("NAMESPACE", ignore(ascii_case))]
    KwNamespace,

    /// `END_NAMESPACE`
    #[token("END_NAMESPACE", ignore(ascii_case))]
    KwEndNamespace,

    /// `USING`
    #[token("USING", ignore(ascii_case))]
    KwUsing,

    /// `ACTION`
    #[token("ACTION", ignore(ascii_case))]
    KwAction,

    /// `END_ACTION`
    #[token("END_ACTION", ignore(ascii_case))]
    KwEndAction,

    // =========================================================================
    // KEYWORDS - Variable Declarations
    // =========================================================================
    /// `VAR`
    #[token("VAR", ignore(ascii_case))]
    KwVar,

    /// `END_VAR`
    #[token("END_VAR", ignore(ascii_case))]
    KwEndVar,

    /// `VAR_INPUT`
    #[token("VAR_INPUT", ignore(ascii_case))]
    KwVarInput,

    /// `VAR_OUTPUT`
    #[token("VAR_OUTPUT", ignore(ascii_case))]
    KwVarOutput,

    /// `VAR_IN_OUT`
    #[token("VAR_IN_OUT", ignore(ascii_case))]
    KwVarInOut,

    /// `VAR_TEMP`
    #[token("VAR_TEMP", ignore(ascii_case))]
    KwVarTemp,

    /// `VAR_GLOBAL`
    #[token("VAR_GLOBAL", ignore(ascii_case))]
    KwVarGlobal,

    /// `VAR_EXTERNAL`
    #[token("VAR_EXTERNAL", ignore(ascii_case))]
    KwVarExternal,

    /// `VAR_ACCESS`
    #[token("VAR_ACCESS", ignore(ascii_case))]
    KwVarAccess,

    /// `VAR_CONFIG`
    #[token("VAR_CONFIG", ignore(ascii_case))]
    KwVarConfig,

    /// `VAR_STAT`
    #[token("VAR_STAT", ignore(ascii_case))]
    KwVarStat,

    // =========================================================================
    // KEYWORDS - Variable Modifiers
    // =========================================================================
    /// `CONSTANT`
    #[token("CONSTANT", ignore(ascii_case))]
    KwConstant,

    /// `RETAIN`
    #[token("RETAIN", ignore(ascii_case))]
    KwRetain,

    /// `NON_RETAIN`
    #[token("NON_RETAIN", ignore(ascii_case))]
    KwNonRetain,

    /// `PERSISTENT`
    #[token("PERSISTENT", ignore(ascii_case))]
    KwPersistent,

    /// `PUBLIC`
    #[token("PUBLIC", ignore(ascii_case))]
    KwPublic,

    /// `PRIVATE`
    #[token("PRIVATE", ignore(ascii_case))]
    KwPrivate,

    /// `PROTECTED`
    #[token("PROTECTED", ignore(ascii_case))]
    KwProtected,

    /// `INTERNAL`
    #[token("INTERNAL", ignore(ascii_case))]
    KwInternal,

    /// `FINAL`
    #[token("FINAL", ignore(ascii_case))]
    KwFinal,

    /// `ABSTRACT`
    #[token("ABSTRACT", ignore(ascii_case))]
    KwAbstract,

    /// `OVERRIDE`
    #[token("OVERRIDE", ignore(ascii_case))]
    KwOverride,

    // =========================================================================
    // KEYWORDS - Type Definitions
    // =========================================================================
    /// `TYPE`
    #[token("TYPE", ignore(ascii_case))]
    KwType,

    /// `END_TYPE`
    #[token("END_TYPE", ignore(ascii_case))]
    KwEndType,

    /// `STRUCT`
    #[token("STRUCT", ignore(ascii_case))]
    KwStruct,

    /// `END_STRUCT`
    #[token("END_STRUCT", ignore(ascii_case))]
    KwEndStruct,

    /// `UNION`
    #[token("UNION", ignore(ascii_case))]
    KwUnion,

    /// `END_UNION`
    #[token("END_UNION", ignore(ascii_case))]
    KwEndUnion,

    /// `ARRAY`
    #[token("ARRAY", ignore(ascii_case))]
    KwArray,

    /// `OF`
    #[token("OF", ignore(ascii_case))]
    KwOf,

    /// `STRING`
    #[token("STRING", ignore(ascii_case))]
    KwString,

    /// `WSTRING`
    #[token("WSTRING", ignore(ascii_case))]
    KwWString,

    /// `POINTER`
    #[token("POINTER", ignore(ascii_case))]
    KwPointer,

    /// `REF`
    #[token("REF", ignore(ascii_case))]
    KwRef,

    /// `REF_TO`
    #[token("REF_TO", ignore(ascii_case))]
    KwRefTo,

    /// `TO`
    #[token("TO", ignore(ascii_case))]
    KwTo,

    // =========================================================================
    // KEYWORDS - OOP
    // =========================================================================
    /// `EXTENDS`
    #[token("EXTENDS", ignore(ascii_case))]
    KwExtends,

    /// `IMPLEMENTS`
    #[token("IMPLEMENTS", ignore(ascii_case))]
    KwImplements,

    /// `THIS`
    #[token("THIS", ignore(ascii_case))]
    KwThis,

    /// `SUPER`
    #[token("SUPER", ignore(ascii_case))]
    KwSuper,

    /// `NEW`
    #[token("NEW", ignore(ascii_case))]
    KwNew,

    /// `__NEW`
    #[token("__NEW", ignore(ascii_case))]
    KwNewDunder,

    /// `__DELETE`
    #[token("__DELETE", ignore(ascii_case))]
    KwDeleteDunder,

    // =========================================================================
    // KEYWORDS - Control Flow
    // =========================================================================
    /// `IF`
    #[token("IF", ignore(ascii_case))]
    KwIf,

    /// `THEN`
    #[token("THEN", ignore(ascii_case))]
    KwThen,

    /// `ELSIF`
    #[token("ELSIF", ignore(ascii_case))]
    KwElsif,

    /// `ELSE`
    #[token("ELSE", ignore(ascii_case))]
    KwElse,

    /// `END_IF`
    #[token("END_IF", ignore(ascii_case))]
    KwEndIf,

    /// `CASE`
    #[token("CASE", ignore(ascii_case))]
    KwCase,

    /// `END_CASE`
    #[token("END_CASE", ignore(ascii_case))]
    KwEndCase,

    /// `FOR`
    #[token("FOR", ignore(ascii_case))]
    KwFor,

    /// `END_FOR`
    #[token("END_FOR", ignore(ascii_case))]
    KwEndFor,

    /// `BY`
    #[token("BY", ignore(ascii_case))]
    KwBy,

    /// `DO`
    #[token("DO", ignore(ascii_case))]
    KwDo,

    /// `WHILE`
    #[token("WHILE", ignore(ascii_case))]
    KwWhile,

    /// `END_WHILE`
    #[token("END_WHILE", ignore(ascii_case))]
    KwEndWhile,

    /// `REPEAT`
    #[token("REPEAT", ignore(ascii_case))]
    KwRepeat,

    /// `UNTIL`
    #[token("UNTIL", ignore(ascii_case))]
    KwUntil,

    /// `END_REPEAT`
    #[token("END_REPEAT", ignore(ascii_case))]
    KwEndRepeat,

    /// `RETURN`
    #[token("RETURN", ignore(ascii_case))]
    KwReturn,

    /// `EXIT`
    #[token("EXIT", ignore(ascii_case))]
    KwExit,

    /// `CONTINUE`
    #[token("CONTINUE", ignore(ascii_case))]
    KwContinue,

    /// `JMP`
    #[token("JMP", ignore(ascii_case))]
    KwJmp,

    // =========================================================================
    // KEYWORDS - SFC Elements
    // =========================================================================
    /// `STEP`
    #[token("STEP", ignore(ascii_case))]
    KwStep,

    /// `END_STEP`
    #[token("END_STEP", ignore(ascii_case))]
    KwEndStep,

    /// `INITIAL_STEP`
    #[token("INITIAL_STEP", ignore(ascii_case))]
    KwInitialStep,

    /// `TRANSITION`
    #[token("TRANSITION", ignore(ascii_case))]
    KwTransition,

    /// `END_TRANSITION`
    #[token("END_TRANSITION", ignore(ascii_case))]
    KwEndTransition,

    /// `FROM`
    #[token("FROM", ignore(ascii_case))]
    KwFrom,

    // =========================================================================
    // KEYWORDS - Logical Operators
    // =========================================================================
    /// `AND`
    #[token("AND", ignore(ascii_case))]
    KwAnd,

    /// `OR`
    #[token("OR", ignore(ascii_case))]
    KwOr,

    /// `XOR`
    #[token("XOR", ignore(ascii_case))]
    KwXor,

    /// `NOT`
    #[token("NOT", ignore(ascii_case))]
    KwNot,

    /// `MOD`
    #[token("MOD", ignore(ascii_case))]
    KwMod,

    // =========================================================================
    // KEYWORDS - Elementary Data Types (IEC 61131-3)
    // =========================================================================
    // Boolean
    /// `BOOL`
    #[token("BOOL", ignore(ascii_case))]
    KwBool,

    // Integer types - signed
    /// `SINT` - Short Integer (8-bit signed)
    #[token("SINT", ignore(ascii_case))]
    KwSInt,

    /// `INT` - Integer (16-bit signed)
    #[token("INT", ignore(ascii_case))]
    KwInt,

    /// `DINT` - Double Integer (32-bit signed)
    #[token("DINT", ignore(ascii_case))]
    KwDInt,

    /// `LINT` - Long Integer (64-bit signed)
    #[token("LINT", ignore(ascii_case))]
    KwLInt,

    // Integer types - unsigned
    /// `USINT` - Unsigned Short Integer (8-bit)
    #[token("USINT", ignore(ascii_case))]
    KwUSInt,

    /// `UINT` - Unsigned Integer (16-bit)
    #[token("UINT", ignore(ascii_case))]
    KwUInt,

    /// `UDINT` - Unsigned Double Integer (32-bit)
    #[token("UDINT", ignore(ascii_case))]
    KwUDInt,

    /// `ULINT` - Unsigned Long Integer (64-bit)
    #[token("ULINT", ignore(ascii_case))]
    KwULInt,

    // Floating point types
    /// `REAL` - 32-bit floating point
    #[token("REAL", ignore(ascii_case))]
    KwReal,

    /// `LREAL` - 64-bit floating point
    #[token("LREAL", ignore(ascii_case))]
    KwLReal,

    // Bit string types
    /// `BYTE` - 8-bit bit string
    #[token("BYTE", ignore(ascii_case))]
    KwByte,

    /// `WORD` - 16-bit bit string
    #[token("WORD", ignore(ascii_case))]
    KwWord,

    /// `DWORD` - 32-bit bit string
    #[token("DWORD", ignore(ascii_case))]
    KwDWord,

    /// `LWORD` - 64-bit bit string
    #[token("LWORD", ignore(ascii_case))]
    KwLWord,

    // Time types
    /// `TIME`
    #[token("TIME", ignore(ascii_case))]
    KwTime,

    /// `LTIME`
    #[token("LTIME", ignore(ascii_case))]
    KwLTime,

    /// `DATE`
    #[token("DATE", ignore(ascii_case))]
    KwDate,

    /// `LDATE`
    #[token("LDATE", ignore(ascii_case))]
    KwLDate,

    /// `TIME_OF_DAY` / `TOD`
    #[token("TIME_OF_DAY", ignore(ascii_case))]
    #[token("TOD", ignore(ascii_case))]
    KwTimeOfDay,

    /// `LTIME_OF_DAY` / `LTOD`
    #[token("LTIME_OF_DAY", ignore(ascii_case))]
    #[token("LTOD", ignore(ascii_case))]
    KwLTimeOfDay,

    /// `DATE_AND_TIME` / `DT`
    #[token("DATE_AND_TIME", ignore(ascii_case))]
    #[token("DT", ignore(ascii_case))]
    KwDateAndTime,

    /// `LDATE_AND_TIME` / `LDT`
    #[token("LDATE_AND_TIME", ignore(ascii_case))]
    #[token("LDT", ignore(ascii_case))]
    KwLDateAndTime,

    /// `CHAR`
    #[token("CHAR", ignore(ascii_case))]
    KwChar,

    /// `WCHAR`
    #[token("WCHAR", ignore(ascii_case))]
    KwWChar,

    // Other special types
    /// `ANY`
    #[token("ANY", ignore(ascii_case))]
    KwAny,

    /// `ANY_DERIVED`
    #[token("ANY_DERIVED", ignore(ascii_case))]
    KwAnyDerived,

    /// `ANY_ELEMENTARY`
    #[token("ANY_ELEMENTARY", ignore(ascii_case))]
    KwAnyElementary,

    /// `ANY_MAGNITUDE`
    #[token("ANY_MAGNITUDE", ignore(ascii_case))]
    KwAnyMagnitude,

    /// `ANY_INT`
    #[token("ANY_INT", ignore(ascii_case))]
    KwAnyInt,

    /// `ANY_UNSIGNED`
    #[token("ANY_UNSIGNED", ignore(ascii_case))]
    KwAnyUnsigned,

    /// `ANY_SIGNED`
    #[token("ANY_SIGNED", ignore(ascii_case))]
    KwAnySigned,

    /// `ANY_REAL`
    #[token("ANY_REAL", ignore(ascii_case))]
    KwAnyReal,

    /// `ANY_NUM`
    #[token("ANY_NUM", ignore(ascii_case))]
    KwAnyNum,

    /// `ANY_DURATION`
    #[token("ANY_DURATION", ignore(ascii_case))]
    KwAnyDuration,

    /// `ANY_BIT`
    #[token("ANY_BIT", ignore(ascii_case))]
    KwAnyBit,

    /// `ANY_CHARS`
    #[token("ANY_CHARS", ignore(ascii_case))]
    KwAnyChars,

    /// `ANY_STRING`
    #[token("ANY_STRING", ignore(ascii_case))]
    KwAnyString,

    /// `ANY_CHAR`
    #[token("ANY_CHAR", ignore(ascii_case))]
    KwAnyChar,

    /// `ANY_DATE`
    #[token("ANY_DATE", ignore(ascii_case))]
    KwAnyDate,

    // =========================================================================
    // KEYWORDS - Boolean Literals
    // =========================================================================
    /// `TRUE`
    #[token("TRUE", ignore(ascii_case))]
    KwTrue,

    /// `FALSE`
    #[token("FALSE", ignore(ascii_case))]
    KwFalse,

    /// `NULL`
    #[token("NULL", ignore(ascii_case))]
    KwNull,

    // =========================================================================
    // KEYWORDS - Configuration
    // =========================================================================
    /// `CONFIGURATION`
    #[token("CONFIGURATION", ignore(ascii_case))]
    KwConfiguration,

    /// `END_CONFIGURATION`
    #[token("END_CONFIGURATION", ignore(ascii_case))]
    KwEndConfiguration,

    /// `RESOURCE`
    #[token("RESOURCE", ignore(ascii_case))]
    KwResource,

    /// `END_RESOURCE`
    #[token("END_RESOURCE", ignore(ascii_case))]
    KwEndResource,

    /// `ON`
    #[token("ON", ignore(ascii_case))]
    KwOn,

    /// `READ_WRITE`
    #[token("READ_WRITE", ignore(ascii_case))]
    KwReadWrite,

    /// `READ_ONLY`
    #[token("READ_ONLY", ignore(ascii_case))]
    KwReadOnly,

    // =========================================================================
    // KEYWORDS - Task Configuration
    // =========================================================================
    /// `TASK`
    #[token("TASK", ignore(ascii_case))]
    KwTask,

    /// `WITH`
    #[token("WITH", ignore(ascii_case))]
    KwWith,

    /// `AT`
    #[token("AT", ignore(ascii_case))]
    KwAt,

    // =========================================================================
    // KEYWORDS - Special
    // =========================================================================
    /// `EN`
    #[token("EN", ignore(ascii_case))]
    KwEn,

    /// `ENO`
    #[token("ENO", ignore(ascii_case))]
    KwEno,

    /// `R_EDGE`
    #[token("R_EDGE", ignore(ascii_case))]
    KwREdge,

    /// `F_EDGE`
    #[token("F_EDGE", ignore(ascii_case))]
    KwFEdge,

    // =========================================================================
    // KEYWORDS - References and Addresses
    // =========================================================================
    /// `ADR`
    #[token("ADR", ignore(ascii_case))]
    KwAdr,

    /// `SIZEOF`
    #[token("SIZEOF", ignore(ascii_case))]
    KwSizeOf,

    // =========================================================================
    // KEYWORDS - Property Accessors
    // =========================================================================
    /// `GET`
    #[token("GET", ignore(ascii_case))]
    KwGet,

    /// `END_GET`  
    #[token("END_GET", ignore(ascii_case))]
    KwEndGet,

    /// `SET`
    #[token("SET", ignore(ascii_case))]
    KwSet,

    /// `END_SET`
    #[token("END_SET", ignore(ascii_case))]
    KwEndSet,

    // =========================================================================
    // LITERALS
    // =========================================================================
    /// Integer literal: 123, 16#FF, 2#1010, 8#77
    /// Supports underscores: 1_000_000
    #[regex(r"[0-9]([0-9]|_[0-9])*")]
    #[regex(r"16#[0-9A-Fa-f]([0-9A-Fa-f]|_[0-9A-Fa-f])*")]
    #[regex(r"2#[01]([01]|_[01])*")]
    #[regex(r"8#[0-7]([0-7]|_[0-7])*")]
    IntLiteral,

    /// Real literal: 3.14, 1.0E10, 2.5e-3
    #[regex(r"[0-9]([0-9]|_[0-9])*\.[0-9]([0-9]|_[0-9])*([eE][+-]?[0-9]([0-9]|_[0-9])*)?")]
    RealLiteral,

    /// Time literal: T#1h30m, TIME#-5s, LT#14.7s, LTIME#5m_30s_500ms_100.1us
    #[regex(
        r"(?:T|TIME|LT|LTIME)#[+-]?(?:[0-9]+(?:\.[0-9]+)?(?:ms|us|ns|d|h|m|s))(?:_?(?:[0-9]+(?:\.[0-9]+)?(?:ms|us|ns|d|h|m|s)))*",
        ignore(ascii_case)
    )]
    TimeLiteral,

    /// Date literal: D#2024-01-15, DATE#2024-01-15, LDATE#2012-02-29, LD#1984-06-25
    #[regex(r"(?:DATE|D|LDATE|LD)#[0-9]{4}-[0-9]{2}-[0-9]{2}", ignore(ascii_case))]
    DateLiteral,

    /// Time of day literal: TOD#14:30:00, LTOD#15:36:55.360_227_400
    #[regex(r"TOD#[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9][0-9_]*)?", ignore(ascii_case))]
    #[regex(
        r"TIME_OF_DAY#[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9][0-9_]*)?",
        ignore(ascii_case)
    )]
    #[regex(
        r"LTOD#[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9][0-9_]*)?",
        ignore(ascii_case)
    )]
    #[regex(
        r"LTIME_OF_DAY#[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9][0-9_]*)?",
        ignore(ascii_case)
    )]
    TimeOfDayLiteral,

    /// Date and time literal: DT#2024-01-15-14:30:00, LDT#1984-06-25-15:36:55.360_227_400
    #[regex(
        r"DT#[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9][0-9_]*)?",
        ignore(ascii_case)
    )]
    #[regex(
        r"DATE_AND_TIME#[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9][0-9_]*)?",
        ignore(ascii_case)
    )]
    #[regex(
        r"LDT#[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9][0-9_]*)?",
        ignore(ascii_case)
    )]
    #[regex(
        r"LDATE_AND_TIME#[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9][0-9_]*)?",
        ignore(ascii_case)
    )]
    DateAndTimeLiteral,

    /// Single-quoted string: 'hello$Nworld'
    #[regex(
        r"'([^$'\r\n]|\$\$|\$[LlNnPpRrTt]|\$'|\$[0-9A-Fa-f]{2})*'",
        priority = 2
    )]
    StringLiteral,

    /// Wide string: "hello$Nworld"
    #[regex(
        r#""([^$"\r\n]|\$\$|\$[LlNnPpRrTt]|\$"|\$[0-9A-Fa-f]{4})*""#,
        priority = 2
    )]
    WideStringLiteral,

    /// Typed literal prefix: INT#, REAL#, BOOL#, etc.
    /// This captures the type prefix, followed by #
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*#")]
    TypedLiteralPrefix,

    // =========================================================================
    // DIRECT ADDRESSES (Hardware I/O)
    // =========================================================================
    /// Direct address: %IX0.0, %QW10, %MD100
    /// Format: `%[I|Q|M][X|B|W|D|L]<address>`
    #[regex(r"%[IQM]\*")]
    #[regex(r"%[IQM][XBWDL]?[0-9]+(\.[0-9]+)*")]
    #[regex(r"%[XBWDL][0-9]+")]
    DirectAddress,

    // =========================================================================
    // IDENTIFIERS
    // =========================================================================
    /// Identifier: starts with letter or underscore, contains letters, digits, underscores
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*")]
    Ident,

    // =========================================================================
    // SPECIAL TOKENS
    // =========================================================================
    /// Lexer error - unrecognized character
    #[regex(r"'[^'\r\n]*'", priority = 1)]
    #[regex(r#""[^"\r\n]*""#, priority = 1)]
    #[default]
    Error,

    /// End of file marker (not produced by lexer, added by parser)
    Eof,
}

impl TokenKind {
    /// Returns `true` if this token is trivia (whitespace, comment, or pragma).
    #[inline]
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::LineComment | Self::BlockComment | Self::Pragma
        )
    }

    /// Returns `true` if this token is a keyword.
    pub fn is_keyword(self) -> bool {
        matches!(
            self,
            Self::KwProgram
                | Self::KwEndProgram
                | Self::KwTestProgram
                | Self::KwEndTestProgram
                | Self::KwFunction
                | Self::KwEndFunction
                | Self::KwFunctionBlock
                | Self::KwEndFunctionBlock
                | Self::KwTestFunctionBlock
                | Self::KwEndTestFunctionBlock
                | Self::KwClass
                | Self::KwEndClass
                | Self::KwMethod
                | Self::KwEndMethod
                | Self::KwProperty
                | Self::KwEndProperty
                | Self::KwInterface
                | Self::KwEndInterface
                | Self::KwNamespace
                | Self::KwEndNamespace
                | Self::KwUsing
                | Self::KwAction
                | Self::KwEndAction
                | Self::KwVar
                | Self::KwEndVar
                | Self::KwVarInput
                | Self::KwVarOutput
                | Self::KwVarInOut
                | Self::KwVarTemp
                | Self::KwVarGlobal
                | Self::KwVarExternal
                | Self::KwVarAccess
                | Self::KwVarConfig
                | Self::KwVarStat
                | Self::KwConstant
                | Self::KwRetain
                | Self::KwNonRetain
                | Self::KwPersistent
                | Self::KwPublic
                | Self::KwPrivate
                | Self::KwProtected
                | Self::KwInternal
                | Self::KwFinal
                | Self::KwAbstract
                | Self::KwOverride
                | Self::KwType
                | Self::KwEndType
                | Self::KwStruct
                | Self::KwEndStruct
                | Self::KwUnion
                | Self::KwEndUnion
                | Self::KwArray
                | Self::KwOf
                | Self::KwString
                | Self::KwWString
                | Self::KwPointer
                | Self::KwRef
                | Self::KwRefTo
                | Self::KwTo
                | Self::KwExtends
                | Self::KwImplements
                | Self::KwThis
                | Self::KwSuper
                | Self::KwNew
                | Self::KwNewDunder
                | Self::KwDeleteDunder
                | Self::KwIf
                | Self::KwThen
                | Self::KwElsif
                | Self::KwElse
                | Self::KwEndIf
                | Self::KwCase
                | Self::KwEndCase
                | Self::KwFor
                | Self::KwEndFor
                | Self::KwBy
                | Self::KwDo
                | Self::KwWhile
                | Self::KwEndWhile
                | Self::KwRepeat
                | Self::KwUntil
                | Self::KwEndRepeat
                | Self::KwReturn
                | Self::KwExit
                | Self::KwContinue
                | Self::KwJmp
                | Self::KwStep
                | Self::KwEndStep
                | Self::KwInitialStep
                | Self::KwTransition
                | Self::KwEndTransition
                | Self::KwFrom
                | Self::KwAnd
                | Self::KwOr
                | Self::KwXor
                | Self::KwNot
                | Self::KwMod
                | Self::KwBool
                | Self::KwSInt
                | Self::KwInt
                | Self::KwDInt
                | Self::KwLInt
                | Self::KwUSInt
                | Self::KwUInt
                | Self::KwUDInt
                | Self::KwULInt
                | Self::KwReal
                | Self::KwLReal
                | Self::KwByte
                | Self::KwWord
                | Self::KwDWord
                | Self::KwLWord
                | Self::KwTime
                | Self::KwLTime
                | Self::KwDate
                | Self::KwLDate
                | Self::KwTimeOfDay
                | Self::KwLTimeOfDay
                | Self::KwDateAndTime
                | Self::KwLDateAndTime
                | Self::KwChar
                | Self::KwWChar
                | Self::KwAny
                | Self::KwAnyDerived
                | Self::KwAnyElementary
                | Self::KwAnyMagnitude
                | Self::KwAnyInt
                | Self::KwAnyUnsigned
                | Self::KwAnySigned
                | Self::KwAnyReal
                | Self::KwAnyNum
                | Self::KwAnyDuration
                | Self::KwAnyBit
                | Self::KwAnyChars
                | Self::KwAnyString
                | Self::KwAnyChar
                | Self::KwAnyDate
                | Self::KwTrue
                | Self::KwFalse
                | Self::KwNull
                | Self::KwConfiguration
                | Self::KwEndConfiguration
                | Self::KwResource
                | Self::KwEndResource
                | Self::KwOn
                | Self::KwReadWrite
                | Self::KwReadOnly
                | Self::KwTask
                | Self::KwWith
                | Self::KwAt
                | Self::KwEn
                | Self::KwEno
                | Self::KwREdge
                | Self::KwFEdge
                | Self::KwAdr
                | Self::KwSizeOf
                | Self::KwGet
                | Self::KwEndGet
                | Self::KwSet
                | Self::KwEndSet
        )
    }

    /// Returns `true` if this token is a type keyword.
    pub fn is_type_keyword(self) -> bool {
        matches!(
            self,
            Self::KwBool
                | Self::KwSInt
                | Self::KwInt
                | Self::KwDInt
                | Self::KwLInt
                | Self::KwUSInt
                | Self::KwUInt
                | Self::KwUDInt
                | Self::KwULInt
                | Self::KwReal
                | Self::KwLReal
                | Self::KwByte
                | Self::KwWord
                | Self::KwDWord
                | Self::KwLWord
                | Self::KwTime
                | Self::KwLTime
                | Self::KwDate
                | Self::KwLDate
                | Self::KwTimeOfDay
                | Self::KwLTimeOfDay
                | Self::KwDateAndTime
                | Self::KwLDateAndTime
                | Self::KwString
                | Self::KwWString
                | Self::KwChar
                | Self::KwWChar
                | Self::KwAny
                | Self::KwAnyDerived
                | Self::KwAnyElementary
                | Self::KwAnyMagnitude
                | Self::KwAnyInt
                | Self::KwAnyUnsigned
                | Self::KwAnySigned
                | Self::KwAnyReal
                | Self::KwAnyNum
                | Self::KwAnyDuration
                | Self::KwAnyBit
                | Self::KwAnyChars
                | Self::KwAnyString
                | Self::KwAnyChar
                | Self::KwAnyDate
        )
    }

    /// Returns `true` if this token is a variable block keyword.
    pub fn is_var_keyword(self) -> bool {
        matches!(
            self,
            Self::KwVar
                | Self::KwVarInput
                | Self::KwVarOutput
                | Self::KwVarInOut
                | Self::KwVarTemp
                | Self::KwVarGlobal
                | Self::KwVarExternal
                | Self::KwVarAccess
                | Self::KwVarConfig
                | Self::KwVarStat
        )
    }

    /// Returns `true` if this token can start an expression.
    pub fn can_start_expr(self) -> bool {
        matches!(
            self,
            Self::Ident
                | Self::KwEn
                | Self::KwEno
                | Self::IntLiteral
                | Self::RealLiteral
                | Self::StringLiteral
                | Self::WideStringLiteral
                | Self::TimeLiteral
                | Self::DateLiteral
                | Self::TimeOfDayLiteral
                | Self::DateAndTimeLiteral
                | Self::KwTrue
                | Self::KwFalse
                | Self::KwNull
                | Self::KwNot
                | Self::TypedLiteralPrefix
                | Self::LParen
                | Self::Minus
                | Self::Plus
                | Self::KwThis
                | Self::KwSuper
                | Self::KwNew
                | Self::KwNewDunder
                | Self::KwDeleteDunder
                | Self::KwRef
                | Self::KwAdr
                | Self::KwSizeOf
                | Self::DirectAddress
        )
    }

    /// Returns `true` if this token can start a statement.
    pub fn can_start_statement(self) -> bool {
        matches!(
            self,
            Self::Ident
                | Self::DirectAddress
                | Self::KwThis
                | Self::KwSuper
                | Self::KwNew
                | Self::KwNewDunder
                | Self::KwDeleteDunder
                | Self::KwRef
                | Self::KwAdr
                | Self::KwSizeOf
                | Self::KwIf
                | Self::KwCase
                | Self::KwFor
                | Self::KwWhile
                | Self::KwRepeat
                | Self::KwReturn
                | Self::KwExit
                | Self::KwContinue
                | Self::KwJmp
                | Self::Semicolon // Empty statement
        )
    }

    /// Returns `true` if this token is a comparison operator.
    pub fn is_comparison_op(self) -> bool {
        matches!(
            self,
            Self::Eq | Self::Neq | Self::Lt | Self::LtEq | Self::Gt | Self::GtEq
        )
    }

    /// Returns `true` if this token is an additive operator.
    pub fn is_additive_op(self) -> bool {
        matches!(self, Self::Plus | Self::Minus)
    }

    /// Returns `true` if this token is a multiplicative operator.
    pub fn is_multiplicative_op(self) -> bool {
        matches!(self, Self::Star | Self::Slash | Self::KwMod)
    }

    /// Returns the binding power for Pratt parsing (left, right).
    /// Returns None if not an infix operator.
    pub fn infix_binding_power(self) -> Option<(u8, u8)> {
        Some(match self {
            Self::KwOr => (1, 2),
            Self::KwXor => (3, 4),
            Self::KwAnd | Self::Ampersand => (5, 6),
            Self::Eq | Self::Neq | Self::Lt | Self::LtEq | Self::Gt | Self::GtEq => (7, 8),
            Self::Plus | Self::Minus => (9, 10),
            Self::Star | Self::Slash | Self::KwMod => (11, 12),
            Self::Power => (14, 13), // Right associative
            _ => return None,
        })
    }

    /// Returns the binding power for prefix operators.
    pub fn prefix_binding_power(self) -> Option<u8> {
        Some(match self {
            Self::KwNot | Self::Plus | Self::Minus => 15,
            _ => return None,
        })
    }
}

impl From<TokenKind> for rowan::SyntaxKind {
    fn from(kind: TokenKind) -> Self {
        Self(kind as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(input: &str) -> Vec<(TokenKind, &str)> {
        TokenKind::lexer(input)
            .spanned()
            .map(|(tok, span)| (tok.unwrap_or(TokenKind::Error), &input[span]))
            .collect()
    }

    #[test]
    fn test_keywords_case_insensitive() {
        let tokens = lex("PROGRAM program Program PrOgRaM");
        assert!(tokens
            .iter()
            .filter(|(k, _)| !k.is_trivia())
            .all(|(kind, _)| *kind == TokenKind::KwProgram));
    }

    #[test]
    fn test_additional_keywords() {
        let tokens = lex(
            "CHAR WCHAR LDATE ANY_DERIVED ANY_ELEMENTARY ANY_MAGNITUDE ANY_UNSIGNED \
             ANY_SIGNED ANY_DURATION ANY_CHARS ANY_CHAR EN ENO STEP END_STEP INITIAL_STEP \
             TRANSITION END_TRANSITION FROM R_EDGE F_EDGE",
        );
        let kinds: Vec<_> = tokens
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !k.is_trivia())
            .collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::KwChar,
                TokenKind::KwWChar,
                TokenKind::KwLDate,
                TokenKind::KwAnyDerived,
                TokenKind::KwAnyElementary,
                TokenKind::KwAnyMagnitude,
                TokenKind::KwAnyUnsigned,
                TokenKind::KwAnySigned,
                TokenKind::KwAnyDuration,
                TokenKind::KwAnyChars,
                TokenKind::KwAnyChar,
                TokenKind::KwEn,
                TokenKind::KwEno,
                TokenKind::KwStep,
                TokenKind::KwEndStep,
                TokenKind::KwInitialStep,
                TokenKind::KwTransition,
                TokenKind::KwEndTransition,
                TokenKind::KwFrom,
                TokenKind::KwREdge,
                TokenKind::KwFEdge,
            ]
        );
    }

    #[test]
    fn test_basic_operators() {
        let tokens = lex(":= = <> < <= > >= + - * / ** &");
        let kinds: Vec<_> = tokens
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !k.is_trivia())
            .collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Assign,
                TokenKind::Eq,
                TokenKind::Neq,
                TokenKind::Lt,
                TokenKind::LtEq,
                TokenKind::Gt,
                TokenKind::GtEq,
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Star,
                TokenKind::Slash,
                TokenKind::Power,
                TokenKind::Ampersand
            ]
        );
    }

    #[test]
    // IEC 61131-3 Ed.3 Table 5 (numeric literals)
    fn test_integer_literals() {
        let tokens = lex("123 16#FF 2#1010 8#77 1_000_000");
        let kinds: Vec<_> = tokens
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !k.is_trivia())
            .collect();
        assert!(kinds.iter().all(|k| *k == TokenKind::IntLiteral));
    }

    #[test]
    // IEC 61131-3 Ed.3 Table 5 (numeric literals)
    fn test_real_literals() {
        let tokens = lex("3.14 1.0E10 2.5e-3 1_000.000_1");
        let kinds: Vec<_> = tokens
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !k.is_trivia())
            .collect();
        assert!(kinds.iter().all(|k| *k == TokenKind::RealLiteral));
    }

    #[test]
    // IEC 61131-3 Ed.3 Table 3 (comments)
    fn test_comments() {
        let tokens = lex("// line comment\n(* block \n comment *)");
        let kinds: Vec<_> = tokens.iter().map(|(k, _)| *k).collect();
        assert!(kinds.contains(&TokenKind::LineComment));
        assert!(kinds.contains(&TokenKind::BlockComment));
    }

    #[test]
    fn test_direct_addresses() {
        let tokens = lex("%IX0.0 %QW10 %MD100 %IB5 %I* %Q* %M*");
        let kinds: Vec<_> = tokens
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !k.is_trivia())
            .collect();
        assert!(kinds.iter().all(|k| *k == TokenKind::DirectAddress));
    }

    #[test]
    // IEC 61131-3 Ed.3 Tables 6-7 (string literals)
    fn test_strings() {
        let tokens = lex(r#"'hello' "world""#);
        let kinds: Vec<_> = tokens
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !k.is_trivia())
            .collect();
        assert_eq!(
            kinds,
            vec![TokenKind::StringLiteral, TokenKind::WideStringLiteral]
        );
    }

    #[test]
    // IEC 61131-3 Ed.3 Tables 6-7 (string escapes)
    fn test_string_escapes() {
        let tokens = lex(r#"'$N$L$$$'' "$T$R$$$"" '$0A' "$00C4""#);
        let kinds: Vec<_> = tokens
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !k.is_trivia())
            .collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::StringLiteral,
                TokenKind::WideStringLiteral,
                TokenKind::StringLiteral,
                TokenKind::WideStringLiteral
            ]
        );
    }

    #[test]
    // IEC 61131-3 Ed.3 Tables 6-7 (string escapes)
    fn test_invalid_string_escapes() {
        let tokens = lex(r#"'$Q' "$0G" "$123""#);
        let kinds: Vec<_> = tokens
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !k.is_trivia())
            .collect();
        assert!(kinds.contains(&TokenKind::Error));
        assert!(!kinds.contains(&TokenKind::StringLiteral));
        assert!(!kinds.contains(&TokenKind::WideStringLiteral));
    }

    #[test]
    // IEC 61131-3 Ed.3 Tables 8-9 (duration/date-time literals)
    fn test_time_literals() {
        let tokens = lex(
            "T#1h30m TIME#5s t#100ms LT#14.7s LTIME#5m_30s_500ms_100.1us t#12h4m34ms230us400ns T#-14ms",
        );
        let kinds: Vec<_> = tokens
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !k.is_trivia())
            .collect();
        assert!(kinds.iter().all(|k| *k == TokenKind::TimeLiteral));
    }

    #[test]
    fn test_function_block_keywords() {
        let tokens = lex("FUNCTION_BLOCK FB_Test END_FUNCTION_BLOCK");
        let kinds: Vec<_> = tokens
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !k.is_trivia())
            .collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::KwFunctionBlock,
                TokenKind::Ident,
                TokenKind::KwEndFunctionBlock
            ]
        );
    }

    #[test]
    fn test_test_pou_keywords() {
        let tokens = lex(
            "TEST_PROGRAM Test END_TEST_PROGRAM TEST_FUNCTION_BLOCK FB_Test END_TEST_FUNCTION_BLOCK",
        );
        let kinds: Vec<_> = tokens
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !k.is_trivia())
            .collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::KwTestProgram,
                TokenKind::Ident,
                TokenKind::KwEndTestProgram,
                TokenKind::KwTestFunctionBlock,
                TokenKind::Ident,
                TokenKind::KwEndTestFunctionBlock
            ]
        );
    }

    #[test]
    fn test_class_and_configuration_keywords() {
        let tokens = lex(
            "CLASS C END_CLASS CONFIGURATION Conf END_CONFIGURATION RESOURCE Res END_RESOURCE ON READ_WRITE READ_ONLY USING",
        );
        let kinds: Vec<_> = tokens
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !k.is_trivia())
            .collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::KwClass,
                TokenKind::Ident,
                TokenKind::KwEndClass,
                TokenKind::KwConfiguration,
                TokenKind::Ident,
                TokenKind::KwEndConfiguration,
                TokenKind::KwResource,
                TokenKind::Ident,
                TokenKind::KwEndResource,
                TokenKind::KwOn,
                TokenKind::KwReadWrite,
                TokenKind::KwReadOnly,
                TokenKind::KwUsing
            ]
        );
    }

    #[test]
    fn test_var_keywords() {
        let tokens = lex("VAR VAR_INPUT VAR_OUTPUT VAR_IN_OUT VAR_TEMP END_VAR");
        let kinds: Vec<_> = tokens
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !k.is_trivia())
            .collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::KwVar,
                TokenKind::KwVarInput,
                TokenKind::KwVarOutput,
                TokenKind::KwVarInOut,
                TokenKind::KwVarTemp,
                TokenKind::KwEndVar
            ]
        );
    }

    #[test]
    fn test_pragmas() {
        // IEC 61131-3 Table 4 examples
        let tokens = lex("{VERSION 2.0} {AUTHOR JHC} {x:= 256, y:= 384}");
        let kinds: Vec<_> = tokens.iter().map(|(k, _)| *k).collect();
        assert!(kinds.contains(&TokenKind::Pragma));
        // Pragmas are trivia, so filtering them should leave nothing
        let non_trivia: Vec<_> = tokens.iter().filter(|(k, _)| !k.is_trivia()).collect();
        assert!(non_trivia.is_empty());
    }

    #[test]
    fn test_pragma_with_code() {
        let tokens = lex("{VERSION 2.0} x := 42;");
        let kinds: Vec<_> = tokens
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !k.is_trivia())
            .collect();
        // Pragma is trivia, only x, :=, 42, ; remain
        assert_eq!(
            kinds,
            vec![
                TokenKind::Ident,
                TokenKind::Assign,
                TokenKind::IntLiteral,
                TokenKind::Semicolon
            ]
        );
    }

    #[test]
    fn test_pragma_content_preserved() {
        let tokens = lex("{AUTHOR JHC}");
        let pragma_tokens: Vec<_> = tokens
            .iter()
            .filter(|(k, _)| *k == TokenKind::Pragma)
            .collect();
        assert_eq!(pragma_tokens.len(), 1);
        assert_eq!(pragma_tokens[0].1, "{AUTHOR JHC}");
    }
}
