use crate::eval::{FunctionBlockDef, Param};
use trust_hir::symbols::ParamDirection;
use trust_hir::TypeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFbKind {
    Rs,
    Sr,
    RTrig,
    FTrig,
    Ctu,
    Ctd,
    Ctud,
    Tp,
    Ton,
    Tof,
}

pub fn builtin_kind(name: &str) -> Option<BuiltinFbKind> {
    let upper = name.to_ascii_uppercase();
    match upper.as_str() {
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
        "TP" | "TP_LTIME" => Some(BuiltinFbKind::Tp),
        "TON" | "TON_LTIME" => Some(BuiltinFbKind::Ton),
        "TOF" | "TOF_LTIME" => Some(BuiltinFbKind::Tof),
        _ => None,
    }
}

pub fn standard_function_blocks() -> Vec<FunctionBlockDef> {
    fn fb(name: &str, params: &[(&str, TypeId, ParamDirection)]) -> FunctionBlockDef {
        FunctionBlockDef {
            name: name.into(),
            base: None,
            params: params
                .iter()
                .map(|(param_name, type_id, direction)| Param {
                    name: (*param_name).into(),
                    type_id: *type_id,
                    direction: *direction,
                    address: None,
                    default: None,
                })
                .collect(),
            vars: Vec::new(),
            temps: Vec::new(),
            using: Vec::new(),
            methods: Vec::new(),
            body: Vec::new(),
        }
    }

    let mut defs = vec![
        fb(
            "RS",
            &[
                ("S", TypeId::BOOL, ParamDirection::In),
                ("R1", TypeId::BOOL, ParamDirection::In),
                ("Q1", TypeId::BOOL, ParamDirection::Out),
            ],
        ),
        fb(
            "SR",
            &[
                ("S1", TypeId::BOOL, ParamDirection::In),
                ("R", TypeId::BOOL, ParamDirection::In),
                ("Q1", TypeId::BOOL, ParamDirection::Out),
            ],
        ),
        fb(
            "R_TRIG",
            &[
                ("CLK", TypeId::BOOL, ParamDirection::In),
                ("Q", TypeId::BOOL, ParamDirection::Out),
            ],
        ),
        fb(
            "F_TRIG",
            &[
                ("CLK", TypeId::BOOL, ParamDirection::In),
                ("Q", TypeId::BOOL, ParamDirection::Out),
            ],
        ),
        fb(
            "DIFU",
            &[
                ("CLK", TypeId::BOOL, ParamDirection::In),
                ("Q", TypeId::BOOL, ParamDirection::Out),
            ],
        ),
        fb(
            "DIFD",
            &[
                ("CLK", TypeId::BOOL, ParamDirection::In),
                ("Q", TypeId::BOOL, ParamDirection::Out),
            ],
        ),
    ];

    let counter_types = [
        ("", TypeId::ANY_INT),
        ("_INT", TypeId::INT),
        ("_DINT", TypeId::DINT),
        ("_LINT", TypeId::LINT),
        ("_UDINT", TypeId::UDINT),
        ("_ULINT", TypeId::ULINT),
    ];

    for (suffix, type_id) in counter_types {
        defs.push(fb(
            &format!("CTU{suffix}"),
            &[
                ("CU", TypeId::BOOL, ParamDirection::In),
                ("R", TypeId::BOOL, ParamDirection::In),
                ("PV", type_id, ParamDirection::In),
                ("Q", TypeId::BOOL, ParamDirection::Out),
                ("CV", type_id, ParamDirection::Out),
            ],
        ));
        defs.push(fb(
            &format!("CTD{suffix}"),
            &[
                ("CD", TypeId::BOOL, ParamDirection::In),
                ("LD", TypeId::BOOL, ParamDirection::In),
                ("PV", type_id, ParamDirection::In),
                ("Q", TypeId::BOOL, ParamDirection::Out),
                ("CV", type_id, ParamDirection::Out),
            ],
        ));
        defs.push(fb(
            &format!("CTUD{suffix}"),
            &[
                ("CU", TypeId::BOOL, ParamDirection::In),
                ("CD", TypeId::BOOL, ParamDirection::In),
                ("R", TypeId::BOOL, ParamDirection::In),
                ("LD", TypeId::BOOL, ParamDirection::In),
                ("PV", type_id, ParamDirection::In),
                ("QU", TypeId::BOOL, ParamDirection::Out),
                ("QD", TypeId::BOOL, ParamDirection::Out),
                ("CV", type_id, ParamDirection::Out),
            ],
        ));
    }

    let timers = [
        ("TP", TypeId::TIME),
        ("TON", TypeId::TIME),
        ("TOF", TypeId::TIME),
        ("TP_LTIME", TypeId::LTIME),
        ("TON_LTIME", TypeId::LTIME),
        ("TOF_LTIME", TypeId::LTIME),
    ];

    for (name, time_type) in timers {
        defs.push(fb(
            name,
            &[
                ("IN", TypeId::BOOL, ParamDirection::In),
                ("PT", time_type, ParamDirection::In),
                ("Q", TypeId::BOOL, ParamDirection::Out),
                ("ET", time_type, ParamDirection::Out),
            ],
        ));
    }

    defs
}
