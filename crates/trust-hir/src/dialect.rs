//! The language rules a project is analyzed under: IEC 61131-3 as truST reads it, or the
//! departures of a vendor profile (specification 14, `[project] vendor_profile`).

/// The rules a project's Structured Text follows where a vendor departs from IEC 61131-3.
///
/// The `[project] vendor_profile` of the project selects it, by the names that select the
/// conversion rules (specification 07 §2.10): `codesys`, `beckhoff` and `twincat` (TwinCAT is
/// built on CODESYS) select [`Dialect::Codesys`]; `iec`, `siemens`, `mitsubishi` and `gxworks3`, an
/// empty name or none select [`Dialect::Iec`]. Each departure of a profile is recorded in
/// `docs/IEC_DEVIATIONS.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Dialect {
    /// IEC 61131-3 Ed.3 as truST reads it (`docs/IEC_DECISIONS.md`).
    #[default]
    Iec,
    /// The IEC rules with the CODESYS departures: public function block variables, assignment to
    /// a function block's inputs in its body, access to a program's variables by its name, and
    /// references dereferenced without `^`.
    Codesys,
}

impl Dialect {
    /// The rules of a `vendor_profile` name (trimmed, any case); `None` for a name that is no
    /// vendor profile, which the caller reports instead of guessing.
    #[must_use]
    pub fn from_vendor_profile(name: &str) -> Option<Self> {
        let name = name.trim();
        let named = |candidates: &[&str]| {
            candidates
                .iter()
                .any(|candidate| name.eq_ignore_ascii_case(candidate))
        };
        if named(&["codesys", "beckhoff", "twincat"]) {
            Some(Self::Codesys)
        } else if name.is_empty() || named(&["iec", "siemens", "mitsubishi", "gxworks3"]) {
            Some(Self::Iec)
        } else {
            None
        }
    }

    /// Whether the CODESYS departures apply.
    #[must_use]
    pub fn is_codesys(self) -> bool {
        self == Self::Codesys
    }
}

#[cfg(test)]
mod tests {
    use super::Dialect;

    /// Every vendor profile of specification 14 selects the rules its conversion profile selects
    /// (specification 07 §2.10), any case and with white space around it; an empty name is IEC;
    /// a misspelled one selects nothing. Catches TwinCAT or Beckhoff analyzed under the IEC rules
    /// while they convert under CODESYS's, and a typo silently analyzed as IEC.
    #[test]
    fn the_vendor_profiles() {
        for name in ["codesys", "CODESYS", " beckhoff ", "TwinCAT"] {
            assert_eq!(
                Dialect::from_vendor_profile(name),
                Some(Dialect::Codesys),
                "{name}"
            );
        }
        for name in ["iec", "Siemens", "mitsubishi", "GXWorks3", "", "  "] {
            assert_eq!(
                Dialect::from_vendor_profile(name),
                Some(Dialect::Iec),
                "{name:?}"
            );
        }
        assert_eq!(Dialect::from_vendor_profile("codesis"), None);
        assert_eq!(Dialect::default(), Dialect::Iec);
    }
}
