//! Direct addresses that a visual program names (specification 03 §5, specification 25 §11.4).
//!
//! A visual program may name an I/O or memory location directly, for example a Ladder contact on
//! `%IX0.0` or a Statechart action writing `%QX0.0`. The companion lowers each address to a global
//! `ld_io_<address>` that the runtime entry declares `AT` the address, with the type of the
//! address size (specification 03 §5: `X` `BOOL`, `B` `BYTE`, `W` `WORD`, `D` `DWORD`, `L`
//! `LWORD`). An address is recognised in the form the editors write, `%` then `I`, `Q` or `M`, a
//! size letter, a byte and an optional bit or word part, and only when the runtime's own address
//! parser (`trust_runtime_core::io_address`) accepts it.

use trust_runtime_core::io_address::{IoAddress, IoSize};

use crate::naming::sanitize_identifier;
use crate::Global;

/// The canonical (upper-case) spelling of `text` (trimmed) when it is one direct address.
#[must_use]
pub fn direct_address(text: &str) -> Option<String> {
    let candidate = text.trim().to_ascii_uppercase();
    (token_length(candidate.as_bytes()) == Some(candidate.len())
        && IoAddress::parse(&candidate).is_ok())
    .then_some(candidate)
}

/// Why `text` (trimmed), written like a direct address, is none: the runtime's address parser
/// refuses it, for example `%IX0.9` (a byte has bits 0 to 7). `None` when `text` is a direct
/// address or is not written like one.
#[must_use]
pub fn refused_address(text: &str) -> Option<String> {
    let candidate = text.trim().to_ascii_uppercase();
    (token_length(candidate.as_bytes()) == Some(candidate.len())
        && IoAddress::parse(&candidate).is_err())
    .then(|| format!("'{}' is not a valid direct address", text.trim()))
}

/// The name of the global that stands for `address`: `ld_io_<address without %>`, with `.`
/// written as `_` (`%IX0.0` is `ld_io_IX0_0`).
#[must_use]
pub fn alias_name(address: &str) -> String {
    let upper = address.trim().to_ascii_uppercase();
    let suffix = upper.strip_prefix('%').unwrap_or(&upper);
    sanitize_identifier(&format!("ld_io_{suffix}"), "ld_io_address")
}

/// The type of the address size (specification 03 §5); `BOOL` when the text is no address.
#[must_use]
pub fn address_type(address: &str) -> &'static str {
    match direct_address(address).and_then(|canonical| IoAddress::parse(&canonical).ok()) {
        Some(IoAddress {
            size: IoSize::Byte, ..
        }) => "BYTE",
        Some(IoAddress {
            size: IoSize::Word, ..
        }) => "WORD",
        Some(IoAddress {
            size: IoSize::DWord,
            ..
        }) => "DWORD",
        Some(IoAddress {
            size: IoSize::LWord,
            ..
        }) => "LWORD",
        _ => "BOOL",
    }
}

/// The global of the runtime entry that stands for `address` (already canonical).
#[must_use]
pub fn alias_global(address: &str) -> Global {
    Global {
        name: alias_name(address),
        address: Some(address.to_owned()),
        ty: address_type(address).to_owned(),
        initial: None,
    }
}

/// The distinct direct addresses in `text`, canonical, in the order of their first appearance.
#[must_use]
pub fn find_addresses(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for_each_token(text, |token| {
        if let Some(address) = direct_address(token)
            && !found.contains(&address)
        {
            found.push(address);
        }
    });
    found
}

/// `text` with each direct address replaced by the name of its global.
#[must_use]
pub fn rewrite_addresses(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut copied = 0;
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%'
            && let Some(length) = token_length(&bytes[at..])
            && let Some(address) = direct_address(&text[at..at + length])
        {
            out.push_str(&text[copied..at]);
            out.push_str(&alias_name(&address));
            at += length;
            copied = at;
            continue;
        }
        at += 1;
    }
    out.push_str(&text[copied..]);
    out
}

/// A generated `FUNCTION_BLOCK` text with its direct addresses replaced by their globals and a
/// `VAR_EXTERNAL` section for those globals inserted after the `FUNCTION_BLOCK` line; with the
/// globals the runtime entry declares. A text without addresses is returned unchanged.
#[must_use]
pub fn with_external_globals(source: &str) -> (String, Vec<Global>) {
    let globals: Vec<Global> = find_addresses(source)
        .iter()
        .map(|address| alias_global(address))
        .collect();
    if globals.is_empty() {
        return (source.to_owned(), globals);
    }
    let rewritten = rewrite_addresses(source);
    let mut lines: Vec<String> = rewritten
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_owned())
        .collect();
    let Some(header) = lines
        .iter()
        .position(|line| starts_with_keyword(line, "FUNCTION_BLOCK"))
    else {
        return (rewritten, globals);
    };
    let mut section = vec!["VAR_EXTERNAL".to_owned()];
    section.extend(
        globals
            .iter()
            .map(|global| format!("  {} : {};", global.name, global.ty)),
    );
    section.push("END_VAR".to_owned());
    section.push(String::new());
    let at = header + 1;
    lines.splice(at..at, section);
    (lines.join("\n"), globals)
}

