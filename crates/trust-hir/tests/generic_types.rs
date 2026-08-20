use smol_str::SmolStr;
use trust_hir::types::TypeRegistry;
use trust_hir::{Type, TypeId};

fn assert_members(registry: &TypeRegistry, generic: TypeId, members: &[TypeId]) {
    for member in members {
        assert!(
            registry.is_assignable(generic, *member),
            "{} must contain {}",
            registry.type_name(generic).unwrap_or_default(),
            registry.type_name(*member).unwrap_or_default()
        );
    }
}

fn assert_excludes(registry: &TypeRegistry, generic: TypeId, members: &[TypeId]) {
    for member in members {
        assert!(
            !registry.is_assignable(generic, *member),
            "{} must exclude {}",
            registry.type_name(generic).unwrap_or_default(),
            registry.type_name(*member).unwrap_or_default()
        );
    }
}

fn register_subrange(registry: &mut TypeRegistry, name: &str, base: TypeId) -> TypeId {
    registry.register(
        name,
        Type::Subrange {
            base,
            lower: 1,
            upper: 9,
        },
    )
}

fn register_alias(registry: &mut TypeRegistry, name: &str, target: TypeId) -> TypeId {
    registry.register(
        name,
        Type::Alias {
            name: SmolStr::new(name),
            target,
        },
    )
}

#[test]
fn generic_type_names_are_complete_canonical_and_case_insensitive() {
    let registry = TypeRegistry::new();
    for (name, expected) in [
        ("ANY", TypeId::ANY),
        ("ANY_DERIVED", TypeId::ANY_DERIVED),
        ("ANY_ELEMENTARY", TypeId::ANY_ELEMENTARY),
        ("ANY_MAGNITUDE", TypeId::ANY_MAGNITUDE),
        ("ANY_NUM", TypeId::ANY_NUM),
        ("ANY_INT", TypeId::ANY_INT),
        ("ANY_SIGNED", TypeId::ANY_SIGNED),
        ("ANY_UNSIGNED", TypeId::ANY_UNSIGNED),
        ("ANY_REAL", TypeId::ANY_REAL),
        ("ANY_DURATION", TypeId::ANY_DURATION),
        ("ANY_BIT", TypeId::ANY_BIT),
        ("ANY_CHARS", TypeId::ANY_CHARS),
        ("ANY_STRING", TypeId::ANY_STRING),
        ("ANY_CHAR", TypeId::ANY_CHAR),
        ("ANY_DATE", TypeId::ANY_DATE),
    ] {
        assert_eq!(registry.lookup(name), Some(expected));
        assert_eq!(registry.lookup(&name.to_ascii_lowercase()), Some(expected));
        assert_eq!(registry.type_name(expected).as_deref(), Some(name));
    }
}

#[test]
fn any_contains_every_concrete_elementary_leaf() {
    let registry = TypeRegistry::new();
    assert_members(
        &registry,
        TypeId::ANY,
        &[
            TypeId::BOOL,
            TypeId::SINT,
            TypeId::INT,
            TypeId::DINT,
            TypeId::LINT,
            TypeId::USINT,
            TypeId::UINT,
            TypeId::UDINT,
            TypeId::ULINT,
            TypeId::REAL,
            TypeId::LREAL,
            TypeId::BYTE,
            TypeId::WORD,
            TypeId::DWORD,
            TypeId::LWORD,
            TypeId::TIME,
            TypeId::LTIME,
            TypeId::DATE,
            TypeId::LDATE,
            TypeId::TOD,
            TypeId::LTOD,
            TypeId::DT,
            TypeId::LDT,
            TypeId::STRING,
            TypeId::WSTRING,
            TypeId::CHAR,
            TypeId::WCHAR,
        ],
    );
}

