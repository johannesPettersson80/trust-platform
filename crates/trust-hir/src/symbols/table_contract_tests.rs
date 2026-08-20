use super::*;
use crate::semantic::{DeclarationKind, DeclarationReferenceKind, SemanticOutcome, SemanticRole};
use crate::symbols::{SymbolOrigin, VarQualifier};
use crate::types::{InitializerRecord, StructField, UnionVariant};
use text_size::{TextRange, TextSize};

fn range(start: u32, end: u32) -> TextRange {
    TextRange::new(TextSize::from(start), TextSize::from(end))
}

fn symbol(name: &str, kind: SymbolKind, type_id: TypeId, source_range: TextRange) -> Symbol {
    Symbol::new(SymbolId::UNKNOWN, name, kind, type_id, source_range)
}

fn add_namespace(table: &mut SymbolTable, name: &str, source_range: TextRange) -> SymbolId {
    table.add_symbol(symbol(
        name,
        SymbolKind::Namespace,
        TypeId::VOID,
        source_range,
    ))
}

fn add_namespace_member(
    table: &mut SymbolTable,
    namespace: SymbolId,
    name: &str,
    source_range: TextRange,
) -> SymbolId {
    let scope = table.push_scope(ScopeKind::Namespace, Some(namespace));
    let mut member = symbol(
        name,
        SymbolKind::Variable {
            qualifier: VarQualifier::Global,
        },
        TypeId::INT,
        source_range,
    );
    member.parent = Some(namespace);
    let id = table.add_symbol(member);
    assert_eq!(table.current_scope(), scope);
    table.pop_scope();
    id
}

#[test]
fn construction_registers_global_scope_builtin_types_and_function_blocks() {
    let table = SymbolTable::new();

    assert_eq!(table.current_scope(), ScopeId::GLOBAL);
    assert_eq!(table.scope_count(), 1);
    let global = table.get_scope(ScopeId::GLOBAL).expect("global scope");
    assert_eq!(global.kind, ScopeKind::Global);
    assert_eq!(global.parent, None);
    assert_eq!(table.lookup_registered_type_name("iNt"), Some(TypeId::INT));
    assert!(matches!(table.type_by_id(TypeId::BOOL), Some(Type::Bool)));
    let ton = table.lookup("ton").expect("builtin TON symbol");
    assert!(matches!(
        table.get(ton).map(|symbol| &symbol.kind),
        Some(SymbolKind::FunctionBlock)
    ));
    assert!(!table.is_empty());
}

#[test]
fn child_scope_push_pop_and_owner_mapping_are_stable() {
    let mut table = SymbolTable::new();
    let owner = table.add_symbol(symbol(
        "Main",
        SymbolKind::Program,
        TypeId::VOID,
        range(1, 5),
    ));

    let child = table.push_scope(ScopeKind::Program, Some(owner));
    assert_eq!(table.current_scope(), child);
    assert_eq!(table.scope_for_owner(owner), Some(child));
    let scope = table.get_scope(child).expect("child scope");
    assert_eq!(scope.parent, Some(ScopeId::GLOBAL));
    assert_eq!(scope.owner, Some(owner));

    table.pop_scope();
    assert_eq!(table.current_scope(), ScopeId::GLOBAL);
    table.pop_scope();
    assert_eq!(table.current_scope(), ScopeId::GLOBAL);
}

#[test]
fn ensure_scope_for_owner_is_idempotent_and_uses_parent_owner_scope() {
    let mut table = SymbolTable::new();
    let parent = table.add_symbol(symbol(
        "Parent",
        SymbolKind::Class,
        TypeId::VOID,
        range(1, 2),
    ));
    let parent_scope = table
        .ensure_scope_for_owner(parent, ScopeKind::Class)
        .expect("parent scope");

    let mut child_symbol = symbol(
        "Method",
        SymbolKind::Method {
            return_type: None,
            parameters: Vec::new(),
        },
        TypeId::VOID,
        range(3, 4),
    );
    child_symbol.parent = Some(parent);
    let child = table.add_symbol_raw(child_symbol);
    let child_scope = table
        .ensure_scope_for_owner(child, ScopeKind::Method)
        .expect("child scope");

    assert_eq!(
        table.ensure_scope_for_owner(child, ScopeKind::Block),
        Some(child_scope)
    );
    assert_eq!(
        table.get_scope(child_scope).and_then(|scope| scope.parent),
        Some(parent_scope)
    );
}

