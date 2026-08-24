use std::collections::BTreeSet;

use trust_hir::symbols::ParamDirection;
use trust_hir::TypeId;
use trust_runtime::error::RuntimeError;
use trust_runtime::memory::VariableStorage;
use trust_runtime::stdlib::fbs::{
    builtin_kind, builtin_kind_uppercase, execute_builtin_in_storage, standard_function_blocks,
    BuiltinFbKind, Ctd, Ctu, Ctud, FTrig, RTrig, Rs, Sr, Tof, Ton, Tp,
};
use trust_runtime::value::{Duration, Value};

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

#[test]
fn sr_truth_table_is_set_dominant_and_retains_state() {
    let mut sr = Sr::new();

    assert!(!sr.step(false, false));
    assert!(!sr.step(false, true));
    assert!(sr.step(true, false));
    assert!(sr.step(false, false));
    assert!(sr.step(true, true));
}

#[test]
fn rs_truth_table_is_reset_dominant_and_retains_state() {
    let mut rs = Rs::new();

    assert!(!rs.step(false, false));
    assert!(rs.step(true, false));
    assert!(rs.step(false, false));
    assert!(!rs.step(false, true));
    assert!(!rs.step(true, true));
}

#[test]
fn rising_trigger_pulses_once_per_low_to_high_transition() {
    let mut trigger = RTrig::new();
    let outputs = [false, true, true, false, false, true]
        .into_iter()
        .map(|clock| trigger.step(clock))
        .collect::<Vec<_>>();

    assert_eq!(outputs, [false, true, false, false, false, true]);
}

#[test]
fn falling_trigger_cold_low_pulses_then_requires_high_to_rearm() {
    let mut trigger = FTrig::new();
    let outputs = [false, false, true, true, false, false]
        .into_iter()
        .map(|clock| trigger.step(clock))
        .collect::<Vec<_>>();

    assert_eq!(outputs, [true, false, false, false, true, false]);
}

#[test]
fn ctu_counts_only_rising_edges_and_reset_dominates_edge() {
    let mut counter = Ctu::new();

    assert_eq!(counter.step(true, false, 2).cv, 1);
    assert_eq!(counter.step(true, false, 2).cv, 1);
    assert_eq!(counter.step(false, false, 2).cv, 1);
    let reached = counter.step(true, false, 2);
    assert_eq!(reached.cv, 2);
    assert!(reached.q);

    let reset = counter.step(false, true, 2);
    assert_eq!(reset.cv, 0);
    assert!(!reset.q);
    let dominated = counter.step(true, true, 2);
    assert_eq!(dominated.cv, 0);
}

#[test]
fn ctd_load_dominates_edge_and_held_high_does_not_repeat() {
    let mut counter = Ctd::new();

    let loaded = counter.step(true, true, 3);
    assert_eq!(loaded.cv, 3);
    assert!(!loaded.q);
    assert_eq!(counter.step(true, false, 3).cv, 3);
    counter.step(false, false, 3);
    assert_eq!(counter.step(true, false, 3).cv, 2);
}

#[test]
fn ctud_priority_is_reset_then_load_then_edges() {
    let mut counter = Ctud::new();

    let reset = counter.step(true, false, true, true, 7);
    assert_eq!(reset.cv, 0);
    counter.step(false, false, false, false, 7);

    let loaded = counter.step(true, false, false, true, 7);
    assert_eq!(loaded.cv, 7);
    assert!(loaded.qu);
    counter.step(false, false, false, false, 7);

    let cancelled = counter.step(true, true, false, false, 7);
    assert_eq!(cancelled.cv, 7);
}

#[test]
fn signed_counter_steps_saturate_instead_of_wrapping() {
    let mut up_down = Ctud::new();

    up_down.step(false, false, false, true, i16::MAX);
    up_down.step(false, false, false, false, i16::MAX);
    let upper = up_down.step(true, false, false, false, i16::MAX);
    assert_eq!(upper.cv, i16::MAX);

    up_down.step(false, false, false, true, i16::MIN);
    up_down.step(false, false, false, false, i16::MIN);
    let lower = up_down.step(false, true, false, false, i16::MIN);
    assert_eq!(lower.cv, i16::MIN);

    let mut down = Ctd::new();
    down.step(false, true, i16::MIN);
    let lower = down.step(true, false, i16::MIN);
    assert_eq!(lower.cv, i16::MIN);
}

#[test]
fn counter_outputs_are_recomputed_from_post_transition_value() {
    let mut up = Ctu::new();
    let initial = up.step(false, false, -1);
    assert_eq!(initial.cv, 0);
    assert!(initial.q);

    let mut down = Ctd::new();
    let loaded = down.step(false, true, 0);
    assert_eq!(loaded.cv, 0);
    assert!(loaded.q);

    let mut up_down = Ctud::new();
    let initial = up_down.step(false, false, false, false, 0);
    assert!(initial.qu);
    assert!(initial.qd);
}

