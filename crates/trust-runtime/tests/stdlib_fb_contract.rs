//! The standard function block registry: lookup by name and alias, unique and complete
//! definitions, and the counter and timer signatures. Their behaviour in a running program is pinned
//! by the `oracle_fb_*` tests.

use std::collections::BTreeSet;

use trust_hir::symbols::ParamDirection;
use trust_hir::TypeId;
use trust_runtime::stdlib::fbs::{
    builtin_kind, builtin_kind_uppercase, standard_function_blocks, BuiltinFbKind,
};

#[test]
fn builtin_lookup_is_case_insensitive_and_includes_documented_aliases() {
    let cases = [
        ("rs", BuiltinFbKind::Rs),
        ("Sr", BuiltinFbKind::Sr),
        ("r_trig", BuiltinFbKind::RTrig),
        ("DIFU", BuiltinFbKind::RTrig),
        ("f_TrIg", BuiltinFbKind::FTrig),
        ("difd", BuiltinFbKind::FTrig),
        ("ctu", BuiltinFbKind::Ctu),
        ("CTU_DINT", BuiltinFbKind::Ctu),
        ("ctd_ulint", BuiltinFbKind::Ctd),
        ("CtUd_UdInT", BuiltinFbKind::Ctud),
        ("tp_time", BuiltinFbKind::Tp),
        ("TON_LTIME", BuiltinFbKind::Ton),
        ("tof", BuiltinFbKind::Tof),
    ];

    for (name, expected) in cases {
        assert_eq!(builtin_kind(name), Some(expected), "{name}");
    }
    assert_eq!(builtin_kind("not_a_standard_fb"), None);
    assert_eq!(
        builtin_kind_uppercase("TON_LTIME"),
        Some(BuiltinFbKind::Ton)
    );
}

#[test]
fn standard_function_block_definitions_are_unique_and_complete() {
    let definitions = standard_function_blocks();
    let names = definitions
        .iter()
        .map(|definition| definition.name.as_str())
        .collect::<BTreeSet<_>>();

    assert_eq!(definitions.len(), 33);
    assert_eq!(names.len(), definitions.len());
    for required in [
        "RS",
        "SR",
        "R_TRIG",
        "F_TRIG",
        "DIFU",
        "DIFD",
        "CTU",
        "CTD_DINT",
        "CTUD_ULINT",
        "TP",
        "TON_TIME",
        "TOF_LTIME",
    ] {
        assert!(names.contains(required), "missing {required}");
    }
}

#[test]
fn registry_counter_signatures_keep_preset_and_current_value_types_equal() {
    let definitions = standard_function_blocks();

    for definition in definitions.iter().filter(|definition| {
        definition.name.starts_with("CTU")
            || definition.name.starts_with("CTD")
            || definition.name.starts_with("CTUD")
    }) {
        let pv = definition
            .params
            .iter()
            .find(|parameter| parameter.name == "PV")
            .expect("counter must expose PV");
        let cv = definition
            .params
            .iter()
            .find(|parameter| parameter.name == "CV")
            .expect("counter must expose CV");
        assert_eq!(pv.type_id, cv.type_id, "{}", definition.name);
        assert_eq!(pv.direction, ParamDirection::In, "{}", definition.name);
        assert_eq!(cv.direction, ParamDirection::Out, "{}", definition.name);
    }
}

#[test]
fn registry_timer_signatures_keep_preset_and_elapsed_types_equal() {
    let definitions = standard_function_blocks();

    for definition in definitions.iter().filter(|definition| {
        definition.name.starts_with("TP")
            || definition.name.starts_with("TON")
            || definition.name.starts_with("TOF")
    }) {
        let pt = definition
            .params
            .iter()
            .find(|parameter| parameter.name == "PT")
            .expect("timer must expose PT");
        let et = definition
            .params
            .iter()
            .find(|parameter| parameter.name == "ET")
            .expect("timer must expose ET");
        assert_eq!(pt.type_id, et.type_id, "{}", definition.name);
        assert!(
            matches!(pt.type_id, TypeId::TIME | TypeId::LTIME),
            "{}",
            definition.name
        );
        assert_eq!(pt.direction, ParamDirection::In, "{}", definition.name);
        assert_eq!(et.direction, ParamDirection::Out, "{}", definition.name);
    }
}