#[test]
fn ensure_scope_for_missing_owner_fails_without_creating_scope() {
    let mut table = SymbolTable::new();
    let before = table.scope_count();

    assert_eq!(
        table.ensure_scope_for_owner(SymbolId(u32::MAX - 1), ScopeKind::Program),
        None
    );
    assert_eq!(table.scope_count(), before);
}

#[test]
fn lexical_resolution_is_case_insensitive_and_local_shadows_parent() {
    let mut table = SymbolTable::new();
    let global = table.add_symbol(symbol(
        "Value",
        SymbolKind::Variable {
            qualifier: VarQualifier::Global,
        },
        TypeId::INT,
        range(1, 2),
    ));
    let scope = table.push_scope(ScopeKind::Block, None);
    let local = table.add_symbol(symbol(
        "VALUE",
        SymbolKind::Variable {
            qualifier: VarQualifier::Local,
        },
        TypeId::DINT,
        range(3, 4),
    ));

    assert_eq!(table.resolve("value", scope), Some(local));
    assert_eq!(table.resolve_current("VaLuE"), Some(local));
    assert_ne!(global, local);
    table.pop_scope();
    assert_eq!(table.resolve_current("value"), Some(global));
}

#[test]
fn using_resolution_finds_one_target_and_deduplicates_repeated_path() {
    let mut table = SymbolTable::new();
    let namespace = add_namespace(&mut table, "Library", range(1, 2));
    let target = add_namespace_member(&mut table, namespace, "Shared", range(3, 4));
    let scope = table.push_scope(ScopeKind::Program, None);
    table.add_using_directive(vec![SmolStr::new("Library")], range(5, 6));
    table.add_using_directive(vec![SmolStr::new("library")], range(7, 8));

    let current = table.get_scope(scope).expect("program scope");
    assert_eq!(
        table.resolve_using_in_scope(current, "shared"),
        UsingResolution::Single(target)
    );
    assert_eq!(table.resolve("SHARED", scope), Some(target));
}

#[test]
fn using_resolution_reports_ambiguity_and_local_declaration_wins() {
    let mut table = SymbolTable::new();
    let left_ns = add_namespace(&mut table, "Left", range(1, 2));
    let right_ns = add_namespace(&mut table, "Right", range(3, 4));
    let left = add_namespace_member(&mut table, left_ns, "Shared", range(5, 6));
    let right = add_namespace_member(&mut table, right_ns, "Shared", range(7, 8));
    assert_ne!(left, right);

    let scope = table.push_scope(ScopeKind::Program, None);
    table.add_using_directive(vec![SmolStr::new("Left")], range(9, 10));
    table.add_using_directive(vec![SmolStr::new("Right")], range(11, 12));
    let current = table.get_scope(scope).expect("program scope");
    assert_eq!(
        table.resolve_using_in_scope(current, "Shared"),
        UsingResolution::Ambiguous
    );
    assert_eq!(table.resolve("Shared", scope), None);

    let local = table.add_symbol(symbol(
        "Shared",
        SymbolKind::Variable {
            qualifier: VarQualifier::Local,
        },
        TypeId::INT,
        range(13, 14),
    ));
    assert_eq!(table.resolve("Shared", scope), Some(local));
}

