//! The sentences of the Apply section of a study (specification 35, A8.1, B6.4, B6.5 and A11.1). The rules that decide them are in
//! `study::apply`.

/// Restore previous settings is refused: no Apply's settings are applied, or the Restore already returned them (B6.4).
pub const RESTORE_UNAVAILABLE: &str = "Restore previous settings is not available: no Apply has applied settings that it could return, or those settings were already restored.";

/// Keep in project is refused: it follows an Apply whose settings are applied, and it is not offered after a Restore (B6.5).
pub const KEEP_UNAVAILABLE: &str = "Keep in project is not available: it follows an Apply whose settings are applied, and it is not offered after Restore previous settings.";

/// The sentence of a section that breaks one of its rules (B10.4): it names the rule, and the section is not kept.
#[must_use]
pub fn broken_section(reason: &str) -> String {
    format!("The apply section of the study cannot be used: {reason}.")
}
