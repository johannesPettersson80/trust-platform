use super::*;

fn variable(name: &str, ty: TypeId) -> VarDef {
    VarDef {
        name: name.into(),
        type_id: ty,
        initializer: None,
        retain: crate::RetainPolicy::Unspecified,
        static_storage: false,
        external: false,
        in_out: false,
        constant: false,
        address: None,
    }
}
fn builder() -> RuntimeBuilder {
    let mut builder = RuntimeBuilder::new();
    builder.register_program(ProgramDef {
        name: "Main".into(),
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        body: Vec::new(),
        methods: Vec::new(),
    });
    builder
}

#[test]
fn declared_global_seed_is_installed_and_unknown_seed_is_rejected() {
    let mut declared = builder();
    declared
        .register_global(variable("count", TypeId::INT))
        .unwrap();
    declared.storage_mut().set_global("count", Value::Int(37));
    let runtime = declared.build().unwrap();
    assert_eq!(runtime.storage().get_global("count"), Some(&Value::Int(37)));
    let mut unknown = builder();
    unknown.storage_mut().set_global("count", Value::Int(37));
    assert!(unknown.build().is_err());
}

#[test]
fn wrong_type_seed_rejects_candidate_without_mutating_existing_installation() {
    let mut good = builder();
    good.register_global(variable("count", TypeId::INT))
        .unwrap();
    good.storage_mut().set_global("count", Value::Int(37));
    let installed = good.build().unwrap();
    let mut candidate = builder();
    candidate
        .register_global(variable("count", TypeId::INT))
        .unwrap();
    candidate
        .storage_mut()
        .set_global("count", Value::String("invalid".into()));
    assert!(candidate.build().is_err());
    assert_eq!(
        installed.storage().get_global("count"),
        Some(&Value::Int(37))
    );
}

#[test]
fn class_array_seed_preserves_owned_identity_alias_and_inherited_constant() {
    let mut builder = builder();
    let mut base_var = variable("fixed", TypeId::INT);
    base_var.constant = true;
    base_var.initializer = Some(Expr::Literal(Value::Int(3)));
    builder.register_class(crate::program_model::ClassDef {
        name: "Base".into(),
        base: None,
        interfaces: Vec::new(),
        vars: vec![base_var],
        using: Vec::new(),
        methods: Vec::new(),
    });
    builder.registry_mut().register(
        "Base",
        Type::Class {
            name: "Base".into(),
        },
    );
    builder.register_function_block(crate::program_model::FunctionBlockDef {
        name: "Nested".into(),
        base: None,
        interfaces: Vec::new(),
        params: Vec::new(),
        vars: vec![variable("stored", TypeId::INT)],
        temps: Vec::new(),
        using: Vec::new(),
        methods: Vec::new(),
        body: Vec::new(),
    });
    let nested_ty = builder.registry_mut().register(
        "Nested",
        Type::FunctionBlock {
            name: "Nested".into(),
        },
    );
    builder.register_class(crate::program_model::ClassDef {
        name: "Child".into(),
        base: Some("Base".into()),
        interfaces: Vec::new(),
        vars: vec![
            variable("value", TypeId::INT),
            variable("nested", nested_ty),
        ],
        using: Vec::new(),
        methods: vec![crate::program_model::MethodDef {
            name: "Read".into(),
            return_type: None,
            params: Vec::new(),
            locals: Vec::new(),
            static_locals: vec![variable("calls", TypeId::INT)],
            using: Vec::new(),
            body: Vec::new(),
        }],
    });
    let child_ty = builder.registry_mut().register(
        "Child",
        Type::Class {
            name: "Child".into(),
        },
    );
    let array_ty = builder
        .registry_mut()
        .register_array(child_ty, vec![(1, 2)]);
    builder
        .register_global(variable("items", array_ty))
        .unwrap();
    let mut ids = Vec::new();
    for number in [11, 22] {
        let parent = builder.storage_mut().create_instance("Base");
        builder
            .storage_mut()
            .set_instance_var(parent, "fixed", Value::Int(number + 1));
        let child = builder.storage_mut().create_instance("Child");
        builder
            .storage_mut()
            .get_instance_mut(child)
            .unwrap()
            .parent = Some(parent);
        builder
            .storage_mut()
            .set_instance_var(child, "value", Value::Int(number));
        let nested = builder.storage_mut().create_instance("Nested");
        builder
            .storage_mut()
            .set_instance_var(nested, "stored", Value::Int(number + 2));
        builder
            .storage_mut()
            .set_instance_var(child, "nested", Value::Instance(nested));
        let owner =
            crate::program_model::method_static_storage_owner(&"Child".into(), &"Read".into());
        builder.storage_mut().set_instance_var(
            child,
            crate::program_model::static_storage_name(&owner, &"calls".into()),
            Value::Int(number + 3),
        );
        ids.push(child);
    }
    builder.storage_mut().set_global(
        "items",
        Value::Array(Box::new(crate::value::ArrayValue::from_canonical_parts(
            ids.iter().copied().map(Value::Instance).collect(),
            vec![(1, 2)],
        ))),
    );
    let reference = ValueRef {
        location: MemoryLocation::Instance(ids[1]),
        offset: 0,
        path: Default::default(),
    };
    builder
        .access_map_mut()
        .bind("selected".into(), reference, None);
    let mut runtime = builder.build().unwrap();
    assert_eq!(runtime.read_access("selected"), Some(Value::Int(22)));
    runtime.write_access("selected", Value::Int(41)).unwrap();
    let Value::Array(array) = runtime.storage().get_global("items").unwrap() else {
        panic!("array")
    };
    let Value::Instance(first) = array.elements()[0] else {
        panic!("first")
    };
    let Value::Instance(second) = array.elements()[1] else {
        panic!("second")
    };
    assert_ne!(first, second);
    assert_eq!(
        runtime.storage().get_instance_var(first, "value"),
        Some(&Value::Int(11))
    );
    assert_eq!(
        runtime.storage().get_instance_var(second, "value"),
        Some(&Value::Int(41))
    );
    let parent = runtime
        .storage()
        .get_instance(first)
        .unwrap()
        .parent
        .unwrap();
    assert_eq!(
        runtime.storage().get_instance_var(parent, "fixed"),
        Some(&Value::Int(12))
    );
    let Some(Value::Instance(nested)) = runtime.storage().get_instance_var(first, "nested") else {
        panic!("nested FB")
    };
    assert_eq!(
        runtime.storage().get_instance_var(*nested, "stored"),
        Some(&Value::Int(13))
    );
    let owner = crate::program_model::method_static_storage_owner(&"Child".into(), &"Read".into());
    assert_eq!(
        runtime.storage().get_instance_var(
            first,
            &crate::program_model::static_storage_name(&owner, &"calls".into())
        ),
        Some(&Value::Int(14))
    );
}

