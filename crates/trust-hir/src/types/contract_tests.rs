use text_size::{TextRange, TextSize};

use super::{
    is_accuracy_preserving_implicit_conversion, ArrayDimensionExt, InitializerCatalog,
    InitializerId, InitializerRecord, StructField, Type, TypeId, TypeRegistry, UnionVariant,
    POINTER_REFERENCE_HANDLE_SIZE_BYTES,
};

fn range(start: u32, end: u32) -> TextRange {
    TextRange::new(TextSize::from(start), TextSize::from(end))
}

#[test]
fn builtin_name_lookup_is_ascii_case_insensitive_and_round_trips_canonical_names() {
    let cases = [
        ("BOOL", TypeId::BOOL),
        ("SINT", TypeId::SINT),
        ("INT", TypeId::INT),
        ("DINT", TypeId::DINT),
        ("LINT", TypeId::LINT),
        ("USINT", TypeId::USINT),
        ("UINT", TypeId::UINT),
        ("UDINT", TypeId::UDINT),
        ("ULINT", TypeId::ULINT),
        ("REAL", TypeId::REAL),
        ("LREAL", TypeId::LREAL),
        ("BYTE", TypeId::BYTE),
        ("WORD", TypeId::WORD),
        ("DWORD", TypeId::DWORD),
        ("LWORD", TypeId::LWORD),
        ("TIME", TypeId::TIME),
        ("LTIME", TypeId::LTIME),
        ("DATE", TypeId::DATE),
        ("LDATE", TypeId::LDATE),
        ("TIME_OF_DAY", TypeId::TOD),
        ("LTIME_OF_DAY", TypeId::LTOD),
        ("DATE_AND_TIME", TypeId::DT),
        ("LDATE_AND_TIME", TypeId::LDT),
        ("STRING", TypeId::STRING),
        ("WSTRING", TypeId::WSTRING),
        ("CHAR", TypeId::CHAR),
        ("WCHAR", TypeId::WCHAR),
        ("ANY", TypeId::ANY),
        ("ANY_DERIVED", TypeId::ANY_DERIVED),
        ("ANY_ELEMENTARY", TypeId::ANY_ELEMENTARY),
        ("ANY_MAGNITUDE", TypeId::ANY_MAGNITUDE),
        ("ANY_INT", TypeId::ANY_INT),
        ("ANY_UNSIGNED", TypeId::ANY_UNSIGNED),
        ("ANY_SIGNED", TypeId::ANY_SIGNED),
        ("ANY_REAL", TypeId::ANY_REAL),
        ("ANY_NUM", TypeId::ANY_NUM),
        ("ANY_DURATION", TypeId::ANY_DURATION),
        ("ANY_BIT", TypeId::ANY_BIT),
        ("ANY_CHARS", TypeId::ANY_CHARS),
        ("ANY_STRING", TypeId::ANY_STRING),
        ("ANY_CHAR", TypeId::ANY_CHAR),
        ("ANY_DATE", TypeId::ANY_DATE),
    ];

    for (canonical, id) in cases {
        assert_eq!(
            TypeId::from_builtin_name(canonical),
            Some(id),
            "{canonical}"
        );
        assert_eq!(
            TypeId::from_builtin_name(&canonical.to_ascii_lowercase()),
            Some(id),
            "{canonical}"
        );
        assert_eq!(id.builtin_name(), Some(canonical), "{canonical}");
    }
    assert_eq!(TypeId::from_builtin_name("TOD"), Some(TypeId::TOD));
    assert_eq!(TypeId::from_builtin_name("LTOD"), Some(TypeId::LTOD));
    assert_eq!(TypeId::from_builtin_name("DT"), Some(TypeId::DT));
    assert_eq!(TypeId::from_builtin_name("LDT"), Some(TypeId::LDT));
}

#[test]
fn non_builtin_ids_and_near_miss_names_do_not_acquire_builtin_identity() {
    for name in ["", "INTEGER", "ANY_INTEGER", "DATE_TIME", "STRING[8]"] {
        assert_eq!(TypeId::from_builtin_name(name), None, "{name}");
    }
    assert_eq!(TypeId::UNKNOWN.builtin_name(), None);
    assert_eq!(TypeId::VOID.builtin_name(), None);
    assert_eq!(TypeId(TypeId::USER_TYPES_START).builtin_name(), None);
}