#[test]
fn qualified_resolution_traverses_namespaces_only() {
    let mut table = SymbolTable::new();
    let outer = add_namespace(&mut table, "Outer", range(1, 2));
    let outer_scope = table.push_scope(ScopeKind::Namespace, Some(outer));
    let mut inner_symbol = symbol("Inner", SymbolKind::Namespace, TypeId::VOID, range(3, 4));
    inner_symbol.parent = Some(outer);
    let inner = table.add_symbol(inner_symbol);
    let inner_scope = table.push_scope(ScopeKind::Namespace, Some(inner));
    let mut leaf_symbol = symbol(
        "Leaf",
        SymbolKind::Variable {
            qualifier: VarQualifier::Global,
        },
        TypeId::INT,
        range(5, 6),
    );
    leaf_symbol.parent = Some(inner);
    let leaf = table.add_symbol(leaf_symbol);
    table.set_current_scope(outer_scope);
    table.pop_scope();

    assert_eq!(
        table.resolve_global_or_qualified_name("outer.inner.leaf"),
        Some(leaf)
    );
    assert_eq!(
        table.resolve_qualified(&[
            SmolStr::new("OUTER"),
            SmolStr::new("INNER"),
            SmolStr::new("LEAF")
        ]),
        Some(leaf)
    );
    assert_eq!(table.lookup_in_scope(inner_scope, "leaf"), Some(leaf));

    let not_namespace = table.add_symbol(symbol(
        "Plain",
        SymbolKind::Program,
        TypeId::VOID,
        range(7, 8),
    ));
    let mut nested = symbol(
        "Child",
        SymbolKind::Variable {
            qualifier: VarQualifier::Global,
        },
        TypeId::INT,
        range(9, 10),
    );
    nested.parent = Some(not_namespace);
    table.add_symbol_raw(nested);
    assert_eq!(table.resolve_global_or_qualified_name("Plain.Child"), None);
}

#[test]
fn defining_existing_name_reports_previous_symbol_and_replaces_local_lookup() {
    let mut table = SymbolTable::new();
    let scope = table.push_scope(ScopeKind::Block, None);
    let first = table.add_symbol_raw(symbol(
        "First",
        SymbolKind::Constant,
        TypeId::INT,
        range(1, 2),
    ));
    let second = table.add_symbol_raw(symbol(
        "Second",
        SymbolKind::Constant,
        TypeId::INT,
        range(3, 4),
    ));

    assert_eq!(
        table.define_in_scope(scope, SmolStr::new("same"), first),
        None
    );
    assert_eq!(
        table.define_in_scope(scope, SmolStr::new("SAME"), second),
        Some(first)
    );
    assert_eq!(table.lookup_in_scope(scope, "same"), Some(second));
}

#[test]
fn local_name_range_index_is_exact_and_excludes_imported_symbols() {
    let mut table = SymbolTable::new();
    let local_range = range(10, 20);
    let local = table.add_symbol(symbol(
        "Local",
        SymbolKind::Constant,
        TypeId::INT,
        local_range,
    ));
    assert_eq!(
        table.lookup_by_name_range("LOCAL", local_range),
        Some(local)
    );
    assert_eq!(table.lookup_by_name_range("Local", range(10, 19)), None);

    let mut imported_symbol = symbol("Imported", SymbolKind::Constant, TypeId::INT, range(30, 40));
    imported_symbol.origin = Some(SymbolOrigin {
        file_id: FileId(9),
        symbol_id: SymbolId(77),
    });
    table.add_symbol_raw(imported_symbol);
    assert_eq!(table.lookup_by_name_range("Imported", range(30, 40)), None);
}

#[test]
fn import_collisions_retain_order_names_and_ranges() {
    let mut table = SymbolTable::new();
    table.record_import_collision(SmolStr::new("A"), range(1, 2), range(3, 4));
    table.record_import_collision(SmolStr::new("B"), range(5, 6), range(7, 8));

    assert_eq!(table.import_collisions().len(), 2);
    assert_eq!(table.import_collisions()[0].name, "A");
    assert_eq!(table.import_collisions()[0].existing_range, range(1, 2));
    assert_eq!(table.import_collisions()[0].duplicate_range, range(3, 4));
    assert_eq!(table.import_collisions()[1].name, "B");
}

#[test]
fn type_registration_is_case_insensitive_stable_and_case_preserving() {
    let mut table = SymbolTable::new();
    let first = table.register_struct_type("CamelCase", Vec::new());
    let duplicate = table.register_type(
        "camelcase",
        Type::Enum {
            name: SmolStr::new("replacement"),
            base: TypeId::INT,
            values: Vec::new(),
        },
    );

    assert_eq!(duplicate, first);
    assert_eq!(table.lookup_registered_type_name("CAMELCASE"), Some(first));
    assert_eq!(table.type_name(first).as_deref(), Some("CamelCase"));
    assert!(matches!(
        table.type_by_id(first),
        Some(Type::Struct { name, .. }) if name == "CamelCase"
    ));
}

