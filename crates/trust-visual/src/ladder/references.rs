//! Symbol references in Ladder operands (specification 16 §5).
//!
//! An operand names a symbol, a direct address or a literal. A symbol may be qualified with its
//! scope: `LOCAL::Name`, `GLOBAL::Name`, `LOCAL.Name`, `GLOBAL.Name`, or the short `L::Name` and
//! `G::Name`, in any case. An unqualified name resolves to the local of that name first, then to the
//! global.

use crate::js;
use crate::naming::is_simple_identifier;

/// The scope a qualified reference names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// `LOCAL::`, `L::`, `LOCAL.`: a variable of the companion function block.
    Local,
    /// `GLOBAL::`, `G::`, `GLOBAL.`: a global of the runtime entry.
    Global,
}

impl Scope {
    /// The scope of a declared variable: `local` is local, anything else (and no scope) is global
    /// (specification 16 §2).
    #[must_use]
    pub fn of_variable(scope: Option<&str>) -> Self {
        if scope == Some("local") {
            Self::Local
        } else {
            Self::Global
        }
    }
}

/// A symbol reference: the scope it names, if any, and the name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference<'a> {
    /// The scope the operand names, or `None` when it is unqualified.
    pub scope: Option<Scope>,
    /// The name after the qualifier, trimmed.
    pub name: &'a str,
}

/// The qualifiers, longest first so `LOCAL::` is not read as `LOCAL.`.
const QUALIFIERS: [(&str, Scope); 6] = [
    ("LOCAL::", Scope::Local),
    ("GLOBAL::", Scope::Global),
    ("L::", Scope::Local),
    ("G::", Scope::Global),
    ("LOCAL.", Scope::Local),
    ("GLOBAL.", Scope::Global),
];

/// The symbol reference of an operand; `None` for a blank operand or one that starts with `%`
/// (a direct address).
#[must_use]
pub fn parse(operand: &str) -> Option<Reference<'_>> {
    let trimmed = js::trim(operand);
    if trimmed.is_empty() || trimmed.starts_with('%') {
        return None;
    }
    for (qualifier, scope) in QUALIFIERS {
        if trimmed.len() >= qualifier.len()
            && trimmed.as_bytes()[..qualifier.len()].eq_ignore_ascii_case(qualifier.as_bytes())
        {
            return Some(Reference {
                scope: Some(scope),
                name: js::trim(&trimmed[qualifier.len()..]),
            });
        }
    }
    Some(Reference {
        scope: None,
        name: trimmed,
    })
}

/// The qualified reference of an operand whose name is an identifier, or `None`.
#[must_use]
pub fn qualified(operand: &str) -> Option<(Scope, &str)> {
    let reference = parse(operand)?;
    let scope = reference.scope?;
    is_simple_identifier(reference.name).then_some((scope, reference.name))
}

/// A name compared without case, as IEC compares identifiers.
#[must_use]
pub fn key(name: &str) -> String {
    js::trim(name).to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualifiers_name_a_scope_in_any_case() {
        // Specification 16 §5 names LOCAL::, GLOBAL::, LOCAL. and GLOBAL.; the editor also writes
        // L:: and G::. Catches `LOCAL::` read as `LOCAL.` plus `:Name`.
        assert_eq!(
            parse(" local::Motor "),
            Some(Reference {
                scope: Some(Scope::Local),
                name: "Motor"
            })
        );
        assert_eq!(
            parse("G::Pump"),
            Some(Reference {
                scope: Some(Scope::Global),
                name: "Pump"
            })
        );
        assert_eq!(
            parse("GLOBAL.Valve"),
            Some(Reference {
                scope: Some(Scope::Global),
                name: "Valve"
            })
        );
        assert_eq!(
            parse("Start"),
            Some(Reference {
                scope: None,
                name: "Start"
            })
        );
        assert_eq!(parse("%IX0.0"), None);
        assert_eq!(parse("  "), None);
        assert_eq!(qualified("LOCAL::x"), Some((Scope::Local, "x")));
        assert_eq!(qualified("LOCAL::1x"), None);
        assert_eq!(qualified("x"), None);
    }
}