#[test]
fn registry_contains_every_builtin_under_its_declared_short_name() {
    let registry = TypeRegistry::new();
    let cases = [
        ("UNKNOWN", TypeId::UNKNOWN),
        ("VOID", TypeId::VOID),
        ("NULL", TypeId::NULL),
        ("BOOL", TypeId::BOOL),
        ("SINT", TypeId::SINT),
        ("INT", TypeId::INT),
        ("DINT", TypeId::DINT),
        ("LINT", TypeId::LINT),
        ("USINT", TypeId::USINT),
        ("UINT", TypeId::UINT),
        ("UDINT", TypeId::UDINT),
        ("ULINT", TypeId::ULINT),
        ("REAL", TypeId::REAL),
        ("LREAL", TypeId::LREAL),
        ("BYTE", TypeId::BYTE),
        ("WORD", TypeId::WORD),
        ("DWORD", TypeId::DWORD),
        ("LWORD", TypeId::LWORD),
        ("TIME", TypeId::TIME),
        ("LTIME", TypeId::LTIME),
        ("DATE", TypeId::DATE),
        ("LDATE", TypeId::LDATE),
        ("TOD", TypeId::TOD),
        ("LTOD", TypeId::LTOD),
        ("DT", TypeId::DT),
        ("LDT", TypeId::LDT),
        ("STRING", TypeId::STRING),
        ("WSTRING", TypeId::WSTRING),
        ("CHAR", TypeId::CHAR),
        ("WCHAR", TypeId::WCHAR),
        ("ANY", TypeId::ANY),
        ("ANY_DERIVED", TypeId::ANY_DERIVED),
        ("ANY_ELEMENTARY", TypeId::ANY_ELEMENTARY),
        ("ANY_MAGNITUDE", TypeId::ANY_MAGNITUDE),
        ("ANY_INT", TypeId::ANY_INT),
        ("ANY_UNSIGNED", TypeId::ANY_UNSIGNED),
        ("ANY_SIGNED", TypeId::ANY_SIGNED),
        ("ANY_REAL", TypeId::ANY_REAL),
        ("ANY_NUM", TypeId::ANY_NUM),
        ("ANY_DURATION", TypeId::ANY_DURATION),
        ("ANY_BIT", TypeId::ANY_BIT),
        ("ANY_CHARS", TypeId::ANY_CHARS),
        ("ANY_STRING", TypeId::ANY_STRING),
        ("ANY_CHAR", TypeId::ANY_CHAR),
        ("ANY_DATE", TypeId::ANY_DATE),
    ];

    for (name, id) in cases {
        assert_eq!(registry.lookup(name), Some(id), "{name}");
        assert_eq!(
            registry.lookup(&name.to_ascii_lowercase()),
            Some(id),
            "{name}"
        );
        assert!(registry.get(id).is_some(), "{name}");
    }
}

#[test]
fn type_classification_partitions_cover_elementary_and_non_elementary_families() {
    let numeric = [
        Type::SInt,
        Type::Int,
        Type::DInt,
        Type::LInt,
        Type::USInt,
        Type::UInt,
        Type::UDInt,
        Type::ULInt,
        Type::Real,
        Type::LReal,
        Type::Subrange {
            base: TypeId::INT,
            lower: -1,
            upper: 1,
        },
    ];
    assert!(numeric.iter().all(Type::is_numeric));
    assert!(numeric[..8].iter().all(Type::is_integer));
    assert!(!Type::Real.is_integer());
    assert!(!Type::LReal.is_integer());

    assert!([Type::SInt, Type::Int, Type::DInt, Type::LInt]
        .iter()
        .all(Type::is_signed));
    assert!([Type::USInt, Type::UInt, Type::UDInt, Type::ULInt]
        .iter()
        .all(Type::is_unsigned));
    assert!([Type::Real, Type::LReal].iter().all(Type::is_float));
    assert!(
        [Type::Bool, Type::Byte, Type::Word, Type::DWord, Type::LWord]
            .iter()
            .all(Type::is_bit_string)
    );

    assert!(Type::String { max_len: Some(7) }.is_string());
    assert!(Type::WString { max_len: None }.is_string());
    assert!(Type::Char.is_char());
    assert!(Type::WChar.is_char());
    assert!(Type::Char.is_chars());
    assert!(Type::String { max_len: None }.is_chars());
    assert!(!Type::Byte.is_chars());
}