#[test]
fn storage_counter_preserves_generic_integer_family_and_edge_memory() {
    let mut storage = VariableStorage::default();
    let instance = storage.create_instance("CTU");
    for (name, value) in [
        ("CU", Value::Bool(true)),
        ("R", Value::Bool(false)),
        ("PV", Value::DInt(2)),
        ("Q", Value::Bool(false)),
        ("CV", Value::Null),
    ] {
        assert!(storage.set_instance_var(instance, name, value));
    }

    execute_builtin_in_storage(&mut storage, Duration::ZERO, instance, BuiltinFbKind::Ctu)
        .expect("first CTU edge");
    assert_eq!(
        storage.get_instance_var(instance, "CV"),
        Some(&Value::DInt(1))
    );
    assert_eq!(
        storage.get_instance_var(instance, "Q"),
        Some(&Value::Bool(false))
    );

    execute_builtin_in_storage(&mut storage, Duration::ZERO, instance, BuiltinFbKind::Ctu)
        .expect("held CU");
    assert_eq!(
        storage.get_instance_var(instance, "CV"),
        Some(&Value::DInt(1))
    );
}

#[test]
fn storage_counter_type_mismatch_does_not_publish_outputs() {
    let mut storage = VariableStorage::default();
    let instance = storage.create_instance("CTU");
    for (name, value) in [
        ("CU", Value::Bool(true)),
        ("R", Value::Bool(false)),
        ("PV", Value::DInt(2)),
        ("Q", Value::Bool(false)),
        ("CV", Value::Int(0)),
    ] {
        assert!(storage.set_instance_var(instance, name, value));
    }

    assert_eq!(
        execute_builtin_in_storage(&mut storage, Duration::ZERO, instance, BuiltinFbKind::Ctu,),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        storage.get_instance_var(instance, "CV"),
        Some(&Value::Int(0))
    );
    assert_eq!(
        storage.get_instance_var(instance, "Q"),
        Some(&Value::Bool(false))
    );
}

#[test]
fn ton_delays_output_and_resets_immediately_when_input_falls() {
    let mut timer = Ton::new();
    let preset = Duration::from_millis(10);

    let partial = timer.step(true, preset, Duration::from_millis(6));
    assert!(!partial.q);
    assert_eq!(partial.et, Duration::from_millis(6));
    let reset = timer.step(false, preset, Duration::from_millis(3));
    assert!(!reset.q);
    assert_eq!(reset.et, Duration::ZERO);
}

#[test]
fn tof_holds_expiry_plateau_until_high_input_rearms() {
    let mut timer = Tof::new();
    let preset = Duration::from_millis(10);

    assert!(timer.step(true, preset, Duration::ZERO).q);
    let expired = timer.step(false, preset, preset);
    assert!(!expired.q);
    assert_eq!(expired.et, preset);
    let held = timer.step(false, Duration::from_millis(20), Duration::from_millis(5));
    assert!(!held.q);
    assert_eq!(held.et, preset);
    let rearmed = timer.step(true, Duration::from_millis(20), Duration::ZERO);
    assert!(rearmed.q);
    assert_eq!(rearmed.et, Duration::ZERO);
}

#[test]
fn tp_falling_input_does_not_cancel_pulse_and_low_rearms_next_edge() {
    let mut timer = Tp::new();
    let preset = Duration::from_millis(10);

    let started = timer.step(true, preset, Duration::from_millis(3));
    assert!(started.q);
    let still_active = timer.step(false, preset, Duration::from_millis(3));
    assert!(still_active.q);
    assert_eq!(still_active.et, Duration::from_millis(6));
    let expired_low = timer.step(false, preset, Duration::from_millis(4));
    assert!(!expired_low.q);
    assert_eq!(expired_low.et, Duration::ZERO);
    let restarted = timer.step(true, preset, Duration::from_millis(2));
    assert!(restarted.q);
    assert_eq!(restarted.et, Duration::from_millis(2));
}

#[test]
fn timers_normalize_non_positive_presets_to_zero() {
    let negative = Duration::from_nanos(-1);

    let mut ton = Ton::new();
    let ton_out = ton.step(true, negative, Duration::ZERO);
    assert!(ton_out.q);
    assert_eq!(ton_out.et, Duration::ZERO);

    let mut tp = Tp::new();
    let tp_out = tp.step(true, negative, Duration::ZERO);
    assert!(!tp_out.q);
    assert_eq!(tp_out.et, Duration::ZERO);

    let mut tof = Tof::new();
    assert!(tof.step(true, negative, Duration::ZERO).q);
    let tof_out = tof.step(false, negative, Duration::ZERO);
    assert!(!tof_out.q);
    assert_eq!(tof_out.et, Duration::ZERO);
}

