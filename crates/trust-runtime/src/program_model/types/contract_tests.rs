use super::*;

use crate::program_model::{Expr, Stmt};
use crate::value::Value;

fn parameter(name: &str, direction: ParamDirection) -> Param {
    Param {
        name: name.into(),
        type_id: TypeId::DINT,
        direction,
        address: Some(IoAddress::parse("%MW4").expect("test address")),
        default: Some(Expr::Literal(Value::DInt(7))),
    }
}

fn variable(name: &str) -> VarDef {
    VarDef {
        name: name.into(),
        type_id: TypeId::INT,
        initializer: Some(Expr::Literal(Value::Int(3))),
        retain: crate::RetainPolicy::Persistent,
        static_storage: true,
        external: true,
        in_out: false,
        constant: true,
        address: Some(IoAddress::parse("%IW2").expect("test address")),
    }
}

fn statement(value: i16) -> Stmt {
    Stmt::Expr {
        expr: Expr::Literal(Value::Int(value)),
        location: None,
    }
}

fn method(name: &str) -> MethodDef {
    MethodDef {
        name: name.into(),
        return_type: Some(TypeId::DINT),
        params: vec![parameter("input", ParamDirection::In)],
        locals: vec![variable("local")],
        static_locals: vec![variable("static")],
        using: vec!["Lib.One".into(), "Lib.Two".into()],
        body: vec![statement(1), statement(2)],
    }
}

#[test]
fn parameter_preserves_name_type_direction_address_and_default() {
    for direction in [
        ParamDirection::In,
        ParamDirection::Out,
        ParamDirection::InOut,
    ] {
        let param = parameter("value", direction);
        let cloned = param.clone();

        assert_eq!(cloned.name, "value");
        assert_eq!(cloned.type_id, TypeId::DINT);
        assert_eq!(cloned.direction, direction);
        assert_eq!(
            cloned.address,
            Some(IoAddress::parse("%MW4").expect("test address"))
        );
        assert!(matches!(
            cloned.default,
            Some(Expr::Literal(Value::DInt(7)))
        ));
    }
}

#[test]
fn variable_preserves_all_independent_storage_and_qualifier_fields() {
    let variable = variable("state").clone();

    assert_eq!(variable.name, "state");
    assert_eq!(variable.type_id, TypeId::INT);
    assert!(matches!(
        variable.initializer,
        Some(Expr::Literal(Value::Int(3)))
    ));
    assert_eq!(variable.retain, crate::RetainPolicy::Persistent);
    assert!(variable.static_storage);
    assert!(variable.external);
    assert!(variable.constant);
    assert_eq!(
        variable.address,
        Some(IoAddress::parse("%IW2").expect("test address"))
    );
}

#[test]
fn variable_flags_are_not_inferred_from_each_other() {
    let variable = VarDef {
        name: "plain".into(),
        type_id: TypeId::BOOL,
        initializer: None,
        retain: crate::RetainPolicy::Unspecified,
        static_storage: false,
        external: false,
        in_out: false,
        constant: false,
        address: None,
    };
    let cloned = variable.clone();

    assert_eq!(cloned.retain, crate::RetainPolicy::Unspecified);
    assert!(!cloned.static_storage);
    assert!(!cloned.external);
    assert!(!cloned.constant);
    assert_eq!(cloned.address, None);
    assert!(cloned.initializer.is_none());
}

