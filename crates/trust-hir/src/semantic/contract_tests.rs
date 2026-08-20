use super::*;

use std::cell::Cell;

fn range(start: u32, end: u32) -> TextRange {
    TextRange::new(start.into(), end.into())
}

fn name(value: &str) -> QualifiedName {
    QualifiedName::from_dotted(value).expect("qualified name")
}

#[allow(clippy::too_many_arguments)]
fn declaration(
    symbol: u32,
    qualified: &str,
    file: u32,
    start: u32,
    kind: DeclarationKind,
    role: SemanticRole,
    owner: Option<u32>,
    owner_scope: Option<u32>,
    owned_scope: Option<u32>,
    imported: bool,
) -> DeclarationRecord {
    DeclarationRecord::new(
        SymbolId(symbol),
        name(qualified),
        SourceIdentity::new(FileId(file), range(start, start + 3)),
        TypeId::DINT,
        kind,
        role,
        owner.map(SymbolId),
        owner_scope.map(ScopeId),
        owned_scope.map(ScopeId),
        imported,
    )
}

fn reference(
    owner: u32,
    kind: DeclarationReferenceKind,
    qualified: &str,
    resolved: u32,
) -> DeclarationReferenceRecord {
    DeclarationReferenceRecord::new(
        SymbolId(owner),
        kind,
        name(qualified),
        Some(range(owner, owner + 1)),
        SemanticOutcome::Resolved(SymbolId(resolved)),
    )
}

#[test]
fn qualified_name_requires_at_least_one_nonempty_part() {
    assert!(QualifiedName::new(Vec::new()).is_none());
    assert!(QualifiedName::from_dotted("").is_none());
    assert!(QualifiedName::from_dotted(".").is_none());
    assert!(QualifiedName::from_dotted("...").is_none());
}

#[test]
fn qualified_name_preserves_parts_case_and_ignores_empty_separators() {
    let qualified =
        QualifiedName::from_dotted(".Plant..Line.Controller.").expect("normalized qualified name");

    assert_eq!(
        qualified.parts(),
        &[
            SmolStr::new("Plant"),
            SmolStr::new("Line"),
            SmolStr::new("Controller")
        ]
    );
    assert_eq!(qualified.display(), "Plant.Line.Controller");
}

#[test]
fn qualified_name_new_preserves_supplied_part_text_without_case_folding() {
    let qualified = QualifiedName::new(vec![
        SmolStr::new("Mixed"),
        SmolStr::new("cASE"),
        SmolStr::new("Value"),
    ])
    .expect("qualified name");

    assert_eq!(qualified.display(), "Mixed.cASE.Value");
}

#[test]
fn source_identity_round_trips_file_and_exact_range() {
    let source = SourceIdentity::new(FileId(42), range(10, 90));

    assert_eq!(source.file_id(), FileId(42));
    assert_eq!(source.range(), range(10, 90));
}

#[test]
fn scope_path_round_trips_scope_and_owner_chain_order() {
    let path = ScopePath::new(ScopeId(7), vec![SymbolId(1), SymbolId(4), SymbolId(9)]);

    assert_eq!(path.scope_id(), ScopeId(7));
    assert_eq!(path.owners(), &[SymbolId(1), SymbolId(4), SymbolId(9)]);
}

#[test]
fn semantic_outcome_map_transforms_resolved_payload_once() {
    let calls = Cell::new(0);
    let mapped = SemanticOutcome::Resolved(SymbolId(9)).map(|symbol| {
        calls.set(calls.get() + 1);
        symbol.0 * 2
    });

    assert_eq!(mapped, SemanticOutcome::Resolved(18));
    assert_eq!(calls.get(), 1);
}

#[test]
fn semantic_outcome_map_preserves_every_nonresolved_payload_without_calling_mapper() {
    let qualified = name("A.Target");
    let source_range = Some(range(2, 8));
    let outcomes = [
        SemanticOutcome::<SymbolId>::Unknown {
            name: Some(qualified.clone()),
            range: source_range,
        },
        SemanticOutcome::Ambiguous {
            name: qualified.clone(),
            range: source_range,
        },
        SemanticOutcome::WrongKind {
            symbol_id: SymbolId(3),
            expected: SemanticRole::Type,
            actual: SemanticRole::Value,
            range: source_range,
        },
        SemanticOutcome::SuppressedCascade {
            primary: DiagnosticCode::CannotResolve,
            range: source_range,
        },
        SemanticOutcome::InvariantViolation {
            message: SmolStr::new("invariant"),
            range: source_range,
        },
    ];

    for outcome in outcomes {
        let calls = Cell::new(0);
        let expected = outcome.clone();
        let mapped = outcome.map(|symbol| {
            calls.set(calls.get() + 1);
            symbol.0
        });

        assert_eq!(calls.get(), 0);
        assert_eq!(
            format!("{mapped:?}"),
            format!("{expected:?}"),
            "classification changed"
        );
    }
}