#[test]
fn placeholder_alias_is_completed_in_place_without_changing_identity() {
    let mut table = SymbolTable::new();
    let id = table.register_type(
        "Forward",
        Type::Alias {
            name: SmolStr::new("Forward"),
            target: TypeId::UNKNOWN,
        },
    );
    let completed = table.register_type(
        "FORWARD",
        Type::Alias {
            name: SmolStr::new("Forward"),
            target: TypeId::DINT,
        },
    );

    assert_eq!(completed, id);
    assert!(matches!(
        table.type_by_id(id),
        Some(Type::Alias {
            target: TypeId::DINT,
            ..
        })
    ));
}

#[test]
fn compound_type_registration_retains_structural_payload() {
    let mut table = SymbolTable::new();
    let struct_id = table.register_struct_type(
        "Packet",
        vec![StructField {
            name: SmolStr::new("Count"),
            type_id: TypeId::DINT,
            address: Some(SmolStr::new("%MW0")),
            default_initializer: None,
        }],
    );
    let union_id = table.register_union_type(
        "Choice",
        vec![UnionVariant {
            name: SmolStr::new("WordValue"),
            type_id: TypeId::WORD,
            address: None,
            default_initializer: None,
        }],
    );
    let array_id = table.register_array_type(TypeId::INT, vec![(1, 3), (-1, 1)]);
    let pointer_id = table.register_pointer_type(struct_id);
    let reference_id = table.register_reference_type(union_id);
    let subrange_id = table.register_subrange_type(TypeId::INT, -2, 2);

    assert!(matches!(
        table.type_by_id(struct_id),
        Some(Type::Struct { fields, .. })
            if fields.len() == 1
                && fields[0].name == "Count"
                && fields[0].address.as_deref() == Some("%MW0")
    ));
    assert!(matches!(
        table.type_by_id(union_id),
        Some(Type::Union { variants, .. })
            if variants.len() == 1 && variants[0].type_id == TypeId::WORD
    ));
    assert!(matches!(
        table.type_by_id(array_id),
        Some(Type::Array { element: TypeId::INT, dimensions })
            if dimensions == &vec![(1, 3), (-1, 1)]
    ));
    assert!(matches!(
        table.type_by_id(pointer_id),
        Some(Type::Pointer { target }) if *target == struct_id
    ));
    assert!(matches!(
        table.type_by_id(reference_id),
        Some(Type::Reference { target }) if *target == union_id
    ));
    assert!(matches!(
        table.type_by_id(subrange_id),
        Some(Type::Subrange {
            base: TypeId::INT,
            lower: -2,
            upper: 2
        })
    ));
}

#[test]
fn initializer_import_uses_fresh_local_identity_and_type_default_binding() {
    let mut source = SymbolTable::new();
    let source_id = source.register_initializer(InitializerRecord {
        range: range(10, 20),
    });
    let mut target = SymbolTable::new();
    let imported = target
        .import_initializer_from(&source, source_id)
        .expect("initializer import");
    let next = target.register_initializer(InitializerRecord {
        range: range(30, 40),
    });
    let type_id = target.register_struct_type("Configured", Vec::new());
    target.set_type_default_initializer(type_id, imported);

    assert_eq!(imported, InitializerId(0));
    assert_eq!(next, InitializerId(1));
    assert_eq!(
        target.initializer(imported).map(|record| record.range),
        Some(range(10, 20))
    );
    assert_eq!(target.type_default_initializer(type_id), Some(imported));
    assert_eq!(target.initializer_catalog().records().len(), 2);
    assert_eq!(
        target.import_initializer_from(&source, InitializerId(99)),
        None
    );
}

#[test]
fn enum_value_resolution_distinguishes_missing_single_and_ambiguous() {
    let mut table = SymbolTable::new();
    table.register_enum_type(
        "Color",
        TypeId::INT,
        vec![(SmolStr::new("RED"), 1), (SmolStr::new("GREEN"), 2)],
    );

    assert_eq!(
        table.resolve_enum_value_by_name("red"),
        EnumValueResolution::Resolved(1)
    );
    assert_eq!(table.enum_value_by_name("GREEN"), Some(2));
    assert_eq!(
        table.resolve_enum_value_by_name("missing"),
        EnumValueResolution::NotFound
    );

    table.register_enum_type("Status", TypeId::INT, vec![(SmolStr::new("RED"), 9)]);
    assert_eq!(
        table.resolve_enum_value_by_name("RED"),
        EnumValueResolution::Ambiguous
    );
    assert_eq!(table.enum_value_by_name("RED"), None);
}

