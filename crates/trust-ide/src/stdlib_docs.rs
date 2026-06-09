//! Standard library documentation helpers for Structured Text.
//!
//! Provides lightweight IEC-referenced docs for standard functions and function blocks
//! to surface in hover/completion.

use once_cell::sync::Lazy;
use rustc_hash::FxHashSet;
use smol_str::SmolStr;

/// A standard function entry with documentation.
#[derive(Debug, Clone)]
pub struct StdlibFunctionEntry {
    /// Function name.
    pub name: SmolStr,
    /// Documentation string.
    pub doc: &'static str,
}

/// Standard library filtering configuration.
#[derive(Debug, Clone, Default)]
pub struct StdlibFilter {
    allowed_functions: Option<FxHashSet<SmolStr>>,
    allowed_function_blocks: Option<FxHashSet<SmolStr>>,
}

impl StdlibFilter {
    /// Allow all standard library entries.
    pub fn allow_all() -> Self {
        Self {
            allowed_functions: None,
            allowed_function_blocks: None,
        }
    }

    /// Build a filter from allow-lists (case-insensitive).
    pub fn with_allowlists(
        functions: Option<Vec<String>>,
        function_blocks: Option<Vec<String>>,
    ) -> Self {
        let allowed_functions = functions.map(|list| normalize_set(&list));
        let allowed_function_blocks = function_blocks.map(|list| normalize_set(&list));
        Self {
            allowed_functions,
            allowed_function_blocks,
        }
    }

    /// Build a filter from a named profile (e.g., full, iec, none).
    pub fn from_profile(profile: &str) -> Self {
        match profile.trim().to_ascii_lowercase().as_str() {
            "none" => StdlibFilter::with_allowlists(Some(Vec::new()), Some(Vec::new())),
            "iec" | "standard" => StdlibFilter::with_allowlists(
                Some(STANDARD_FUNCTION_NAMES.clone()),
                Some(STANDARD_FB_NAME_LIST.clone()),
            ),
            _ => StdlibFilter::allow_all(),
        }
    }

    /// Returns true if the standard function name is allowed.
    pub fn allows_function(&self, name: &str) -> bool {
        allows_name(&self.allowed_functions, name)
    }

    /// Returns true if the standard function block name is allowed.
    pub fn allows_function_block(&self, name: &str) -> bool {
        allows_name(&self.allowed_function_blocks, name)
    }
}

const DOC_CONVERSIONS: &str = "Standard conversion function (IEC 61131-3 Ed.3, Tables 22-27).";
const DOC_NUMERIC_SINGLE: &str = "Standard numeric function (IEC 61131-3 Ed.3, Table 28).";
const DOC_NUMERIC_MULTI: &str = "Standard arithmetic function (IEC 61131-3 Ed.3, Table 29).";
const DOC_BIT_SHIFT: &str = "Standard bit shift/rotate function (IEC 61131-3 Ed.3, Table 30).";
const DOC_BITWISE: &str = "Standard bitwise function (IEC 61131-3 Ed.3, Table 31).";
const DOC_SELECTION: &str = "Standard selection function (IEC 61131-3 Ed.3, Table 32).";
const DOC_COMPARISON: &str = "Standard comparison function (IEC 61131-3 Ed.3, Table 33).";
const DOC_STRING: &str = "Standard string function (IEC 61131-3 Ed.3, Table 34).";
const DOC_TIME_NUMERIC: &str = "Standard time arithmetic function (IEC 61131-3 Ed.3, Table 35).";
const DOC_TIME_SPLIT: &str = "Standard time/date function (IEC 61131-3 Ed.3, Table 36).";
const DOC_VALIDATE: &str = "Standard validate function (IEC 61131-3 Ed.3, Table 39).";

