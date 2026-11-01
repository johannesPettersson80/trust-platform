use crate::program_model::{FunctionBlockDef, Param};
use trust_hir::symbols::ParamDirection;
use trust_hir::TypeId;

/// Return the IEC standard function-block declarations used by hosted authoring.
pub fn standard_function_blocks() -> Vec<FunctionBlockDef> {
    fn fb(name: &str, params: &[(&str, TypeId, ParamDirection)]) -> FunctionBlockDef {
        FunctionBlockDef {
            name: name.into(),
            base: None,
            interfaces: Vec::new(),
            params: params
                .iter()
                .map(|(param_name, type_id, direction)| Param {
                    name: (*param_name).into(),
                    type_id: *type_id,
                    direction: *direction,
                    address: None,
                    default: None,
                    retain: crate::RetainPolicy::Unspecified,
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

    // IEC 61131-3 Ed.3 Table 45, 1a, 2a and 3a: `CTU`, `CTD` and `CTUD` are the INT counters.
    let counter_types = [
        ("", TypeId::INT),
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
        ("TP_TIME", TypeId::TIME),
        ("TON_TIME", TypeId::TIME),
        ("TOF_TIME", TypeId::TIME),
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