#[test]
fn constant_lookup_normalizes_scope_and_name() {
    let mut table = SymbolTable::new();
    let mut values = FxHashMap::default();
    values.insert((Some(SmolStr::new("Motor")), SmolStr::new("Limit")), 42);
    values.insert((None, SmolStr::new("Global")), 7);
    table.set_const_values(values);

    assert_eq!(
        table.const_value(&Some(SmolStr::new("MOTOR")), "limit"),
        Some(42)
    );
    assert_eq!(table.const_value(&None, "GLOBAL"), Some(7));
    assert_eq!(table.const_value(&None, "missing"), None);
}

#[test]
fn extends_and_implements_references_resolve_and_reject_wrong_kind() {
    let mut table = SymbolTable::new();
    let base = table.add_symbol(symbol(
        "Base",
        SymbolKind::FunctionBlock,
        TypeId::VOID,
        range(1, 2),
    ));
    table.ensure_scope_for_owner(base, ScopeKind::FunctionBlock);
    let interface = table.add_symbol(symbol(
        "Runnable",
        SymbolKind::Interface,
        TypeId::VOID,
        range(3, 4),
    ));
    table.ensure_scope_for_owner(interface, ScopeKind::Class);
    let value = table.add_symbol(symbol(
        "NotAType",
        SymbolKind::Constant,
        TypeId::INT,
        range(5, 6),
    ));
    let derived = table.add_symbol(symbol(
        "Derived",
        SymbolKind::Class,
        TypeId::VOID,
        range(7, 8),
    ));
    table.ensure_scope_for_owner(derived, ScopeKind::Class);
    table.set_extends(derived, SmolStr::new("Base"));
    table.set_implements(
        derived,
        vec![SmolStr::new("Runnable"), SmolStr::new("NotAType")],
    );

    assert_eq!(
        table.extends_name(derived).map(SmolStr::as_str),
        Some("Base")
    );
    assert_eq!(
        table.implements_names(derived).map(|names| names.len()),
        Some(2)
    );
    assert_eq!(
        table.resolve_oop_reference_for_owner(derived, "base"),
        Some(base)
    );
    assert_eq!(
        table.resolve_oop_reference_for_owner(derived, "NotAType"),
        None
    );
    assert_eq!(
        table.extends_reference(derived).map(|record| record.kind()),
        Some(DeclarationReferenceKind::Extends)
    );
    let references = table.implements_references(derived);
    assert!(matches!(
        references[0].outcome(),
        SemanticOutcome::Resolved(id) if *id == interface
    ));
    assert!(matches!(
        references[1].outcome(),
        SemanticOutcome::WrongKind { symbol_id, .. } if *symbol_id == value
    ));
}

#[test]
fn oop_reference_without_owner_scope_is_classified_not_global_fallback() {
    let mut table = SymbolTable::new();
    table.add_symbol(symbol("Base", SymbolKind::Class, TypeId::VOID, range(1, 2)));
    let owner = table.add_symbol(symbol(
        "Owner",
        SymbolKind::Class,
        TypeId::VOID,
        range(3, 4),
    ));
    table.set_extends(owner, SmolStr::new("Base"));

    assert_eq!(table.resolve_oop_reference_for_owner(owner, "Base"), None);
    assert!(matches!(
        table
            .extends_reference(owner)
            .map(|record| record.outcome().clone()),
        Some(SemanticOutcome::InvariantViolation { .. })
    ));
}