const DOC_FB_BISTABLE: &str = "Standard bistable function block (IEC 61131-3 Ed.3, Table 43).";
const DOC_FB_EDGE: &str = "Standard edge detection function block (IEC 61131-3 Ed.3, Table 44).";
const DOC_FB_COUNTER: &str = "Standard counter function block (IEC 61131-3 Ed.3, Table 45).";
const DOC_FB_TIMER: &str = "Standard timer function block (IEC 61131-3 Ed.3, Table 46).";

const NUMERIC_SINGLE: &[&str] = &[
    "ABS", "SQRT", "LN", "LOG", "EXP", "SIN", "COS", "TAN", "ASIN", "ACOS", "ATAN", "ATAN2",
];
const NUMERIC_MULTI: &[&str] = &["ADD", "SUB", "MUL", "DIV", "MOD", "EXPT", "MOVE"];
const BIT_SHIFT: &[&str] = &["SHL", "SHR", "ROL", "ROR"];
const BITWISE: &[&str] = &["AND", "OR", "XOR", "NOT"];
const SELECTION: &[&str] = &["SEL", "MAX", "MIN", "LIMIT", "MUX"];
const COMPARISON: &[&str] = &["GT", "GE", "EQ", "LE", "LT", "NE"];
const VALIDATE_FUNCS: &[&str] = &["IS_VALID", "IS_VALID_BCD"];
const STRING_FUNCS: &[&str] = &[
    "LEN", "LEFT", "RIGHT", "MID", "CONCAT", "INSERT", "DELETE", "REPLACE", "FIND",
];
const TIME_NUMERIC: &[&str] = &[
    "ADD_TIME",
    "ADD_LTIME",
    "ADD_TOD_TIME",
    "ADD_LTOD_LTIME",
    "ADD_DT_TIME",
    "ADD_LDT_LTIME",
    "SUB_TIME",
    "SUB_LTIME",
    "SUB_DATE_DATE",
    "SUB_LDATE_LDATE",
    "SUB_TOD_TIME",
    "SUB_LTOD_LTIME",
    "SUB_TOD_TOD",
    "SUB_LTOD_LTOD",
    "SUB_DT_TIME",
    "SUB_LDT_LTIME",
    "SUB_DT_DT",
    "SUB_LDT_LDT",
    "MUL_TIME",
    "MUL_LTIME",
    "DIV_TIME",
    "DIV_LTIME",
];
const TIME_SPLIT: &[&str] = &[
    "CONCAT_DATE_TOD",
    "CONCAT_DATE_LTOD",
    "CONCAT_DATE",
    "CONCAT_TOD",
    "CONCAT_LTOD",
    "CONCAT_DT",
    "CONCAT_LDT",
    "SPLIT_DATE",
    "SPLIT_TOD",
    "SPLIT_LTOD",
    "SPLIT_DT",
    "SPLIT_LDT",
    "DAY_OF_WEEK",
];

const NUMERIC_TYPES: &[&str] = &[
    "LREAL", "REAL", "LINT", "DINT", "INT", "SINT", "ULINT", "UDINT", "UINT", "USINT",
];
const INTEGER_TYPES: &[&str] = &[
    "LINT", "DINT", "INT", "SINT", "ULINT", "UDINT", "UINT", "USINT",
];
const BIT_TYPES: &[&str] = &["LWORD", "DWORD", "WORD", "BYTE"];
const TIME_TYPES: &[&str] = &["TIME", "LTIME", "DATE", "LDATE", "TOD", "LTOD", "DT", "LDT"];
const CHAR_TYPES: &[&str] = &["STRING", "WSTRING", "CHAR", "WCHAR"];

