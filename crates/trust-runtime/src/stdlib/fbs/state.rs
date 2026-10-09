pub(super) const STATE_PREV_CU: &str = "__ST_PREV_CU";
pub(super) const STATE_PREV_CD: &str = "__ST_PREV_CD";
pub(super) const STATE_TRIG_M: &str = "__ST_TRIG_M";
pub(super) const STATE_LAST_TIME: &str = "__ST_LAST_TIME";
pub(super) const STATE_PREV_IN: &str = "__ST_PREV_IN";
pub(super) const STATE_TIMING: &str = "__ST_TIMING";
pub(super) const STATE_ACTIVE: &str = "__ST_ACTIVE";

/// Structural state reserved for source-free instances before native execution.
/// Initial values remain owned by the native implementation's first-use logic.
pub(crate) fn builtin_state_layout(
    kind: super::registry::BuiltinFbKind,
) -> &'static [(&'static str, trust_hir::TypeId)] {
    use super::registry::BuiltinFbKind;
    use trust_hir::TypeId;
    match kind {
        BuiltinFbKind::Rs | BuiltinFbKind::Sr => &[],
        BuiltinFbKind::RTrig | BuiltinFbKind::FTrig => &[(STATE_TRIG_M, TypeId::BOOL)],
        BuiltinFbKind::Ctu => &[(STATE_PREV_CU, TypeId::BOOL)],
        BuiltinFbKind::Ctd => &[(STATE_PREV_CD, TypeId::BOOL)],
        BuiltinFbKind::Ctud => &[(STATE_PREV_CU, TypeId::BOOL), (STATE_PREV_CD, TypeId::BOOL)],
        BuiltinFbKind::Ton => &[(STATE_LAST_TIME, TypeId::LTIME)],
        BuiltinFbKind::Tof => &[
            (STATE_LAST_TIME, TypeId::LTIME),
            (STATE_PREV_IN, TypeId::BOOL),
            (STATE_TIMING, TypeId::BOOL),
        ],
        BuiltinFbKind::Tp => &[
            (STATE_LAST_TIME, TypeId::LTIME),
            (STATE_PREV_IN, TypeId::BOOL),
            (STATE_ACTIVE, TypeId::BOOL),
        ],
    }
}
