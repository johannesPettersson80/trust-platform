//! Plan identity and the visible-slot boundary of automatic initializers (spec 12 §7.11).

use super::super::VmPouEntry;
use super::*;

use crate::bytecode::{BytecodeVersion, PouKind, TypeTable};
use crate::program_model::{Expr, FunctionDef};

fn call_transfer_local(name: &str, initializer: Option<Expr>) -> VarDef {
    VarDef {
        name: name.into(),
        type_id: TypeId::DINT,
        initializer,
        retain: crate::RetainPolicy::Unspecified,
        static_storage: false,
        external: false,
        in_out: false,
        constant: false,
        address: None,
    }
}

fn call_transfer_function(name: &str, local_names: &[&str]) -> FunctionDef {
    FunctionDef {
        name: name.into(),
        return_type: TypeId::DINT,
        params: Vec::new(),
        locals: local_names
            .iter()
            .map(|local| call_transfer_local(local, None))
            .collect(),
        static_locals: Vec::new(),
        using: Vec::new(),
        body: Vec::new(),
    }
}

fn call_transfer_module(pou_name: &str, local_ref_count: u32) -> VmModule {
    let mut module = VmModule::legacy(
        BytecodeVersion::new(1, 1),
        Vec::new(),
        Vec::new(),
        TypeTable::default(),
        Vec::new(),
        Vec::new(),
    )
    .expect("legacy fixture metadata");
    module.define_legacy_pou(
        1,
        PouKind::Function,
        VmPouEntry {
            kind: PouKind::Function,
            name: pou_name.into(),
            code_start: 0,
            code_end: 0,
            local_ref_start: 0,
            local_ref_count,
            primary_instance_owner: None,
        },
        Vec::new(),
        false,
    );
    module
}

fn call_transfer_frame(count: usize) -> VmFrame {
    VmFrame {
        parameter_values_present: Vec::new(),
        activation: None,
        pou_id: Some(1),
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: count as u32,
        locals: vec![Value::Null; count],
        runtime_instance: None,
        instance_owner: None,
    }
}

/// Spec 12 §7.11: a plan is selected by exact POU identity and is never reused for another module.
#[test]
fn call_transfer_plan_cache_never_reuses_a_plan_from_another_module() {
    let mut runtime = crate::runtime::LegacyRuntimeFixture::new();
    runtime.register_function(call_transfer_function("F", &["A"]));
    runtime.register_function(call_transfer_function("G", &["B", "C"]));
    let module_a = call_transfer_module("F", 1);
    let module_b = call_transfer_module("G", 2);

    let plan_a = runtime
        .vm_local_init_plan_cache
        .plan_for(&runtime, &module_a, 1)
        .expect("module A plan");
    assert_eq!(plan_a.frame_owner().as_str(), "F");
    assert_eq!(plan_a.locals().len(), 1);

    let plan_b = runtime
        .vm_local_init_plan_cache
        .plan_for(&runtime, &module_b, 1)
        .expect("module B plan");
    assert_eq!(plan_b.frame_owner().as_str(), "G");
    assert_eq!(plan_b.locals().len(), 2);
}

/// Spec 12 §7.11: the plan is selected by exact POU identity, so two POUs keep their own frames.
#[test]
fn call_transfer_plan_is_selected_by_exact_pou_identity() {
    let mut runtime = crate::runtime::LegacyRuntimeFixture::new();
    runtime.register_function(call_transfer_function("F", &["A"]));
    runtime.register_function(call_transfer_function("G", &["B", "C"]));
    let mut module = call_transfer_module("F", 1);
    module.define_legacy_pou(
        2,
        PouKind::Function,
        VmPouEntry {
            kind: PouKind::Function,
            name: "G".into(),
            code_start: 0,
            code_end: 0,
            local_ref_start: 0,
            local_ref_count: 2,
            primary_instance_owner: None,
        },
        Vec::new(),
        false,
    );
    let first = runtime
        .vm_local_init_plan_cache
        .plan_for(&runtime, &module, 1)
        .expect("POU 1 plan");
    assert_eq!(first.frame_owner().as_str(), "F");
    assert_eq!(first.locals().len(), 1);

    let second = runtime
        .vm_local_init_plan_cache
        .plan_for(&runtime, &module, 2)
        .expect("POU 2 plan");
    assert_eq!(second.frame_owner().as_str(), "G");
    assert_eq!(second.locals().len(), 2);
}

/// Spec 12 §7.11: replacing a definition clears the cached plan, so the new frame layout is used.
#[test]
fn call_transfer_plan_cache_rebuilds_after_a_definition_is_replaced() {
    let mut runtime = crate::runtime::LegacyRuntimeFixture::new();
    runtime.register_function(call_transfer_function("F", &["A"]));
    let module = call_transfer_module("F", 1);
    let first = runtime
        .vm_local_init_plan_cache
        .plan_for(&runtime, &module, 1)
        .expect("first plan");
    assert_eq!(first.locals().len(), 1);

    // Replacing the definition invalidates the runtime's own cache, so the next lookup rebuilds.
    runtime.register_function(call_transfer_function("F", &["A", "D"]));
    let replaced = runtime
        .vm_local_init_plan_cache
        .plan_for(&runtime, &module, 1)
        .expect("replaced plan");
    assert!(!Arc::ptr_eq(&first, &replaced));
    assert_eq!(replaced.locals().len(), 2);
    assert_eq!(replaced.locals()[1].name.as_str(), "D");
}

/// Spec 12 §7.11: an earlier initializer cannot read a later local; the frame is unchanged.
#[test]
fn call_transfer_initializer_cannot_read_a_later_declared_local() {
    let mut runtime = crate::runtime::LegacyRuntimeFixture::new();
    let plan = VmPouInitPlan::Program {
        frame_owner: "Main".into(),
        locals: vec![
            call_transfer_local("First", Some(Expr::Literal(Value::DInt(17)))),
            call_transfer_local("Second", Some(Expr::Name("later".into()))),
            call_transfer_local("Later", Some(Expr::Literal(Value::DInt(5)))),
        ],
    };
    let mut frame = call_transfer_frame(3);
    frame.locals = vec![Value::DInt(-1), Value::DInt(-2), Value::DInt(-3)];
    let before = frame.locals.clone();

    let error = initialize_declared_locals_direct(&mut runtime, &plan, &mut frame)
        .expect_err("a later local must not be readable by an earlier initializer");

    assert!(matches!(
        error,
        RuntimeError::InitFailed {
            owner,
            variable,
            ..
        } if owner == "Main" && variable == "Second"
    ));
    assert_eq!(frame.locals, before);
}