static STANDARD_FUNCTIONS: Lazy<Vec<StdlibFunctionEntry>> = Lazy::new(|| {
    let mut entries = Vec::new();
    let mut seen: FxHashSet<String> = FxHashSet::default();

    let mut push = |name: String, doc: &'static str| {
        let key = name.to_ascii_uppercase();
        if seen.insert(key) {
            entries.push(StdlibFunctionEntry {
                name: SmolStr::new(name),
                doc,
            });
        }
    };

    for name in NUMERIC_SINGLE {
        push((*name).to_string(), DOC_NUMERIC_SINGLE);
    }
    for name in NUMERIC_MULTI {
        push((*name).to_string(), DOC_NUMERIC_MULTI);
    }
    for name in BIT_SHIFT {
        push((*name).to_string(), DOC_BIT_SHIFT);
    }
    for name in BITWISE {
        push((*name).to_string(), DOC_BITWISE);
    }
    for name in SELECTION {
        push((*name).to_string(), DOC_SELECTION);
    }
    for name in COMPARISON {
        push((*name).to_string(), DOC_COMPARISON);
    }
    for name in VALIDATE_FUNCS {
        push((*name).to_string(), DOC_VALIDATE);
    }
    for name in STRING_FUNCS {
        push((*name).to_string(), DOC_STRING);
    }
    for name in TIME_NUMERIC {
        push((*name).to_string(), DOC_TIME_NUMERIC);
    }
    for name in TIME_SPLIT {
        push((*name).to_string(), DOC_TIME_SPLIT);
    }

    // Conversion forms (Table 22)
    for dst in NUMERIC_TYPES
        .iter()
        .chain(BIT_TYPES.iter())
        .chain(TIME_TYPES.iter())
        .chain(CHAR_TYPES.iter())
        .chain(["BOOL"].iter())
    {
        push(format!("TO_{dst}"), DOC_CONVERSIONS);
    }
    push("TRUNC".to_string(), DOC_CONVERSIONS);
    for dst in INTEGER_TYPES {
        push(format!("TRUNC_{dst}"), DOC_CONVERSIONS);
    }

    for src in INTEGER_TYPES {
        for dst in INTEGER_TYPES {
            if src == dst {
                continue;
            }
            push(format!("{src}_TRUNC_{dst}"), DOC_CONVERSIONS);
        }
    }

    for src in INTEGER_TYPES {
        for dst in INTEGER_TYPES {
            if src == dst {
                continue;
            }
            push(format!("{src}_BCD_TO_{dst}"), DOC_CONVERSIONS);
            push(format!("{src}_TO_BCD_{dst}"), DOC_CONVERSIONS);
        }
    }
    for dst in INTEGER_TYPES {
        push(format!("BCD_TO_{dst}"), DOC_CONVERSIONS);
        push(format!("TO_BCD_{dst}"), DOC_CONVERSIONS);
    }

    // Table 23: Numeric conversions
    for src in NUMERIC_TYPES {
        for dst in NUMERIC_TYPES {
            if src == dst {
                continue;
            }
            push(format!("{src}_TO_{dst}"), DOC_CONVERSIONS);
        }
    }

    // Table 24: Bit conversions
    for src in BIT_TYPES {
        for dst in BIT_TYPES {
            if src == dst {
                continue;
            }
            push(format!("{src}_TO_{dst}"), DOC_CONVERSIONS);
        }
    }

    // Table 25: Bit <-> Numeric conversions + BOOL to numeric
    for src in BIT_TYPES {
        for dst in NUMERIC_TYPES {
            push(format!("{src}_TO_{dst}"), DOC_CONVERSIONS);
        }
    }
    for src in NUMERIC_TYPES {
        for dst in BIT_TYPES {
            push(format!("{src}_TO_{dst}"), DOC_CONVERSIONS);
        }
    }
    for dst in NUMERIC_TYPES {
        push(format!("BOOL_TO_{dst}"), DOC_CONVERSIONS);
    }

    // Table 26: Time/date conversions
    for (src, dst) in [
        ("LTIME", "TIME"),
        ("TIME", "LTIME"),
        ("LDT", "DT"),
        ("LDT", "DATE"),
        ("LDT", "LTOD"),
        ("LDT", "TOD"),
        ("DT", "LDT"),
        ("DT", "DATE"),
        ("DT", "LTOD"),
        ("DT", "TOD"),
        ("LTOD", "TOD"),
        ("TOD", "LTOD"),
    ] {
        push(format!("{src}_TO_{dst}"), DOC_CONVERSIONS);
    }

    // Table 27: Character conversions
    for (src, dst) in [
        ("WSTRING", "STRING"),
        ("WSTRING", "WCHAR"),
        ("STRING", "WSTRING"),
        ("STRING", "CHAR"),
        ("WCHAR", "WSTRING"),
        ("WCHAR", "CHAR"),
        ("CHAR", "STRING"),
        ("CHAR", "WCHAR"),
    ] {
        push(format!("{src}_TO_{dst}"), DOC_CONVERSIONS);
    }

    entries
});