#[test]
fn active_timer_uses_current_preset_on_every_step() {
    let mut timer = Ton::new();

    let partial = timer.step(true, Duration::from_millis(10), Duration::from_millis(6));
    assert!(!partial.q);
    let shortened = timer.step(true, Duration::from_millis(5), Duration::ZERO);
    assert!(shortened.q);
    assert_eq!(shortened.et, Duration::from_millis(5));

    let lengthened = timer.step(true, Duration::from_millis(10), Duration::ZERO);
    assert!(!lengthened.q);
    assert_eq!(lengthened.et, Duration::from_millis(5));
}

#[test]
fn negative_direct_timer_delta_contributes_zero() {
    let preset = Duration::from_millis(10);
    let backwards = Duration::from_millis(-3);

    let mut ton = Ton::new();
    assert_eq!(
        ton.step(true, preset, Duration::from_millis(5)).et,
        Duration::from_millis(5)
    );
    assert_eq!(
        ton.step(true, preset, backwards).et,
        Duration::from_millis(5)
    );

    let mut tof = Tof::new();
    tof.step(true, preset, Duration::ZERO);
    assert_eq!(
        tof.step(false, preset, Duration::from_millis(5)).et,
        Duration::from_millis(5)
    );
    assert_eq!(
        tof.step(false, preset, backwards).et,
        Duration::from_millis(5)
    );

    let mut tp = Tp::new();
    assert_eq!(
        tp.step(true, preset, Duration::from_millis(5)).et,
        Duration::from_millis(5)
    );
    assert_eq!(
        tp.step(true, preset, backwards).et,
        Duration::from_millis(5)
    );
}

#[test]
fn storage_timer_first_call_and_backward_clock_contribute_zero() {
    let mut storage = VariableStorage::default();
    let instance = storage.create_instance("TON");
    for (name, value) in [
        ("IN", Value::Bool(true)),
        ("PT", Value::Time(Duration::from_millis(10))),
        ("Q", Value::Bool(false)),
        ("ET", Value::Time(Duration::ZERO)),
    ] {
        assert!(storage.set_instance_var(instance, name, value));
    }

    execute_builtin_in_storage(
        &mut storage,
        Duration::from_millis(5),
        instance,
        BuiltinFbKind::Ton,
    )
    .expect("first TON call");
    assert_eq!(
        storage.get_instance_var(instance, "ET"),
        Some(&Value::Time(Duration::ZERO))
    );

    execute_builtin_in_storage(
        &mut storage,
        Duration::from_millis(8),
        instance,
        BuiltinFbKind::Ton,
    )
    .expect("forward clock");
    assert_eq!(
        storage.get_instance_var(instance, "ET"),
        Some(&Value::Time(Duration::from_millis(3)))
    );

    execute_builtin_in_storage(
        &mut storage,
        Duration::from_millis(7),
        instance,
        BuiltinFbKind::Ton,
    )
    .expect("backward clock");
    assert_eq!(
        storage.get_instance_var(instance, "ET"),
        Some(&Value::Time(Duration::from_millis(3)))
    );

    execute_builtin_in_storage(
        &mut storage,
        Duration::from_millis(9),
        instance,
        BuiltinFbKind::Ton,
    )
    .expect("new forward baseline");
    assert_eq!(
        storage.get_instance_var(instance, "ET"),
        Some(&Value::Time(Duration::from_millis(5)))
    );
}

#[test]
fn storage_timer_preserves_ltime_output_family() {
    let mut storage = VariableStorage::default();
    let instance = storage.create_instance("TON_LTIME");
    for (name, value) in [
        ("IN", Value::Bool(true)),
        ("PT", Value::LTime(Duration::from_millis(2))),
        ("Q", Value::Bool(false)),
        ("ET", Value::Null),
    ] {
        assert!(storage.set_instance_var(instance, name, value));
    }

    execute_builtin_in_storage(&mut storage, Duration::ZERO, instance, BuiltinFbKind::Ton)
        .expect("first LTIME timer call");
    execute_builtin_in_storage(
        &mut storage,
        Duration::from_millis(2),
        instance,
        BuiltinFbKind::Ton,
    )
    .expect("second LTIME timer call");

    assert_eq!(
        storage.get_instance_var(instance, "Q"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        storage.get_instance_var(instance, "ET"),
        Some(&Value::LTime(Duration::from_millis(2)))
    );
}

#[test]
fn storage_timer_type_mismatch_does_not_publish_outputs() {
    let mut storage = VariableStorage::default();
    let instance = storage.create_instance("TON");
    for (name, value) in [
        ("IN", Value::Bool(true)),
        ("PT", Value::Time(Duration::from_millis(2))),
        ("Q", Value::Bool(false)),
        ("ET", Value::Int(0)),
    ] {
        assert!(storage.set_instance_var(instance, name, value));
    }

    assert_eq!(
        execute_builtin_in_storage(
            &mut storage,
            Duration::from_millis(1),
            instance,
            BuiltinFbKind::Ton,
        ),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        storage.get_instance_var(instance, "Q"),
        Some(&Value::Bool(false))
    );
    assert_eq!(
        storage.get_instance_var(instance, "ET"),
        Some(&Value::Int(0))
    );
}