#[test]
fn any_contains_every_concrete_derived_shape() {
    let mut registry = TypeRegistry::new();
    let array = registry.register_array(TypeId::INT, vec![(0, 2)]);
    let structure = registry.register_struct("Packet", Vec::new());
    let union = registry.register_union("Choice", Vec::new());
    let enumeration = registry.register_enum("State", TypeId::INT, vec![(SmolStr::new("Idle"), 0)]);
    let pointer = registry.register_pointer(TypeId::INT);
    let reference = registry.register_reference(TypeId::INT);
    let function_block = registry.register(
        "Controller",
        Type::FunctionBlock {
            name: SmolStr::new("Controller"),
        },
    );
    let class = registry.register(
        "Device",
        Type::Class {
            name: SmolStr::new("Device"),
        },
    );
    let interface = registry.register(
        "IDevice",
        Type::Interface {
            name: SmolStr::new("IDevice"),
        },
    );
    let subrange = register_subrange(&mut registry, "Count", TypeId::UINT);
    let alias = register_alias(&mut registry, "CountAlias", subrange);

    assert_members(
        &registry,
        TypeId::ANY,
        &[
            array,
            structure,
            union,
            enumeration,
            pointer,
            reference,
            function_block,
            class,
            interface,
            subrange,
            alias,
        ],
    );
}

#[test]
fn any_excludes_non_value_and_generic_pseudo_types() {
    let registry = TypeRegistry::new();
    assert_excludes(
        &registry,
        TypeId::ANY,
        &[
            TypeId::UNKNOWN,
            TypeId::VOID,
            TypeId::NULL,
            TypeId::ANY_DERIVED,
            TypeId::ANY_ELEMENTARY,
            TypeId::ANY_MAGNITUDE,
            TypeId::ANY_NUM,
            TypeId::ANY_INT,
            TypeId::ANY_SIGNED,
            TypeId::ANY_UNSIGNED,
            TypeId::ANY_REAL,
            TypeId::ANY_DURATION,
            TypeId::ANY_BIT,
            TypeId::ANY_CHARS,
            TypeId::ANY_STRING,
            TypeId::ANY_CHAR,
            TypeId::ANY_DATE,
        ],
    );
}

#[test]
fn any_elementary_contains_all_and_only_elementary_leaves() {
    let mut registry = TypeRegistry::new();
    assert_members(
        &registry,
        TypeId::ANY_ELEMENTARY,
        &[
            TypeId::BOOL,
            TypeId::INT,
            TypeId::ULINT,
            TypeId::LREAL,
            TypeId::LWORD,
            TypeId::LTIME,
            TypeId::LDATE,
            TypeId::LTOD,
            TypeId::LDT,
            TypeId::STRING,
            TypeId::WSTRING,
            TypeId::CHAR,
            TypeId::WCHAR,
        ],
    );
    let structure = registry.register_struct("Packet", Vec::new());
    let enumeration = registry.register_enum("State", TypeId::INT, vec![(SmolStr::new("Idle"), 0)]);
    let subrange = register_subrange(&mut registry, "Count", TypeId::INT);
    assert_excludes(
        &registry,
        TypeId::ANY_ELEMENTARY,
        &[structure, enumeration, subrange, TypeId::NULL],
    );
}

#[test]
fn any_derived_contains_non_alias_derived_shapes_but_not_subranges() {
    let mut registry = TypeRegistry::new();
    let array = registry.register_array(TypeId::INT, vec![(0, 1)]);
    let structure = registry.register_struct("Packet", Vec::new());
    let union = registry.register_union("Choice", Vec::new());
    let enumeration = registry.register_enum("State", TypeId::INT, vec![(SmolStr::new("Idle"), 0)]);
    let pointer = registry.register_pointer(TypeId::INT);
    let reference = registry.register_reference(TypeId::INT);
    let subrange = register_subrange(&mut registry, "Count", TypeId::INT);
    assert_members(
        &registry,
        TypeId::ANY_DERIVED,
        &[array, structure, union, enumeration, pointer, reference],
    );
    assert_excludes(
        &registry,
        TypeId::ANY_DERIVED,
        &[TypeId::INT, TypeId::STRING, subrange],
    );
}