static STANDARD_FUNCTION_SET: Lazy<FxHashSet<SmolStr>> = Lazy::new(|| {
    STANDARD_FUNCTIONS
        .iter()
        .map(|entry| normalize_name(entry.name.as_str()))
        .collect()
});

const STANDARD_FB_NAMES: &[(&str, &str)] = &[
    ("RS", DOC_FB_BISTABLE),
    ("SR", DOC_FB_BISTABLE),
    ("R_TRIG", DOC_FB_EDGE),
    ("F_TRIG", DOC_FB_EDGE),
    ("CTU", DOC_FB_COUNTER),
    ("CTD", DOC_FB_COUNTER),
    ("CTUD", DOC_FB_COUNTER),
    ("CTU_INT", DOC_FB_COUNTER),
    ("CTD_INT", DOC_FB_COUNTER),
    ("CTUD_INT", DOC_FB_COUNTER),
    ("CTU_DINT", DOC_FB_COUNTER),
    ("CTD_DINT", DOC_FB_COUNTER),
    ("CTUD_DINT", DOC_FB_COUNTER),
    ("CTU_LINT", DOC_FB_COUNTER),
    ("CTD_LINT", DOC_FB_COUNTER),
    ("CTUD_LINT", DOC_FB_COUNTER),
    ("CTU_UDINT", DOC_FB_COUNTER),
    ("CTD_UDINT", DOC_FB_COUNTER),
    ("CTUD_UDINT", DOC_FB_COUNTER),
    ("CTU_ULINT", DOC_FB_COUNTER),
    ("CTD_ULINT", DOC_FB_COUNTER),
    ("CTUD_ULINT", DOC_FB_COUNTER),
    ("TP", DOC_FB_TIMER),
    ("TON", DOC_FB_TIMER),
    ("TOF", DOC_FB_TIMER),
    ("TP_LTIME", DOC_FB_TIMER),
    ("TON_LTIME", DOC_FB_TIMER),
    ("TOF_LTIME", DOC_FB_TIMER),
];

static STANDARD_FB_SET: Lazy<FxHashSet<SmolStr>> = Lazy::new(|| {
    STANDARD_FB_NAMES
        .iter()
        .map(|(name, _)| normalize_name(name))
        .collect()
});

static STANDARD_FUNCTION_NAMES: Lazy<Vec<String>> = Lazy::new(|| {
    standard_function_entries()
        .iter()
        .map(|entry| entry.name.as_str().to_string())
        .collect()
});

static STANDARD_FB_NAME_LIST: Lazy<Vec<String>> = Lazy::new(|| {
    standard_fb_names()
        .iter()
        .map(|name| (*name).to_string())
        .collect()
});

/// Returns the standard function entries (name + doc).
pub fn standard_function_entries() -> &'static [StdlibFunctionEntry] {
    STANDARD_FUNCTIONS.as_slice()
}

/// Returns true if the name matches a standard function.
pub fn is_standard_function_name(name: &str) -> bool {
    STANDARD_FUNCTION_SET.contains(&normalize_name(name))
}

