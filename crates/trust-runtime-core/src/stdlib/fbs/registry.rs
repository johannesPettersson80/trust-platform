/// Recognized native function-block implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFbKind {
    /// Reset-dominant bistable state.
    Rs,
    /// Set-dominant bistable state.
    Sr,
    /// Rising-edge trigger state.
    RTrig,
    /// Falling-edge trigger state.
    FTrig,
    /// Up-counter state.
    Ctu,
    /// Down-counter state.
    Ctd,
    /// Up/down-counter state.
    Ctud,
    /// Pulse timer state.
    Tp,
    /// On-delay timer state.
    Ton,
    /// Off-delay timer state.
    Tof,
}

/// Recognize a native function-block name case-insensitively.
pub fn builtin_kind(name: &str) -> Option<BuiltinFbKind> {
    let upper = name.to_ascii_uppercase();
    builtin_kind_uppercase(upper.as_str())
}

/// Recognize an already canonical uppercase native function-block name.
pub fn builtin_kind_uppercase(name_upper: &str) -> Option<BuiltinFbKind> {
    match name_upper {
        "RS" => Some(BuiltinFbKind::Rs),
        "SR" => Some(BuiltinFbKind::Sr),
        "R_TRIG" | "DIFU" => Some(BuiltinFbKind::RTrig),
        "F_TRIG" | "DIFD" => Some(BuiltinFbKind::FTrig),
        "CTU" | "CTU_INT" | "CTU_DINT" | "CTU_LINT" | "CTU_UDINT" | "CTU_ULINT" => {
            Some(BuiltinFbKind::Ctu)
        }
        "CTD" | "CTD_INT" | "CTD_DINT" | "CTD_LINT" | "CTD_UDINT" | "CTD_ULINT" => {
            Some(BuiltinFbKind::Ctd)
        }
        "CTUD" | "CTUD_INT" | "CTUD_DINT" | "CTUD_LINT" | "CTUD_UDINT" | "CTUD_ULINT" => {
            Some(BuiltinFbKind::Ctud)
        }
        "TP" | "TP_TIME" | "TP_LTIME" => Some(BuiltinFbKind::Tp),
        "TON" | "TON_TIME" | "TON_LTIME" => Some(BuiltinFbKind::Ton),
        "TOF" | "TOF_TIME" | "TOF_LTIME" => Some(BuiltinFbKind::Tof),
        _ => None,
    }
}