/// Whether `line` starts, after blanks, with `keyword` as a whole word (any case).
fn starts_with_keyword(line: &str, keyword: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.len() >= keyword.len()
        && trimmed.as_bytes()[..keyword.len()].eq_ignore_ascii_case(keyword.as_bytes())
        && !trimmed[keyword.len()..].starts_with(|ch: char| ch.is_ascii_alphanumeric() || ch == '_')
}

/// Calls `visit` with each address-shaped token in `text`, left to right.
fn for_each_token(text: &str, mut visit: impl FnMut(&str)) {
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%'
            && let Some(length) = token_length(&bytes[at..])
        {
            visit(&text[at..at + length]);
            at += length;
            continue;
        }
        at += 1;
    }
}

/// The length of the address-shaped token at the start of `bytes`: `%`, one of `IQM`, one of
/// `XBWDL` (any case), digits, and an optional `.` with digits.
fn token_length(bytes: &[u8]) -> Option<usize> {
    if bytes.first() != Some(&b'%') {
        return None;
    }
    if !matches!(
        bytes.get(1).map(u8::to_ascii_uppercase),
        Some(b'I' | b'Q' | b'M')
    ) {
        return None;
    }
    if !matches!(
        bytes.get(2).map(u8::to_ascii_uppercase),
        Some(b'X' | b'B' | b'W' | b'D' | b'L')
    ) {
        return None;
    }
    let digits = |from: usize| {
        bytes[from..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count()
    };
    let byte_digits = digits(3);
    if byte_digits == 0 {
        return None;
    }
    let mut length = 3 + byte_digits;
    if bytes.get(length) == Some(&b'.') {
        let part_digits = digits(length + 1);
        if part_digits > 0 {
            length += 1 + part_digits;
        }
    }
    Some(length)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_is_recognised_in_the_editors_form_when_the_runtime_accepts_it() {
        // Catches a recogniser that differs from the runtime: `%IX0.9` names bit 9 of a byte,
        // which the runtime's parser refuses, and a bare `%I0.0` has no size letter.
        assert_eq!(direct_address(" %ix0.1 ").as_deref(), Some("%IX0.1"));
        assert_eq!(direct_address("%QW10").as_deref(), Some("%QW10"));
        assert_eq!(direct_address("%MD4.1").as_deref(), Some("%MD4.1"));
        assert_eq!(direct_address("%IX0.9"), None);
        assert_eq!(direct_address("%I0.0"), None);
        assert_eq!(direct_address("%IX0.0.1"), None);
        assert_eq!(direct_address("Start"), None);
        assert_eq!(
            refused_address(" %ix0.9 ").as_deref(),
            Some("'%ix0.9' is not a valid direct address")
        );
        assert_eq!(refused_address("%IX0.7"), None);
        assert_eq!(refused_address("Start"), None);
    }

    #[test]
    fn the_size_letter_gives_the_type_of_specification_03() {
        // Specification 03 §5: X BOOL, B BYTE, W WORD, D DWORD, L LWORD.
        assert_eq!(address_type("%IX0.0"), "BOOL");
        assert_eq!(address_type("%QB1"), "BYTE");
        assert_eq!(address_type("%MW2"), "WORD");
        assert_eq!(address_type("%ID4"), "DWORD");
        assert_eq!(address_type("%QL8"), "LWORD");
        assert_eq!(alias_name("%qx0.10"), "ld_io_QX0_10");
    }

    #[test]
    fn a_function_block_gets_its_addresses_as_external_globals() {
        // Worked by hand: two distinct addresses (the second use of %QX0.0 is the same global), in
        // order of first use, rewritten in the body and declared after the header line.
        let source = "FUNCTION_BLOCK FB_x\nVAR\nEND_VAR\n%QX0.0 := %ix0.1;\n%QX0.0 := FALSE;\nEND_FUNCTION_BLOCK";
        let (text, globals) = with_external_globals(source);
        assert_eq!(
            text,
            "FUNCTION_BLOCK FB_x\nVAR_EXTERNAL\n  ld_io_QX0_0 : BOOL;\n  ld_io_IX0_1 : BOOL;\nEND_VAR\n\nVAR\nEND_VAR\nld_io_QX0_0 := ld_io_IX0_1;\nld_io_QX0_0 := FALSE;\nEND_FUNCTION_BLOCK"
        );
        assert_eq!(
            globals,
            vec![
                Global {
                    name: "ld_io_QX0_0".into(),
                    address: Some("%QX0.0".into()),
                    ty: "BOOL".into(),
                    initial: None
                },
                Global {
                    name: "ld_io_IX0_1".into(),
                    address: Some("%IX0.1".into()),
                    ty: "BOOL".into(),
                    initial: None
                },
            ]
        );
    }

    #[test]
    fn a_text_without_addresses_is_unchanged() {
        let source = "FUNCTION_BLOCK FB_x\r\nEND_FUNCTION_BLOCK";
        assert_eq!(
            with_external_globals(source),
            (source.to_owned(), Vec::new())
        );
    }
}
