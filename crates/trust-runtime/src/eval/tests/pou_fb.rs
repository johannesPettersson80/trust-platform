use super::common;

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_hir::symbols::ParamDirection;
use trust_hir::types::TypeRegistry;
use trust_hir::TypeId;
use trust_runtime::eval::{
    call_function_block, expr::Expr, ops::BinaryOp, stmt::Stmt, FunctionBlockDef, Param, VarDef,
};
use trust_runtime::instance::create_fb_instance;
use trust_runtime::memory::VariableStorage;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::{ArrayValue, Value};

#[test]
fn fb_stateful() {
    let registry = TypeRegistry::new();
    let mut storage = VariableStorage::new();
    let fb = FunctionBlockDef {
        name: "Counter".into(),
        base: None,
        params: vec![],
        vars: vec![VarDef {
            name: "count".into(),
            type_id: TypeId::INT,
            initializer: None,
            retain: trust_runtime::RetainPolicy::Unspecified,
            external: false,
            static_storage: false,
            constant: false,
            address: None,
        }],
        temps: Vec::new(),
        using: Vec::new(),
        methods: Vec::new(),
        body: vec![Stmt::Assign {
            target: trust_runtime::eval::expr::LValue::Name("count".into()),
            value: Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Name("count".into())),
                right: Box::new(Expr::Literal(Value::Int(1))),
            },
            location: None,
        }],
    };

    let function_blocks: IndexMap<SmolStr, FunctionBlockDef> = IndexMap::new();
    let functions: IndexMap<SmolStr, trust_runtime::eval::FunctionDef> = IndexMap::new();
    let classes: IndexMap<SmolStr, trust_runtime::eval::ClassDef> = IndexMap::new();
    let instance_id = create_fb_instance(
        &mut storage,
        &registry,
        &trust_runtime::value::DateTimeProfile::default(),
        &classes,
        &function_blocks,
        &functions,
        &StandardLibrary::new(),
        &trust_runtime::program_model::InitializerCatalog::default(),
        &fb,
    )
    .unwrap();
    let mut ctx = common::make_context(&mut storage, &registry);

    call_function_block(&mut ctx, &fb, instance_id, &[]).unwrap();
    call_function_block(&mut ctx, &fb, instance_id, &[]).unwrap();

    assert_eq!(
        storage.get_instance_var(instance_id, "count"),
        Some(&Value::Int(2))
    );
}

#[test]
fn fb_omitted_var_input_reuses_stored_value_after_explicit_update() {
    let registry = TypeRegistry::new();
    let mut storage = VariableStorage::new();
    let fb = FunctionBlockDef {
        name: "Adjust".into(),
        base: None,
        params: vec![
            Param {
                name: "base".into(),
                type_id: TypeId::INT,
                direction: ParamDirection::In,
                address: None,
                default: None,
            },
            Param {
                name: "inc".into(),
                type_id: TypeId::INT,
                direction: ParamDirection::In,
                address: None,
                default: Some(Expr::Literal(Value::Int(5))),
            },
        ],
        vars: vec![VarDef {
            name: "result".into(),
            type_id: TypeId::INT,
            initializer: None,
            retain: trust_runtime::RetainPolicy::Unspecified,
            external: false,
            static_storage: false,
            constant: false,
            address: None,
        }],
        temps: Vec::new(),
        using: Vec::new(),
        methods: Vec::new(),
        body: vec![Stmt::Assign {
            target: trust_runtime::eval::expr::LValue::Name("result".into()),
            value: Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Name("base".into())),
                right: Box::new(Expr::Name("inc".into())),
            },
            location: None,
        }],
    };

    let function_blocks: IndexMap<SmolStr, FunctionBlockDef> = IndexMap::new();
    let functions: IndexMap<SmolStr, trust_runtime::eval::FunctionDef> = IndexMap::new();
    let classes: IndexMap<SmolStr, trust_runtime::eval::ClassDef> = IndexMap::new();
    let instance_id = create_fb_instance(
        &mut storage,
        &registry,
        &trust_runtime::value::DateTimeProfile::default(),
        &classes,
        &function_blocks,
        &functions,
        &StandardLibrary::new(),
        &trust_runtime::program_model::InitializerCatalog::default(),
        &fb,
    )
    .unwrap();
    let mut ctx = common::make_context(&mut storage, &registry);

    call_function_block(
        &mut ctx,
        &fb,
        instance_id,
        &[trust_runtime::eval::CallArg {
            name: Some("base".into()),
            value: trust_runtime::eval::ArgValue::Expr(Expr::Literal(Value::Int(3))),
        }],
    )
    .unwrap();
    assert_eq!(
        ctx.storage.get_instance_var(instance_id, "result"),
        Some(&Value::Int(8))
    );

    call_function_block(
        &mut ctx,
        &fb,
        instance_id,
        &[
            trust_runtime::eval::CallArg {
                name: Some("base".into()),
                value: trust_runtime::eval::ArgValue::Expr(Expr::Literal(Value::Int(3))),
            },
            trust_runtime::eval::CallArg {
                name: Some("inc".into()),
                value: trust_runtime::eval::ArgValue::Expr(Expr::Literal(Value::Int(9))),
            },
        ],
    )
    .unwrap();
    assert_eq!(
        ctx.storage.get_instance_var(instance_id, "result"),
        Some(&Value::Int(12))
    );

    call_function_block(
        &mut ctx,
        &fb,
        instance_id,
        &[trust_runtime::eval::CallArg {
            name: Some("base".into()),
            value: trust_runtime::eval::ArgValue::Expr(Expr::Literal(Value::Int(3))),
        }],
    )
    .unwrap();
    assert_eq!(
        ctx.storage.get_instance_var(instance_id, "result"),
        Some(&Value::Int(12))
    );
}