#[test]
fn cyclic_or_orphan_owned_seed_is_rejected() {
    let mut orphan = builder();
    orphan.storage_mut().create_instance("undeclared");
    assert!(orphan.build().is_err());
    let mut stale = builder();
    stale.storage_mut().push_frame("old_call");
    assert!(stale.build().is_err());
}
#[test]
fn function_static_seed_is_serialized_under_its_declared_owner() {
    let mut builder = builder();
    let mut local = variable("calls", TypeId::INT);
    local.static_storage = true;
    builder.register_function(crate::program_model::FunctionDef {
        name: "Count".into(),
        return_type: TypeId::INT,
        params: Vec::new(),
        locals: Vec::new(),
        static_locals: vec![local],
        using: Vec::new(),
        body: Vec::new(),
    });
    let name = crate::program_model::static_storage_name(&"Count".into(), &"calls".into());
    builder
        .storage_mut()
        .set_global(name.clone(), Value::Int(19));
    let runtime = builder.build().unwrap();
    assert_eq!(runtime.storage().get_global(&name), Some(&Value::Int(19)));
}

#[test]
fn image_seed_preserves_flat_extent_and_bound_hierarchical_value() {
    let mut builder = builder();
    builder
        .register_global(variable("remote", TypeId::BOOL))
        .unwrap();
    let hierarchy = crate::io::IoAddress::parse("%QX3.4.0").unwrap();
    builder
        .io_mut()
        .bind_typed("remote", hierarchy.clone(), TypeId::BOOL);
    builder.io_mut().try_resize(4, 8, 3).unwrap();
    builder
        .io_mut()
        .write(
            &crate::io::IoAddress::parse("%MB0").unwrap(),
            Value::Byte(17),
        )
        .unwrap();
    builder
        .io_mut()
        .write(&hierarchy, Value::Bool(true))
        .unwrap();
    let runtime = builder.build().unwrap();
    assert_eq!(runtime.io().inputs().len(), 4);
    assert_eq!(runtime.io().outputs().len(), 8);
    assert_eq!(runtime.io().memory(), &[17, 0, 0]);
    assert_eq!(runtime.io().read(&hierarchy).unwrap(), Value::Bool(true));
}