#[test]
fn any_magnitude_contains_numeric_and_duration_families_only() {
    let registry = TypeRegistry::new();
    assert_members(
        &registry,
        TypeId::ANY_MAGNITUDE,
        &[
            TypeId::SINT,
            TypeId::ULINT,
            TypeId::REAL,
            TypeId::LREAL,
            TypeId::TIME,
            TypeId::LTIME,
        ],
    );
    assert_excludes(
        &registry,
        TypeId::ANY_MAGNITUDE,
        &[TypeId::BOOL, TypeId::LWORD, TypeId::DATE, TypeId::STRING],
    );
}

#[test]
fn any_num_contains_integer_real_and_integer_subrange_only() {
    let mut registry = TypeRegistry::new();
    let signed_range = register_subrange(&mut registry, "SignedRange", TypeId::INT);
    let unsigned_range = register_subrange(&mut registry, "UnsignedRange", TypeId::UINT);
    assert_members(
        &registry,
        TypeId::ANY_NUM,
        &[
            TypeId::SINT,
            TypeId::LINT,
            TypeId::USINT,
            TypeId::ULINT,
            TypeId::REAL,
            TypeId::LREAL,
            signed_range,
            unsigned_range,
        ],
    );
    assert_excludes(
        &registry,
        TypeId::ANY_NUM,
        &[TypeId::BOOL, TypeId::TIME, TypeId::WORD, TypeId::DATE],
    );
}

#[test]
fn any_int_contains_signed_unsigned_and_subranges_but_not_enum() {
    let mut registry = TypeRegistry::new();
    let signed_range = register_subrange(&mut registry, "SignedRange", TypeId::DINT);
    let unsigned_range = register_subrange(&mut registry, "UnsignedRange", TypeId::UDINT);
    let enumeration = registry.register_enum("State", TypeId::INT, vec![(SmolStr::new("Idle"), 0)]);
    assert_members(
        &registry,
        TypeId::ANY_INT,
        &[
            TypeId::SINT,
            TypeId::INT,
            TypeId::DINT,
            TypeId::LINT,
            TypeId::USINT,
            TypeId::UINT,
            TypeId::UDINT,
            TypeId::ULINT,
            signed_range,
            unsigned_range,
        ],
    );
    assert_excludes(
        &registry,
        TypeId::ANY_INT,
        &[TypeId::REAL, TypeId::BOOL, TypeId::WORD, enumeration],
    );
}

#[test]
fn signed_and_unsigned_generic_families_preserve_subrange_base() {
    let mut registry = TypeRegistry::new();
    let signed_range = register_subrange(&mut registry, "SignedRange", TypeId::DINT);
    let unsigned_range = register_subrange(&mut registry, "UnsignedRange", TypeId::UDINT);
    assert_members(
        &registry,
        TypeId::ANY_SIGNED,
        &[
            TypeId::SINT,
            TypeId::INT,
            TypeId::DINT,
            TypeId::LINT,
            signed_range,
        ],
    );
    assert_excludes(
        &registry,
        TypeId::ANY_SIGNED,
        &[TypeId::USINT, TypeId::ULINT, unsigned_range],
    );
    assert_members(
        &registry,
        TypeId::ANY_UNSIGNED,
        &[
            TypeId::USINT,
            TypeId::UINT,
            TypeId::UDINT,
            TypeId::ULINT,
            unsigned_range,
        ],
    );
    assert_excludes(
        &registry,
        TypeId::ANY_UNSIGNED,
        &[TypeId::SINT, TypeId::LINT, signed_range],
    );
}

#[test]
fn any_real_contains_real_widths_only() {
    let registry = TypeRegistry::new();
    assert_members(&registry, TypeId::ANY_REAL, &[TypeId::REAL, TypeId::LREAL]);
    assert_excludes(
        &registry,
        TypeId::ANY_REAL,
        &[TypeId::INT, TypeId::ULINT, TypeId::TIME],
    );
}