#[test]
fn temporal_and_derived_classification_boundaries_are_explicit() {
    assert!([Type::Time, Type::LTime].iter().all(Type::is_duration));
    assert!([
        Type::Date,
        Type::LDate,
        Type::Tod,
        Type::LTod,
        Type::Dt,
        Type::Ldt
    ]
    .iter()
    .all(Type::is_date));
    assert!([
        Type::Time,
        Type::LTime,
        Type::Date,
        Type::LDate,
        Type::Tod,
        Type::LTod,
        Type::Dt,
        Type::Ldt
    ]
    .iter()
    .all(Type::is_time));

    let derived = [
        Type::Array {
            element: TypeId::INT,
            dimensions: vec![(0, 1)],
        },
        Type::Struct {
            name: "S".into(),
            fields: vec![],
        },
        Type::Union {
            name: "U".into(),
            variants: vec![],
        },
        Type::Enum {
            name: "E".into(),
            base: TypeId::INT,
            values: vec![],
        },
        Type::Pointer {
            target: TypeId::INT,
        },
        Type::Reference {
            target: TypeId::INT,
        },
        Type::FunctionBlock { name: "FB".into() },
        Type::Class { name: "C".into() },
        Type::Interface { name: "I".into() },
        Type::Alias {
            name: "A".into(),
            target: TypeId::INT,
        },
    ];
    assert!(derived.iter().all(Type::is_derived));
    assert!(!Type::Subrange {
        base: TypeId::INT,
        lower: 0,
        upper: 10
    }
    .is_derived());
    assert!(!Type::AnyDerived.is_derived());
}

#[test]
fn elementary_classification_excludes_meta_generic_and_compound_types() {
    let elementary = [
        Type::Bool,
        Type::SInt,
        Type::Int,
        Type::DInt,
        Type::LInt,
        Type::USInt,
        Type::UInt,
        Type::UDInt,
        Type::ULInt,
        Type::Real,
        Type::LReal,
        Type::Byte,
        Type::Word,
        Type::DWord,
        Type::LWord,
        Type::Time,
        Type::LTime,
        Type::Date,
        Type::LDate,
        Type::Tod,
        Type::LTod,
        Type::Dt,
        Type::Ldt,
        Type::String { max_len: None },
        Type::WString { max_len: Some(8) },
        Type::Char,
        Type::WChar,
    ];
    assert!(elementary.iter().all(Type::is_elementary));
    assert!(!Type::Unknown.is_elementary());
    assert!(!Type::Void.is_elementary());
    assert!(!Type::Null.is_elementary());
    assert!(!Type::AnyElementary.is_elementary());
    assert!(!Type::Array {
        element: TypeId::INT,
        dimensions: vec![(0, 1)]
    }
    .is_elementary());
}

#[test]
fn bit_size_reports_fixed_widths_and_rejects_unsized_types() {
    let cases = [
        (Type::Bool, 1),
        (Type::SInt, 8),
        (Type::USInt, 8),
        (Type::Byte, 8),
        (Type::Char, 8),
        (Type::Int, 16),
        (Type::UInt, 16),
        (Type::Word, 16),
        (Type::WChar, 16),
        (Type::DInt, 32),
        (Type::UDInt, 32),
        (Type::DWord, 32),
        (Type::Real, 32),
        (Type::Time, 32),
        (Type::LInt, 64),
        (Type::ULInt, 64),
        (Type::LWord, 64),
        (Type::LReal, 64),
        (Type::LTime, 64),
        (Type::LDate, 64),
    ];
    for (ty, width) in cases {
        assert_eq!(ty.bit_size(), Some(width), "{ty:?}");
    }

    for (base, width) in [
        (TypeId::SINT, 8),
        (TypeId::UINT, 16),
        (TypeId::DWORD, 32),
        (TypeId::LINT, 64),
    ] {
        assert_eq!(
            Type::Subrange {
                base,
                lower: 0,
                upper: 1
            }
            .bit_size(),
            Some(width)
        );
    }

    assert_eq!(Type::String { max_len: Some(8) }.bit_size(), None);
    assert_eq!(Type::Date.bit_size(), None);
    assert_eq!(
        Type::Subrange {
            base: TypeId(9_999),
            lower: 0,
            upper: 1
        }
        .bit_size(),
        None
    );
}

