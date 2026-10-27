//! Names and literals of the generated Structured Text (specification 25 §11.4).

use std::fmt::Write as _;

/// An IEC identifier made from `raw`: every character other than an ASCII letter, digit or `_`
/// becomes `_`, runs of `_` become one, `_` is trimmed from both ends, and a leading digit gets a
/// `_` in front. An empty result is `fallback`.
#[must_use]
pub fn sanitize_identifier(raw: &str, fallback: &str) -> String {
    let mut normalized = String::new();
    for ch in raw.trim().chars() {
        let ch = if ch.is_ascii_alphanumeric() || ch == '_' {
            ch
        } else {
            '_'
        };
        if ch == '_' && normalized.ends_with('_') {
            continue;
        }
        normalized.push(ch);
    }
    let normalized = normalized.trim_matches('_');
    if normalized.is_empty() {
        return fallback.to_owned();
    }
    if normalized.starts_with(|ch: char| ch.is_ascii_digit()) {
        return format!("_{normalized}");
    }
    normalized.to_owned()
}

/// The companion function block's name: `FB_<base>_<suffix>`, for example
/// `FB_simple_start_stop_LADDER`.
#[must_use]
pub fn fb_name_for_source(base_name: &str, suffix: &str) -> String {
    let id = sanitize_identifier(base_name, "Generated");
    sanitize_identifier(&format!("FB_{id}_{suffix}"), "FB_Generated")
}

/// The input of a statechart event: `EV_<EVENT>` in upper case.
#[must_use]
pub fn event_input_name(event: &str) -> String {
    sanitize_identifier(&format!("EV_{event}").to_uppercase(), "EV_TRIGGER")
}

/// A generated local's name: `<prefix>_<source>`.
#[must_use]
pub fn local_name(prefix: &str, source: &str) -> String {
    sanitize_identifier(
        &format!("{prefix}_{source}"),
        &format!("{prefix}_generated"),
    )
}

/// Whether `text` (trimmed) is one IEC identifier: a letter or `_`, then letters, digits or `_`.
#[must_use]
pub fn is_simple_identifier(text: &str) -> bool {
    is_identifier(text.trim())
}

/// Whether `text`, as it is, is one IEC identifier.
fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// Whether `text` (trimmed) can be assigned to: a direct address (specification 03 §5) or a
/// dotted path of identifiers such as `fb.Output`.
#[must_use]
pub fn is_assignable_reference(text: &str) -> bool {
    let candidate = text.trim();
    if candidate.is_empty() {
        return false;
    }
    if crate::direct_io::direct_address(candidate).is_some() {
        return true;
    }
    candidate.split('.').all(is_identifier)
}

/// A single-quoted Structured Text string holding `value`, with the escapes of specification 01
/// §9: `$$` for `$`, `$'` for `'`, `$L`, `$R`, `$T` and `$P` for line feed, carriage return, tab
/// and form feed, and `$hh` for any other control character.
#[must_use]
pub fn st_string(value: &str) -> String {
    let mut out = String::from("'");
    for ch in value.chars() {
        match ch {
            '$' => out.push_str("$$"),
            '\'' => out.push_str("$'"),
            '\n' => out.push_str("$L"),
            '\r' => out.push_str("$R"),
            '\t' => out.push_str("$T"),
            '\u{c}' => out.push_str("$P"),
            ch if u32::from(ch) < 0x20 => {
                let _ = write!(out, "${:02X}", u32::from(ch));
            }
            ch => out.push(ch),
        }
    }
    out.push('\'');
    out
}

/// `text` made safe inside a `(* … *)` comment: a comment delimiter in it is split by a space
/// and a line break becomes a space, so a name never ends the comment early.
#[must_use]
pub fn comment_text(text: &str) -> String {
    text.replace("(*", "( *")
        .replace("*)", "* )")
        .replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_sanitized_as_the_editors_named_them() {
        // Worked from the rule: non-identifier characters become `_`, runs collapse, ends are
        // trimmed, a leading digit gets `_`. Catches a port that keeps `-` or doubles `_`.
        assert_eq!(
            sanitize_identifier("simple-start-stop", "X"),
            "simple_start_stop"
        );
        assert_eq!(sanitize_identifier("  __a--b__  ", "X"), "a_b");
        assert_eq!(sanitize_identifier("1st", "X"), "_1st");
        assert_eq!(sanitize_identifier("---", "Fallback"), "Fallback");
        assert_eq!(sanitize_identifier("ä b", "X"), "b");
        assert_eq!(
            fb_name_for_source("simple-start-stop", "LADDER"),
            "FB_simple_start_stop_LADDER"
        );
        assert_eq!(fb_name_for_source("", "SFC"), "FB_Generated_SFC");
        assert_eq!(event_input_name("start"), "EV_START");
        assert_eq!(local_name("ld_timer", "T 1"), "ld_timer_T_1");
    }

    #[test]
    fn assignable_references_are_direct_addresses_or_dotted_identifiers() {
        assert!(is_assignable_reference("%QX0.0"));
        assert!(is_assignable_reference("fb.Output"));
        assert!(is_assignable_reference(" Motor "));
        assert!(!is_assignable_reference(""));
        assert!(!is_assignable_reference("a.."));
        assert!(!is_assignable_reference("1abc"));
        assert!(!is_assignable_reference("LOCAL::Motor"));
        assert!(!is_assignable_reference("a. b"));
    }

    #[test]
    fn strings_use_the_dollar_escapes_of_specification_01() {
        // Specification 01 §9: `$'` is the quote, `$$` the dollar. Catches the earlier `''`
        // doubling, which IEC reads as two adjacent strings.
        assert_eq!(st_string("Bob's $5"), "'Bob$'s $$5'");
        assert_eq!(st_string("a\nb\tc"), "'a$Lb$Tc'");
        assert_eq!(st_string("\u{1}"), "'$01'");
    }

    #[test]
    fn comment_text_cannot_end_a_comment() {
        assert_eq!(comment_text("a*)b(*c\nd"), "a* )b( *c d");
    }
}