#[test]
fn alias_resolution_follows_chain_and_reports_self_cycle() {
    let mut table = SymbolTable::new();
    let final_id = table.register_struct_type("Final", Vec::new());
    let middle = table.register_type(
        "Middle",
        Type::Alias {
            name: SmolStr::new("Middle"),
            target: final_id,
        },
    );
    let first = table.register_type(
        "First",
        Type::Alias {
            name: SmolStr::new("First"),
            target: middle,
        },
    );
    let cycle = table.register_type(
        "Cycle",
        Type::Alias {
            name: SmolStr::new("Cycle"),
            target: TypeId::UNKNOWN,
        },
    );
    table.types.insert(
        cycle,
        Type::Alias {
            name: SmolStr::new("Cycle"),
            target: cycle,
        },
    );

    assert_eq!(table.resolve_alias_type(first), final_id);
    assert_eq!(
        table.resolve_alias_type_outcome(first),
        SemanticOutcome::Resolved(final_id)
    );
    assert!(matches!(
        table.resolve_alias_type_outcome(cycle),
        SemanticOutcome::InvariantViolation { .. }
    ));
    assert_eq!(table.resolve_alias_type(cycle), cycle);
}

#[test]
fn hierarchy_member_resolution_prefers_derived_and_stops_on_cycle() {
    let mut table = SymbolTable::new();
    let base = table.add_symbol(symbol("Base", SymbolKind::Class, TypeId::VOID, range(1, 2)));
    table.ensure_scope_for_owner(base, ScopeKind::Class);
    let mut base_member = symbol(
        "Run",
        SymbolKind::Method {
            return_type: None,
            parameters: Vec::new(),
        },
        TypeId::VOID,
        range(3, 4),
    );
    base_member.parent = Some(base);
    let base_member = table.add_symbol_raw(base_member);

    let derived = table.add_symbol(symbol(
        "Derived",
        SymbolKind::Class,
        TypeId::VOID,
        range(5, 6),
    ));
    table.ensure_scope_for_owner(derived, ScopeKind::Class);
    table.set_extends(derived, SmolStr::new("Base"));
    assert_eq!(
        table.resolve_member_symbol_in_hierarchy(derived, "run"),
        Some(base_member)
    );

    let mut derived_member = symbol(
        "RUN",
        SymbolKind::Method {
            return_type: None,
            parameters: Vec::new(),
        },
        TypeId::VOID,
        range(7, 8),
    );
    derived_member.parent = Some(derived);
    let derived_member = table.add_symbol_raw(derived_member);
    assert_eq!(
        table.resolve_member_symbol_in_hierarchy(derived, "run"),
        Some(derived_member)
    );

    table.set_extends(base, SmolStr::new("Derived"));
    assert_eq!(
        table.resolve_member_symbol_in_hierarchy(derived, "missing"),
        None
    );
}

#[test]
fn member_resolution_by_alias_type_reaches_function_block_owner() {
    let mut table = SymbolTable::new();
    let owner = table.add_symbol(symbol(
        "Controller",
        SymbolKind::FunctionBlock,
        TypeId::VOID,
        range(1, 2),
    ));
    table.ensure_scope_for_owner(owner, ScopeKind::FunctionBlock);
    let mut member = symbol(
        "Start",
        SymbolKind::Method {
            return_type: None,
            parameters: Vec::new(),
        },
        TypeId::VOID,
        range(3, 4),
    );
    member.parent = Some(owner);
    let member = table.add_symbol_raw(member);
    let owner_type = table.register_type(
        "ControllerType",
        Type::FunctionBlock {
            name: SmolStr::new("Controller"),
        },
    );
    let alias = table.register_type(
        "ControllerAlias",
        Type::Alias {
            name: SmolStr::new("ControllerAlias"),
            target: owner_type,
        },
    );

    assert_eq!(
        table.resolve_member_symbol_in_type(alias, "START"),
        Some(member)
    );
    assert_eq!(
        table.resolve_member_symbol_in_type(TypeId::INT, "START"),
        None
    );
}