#[test]
fn array_dimension_wildcard_and_display_contract_is_exact() {
    assert!((0, i64::MAX).is_wildcard());
    assert_eq!((0, i64::MAX).display_bounds(), "*");
    assert!(!(1, i64::MAX).is_wildcard());
    assert_eq!((-3, 7).display_bounds(), "-3..7");
    assert_eq!((0, 0).display_bounds(), "0..0");
}

#[test]
fn initializer_catalog_uses_stable_insertion_order_and_explicit_type_defaults() {
    let mut catalog = InitializerCatalog::default();
    let first = catalog.insert(InitializerRecord { range: range(3, 8) });
    let second = catalog.insert(InitializerRecord {
        range: range(13, 21),
    });

    assert_eq!(first, InitializerId(0));
    assert_eq!(second, InitializerId(1));
    assert_eq!(catalog.records().len(), 2);
    assert_eq!(catalog.records()[0].range, range(3, 8));
    assert_eq!(
        catalog.get(second).map(|record| record.range),
        Some(range(13, 21))
    );
    assert_eq!(catalog.get(InitializerId(2)), None);
    assert_eq!(catalog.type_default(TypeId(123)), None);

    catalog.set_type_default(TypeId(123), second);
    assert_eq!(catalog.type_default(TypeId(123)), Some(second));
    assert_eq!(catalog.records()[0].range, range(3, 8));
}

#[test]
fn user_type_ids_are_monotonic_and_reserve_replace_preserves_identity_and_name() {
    let mut registry = TypeRegistry::new();
    let reserved = registry.reserve("Node");
    let next = registry.register(
        "Other",
        Type::Alias {
            name: "Other".into(),
            target: TypeId::INT,
        },
    );

    assert_eq!(reserved, TypeId(TypeId::USER_TYPES_START));
    assert_eq!(next, TypeId(TypeId::USER_TYPES_START + 1));
    assert_eq!(registry.lookup("node"), Some(reserved));
    assert_eq!(registry.get(reserved), Some(&Type::Unknown));

    registry.replace(
        reserved,
        Type::Struct {
            name: "Node".into(),
            fields: vec![StructField {
                name: "value".into(),
                type_id: TypeId::INT,
                address: None,
                default_initializer: None,
            }],
        },
    );

    assert_eq!(registry.lookup("NODE"), Some(reserved));
    assert_eq!(registry.type_name(reserved).as_deref(), Some("Node"));
    assert!(matches!(
        registry.get(reserved),
        Some(Type::Struct { fields, .. }) if fields.len() == 1
    ));
}

#[test]
fn named_constructed_registrations_preserve_complete_payloads_and_canonical_case() {
    let mut registry = TypeRegistry::new();
    let field = StructField {
        name: "Value".into(),
        type_id: TypeId::DINT,
        address: Some("%MW4".into()),
        default_initializer: Some(InitializerId(2)),
    };
    let variant = UnionVariant {
        name: "Bits".into(),
        type_id: TypeId::DWORD,
        address: Some("%MD0".into()),
        default_initializer: Some(InitializerId(3)),
    };
    let struct_id = registry.register_struct("Packet", vec![field.clone()]);
    let union_id = registry.register_union("Overlay", vec![variant.clone()]);
    let enum_id = registry.register_enum(
        "Mode",
        TypeId::USINT,
        vec![("Idle".into(), 0), ("Run".into(), 7)],
    );

    assert_eq!(registry.lookup("packet"), Some(struct_id));
    assert_eq!(registry.type_name(struct_id).as_deref(), Some("Packet"));
    assert_eq!(
        registry.get(struct_id),
        Some(&Type::Struct {
            name: "Packet".into(),
            fields: vec![field],
        })
    );
    assert_eq!(
        registry.get(union_id),
        Some(&Type::Union {
            name: "Overlay".into(),
            variants: vec![variant],
        })
    );
    assert_eq!(
        registry.get(enum_id),
        Some(&Type::Enum {
            name: "Mode".into(),
            base: TypeId::USINT,
            values: vec![("Idle".into(), 0), ("Run".into(), 7)],
        })
    );
}

