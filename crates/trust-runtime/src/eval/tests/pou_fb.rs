use super::common;

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_hir::types::TypeRegistry;
use trust_hir::TypeId;
use trust_runtime::eval::{
    call_function_block, expr::Expr, ops::BinaryOp, stmt::Stmt, FunctionBlockDef, VarDef,
};
use trust_runtime::instance::create_fb_instance;
use trust_runtime::memory::VariableStorage;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::Value;

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
