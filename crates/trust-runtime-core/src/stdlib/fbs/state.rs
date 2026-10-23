/// Internal native-state slot `STATE_PREV_CU`, shared by declaration and execution.
pub const STATE_PREV_CU: &str = "__ST_PREV_CU";
/// Internal native-state slot `STATE_PREV_CD`, shared by declaration and execution.
pub const STATE_PREV_CD: &str = "__ST_PREV_CD";
/// Internal native-state slot `STATE_TRIG_M`, shared by declaration and execution.
pub const STATE_TRIG_M: &str = "__ST_TRIG_M";
/// Internal native-state slot `STATE_LAST_TIME`, shared by declaration and execution.
pub const STATE_LAST_TIME: &str = "__ST_LAST_TIME";
/// Internal native-state slot `STATE_PREV_IN`, shared by declaration and execution.
pub const STATE_PREV_IN: &str = "__ST_PREV_IN";
/// Internal native-state slot `STATE_TIMING`, shared by declaration and execution.
pub const STATE_TIMING: &str = "__ST_TIMING";
/// Internal native-state slot `STATE_ACTIVE`, shared by declaration and execution.
pub const STATE_ACTIVE: &str = "__ST_ACTIVE";

/// Structural state reserved for source-free instances before native execution.
/// Initial values remain owned by the native implementation's first-use logic.
pub fn builtin_state_layout(kind: super::BuiltinFbKind) -> &'static [(&'static str, u16)] {
    use super::BuiltinFbKind;
    match kind {
        BuiltinFbKind::Rs | BuiltinFbKind::Sr => &[],
        BuiltinFbKind::RTrig | BuiltinFbKind::FTrig => &[(STATE_TRIG_M, 1)],
        BuiltinFbKind::Ctu => &[(STATE_PREV_CU, 1)],
        BuiltinFbKind::Ctd => &[(STATE_PREV_CD, 1)],
        BuiltinFbKind::Ctud => &[(STATE_PREV_CU, 1), (STATE_PREV_CD, 1)],
        BuiltinFbKind::Ton => &[(STATE_LAST_TIME, 17)],
        BuiltinFbKind::Tof => &[(STATE_LAST_TIME, 17), (STATE_PREV_IN, 1), (STATE_TIMING, 1)],
        BuiltinFbKind::Tp => &[(STATE_LAST_TIME, 17), (STATE_PREV_IN, 1), (STATE_ACTIVE, 1)],
    }
}