#[test]
fn anonymous_constructed_type_names_are_deterministic_and_payloads_are_preserved() {
    let mut registry = TypeRegistry::new();
    let array = registry.register_array(TypeId::INT, vec![(-1, 2), (0, i64::MAX)]);
    let string = registry.register_string_with_length(17);
    let wstring = registry.register_wstring_with_length(9);
    let pointer = registry.register_pointer(TypeId::DINT);
    let reference = registry.register_reference(TypeId::BOOL);

    assert_eq!(
        registry.type_name(array).as_deref(),
        Some("ARRAY[-1..2, *] OF INT")
    );
    assert_eq!(registry.type_name(string).as_deref(), Some("STRING[17]"));
    assert_eq!(registry.type_name(wstring).as_deref(), Some("WSTRING[9]"));
    assert_eq!(
        registry.type_name(pointer).as_deref(),
        Some("POINTER TO DINT")
    );
    assert_eq!(
        registry.type_name(reference).as_deref(),
        Some("REF_TO BOOL")
    );
    assert_eq!(
        registry.get(array),
        Some(&Type::Array {
            element: TypeId::INT,
            dimensions: vec![(-1, 2), (0, i64::MAX)],
        })
    );
    assert_eq!(
        registry.get(string),
        Some(&Type::String { max_len: Some(17) })
    );
    assert_eq!(
        registry.get(wstring),
        Some(&Type::WString { max_len: Some(9) })
    );
    assert_eq!(
        registry.get(pointer),
        Some(&Type::Pointer {
            target: TypeId::DINT
        })
    );
    assert_eq!(
        registry.get(reference),
        Some(&Type::Reference {
            target: TypeId::BOOL
        })
    );
}

#[test]
fn unknown_constructed_targets_use_question_mark_names_without_becoming_builtin() {
    let mut registry = TypeRegistry::new();
    let unknown = TypeId(9_999);
    let array = registry.register_array(unknown, vec![(1, 1)]);
    let pointer = registry.register_pointer(unknown);
    let reference = registry.register_reference(unknown);

    assert_eq!(
        registry.type_name(array).as_deref(),
        Some("ARRAY[1..1] OF ?")
    );
    assert_eq!(registry.type_name(pointer).as_deref(), Some("POINTER TO ?"));
    assert_eq!(registry.type_name(reference).as_deref(), Some("REF_TO ?"));
    assert_eq!(registry.lookup("?"), None);
}

#[test]
fn assignment_compatibility_accepts_only_accuracy_preserving_scalar_widening() {
    let registry = TypeRegistry::new();
    let accepted = [
        (TypeId::INT, TypeId::SINT),
        (TypeId::DINT, TypeId::INT),
        (TypeId::LINT, TypeId::DINT),
        (TypeId::UINT, TypeId::USINT),
        (TypeId::UDINT, TypeId::UINT),
        (TypeId::ULINT, TypeId::UDINT),
        (TypeId::REAL, TypeId::INT),
        (TypeId::LREAL, TypeId::DINT),
        (TypeId::LREAL, TypeId::REAL),
        (TypeId::WORD, TypeId::BYTE),
        (TypeId::DWORD, TypeId::WORD),
        (TypeId::LWORD, TypeId::DWORD),
    ];
    for (target, source) in accepted {
        assert!(
            registry.is_assignable(target, source),
            "{target:?} <- {source:?}"
        );
    }

    let rejected = [
        (TypeId::SINT, TypeId::INT),
        (TypeId::INT, TypeId::UINT),
        (TypeId::UDINT, TypeId::DINT),
        (TypeId::REAL, TypeId::DINT),
        (TypeId::LREAL, TypeId::LINT),
        (TypeId::BYTE, TypeId::WORD),
        (TypeId::WORD, TypeId::UINT),
        (TypeId::STRING, TypeId::WSTRING),
    ];
    for (target, source) in rejected {
        assert!(
            !registry.is_assignable(target, source),
            "{target:?} <- {source:?}"
        );
    }
}

