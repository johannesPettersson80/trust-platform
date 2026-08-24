use super::{lex_block_comment_c, lex_block_comment_pascal};
use logos::Logos;

/// All token kinds in IEC 61131-3 Structured Text.
#[allow(missing_docs)]
#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
#[derive(Default)]
pub enum TokenKind {
    #[regex(r"[ \t\r\n\x0C]+")]
    Whitespace,
    #[regex(r"//[^\r\n\x0C]*", allow_greedy = true)]
    LineComment,
    #[token("(*", lex_block_comment_pascal)]
    #[token("/*", lex_block_comment_c)]
    BlockComment,
    #[regex(r"\{[^}]*\}")]
    Pragma,
    #[token(";")]
    Semicolon,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token("..")]
    DotDot,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("#")]
    Hash,
    #[token("^")]
    Caret,
    #[token("@")]
    At,
    #[token(":=")]
    Assign,
    #[token("=>")]
    Arrow,
    #[token("?=")]
    RefAssign,
    #[token("=")]
    Eq,
    #[token("<>")]
    Neq,
    #[token("<")]
    Lt,
    #[token("<=")]
    LtEq,
    #[token(">")]
    Gt,
    #[token(">=")]
    GtEq,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("**")]
    Power,
    #[token("&")]
    Ampersand,
    #[token("PROGRAM", ignore(case))]
    KwProgram,
    #[token("END_PROGRAM", ignore(case))]
    KwEndProgram,
    #[token("TEST_PROGRAM", ignore(case))]
    KwTestProgram,
    #[token("END_TEST_PROGRAM", ignore(case))]
    KwEndTestProgram,
    #[token("FUNCTION", ignore(case))]
    KwFunction,
    #[token("END_FUNCTION", ignore(case))]
    KwEndFunction,
    #[token("FUNCTION_BLOCK", ignore(case))]
    KwFunctionBlock,
    #[token("END_FUNCTION_BLOCK", ignore(case))]
    KwEndFunctionBlock,
    #[token("TEST_FUNCTION_BLOCK", ignore(case))]
    KwTestFunctionBlock,
    #[token("END_TEST_FUNCTION_BLOCK", ignore(case))]
    KwEndTestFunctionBlock,
    #[token("CLASS", ignore(case))]
    KwClass,
    #[token("END_CLASS", ignore(case))]
    KwEndClass,
    #[token("METHOD", ignore(case))]
    KwMethod,
    #[token("END_METHOD", ignore(case))]
    KwEndMethod,
    #[token("PROPERTY", ignore(case))]
    KwProperty,
    #[token("END_PROPERTY", ignore(case))]
    KwEndProperty,
    #[token("INTERFACE", ignore(case))]
    KwInterface,
    #[token("END_INTERFACE", ignore(case))]
    KwEndInterface,
    #[token("NAMESPACE", ignore(case))]
    KwNamespace,
    #[token("END_NAMESPACE", ignore(case))]
    KwEndNamespace,
    #[token("USING", ignore(case))]
    KwUsing,
    #[token("ACTION", ignore(case))]
    KwAction,
    #[token("END_ACTION", ignore(case))]
    KwEndAction,
    #[token("VAR", ignore(case))]
    KwVar,
    #[token("END_VAR", ignore(case))]
    KwEndVar,
    #[token("VAR_INPUT", ignore(case))]
    KwVarInput,
    #[token("VAR_OUTPUT", ignore(case))]
    KwVarOutput,
    #[token("VAR_IN_OUT", ignore(case))]
    KwVarInOut,
    #[token("VAR_TEMP", ignore(case))]
    KwVarTemp,
    #[token("VAR_GLOBAL", ignore(case))]
    KwVarGlobal,
    #[token("VAR_EXTERNAL", ignore(case))]
    KwVarExternal,
    #[token("VAR_ACCESS", ignore(case))]
    KwVarAccess,
    #[token("VAR_CONFIG", ignore(case))]
    KwVarConfig,
    #[token("VAR_STAT", ignore(case))]
    KwVarStat,
    #[token("CONSTANT", ignore(case))]
    KwConstant,
    #[token("RETAIN", ignore(case))]
    KwRetain,
    #[token("NON_RETAIN", ignore(case))]
    KwNonRetain,
    #[token("PERSISTENT", ignore(case))]
    KwPersistent,
    #[token("PUBLIC", ignore(case))]
    KwPublic,
    #[token("PRIVATE", ignore(case))]
    KwPrivate,
    #[token("PROTECTED", ignore(case))]
    KwProtected,
    #[token("INTERNAL", ignore(case))]
    KwInternal,
    #[token("FINAL", ignore(case))]
    KwFinal,
    #[token("ABSTRACT", ignore(case))]
    KwAbstract,
    #[token("OVERRIDE", ignore(case))]
    KwOverride,
    #[token("TYPE", ignore(case))]
    KwType,
    #[token("END_TYPE", ignore(case))]
    KwEndType,
    #[token("STRUCT", ignore(case))]
    KwStruct,
    #[token("END_STRUCT", ignore(case))]
    KwEndStruct,
    #[token("OVERLAP", ignore(case))]
    KwOverlap,
    #[token("UNION", ignore(case))]
    KwUnion,
    #[token("END_UNION", ignore(case))]
    KwEndUnion,
    #[token("ARRAY", ignore(case))]
    KwArray,
    #[token("OF", ignore(case))]
    KwOf,
    #[token("STRING", ignore(case))]
    KwString,
    #[token("WSTRING", ignore(case))]
    KwWString,
    #[token("POINTER", ignore(case))]
    KwPointer,
    #[token("REF", ignore(case))]
    KwRef,
    #[token("REF_TO", ignore(case))]
    KwRefTo,
    #[token("TO", ignore(case))]
    KwTo,
    #[token("EXTENDS", ignore(case))]
    KwExtends,
    #[token("IMPLEMENTS", ignore(case))]
    KwImplements,
    #[token("THIS", ignore(case))]
    KwThis,
    #[token("SUPER", ignore(case))]
    KwSuper,
    #[token("NEW", ignore(case))]
    KwNew,
    #[token("__NEW", ignore(case))]
    KwNewDunder,
    #[token("__DELETE", ignore(case))]
    KwDeleteDunder,
    #[token("IF", ignore(case))]
    KwIf,
    #[token("THEN", ignore(case))]
    KwThen,
    #[token("ELSIF", ignore(case))]
    KwElsif,
    #[token("ELSE", ignore(case))]
    KwElse,
    #[token("END_IF", ignore(case))]
    KwEndIf,
    #[token("CASE", ignore(case))]
    KwCase,
    #[token("END_CASE", ignore(case))]
    KwEndCase,
    #[token("FOR", ignore(case))]
    KwFor,
    #[token("END_FOR", ignore(case))]
    KwEndFor,
    #[token("BY", ignore(case))]
    KwBy,
    #[token("DO", ignore(case))]
    KwDo,
    #[token("WHILE", ignore(case))]
    KwWhile,
    #[token("END_WHILE", ignore(case))]
    KwEndWhile,
    #[token("REPEAT", ignore(case))]
    KwRepeat,
    #[token("UNTIL", ignore(case))]
    KwUntil,
    #[token("END_REPEAT", ignore(case))]
    KwEndRepeat,
    #[token("RETURN", ignore(case))]
    KwReturn,
    #[token("EXIT", ignore(case))]
    KwExit,
    #[token("CONTINUE", ignore(case))]
    KwContinue,
    #[token("JMP", ignore(case))]
    KwJmp,
    #[token("STEP", ignore(case))]
    KwStep,
    #[token("END_STEP", ignore(case))]
    KwEndStep,
    #[token("INITIAL_STEP", ignore(case))]
    KwInitialStep,
    #[token("TRANSITION", ignore(case))]
    KwTransition,
    #[token("END_TRANSITION", ignore(case))]
    KwEndTransition,
    #[token("FROM", ignore(case))]
    KwFrom,
    #[token("AND", ignore(case))]
    KwAnd,
    #[token("OR", ignore(case))]
    KwOr,
    #[token("XOR", ignore(case))]
    KwXor,
    #[token("NOT", ignore(case))]
    KwNot,
    #[token("MOD", ignore(case))]
    KwMod,
    #[token("BOOL", ignore(case))]
    KwBool,
    #[token("SINT", ignore(case))]
    KwSInt,
    #[token("INT", ignore(case))]
    KwInt,
    #[token("DINT", ignore(case))]
    KwDInt,
    #[token("LINT", ignore(case))]
    KwLInt,
    #[token("USINT", ignore(case))]
    KwUSInt,
    #[token("UINT", ignore(case))]
    KwUInt,
    #[token("UDINT", ignore(case))]
    KwUDInt,
    #[token("ULINT", ignore(case))]
    KwULInt,
    #[token("REAL", ignore(case))]
    KwReal,
    #[token("LREAL", ignore(case))]
    KwLReal,
    #[token("BYTE", ignore(case))]
    KwByte,
    #[token("WORD", ignore(case))]
    KwWord,
    #[token("DWORD", ignore(case))]
    KwDWord,
    #[token("LWORD", ignore(case))]
    KwLWord,
    #[token("TIME", ignore(case))]
    KwTime,
    #[token("LTIME", ignore(case))]
    KwLTime,
    #[token("DATE", ignore(case))]
    KwDate,
    #[token("LDATE", ignore(case))]
    KwLDate,
    #[token("TIME_OF_DAY", ignore(case))]
    #[token("TOD", ignore(case))]
    KwTimeOfDay,
    #[token("LTIME_OF_DAY", ignore(case))]
    #[token("LTOD", ignore(case))]
    KwLTimeOfDay,
    #[token("DATE_AND_TIME", ignore(case))]
    #[token("DT", ignore(case))]
    KwDateAndTime,
    #[token("LDATE_AND_TIME", ignore(case))]
    #[token("LDT", ignore(case))]
    KwLDateAndTime,
    #[token("CHAR", ignore(case))]
    KwChar,
    #[token("WCHAR", ignore(case))]
    KwWChar,
    #[token("ANY", ignore(case))]
    KwAny,
    #[token("ANY_DERIVED", ignore(case))]
    KwAnyDerived,
    #[token("ANY_ELEMENTARY", ignore(case))]
    KwAnyElementary,
    #[token("ANY_MAGNITUDE", ignore(case))]
    KwAnyMagnitude,
    #[token("ANY_INT", ignore(case))]
    KwAnyInt,
    #[token("ANY_UNSIGNED", ignore(case))]
    KwAnyUnsigned,
    #[token("ANY_SIGNED", ignore(case))]
    KwAnySigned,
    #[token("ANY_REAL", ignore(case))]
    KwAnyReal,
    #[token("ANY_NUM", ignore(case))]
    KwAnyNum,
    #[token("ANY_DURATION", ignore(case))]
    KwAnyDuration,
    #[token("ANY_BIT", ignore(case))]
    KwAnyBit,
    #[token("ANY_CHARS", ignore(case))]
    KwAnyChars,
    #[token("ANY_STRING", ignore(case))]
    KwAnyString,
    #[token("ANY_CHAR", ignore(case))]
    KwAnyChar,
    #[token("ANY_DATE", ignore(case))]
    KwAnyDate,
    #[token("TRUE", ignore(case))]
    KwTrue,
    #[token("FALSE", ignore(case))]
    KwFalse,
    #[token("NULL", ignore(case))]
    KwNull,
    #[token("CONFIGURATION", ignore(case))]
    KwConfiguration,
    #[token("END_CONFIGURATION", ignore(case))]
    KwEndConfiguration,
    #[token("RESOURCE", ignore(case))]
    KwResource,
    #[token("END_RESOURCE", ignore(case))]
    KwEndResource,
    #[token("ON", ignore(case))]
    KwOn,
    #[token("READ_WRITE", ignore(case))]
    KwReadWrite,
    #[token("READ_ONLY", ignore(case))]
    KwReadOnly,
    #[token("TASK", ignore(case))]
    KwTask,
    #[token("WITH", ignore(case))]
    KwWith,
    #[token("AT", ignore(case))]
    KwAt,
    #[token("EN", ignore(case))]
    KwEn,
    #[token("ENO", ignore(case))]
    KwEno,
    #[token("R_EDGE", ignore(case))]
    KwREdge,
    #[token("F_EDGE", ignore(case))]
    KwFEdge,
    #[token("ADR", ignore(case))]
    KwAdr,
    #[token("SIZEOF", ignore(case))]
    KwSizeOf,
    #[token("GET", ignore(case))]
    KwGet,
    #[token("END_GET", ignore(case))]
    KwEndGet,
    #[token("SET", ignore(case))]
    KwSet,
    #[token("END_SET", ignore(case))]
    KwEndSet,
    #[regex(r"[0-9]([0-9]|_[0-9])*")]
    #[regex(r"16#[0-9A-Fa-f]([0-9A-Fa-f]|_[0-9A-Fa-f])*")]
    #[regex(r"2#[01]([01]|_[01])*")]
    #[regex(r"8#[0-7]([0-7]|_[0-7])*")]
    IntLiteral,
    #[regex(r"[0-9]([0-9]|_[0-9])*\.[0-9]([0-9]|_[0-9])*([eE][+-]?[0-9]([0-9]|_[0-9])*)?")]
    RealLiteral,
    #[regex(
        r"(?:T|TIME|LT|LTIME)#[+-]?(?:[0-9]+(?:\.[0-9]+)?(?:ms|us|ns|d|h|m|s))(?:_?(?:[0-9]+(?:\.[0-9]+)?(?:ms|us|ns|d|h|m|s)))*",
        ignore(case)
    )]
    TimeLiteral,
    #[regex(r"(?:DATE|D|LDATE|LD)#[0-9]{4}-[0-9]{2}-[0-9]{2}", ignore(case))]
    DateLiteral,
    #[regex(r"TOD#[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9][0-9_]*)?", ignore(case))]
    #[regex(
        r"TIME_OF_DAY#[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9][0-9_]*)?",
        ignore(case)
    )]
    #[regex(r"LTOD#[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9][0-9_]*)?", ignore(case))]
    #[regex(
        r"LTIME_OF_DAY#[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9][0-9_]*)?",
        ignore(case)
    )]
    TimeOfDayLiteral,
    #[regex(
        r"DT#[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9][0-9_]*)?",
        ignore(case)
    )]
    #[regex(
        r"DATE_AND_TIME#[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9][0-9_]*)?",
        ignore(case)
    )]
    #[regex(
        r"LDT#[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9][0-9_]*)?",
        ignore(case)
    )]
    #[regex(
        r"LDATE_AND_TIME#[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9][0-9_]*)?",
        ignore(case)
    )]
    DateAndTimeLiteral,
    #[regex(
        r"'([^$'\r\n]|\$\$|\$[LlNnPpRrTt]|\$'|\$[0-9A-Fa-f]{2})*\$'''",
        priority = 3
    )]
    #[regex(
        r"'([^$'\r\n]|\$\$|\$[LlNnPpRrTt]|\$'|\$[0-9A-Fa-f]{2})*'",
        priority = 2
    )]
    StringLiteral,
    #[regex(
        r#""([^$"\r\n]|\$\$|\$[LlNnPpRrTt]|\$"|\$[0-9A-Fa-f]{4})*""#,
        priority = 2
    )]
    WideStringLiteral,
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*#")]
    TypedLiteralPrefix,
    #[regex(r"%[IQM]\*")]
    #[regex(r"%[IQM][XBWDL]?[0-9]+(\.[0-9]+)*")]
    #[regex(r"%[XBWDL][0-9]+(_?[0-9]+)*(\.[0-9]+(_?[0-9]+)*)*", ignore(case))]
    DirectAddress,
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*")]
    Ident,
    #[regex(r"'[^'\r\n]*'", priority = 1)]
    #[regex(r#""[^"\r\n]*""#, priority = 1)]
    #[default]
    Error,
    Eof,
}