#[test]
fn any_duration_contains_time_widths_without_becoming_numeric_or_date() {
    let registry = TypeRegistry::new();
    assert_members(
        &registry,
        TypeId::ANY_DURATION,
        &[TypeId::TIME, TypeId::LTIME],
    );
    assert_excludes(
        &registry,
        TypeId::ANY_DURATION,
        &[TypeId::INT, TypeId::DATE, TypeId::TOD, TypeId::DT],
    );
    assert_excludes(&registry, TypeId::ANY_NUM, &[TypeId::TIME, TypeId::LTIME]);
    assert_excludes(&registry, TypeId::ANY_DATE, &[TypeId::TIME, TypeId::LTIME]);
}

#[test]
fn any_bit_contains_bool_and_all_bit_string_widths_only() {
    let registry = TypeRegistry::new();
    assert_members(
        &registry,
        TypeId::ANY_BIT,
        &[
            TypeId::BOOL,
            TypeId::BYTE,
            TypeId::WORD,
            TypeId::DWORD,
            TypeId::LWORD,
        ],
    );
    assert_excludes(
        &registry,
        TypeId::ANY_BIT,
        &[TypeId::INT, TypeId::ULINT, TypeId::CHAR],
    );
    assert_excludes(&registry, TypeId::ANY_INT, &[TypeId::BOOL]);
}

#[test]
fn any_chars_partitions_string_and_character_families() {
    let registry = TypeRegistry::new();
    assert_members(
        &registry,
        TypeId::ANY_CHARS,
        &[TypeId::STRING, TypeId::WSTRING, TypeId::CHAR, TypeId::WCHAR],
    );
    assert_members(
        &registry,
        TypeId::ANY_STRING,
        &[TypeId::STRING, TypeId::WSTRING],
    );
    assert_members(&registry, TypeId::ANY_CHAR, &[TypeId::CHAR, TypeId::WCHAR]);
    assert_excludes(
        &registry,
        TypeId::ANY_STRING,
        &[TypeId::CHAR, TypeId::WCHAR],
    );
    assert_excludes(
        &registry,
        TypeId::ANY_CHAR,
        &[TypeId::STRING, TypeId::WSTRING],
    );
    assert_excludes(&registry, TypeId::ANY_CHARS, &[TypeId::BYTE]);
}

#[test]
fn bounded_strings_keep_their_generic_string_family() {
    let mut registry = TypeRegistry::new();
    let narrow = registry.register_string_with_length(7);
    let wide = registry.register_wstring_with_length(11);
    assert_members(&registry, TypeId::ANY_ELEMENTARY, &[narrow, wide]);
    assert_members(&registry, TypeId::ANY_CHARS, &[narrow, wide]);
    assert_members(&registry, TypeId::ANY_STRING, &[narrow, wide]);
    assert_excludes(&registry, TypeId::ANY_CHAR, &[narrow, wide]);
    assert_excludes(&registry, TypeId::ANY_DERIVED, &[narrow, wide]);
}

#[test]
fn any_date_contains_short_and_long_civil_time_families_only() {
    let registry = TypeRegistry::new();
    assert_members(
        &registry,
        TypeId::ANY_DATE,
        &[
            TypeId::DATE,
            TypeId::LDATE,
            TypeId::TOD,
            TypeId::LTOD,
            TypeId::DT,
            TypeId::LDT,
        ],
    );
    assert_excludes(
        &registry,
        TypeId::ANY_DATE,
        &[TypeId::TIME, TypeId::LTIME, TypeId::STRING],
    );
}