#[test]
fn declaration_catalog_preserves_qualified_scope_and_import_source_identity() {
    let mut table = SymbolTable::new();
    let namespace = add_namespace(&mut table, "Plant", range(1, 5));
    let namespace_scope = table.push_scope(ScopeKind::Namespace, Some(namespace));
    let mut local_symbol = symbol(
        "Speed",
        SymbolKind::Variable {
            qualifier: VarQualifier::Global,
        },
        TypeId::REAL,
        range(6, 11),
    );
    local_symbol.parent = Some(namespace);
    let local = table.add_symbol(local_symbol);
    table.pop_scope();

    let mut imported_symbol = symbol(
        "Remote",
        SymbolKind::Variable {
            qualifier: VarQualifier::Global,
        },
        TypeId::DINT,
        range(20, 26),
    );
    imported_symbol.origin = Some(SymbolOrigin {
        file_id: FileId(9),
        symbol_id: SymbolId(77),
    });
    let imported = table.add_symbol_raw(imported_symbol);

    let catalog = table.declaration_catalog(FileId(3));
    let local_record = catalog.find_qualified("plant.speed").expect("local record");
    assert_eq!(local_record.symbol_id(), local);
    assert_eq!(local_record.kind(), DeclarationKind::Variable);
    assert_eq!(local_record.role(), SemanticRole::Value);
    assert_eq!(local_record.owner_symbol_id(), Some(namespace));
    assert_eq!(local_record.owner_scope_id(), Some(namespace_scope));
    assert_eq!(local_record.source().file_id(), FileId(3));
    assert!(!local_record.is_imported());

    let imported_record = catalog.find_qualified("remote").expect("imported record");
    assert_eq!(imported_record.symbol_id(), imported);
    assert_eq!(imported_record.source().file_id(), FileId(9));
    assert!(imported_record.is_imported());
    assert!(catalog.find_qualified("TON").is_none());
}

#[test]
fn declaration_catalog_retains_classified_inheritance_references() {
    let mut table = SymbolTable::new();
    let base = table.add_symbol(symbol("Base", SymbolKind::Class, TypeId::VOID, range(1, 2)));
    table.ensure_scope_for_owner(base, ScopeKind::Class);
    let derived = table.add_symbol(symbol(
        "Derived",
        SymbolKind::Class,
        TypeId::VOID,
        range(3, 4),
    ));
    table.ensure_scope_for_owner(derived, ScopeKind::Class);
    table.set_extends(derived, SmolStr::new("Base"));
    table.set_implements(derived, vec![SmolStr::new("Missing")]);

    let catalog = table.declaration_catalog(FileId(1));
    assert_eq!(catalog.references().len(), 2);
    let extends = catalog
        .references()
        .iter()
        .find(|reference| reference.kind() == DeclarationReferenceKind::Extends)
        .expect("extends reference");
    assert!(matches!(
        extends.outcome(),
        SemanticOutcome::Resolved(id) if *id == base
    ));
    let implements = catalog
        .references()
        .iter()
        .find(|reference| reference.kind() == DeclarationReferenceKind::Implements)
        .expect("implements reference");
    assert!(matches!(
        implements.outcome(),
        SemanticOutcome::Unknown { .. }
    ));
}

#[test]
fn cyclic_parent_chain_is_omitted_from_declaration_catalog() {
    let mut table = SymbolTable::new();
    let first = table.add_symbol_raw(symbol(
        "First",
        SymbolKind::Class,
        TypeId::VOID,
        range(1, 2),
    ));
    let mut second_symbol = symbol("Second", SymbolKind::Class, TypeId::VOID, range(3, 4));
    second_symbol.parent = Some(first);
    let second = table.add_symbol_raw(second_symbol);
    table.get_mut(first).expect("first symbol").parent = Some(second);

    let catalog = table.declaration_catalog(FileId(1));
    assert!(catalog.find_qualified("First").is_none());
    assert!(catalog.find_qualified("Second").is_none());
}

#[test]
fn iteration_and_length_include_every_retained_symbol_identity() {
    let mut table = SymbolTable::new();
    let before = table.len();
    let first = table.add_symbol(symbol(
        "Duplicate",
        SymbolKind::Program,
        TypeId::VOID,
        range(1, 2),
    ));
    let second = table.add_symbol(symbol(
        "duplicate",
        SymbolKind::Program,
        TypeId::VOID,
        range(3, 4),
    ));

    assert_eq!(table.len(), before + 2);
    assert_ne!(first, second);
    let ids = table
        .iter()
        .map(|symbol| symbol.id)
        .collect::<FxHashSet<_>>();
    assert!(ids.contains(&first));
    assert!(ids.contains(&second));
    assert_eq!(table.lookup("DUPLICATE"), Some(first));
}