#[test]
fn function_preserves_ordered_parameter_local_import_and_body_groups() {
    let function = FunctionDef {
        name: "Compute".into(),
        return_type: TypeId::LINT,
        params: vec![
            parameter("first", ParamDirection::In),
            parameter("second", ParamDirection::Out),
        ],
        locals: vec![variable("local_a"), variable("local_b")],
        static_locals: vec![variable("static_a")],
        using: vec!["First.Space".into(), "Second.Space".into()],
        body: vec![statement(1), statement(2)],
    }
    .clone();

    assert_eq!(function.name, "Compute");
    assert_eq!(function.return_type, TypeId::LINT);
    assert_eq!(
        function
            .params
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert_eq!(
        function
            .locals
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        ["local_a", "local_b"]
    );
    assert_eq!(function.static_locals[0].name, "static_a");
    assert_eq!(
        function
            .using
            .iter()
            .map(SmolStr::as_str)
            .collect::<Vec<_>>(),
        ["First.Space", "Second.Space"]
    );
    assert_eq!(function.body.len(), 2);
}

#[test]
fn function_block_base_keeps_function_block_and_class_kinds_distinct() {
    let fb = FunctionBlockBase::FunctionBlock("BaseFb".into()).clone();
    let class = FunctionBlockBase::Class("BaseClass".into()).clone();

    assert!(matches!(
        fb,
        FunctionBlockBase::FunctionBlock(name) if name == "BaseFb"
    ));
    assert!(matches!(
        class,
        FunctionBlockBase::Class(name) if name == "BaseClass"
    ));
}

#[test]
fn function_block_preserves_all_ordered_definition_groups() {
    let function_block = FunctionBlockDef {
        name: "Controller".into(),
        base: Some(FunctionBlockBase::Class("BaseController".into())),
        interfaces: vec!["IController".into()],
        params: vec![parameter("input", ParamDirection::In)],
        vars: vec![variable("state")],
        temps: vec![variable("scratch")],
        using: vec!["Control.Lib".into(), "Math.Lib".into()],
        methods: vec![method("Start"), method("Stop")],
        body: vec![statement(3), statement(4)],
    }
    .clone();

    assert_eq!(function_block.name, "Controller");
    assert!(matches!(
        function_block.base,
        Some(FunctionBlockBase::Class(name)) if name == "BaseController"
    ));
    assert_eq!(function_block.params[0].name, "input");
    assert_eq!(function_block.vars[0].name, "state");
    assert_eq!(function_block.temps[0].name, "scratch");
    assert_eq!(
        function_block
            .using
            .iter()
            .map(SmolStr::as_str)
            .collect::<Vec<_>>(),
        ["Control.Lib", "Math.Lib"]
    );
    assert_eq!(
        function_block
            .methods
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        ["Start", "Stop"]
    );
    assert_eq!(function_block.body.len(), 2);
}

#[test]
fn method_preserves_optional_result_and_ordered_local_groups() {
    let reviewed_method = method("Apply").clone();

    assert_eq!(reviewed_method.name, "Apply");
    assert_eq!(reviewed_method.return_type, Some(TypeId::DINT));
    assert_eq!(reviewed_method.params[0].name, "input");
    assert_eq!(reviewed_method.locals[0].name, "local");
    assert_eq!(reviewed_method.static_locals[0].name, "static");
    assert_eq!(
        reviewed_method
            .using
            .iter()
            .map(SmolStr::as_str)
            .collect::<Vec<_>>(),
        ["Lib.One", "Lib.Two"]
    );
    assert_eq!(reviewed_method.body.len(), 2);

    let procedure = MethodDef {
        return_type: None,
        ..method("Reset")
    };
    assert_eq!(procedure.return_type, None);
}

#[test]
fn class_preserves_base_variables_imports_and_methods_in_order() {
    let class = ClassDef {
        name: "Derived".into(),
        base: Some("Base".into()),
        interfaces: vec!["IOpen".into()],
        vars: vec![variable("first"), variable("second")],
        using: vec!["Z.Space".into(), "A.Space".into()],
        methods: vec![method("Open"), method("Close")],
    }
    .clone();

    assert_eq!(class.name, "Derived");
    assert_eq!(class.base.as_deref(), Some("Base"));
    assert_eq!(
        class
            .vars
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert_eq!(
        class.using.iter().map(SmolStr::as_str).collect::<Vec<_>>(),
        ["Z.Space", "A.Space"]
    );
    assert_eq!(
        class
            .methods
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        ["Open", "Close"]
    );
}

#[test]
fn interface_preserves_base_imports_and_method_order_without_variables() {
    let interface = InterfaceDef {
        name: "DerivedInterface".into(),
        base: Some("BaseInterface".into()),
        using: vec!["First".into(), "Second".into()],
        methods: vec![method("Read"), method("Write")],
    }
    .clone();

    assert_eq!(interface.name, "DerivedInterface");
    assert_eq!(interface.base.as_deref(), Some("BaseInterface"));
    assert_eq!(
        interface
            .using
            .iter()
            .map(SmolStr::as_str)
            .collect::<Vec<_>>(),
        ["First", "Second"]
    );
    assert_eq!(
        interface
            .methods
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        ["Read", "Write"]
    );
}