#[test]
fn var_input_pointer_deref_write_mutates_callers_storage() {
    let mut registry = TypeRegistry::new();
    let array_type = registry.register_array(TypeId::INT, vec![(0, 3)]);
    let pointer_type = registry.register_pointer(array_type);

    let mut storage = VariableStorage::new();
    storage.set_global(
        "Local",
        Value::Array(Box::new(
            ArrayValue::from_untyped_parts(
                vec![Value::Int(0), Value::Int(0), Value::Int(0), Value::Int(0)],
                vec![(0, 3)],
            )
            .unwrap(),
        )),
    );

    let fb = FunctionBlockDef {
        name: "WriteThrough".into(),
        base: None,
        params: vec![Param {
            name: "PT".into(),
            type_id: pointer_type,
            direction: ParamDirection::In,
            address: None,
            default: None,
        }],
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        methods: Vec::new(),
        body: vec![Stmt::Assign {
            target: trust_runtime::eval::expr::LValue::Index {
                target: Box::new(trust_runtime::eval::expr::LValue::Deref(Box::new(
                    Expr::Name("PT".into()),
                ))),
                indices: vec![Expr::Literal(Value::Int(1))],
            },
            value: Expr::Literal(Value::Int(123)),
            location: None,
        }],
    };

    let function_blocks: IndexMap<SmolStr, FunctionBlockDef> = IndexMap::new();
    let functions: IndexMap<SmolStr, trust_runtime::eval::FunctionDef> = IndexMap::new();
    let classes: IndexMap<SmolStr, trust_runtime::eval::ClassDef> = IndexMap::new();
    let instance_id = create_fb_instance(
        &mut storage,
        &registry,
        &trust_runtime::value::DateTimeProfile::default(),
        &classes,
        &function_blocks,
        &functions,
        &StandardLibrary::new(),
        &trust_runtime::program_model::InitializerCatalog::default(),
        &fb,
    )
    .unwrap();
    let mut ctx = common::make_context(&mut storage, &registry);

    call_function_block(
        &mut ctx,
        &fb,
        instance_id,
        &[trust_runtime::eval::CallArg {
            name: Some("PT".into()),
            value: trust_runtime::eval::ArgValue::Expr(Expr::Ref(
                trust_runtime::eval::expr::LValue::Name("Local".into()),
            )),
        }],
    )
    .unwrap();

    let Some(Value::Array(local)) = ctx.storage.get_global("Local") else {
        panic!("expected Local array");
    };
    assert_eq!(local.elements()[1], Value::Int(123));
}