#[test]
fn declaration_record_accessors_return_every_constructor_field() {
    let record = DeclarationRecord::new(
        SymbolId(8),
        name("Plant.Controller.Speed"),
        SourceIdentity::new(FileId(3), range(11, 16)),
        TypeId::LREAL,
        DeclarationKind::Variable,
        SemanticRole::Value,
        Some(SymbolId(7)),
        Some(ScopeId(6)),
        Some(ScopeId(9)),
        true,
    );

    assert_eq!(record.symbol_id(), SymbolId(8));
    assert_eq!(record.qualified_name().display(), "Plant.Controller.Speed");
    assert_eq!(
        record.source(),
        SourceIdentity::new(FileId(3), range(11, 16))
    );
    assert_eq!(record.type_id(), TypeId::LREAL);
    assert_eq!(record.kind(), DeclarationKind::Variable);
    assert_eq!(record.role(), SemanticRole::Value);
    assert_eq!(record.owner_symbol_id(), Some(SymbolId(7)));
    assert_eq!(record.owner_scope_id(), Some(ScopeId(6)));
    assert_eq!(record.owned_scope_id(), Some(ScopeId(9)));
    assert!(record.is_imported());
}

#[test]
fn local_top_level_declaration_preserves_absent_owner_fields() {
    let record = declaration(
        1,
        "Main",
        0,
        5,
        DeclarationKind::Program,
        SemanticRole::ScopeOwner,
        None,
        None,
        None,
        false,
    );

    assert_eq!(record.owner_symbol_id(), None);
    assert_eq!(record.owner_scope_id(), None);
    assert_eq!(record.owned_scope_id(), None);
    assert!(!record.is_imported());
}

#[test]
fn declaration_reference_accessors_return_every_constructor_field() {
    let outcome = SemanticOutcome::WrongKind {
        symbol_id: SymbolId(11),
        expected: SemanticRole::Type,
        actual: SemanticRole::Value,
        range: Some(range(20, 24)),
    };
    let record = DeclarationReferenceRecord::new(
        SymbolId(7),
        DeclarationReferenceKind::Implements,
        name("Plant.IDevice"),
        Some(range(20, 24)),
        outcome.clone(),
    );

    assert_eq!(record.owner_symbol_id(), SymbolId(7));
    assert_eq!(record.kind(), DeclarationReferenceKind::Implements);
    assert_eq!(record.name().display(), "Plant.IDevice");
    assert_eq!(record.range(), Some(range(20, 24)));
    assert_eq!(record.outcome(), &outcome);
}

#[test]
fn catalog_entries_sort_by_file_then_start_then_qualified_name() {
    let catalog = DeclarationCatalog::new(
        vec![
            declaration(
                4,
                "Zulu",
                1,
                5,
                DeclarationKind::Variable,
                SemanticRole::Value,
                None,
                None,
                None,
                false,
            ),
            declaration(
                3,
                "Beta",
                0,
                10,
                DeclarationKind::Variable,
                SemanticRole::Value,
                None,
                None,
                None,
                false,
            ),
            declaration(
                2,
                "Alpha",
                0,
                10,
                DeclarationKind::Variable,
                SemanticRole::Value,
                None,
                None,
                None,
                false,
            ),
            declaration(
                1,
                "Later",
                0,
                20,
                DeclarationKind::Variable,
                SemanticRole::Value,
                None,
                None,
                None,
                false,
            ),
        ],
        Vec::new(),
    );

    assert_eq!(
        catalog
            .entries()
            .iter()
            .map(|entry| entry.qualified_name().display())
            .collect::<Vec<_>>(),
        vec!["Alpha", "Beta", "Later", "Zulu"]
    );
}

#[test]
fn catalog_references_sort_by_owner_then_name_then_kind() {
    let catalog = DeclarationCatalog::new(
        Vec::new(),
        vec![
            reference(2, DeclarationReferenceKind::Extends, "Base", 10),
            reference(1, DeclarationReferenceKind::Implements, "Device", 11),
            reference(1, DeclarationReferenceKind::Implements, "Base", 12),
            reference(1, DeclarationReferenceKind::Extends, "Base", 13),
        ],
    );

    assert_eq!(
        catalog
            .references()
            .iter()
            .map(|record| {
                (
                    record.owner_symbol_id().0,
                    record.name().display(),
                    record.kind(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (1, "Base".to_owned(), DeclarationReferenceKind::Extends),
            (1, "Base".to_owned(), DeclarationReferenceKind::Implements),
            (1, "Device".to_owned(), DeclarationReferenceKind::Implements),
            (2, "Base".to_owned(), DeclarationReferenceKind::Extends),
        ]
    );
}

#[test]
fn catalog_lookup_is_ascii_case_insensitive_but_requires_complete_name() {
    let catalog = DeclarationCatalog::new(
        vec![declaration(
            1,
            "Plant.Line.Speed",
            0,
            1,
            DeclarationKind::Variable,
            SemanticRole::Value,
            None,
            None,
            None,
            false,
        )],
        Vec::new(),
    );

    assert!(catalog.find_qualified("plant.line.speed").is_some());
    assert!(catalog.find_qualified("PLANT.LINE.SPEED").is_some());
    assert!(catalog.find_qualified("Line.Speed").is_none());
    assert!(catalog.find_qualified("Speed").is_none());
    assert!(catalog.find_qualified("Plant.Line.Speed.More").is_none());
}

#[test]
fn catalog_retains_duplicate_records_instead_of_hiding_declaration_debt() {
    let record = declaration(
        1,
        "Duplicate",
        0,
        1,
        DeclarationKind::Variable,
        SemanticRole::Value,
        None,
        None,
        None,
        false,
    );
    let catalog = DeclarationCatalog::new(vec![record.clone(), record], Vec::new());

    assert_eq!(catalog.entries().len(), 2);
}

#[test]
fn default_catalog_is_empty_in_both_directions() {
    let catalog = DeclarationCatalog::default();

    assert!(catalog.entries().is_empty());
    assert!(catalog.references().is_empty());
    assert!(catalog.find_qualified("Anything").is_none());
}