#[test]
fn directly_derived_alias_inherits_ultimate_elementary_generic_family() {
    let mut registry = TypeRegistry::new();
    let signed = register_alias(&mut registry, "Signed", TypeId::DINT);
    let signed_chain = register_alias(&mut registry, "SignedChain", signed);
    let text = register_alias(&mut registry, "Text", TypeId::STRING);
    let date = register_alias(&mut registry, "DateAlias", TypeId::LDATE);

    assert_members(&registry, TypeId::ANY_SIGNED, &[signed, signed_chain]);
    assert_members(&registry, TypeId::ANY_INT, &[signed, signed_chain]);
    assert_members(&registry, TypeId::ANY_NUM, &[signed, signed_chain]);
    assert_members(&registry, TypeId::ANY_STRING, &[text]);
    assert_members(&registry, TypeId::ANY_DATE, &[date]);
    assert_excludes(
        &registry,
        TypeId::ANY_DERIVED,
        &[signed, signed_chain, text, date],
    );
}

#[test]
fn alias_to_non_alias_derived_shape_remains_any_derived() {
    let mut registry = TypeRegistry::new();
    let structure = registry.register_struct("Packet", Vec::new());
    let alias = register_alias(&mut registry, "PacketAlias", structure);
    let alias_chain = register_alias(&mut registry, "PacketAlias2", alias);
    assert_members(
        &registry,
        TypeId::ANY_DERIVED,
        &[structure, alias, alias_chain],
    );
    assert_excludes(
        &registry,
        TypeId::ANY_ELEMENTARY,
        &[structure, alias, alias_chain],
    );
}

#[test]
fn unresolved_and_cyclic_aliases_fail_closed_for_generic_membership() {
    let mut registry = TypeRegistry::new();
    let unresolved = register_alias(&mut registry, "Unresolved", TypeId(999_999));
    let first = registry.reserve("First");
    let second = registry.reserve("Second");
    registry.replace(
        first,
        Type::Alias {
            name: SmolStr::new("First"),
            target: second,
        },
    );
    registry.replace(
        second,
        Type::Alias {
            name: SmolStr::new("Second"),
            target: first,
        },
    );

    for generic in [
        TypeId::ANY,
        TypeId::ANY_DERIVED,
        TypeId::ANY_ELEMENTARY,
        TypeId::ANY_MAGNITUDE,
        TypeId::ANY_INT,
        TypeId::ANY_STRING,
        TypeId::ANY_DATE,
    ] {
        assert_excludes(&registry, generic, &[unresolved, first, second]);
    }
}

#[test]
fn enumeration_is_any_derived_not_any_integer_despite_integer_base() {
    let mut registry = TypeRegistry::new();
    let enumeration =
        registry.register_enum("State", TypeId::UDINT, vec![(SmolStr::new("Idle"), 0)]);
    assert_members(&registry, TypeId::ANY_DERIVED, &[enumeration]);
    assert_excludes(&registry, TypeId::ANY_INT, &[enumeration]);
    assert_excludes(&registry, TypeId::ANY_UNSIGNED, &[enumeration]);
    assert_excludes(&registry, TypeId::ANY_NUM, &[enumeration]);
}

#[test]
fn generic_membership_does_not_authorize_concrete_cross_family_assignment() {
    let registry = TypeRegistry::new();
    assert!(registry.is_assignable(TypeId::ANY_NUM, TypeId::INT));
    assert!(registry.is_assignable(TypeId::ANY_NUM, TypeId::REAL));
    assert!(!registry.is_assignable(TypeId::INT, TypeId::REAL));
    assert!(!registry.is_assignable(TypeId::REAL, TypeId::DINT));

    assert!(registry.is_assignable(TypeId::ANY_CHARS, TypeId::CHAR));
    assert!(registry.is_assignable(TypeId::ANY_CHARS, TypeId::STRING));
    assert!(!registry.is_assignable(TypeId::CHAR, TypeId::STRING));

    assert!(registry.is_assignable(TypeId::ANY_DATE, TypeId::DATE));
    assert!(registry.is_assignable(TypeId::ANY_DATE, TypeId::DT));
    assert!(!registry.is_assignable(TypeId::DATE, TypeId::DT));
}