#[test]
fn wildcard_array_var_in_out_writes_through_correctly() {
    let mut registry = TypeRegistry::new();
    let wildcard_array = registry.register_array(TypeId::BYTE, vec![(0, i64::MAX)]);

    let mut storage = VariableStorage::new();
    storage.set_global(
        "Small",
        Value::Array(Box::new(
            ArrayValue::from_untyped_parts(
                vec![
                    Value::Byte(0),
                    Value::Byte(0),
                    Value::Byte(0),
                    Value::Byte(0),
                ],
                vec![(0, 3)],
            )
            .unwrap(),
        )),
    );
    storage.set_global(
        "Large",
        Value::Array(Box::new(
            ArrayValue::from_untyped_parts(
                vec![
                    Value::Byte(0),
                    Value::Byte(0),
                    Value::Byte(0),
                    Value::Byte(0),
                    Value::Byte(0),
                    Value::Byte(0),
                    Value::Byte(0),
                    Value::Byte(0),
                ],
                vec![(0, 7)],
            )
            .unwrap(),
        )),
    );

    let fb = FunctionBlockDef {
        name: "WriteWild".into(),
        base: None,
        params: vec![Param {
            name: "arr".into(),
            type_id: wildcard_array,
            direction: ParamDirection::InOut,
            address: None,
            default: None,
        }],
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        methods: Vec::new(),
        body: vec![Stmt::Assign {
            target: trust_runtime::eval::expr::LValue::Index {
                target: Box::new(trust_runtime::eval::expr::LValue::Name("arr".into())),
                indices: vec![Expr::Literal(Value::Int(1))],
            },
            value: Expr::Literal(Value::Byte(9)),
            location: None,
        }],
    };

    let function_blocks: IndexMap<SmolStr, FunctionBlockDef> = IndexMap::new();
    let functions: IndexMap<SmolStr, trust_runtime::eval::FunctionDef> = IndexMap::new();
    let classes: IndexMap<SmolStr, trust_runtime::eval::ClassDef> = IndexMap::new();
    let instance_id = create_fb_instance(
        &mut storage,
        &registry,
        &trust_runtime::value::DateTimeProfile::default(),
        &classes,
        &function_blocks,
        &functions,
        &StandardLibrary::new(),
        &trust_runtime::program_model::InitializerCatalog::default(),
        &fb,
    )
    .unwrap();
    let mut ctx = common::make_context(&mut storage, &registry);

    call_function_block(
        &mut ctx,
        &fb,
        instance_id,
        &[trust_runtime::eval::CallArg {
            name: Some("arr".into()),
            value: trust_runtime::eval::ArgValue::Target(trust_runtime::eval::expr::LValue::Name(
                "Small".into(),
            )),
        }],
    )
    .unwrap();
    call_function_block(
        &mut ctx,
        &fb,
        instance_id,
        &[trust_runtime::eval::CallArg {
            name: Some("arr".into()),
            value: trust_runtime::eval::ArgValue::Target(trust_runtime::eval::expr::LValue::Name(
                "Large".into(),
            )),
        }],
    )
    .unwrap();

    let Some(Value::Array(small)) = ctx.storage.get_global("Small") else {
        panic!("expected Small array");
    };
    let Some(Value::Array(large)) = ctx.storage.get_global("Large") else {
        panic!("expected Large array");
    };
    assert_eq!(small.elements()[1], Value::Byte(9));
    assert_eq!(large.elements()[1], Value::Byte(9));
}

#[test]
fn pointer_to_wildcard_array_writes_through_correctly() {
    let mut registry = TypeRegistry::new();
    let wildcard_array = registry.register_array(TypeId::BYTE, vec![(0, i64::MAX)]);
    let pointer_type = registry.register_pointer(wildcard_array);

    let mut storage = VariableStorage::new();
    storage.set_global(
        "Local",
        Value::Array(Box::new(
            ArrayValue::from_untyped_parts(
                vec![
                    Value::Byte(0),
                    Value::Byte(0),
                    Value::Byte(0),
                    Value::Byte(0),
                ],
                vec![(0, 3)],
            )
            .unwrap(),
        )),
    );

    let fb = FunctionBlockDef {
        name: "WritePointerWild".into(),
        base: None,
        params: vec![Param {
            name: "PT".into(),
            type_id: pointer_type,
            direction: ParamDirection::In,
            address: None,
            default: None,
        }],
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        methods: Vec::new(),
        body: vec![Stmt::Assign {
            target: trust_runtime::eval::expr::LValue::Index {
                target: Box::new(trust_runtime::eval::expr::LValue::Deref(Box::new(
                    Expr::Name("PT".into()),
                ))),
                indices: vec![Expr::Literal(Value::Int(2))],
            },
            value: Expr::Literal(Value::Byte(11)),
            location: None,
        }],
    };

    let function_blocks: IndexMap<SmolStr, FunctionBlockDef> = IndexMap::new();
    let functions: IndexMap<SmolStr, trust_runtime::eval::FunctionDef> = IndexMap::new();
    let classes: IndexMap<SmolStr, trust_runtime::eval::ClassDef> = IndexMap::new();
    let instance_id = create_fb_instance(
        &mut storage,
        &registry,
        &trust_runtime::value::DateTimeProfile::default(),
        &classes,
        &function_blocks,
        &functions,
        &StandardLibrary::new(),
        &trust_runtime::program_model::InitializerCatalog::default(),
        &fb,
    )
    .unwrap();
    let mut ctx = common::make_context(&mut storage, &registry);

    call_function_block(
        &mut ctx,
        &fb,
        instance_id,
        &[trust_runtime::eval::CallArg {
            name: Some("PT".into()),
            value: trust_runtime::eval::ArgValue::Expr(Expr::Ref(
                trust_runtime::eval::expr::LValue::Name("Local".into()),
            )),
        }],
    )
    .unwrap();

    let Some(Value::Array(local)) = ctx.storage.get_global("Local") else {
        panic!("expected Local array");
    };
    assert_eq!(local.elements()[2], Value::Byte(11));
}