#[test]
fn scalar_widening_primitive_matches_registry_conversion_policy() {
    let accepted = [
        (Type::Int, Type::SInt),
        (Type::LInt, Type::DInt),
        (Type::ULInt, Type::UDInt),
        (Type::Real, Type::Int),
        (Type::LReal, Type::Real),
        (Type::DWord, Type::Word),
    ];
    assert!(accepted
        .iter()
        .all(|(target, source)| { is_accuracy_preserving_implicit_conversion(target, source) }));

    let rejected = [
        (Type::Int, Type::UInt),
        (Type::Real, Type::DInt),
        (Type::LReal, Type::LInt),
        (Type::Word, Type::UInt),
        (Type::Time, Type::DInt),
    ];
    assert!(rejected
        .iter()
        .all(|(target, source)| { !is_accuracy_preserving_implicit_conversion(target, source) }));
}

#[test]
fn generic_assignment_targets_cover_each_declared_type_family() {
    let mut registry = TypeRegistry::new();
    let derived = registry.register_struct("Record", vec![]);
    let accepted = [
        (TypeId::ANY, TypeId::BOOL),
        (TypeId::ANY_DERIVED, derived),
        (TypeId::ANY_ELEMENTARY, TypeId::BOOL),
        (TypeId::ANY_MAGNITUDE, TypeId::TIME),
        (TypeId::ANY_INT, TypeId::DINT),
        (TypeId::ANY_UNSIGNED, TypeId::UDINT),
        (TypeId::ANY_SIGNED, TypeId::LINT),
        (TypeId::ANY_REAL, TypeId::REAL),
        (TypeId::ANY_NUM, TypeId::LREAL),
        (TypeId::ANY_DURATION, TypeId::LTIME),
        (TypeId::ANY_BIT, TypeId::DWORD),
        (TypeId::ANY_CHARS, TypeId::WCHAR),
        (TypeId::ANY_CHARS, TypeId::STRING),
        (TypeId::ANY_STRING, TypeId::WSTRING),
        (TypeId::ANY_CHAR, TypeId::CHAR),
        (TypeId::ANY_DATE, TypeId::LDT),
    ];
    for (target, source) in accepted {
        assert!(
            registry.is_assignable(target, source),
            "{target:?} <- {source:?}"
        );
    }

    let rejected = [
        (TypeId::ANY, TypeId::VOID),
        (TypeId::ANY_DERIVED, TypeId::INT),
        (TypeId::ANY_ELEMENTARY, derived),
        (TypeId::ANY_MAGNITUDE, TypeId::DATE),
        (TypeId::ANY_INT, TypeId::REAL),
        (TypeId::ANY_UNSIGNED, TypeId::INT),
        (TypeId::ANY_SIGNED, TypeId::UINT),
        (TypeId::ANY_REAL, TypeId::DINT),
        (TypeId::ANY_DURATION, TypeId::TOD),
        (TypeId::ANY_BIT, TypeId::INT),
        (TypeId::ANY_STRING, TypeId::CHAR),
        (TypeId::ANY_CHAR, TypeId::STRING),
        (TypeId::ANY_DATE, TypeId::TIME),
    ];
    for (target, source) in rejected {
        assert!(
            !registry.is_assignable(target, source),
            "{target:?} <- {source:?}"
        );
    }
}