/// Returns true if the name matches a standard function block.
pub fn is_standard_fb_name(name: &str) -> bool {
    STANDARD_FB_SET.contains(&normalize_name(name))
}

/// Returns documentation for a standard function name (case-insensitive).
pub fn standard_function_doc(name: &str) -> Option<&'static str> {
    let upper = name.to_ascii_uppercase();
    STANDARD_FUNCTIONS
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(&upper))
        .map(|entry| entry.doc)
}

/// Returns documentation for a standard function block name (case-insensitive).
pub fn standard_fb_doc(name: &str) -> Option<&'static str> {
    STANDARD_FB_NAMES
        .iter()
        .find(|(fb_name, _)| fb_name.eq_ignore_ascii_case(name))
        .map(|(_, doc)| *doc)
}

/// Returns the standard function block names (for completion/docs).
pub fn standard_fb_names() -> &'static [&'static str] {
    const NAMES: &[&str] = &[
        "RS",
        "SR",
        "R_TRIG",
        "F_TRIG",
        "CTU",
        "CTD",
        "CTUD",
        "CTU_INT",
        "CTD_INT",
        "CTUD_INT",
        "CTU_DINT",
        "CTD_DINT",
        "CTUD_DINT",
        "CTU_LINT",
        "CTD_LINT",
        "CTUD_LINT",
        "CTU_UDINT",
        "CTD_UDINT",
        "CTUD_UDINT",
        "CTU_ULINT",
        "CTD_ULINT",
        "CTUD_ULINT",
        "TP",
        "TON",
        "TOF",
        "TP_LTIME",
        "TON_LTIME",
        "TOF_LTIME",
    ];
    NAMES
}

fn normalize_name(name: &str) -> SmolStr {
    SmolStr::new(name.to_ascii_uppercase())
}

fn normalize_set(list: &[String]) -> FxHashSet<SmolStr> {
    list.iter().map(|name| normalize_name(name)).collect()
}

fn allows_name(allowed: &Option<FxHashSet<SmolStr>>, name: &str) -> bool {
    allowed
        .as_ref()
        .is_none_or(|set| set.contains(&normalize_name(name)))
}

/// Returns documentation for typed literals by prefix (case-insensitive).
pub fn typed_literal_doc(prefix: &str) -> Option<&'static str> {
    let upper = prefix.to_ascii_uppercase();
    match upper.as_str() {
        "TIME" | "T" | "LTIME" | "LT" => {
            Some("Duration literal (IEC 61131-3 Ed.3, Table 8). Example: T#1s, LTIME#100ms.")
        }
        "DATE" | "D" | "LDATE" | "LD" => {
            Some("Date literal (IEC 61131-3 Ed.3, Table 9). Example: DATE#2024-01-15.")
        }
        "TIME_OF_DAY" | "TOD" | "LTIME_OF_DAY" | "LTOD" => {
            Some("Time-of-day literal (IEC 61131-3 Ed.3, Table 9). Example: TOD#14:30:00.")
        }
        "DATE_AND_TIME" | "DT" | "LDATE_AND_TIME" | "LDT" => Some(
            "Date-and-time literal (IEC 61131-3 Ed.3, Table 9). Example: DT#2024-01-15-14:30:00.",
        ),
        "STRING" | "WSTRING" | "CHAR" | "WCHAR" => {
            Some("Character string literal (IEC 61131-3 Ed.3, Tables 6-7). Example: STRING#'abc'.")
        }
        "BOOL" => Some("Boolean literal (IEC 61131-3 Ed.3, Table 5). Example: BOOL#TRUE."),
        "SINT" | "INT" | "DINT" | "LINT" | "USINT" | "UINT" | "UDINT" | "ULINT" | "REAL"
        | "LREAL" | "BYTE" | "WORD" | "DWORD" | "LWORD" => Some(
            "Numeric literal with type prefix (IEC 61131-3 Ed.3, Table 5). Example: INT#16#FF.",
        ),
        _ => None,
    }
}