#[test]
fn subranges_normalize_to_their_registered_base_for_compatibility() {
    let mut registry = TypeRegistry::new();
    let signed = registry.register(
        "SignedRange",
        Type::Subrange {
            base: TypeId::INT,
            lower: -2,
            upper: 2,
        },
    );
    let unsigned = registry.register(
        "UnsignedRange",
        Type::Subrange {
            base: TypeId::UINT,
            lower: 0,
            upper: 4,
        },
    );
    let missing = registry.register(
        "MissingBaseRange",
        Type::Subrange {
            base: TypeId(9_999),
            lower: 0,
            upper: 1,
        },
    );

    assert!(registry.is_assignable(TypeId::ANY_SIGNED, signed));
    assert!(registry.is_assignable(TypeId::DINT, signed));
    assert!(registry.is_assignable(TypeId::ANY_UNSIGNED, unsigned));
    assert!(!registry.is_assignable(TypeId::ANY_SIGNED, unsigned));
    assert!(!registry.is_assignable(TypeId::INT, missing));
}

#[test]
fn array_compatibility_checks_rank_bounds_wildcards_and_element_types() {
    let mut registry = TypeRegistry::new();
    let exact = registry.register_array(TypeId::INT, vec![(1, 3)]);
    let same = registry.register_array(TypeId::INT, vec![(1, 3)]);
    let wildcard = registry.register_array(TypeId::INT, vec![(0, i64::MAX)]);
    let other_bounds = registry.register_array(TypeId::INT, vec![(0, 2)]);
    let other_rank = registry.register_array(TypeId::INT, vec![(1, 3), (1, 3)]);
    let wider_elements = registry.register_array(TypeId::DINT, vec![(1, 3)]);

    assert!(registry.is_assignable(exact, same));
    assert!(registry.is_assignable(exact, wildcard));
    assert!(registry.is_assignable(wildcard, exact));
    assert!(!registry.is_assignable(exact, other_bounds));
    assert!(!registry.is_assignable(exact, other_rank));
    assert!(registry.is_assignable(wider_elements, exact));
    assert!(!registry.is_assignable(exact, wider_elements));
}

#[test]
fn null_is_assignable_only_to_pointer_and_reference_families() {
    let mut registry = TypeRegistry::new();
    let pointer = registry.register_pointer(TypeId::INT);
    let reference = registry.register_reference(TypeId::INT);
    let same_pointer = registry.register_pointer(TypeId::INT);
    let other_pointer = registry.register_pointer(TypeId::DINT);

    assert!(registry.is_assignable(pointer, TypeId::NULL));
    assert!(registry.is_assignable(reference, TypeId::NULL));
    assert!(!registry.is_assignable(TypeId::INT, TypeId::NULL));
    assert!(registry.is_assignable(pointer, same_pointer));
    assert!(!registry.is_assignable(pointer, other_pointer));
    assert!(!registry.is_assignable(pointer, reference));
}

#[test]
fn string_lengths_do_not_change_family_compatibility_but_width_does() {
    let mut registry = TypeRegistry::new();
    let short = registry.register_string_with_length(3);
    let long = registry.register_string_with_length(100);
    let wide = registry.register_wstring_with_length(3);

    assert!(registry.is_assignable(short, long));
    assert!(registry.is_assignable(long, short));
    assert!(registry.is_assignable(TypeId::STRING, short));
    assert!(registry.is_assignable(TypeId::WSTRING, wide));
    assert!(!registry.is_assignable(short, wide));
    assert!(!registry.is_assignable(TypeId::STRING, TypeId::WSTRING));
}

#[test]
fn missing_type_identities_fail_closed_except_exact_identity() {
    let registry = TypeRegistry::new();
    let missing_a = TypeId(50_000);
    let missing_b = TypeId(50_001);

    assert!(registry.is_assignable(missing_a, missing_a));
    assert!(!registry.is_assignable(TypeId::INT, missing_a));
    assert!(!registry.is_assignable(missing_a, TypeId::INT));
    assert!(!registry.is_assignable(missing_a, missing_b));
    assert_eq!(registry.get(missing_a), None);
    assert_eq!(registry.type_name(missing_a), None);
}

#[test]
fn pointer_reference_size_contract_tracks_the_compilation_target_pointer_width() {
    assert_eq!(
        POINTER_REFERENCE_HANDLE_SIZE_BYTES,
        std::mem::size_of::<usize>() as u64
    );
    assert!(matches!(POINTER_REFERENCE_HANDLE_SIZE_BYTES, 4 | 8));
}
