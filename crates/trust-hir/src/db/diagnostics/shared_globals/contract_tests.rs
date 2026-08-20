use super::*;

use crate::db::{Database, FileId, SemanticDatabase, SourceDatabase};
use crate::diagnostics::{Diagnostic, DiagnosticCode, DiagnosticSeverity};
use trust_syntax::parser::parse;

fn diagnostics_for(sources: &[&str], file: usize) -> Vec<Diagnostic> {
    let mut database = Database::new();
    for (index, source) in sources.iter().enumerate() {
        database.set_source_text(FileId(index as u32), (*source).to_owned());
    }
    database.diagnostics(FileId(file as u32)).as_ref().clone()
}

fn hazards(source: &str) -> Vec<Diagnostic> {
    diagnostics_for(&[source], 0)
        .into_iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::SharedGlobalTaskHazard)
        .collect()
}

fn assert_hazard(source: &str) {
    let found = hazards(source);
    assert!(!found.is_empty(), "expected shared-global hazard");
    assert!(found
        .iter()
        .all(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning));
}

fn assert_no_hazard(source: &str) {
    let found = hazards(source);
    assert!(found.is_empty(), "unexpected hazards: {found:?}");
}

fn two_task_source(writer_body: &str, reader_body: &str) -> String {
    format!(
        r#"
CONFIGURATION Conf
VAR_GLOBAL
    Shared : INT;
END_VAR
RESOURCE R ON CPU
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    TASK Slow (INTERVAL := T#20ms, PRIORITY := 2);
    PROGRAM WriterInstance WITH Fast : Writer;
    PROGRAM ReaderInstance WITH Slow : Reader;
END_RESOURCE
END_CONFIGURATION

PROGRAM Writer
    VAR observed : INT; END_VAR
    {writer_body}
END_PROGRAM

PROGRAM Reader
    VAR observed : INT; END_VAR
    {reader_body}
END_PROGRAM
"#
    )
}

fn parsed_expression(source: &str, kind: SyntaxKind) -> SyntaxNode {
    parse(source)
        .syntax()
        .descendants()
        .find(|node| node.kind() == kind)
        .unwrap_or_else(|| panic!("requested {kind:?} expression in source: {source}"))
}

#[test]
fn program_access_records_reads_and_writes_without_conflating_sets() {
    let read = SymbolId(11);
    let write = SymbolId(12);
    let mut access = ProgramAccess {
        reads: FxHashSet::default(),
        writes: FxHashSet::default(),
    };

    access.record(read, false);
    access.record(write, true);
    access.record(write, true);

    assert_eq!(access.reads, FxHashSet::from_iter([read]));
    assert_eq!(access.writes, FxHashSet::from_iter([write]));
}

#[test]
fn direct_and_wrapped_assignment_targets_are_writes() {
    let direct = parsed_expression(
        "PROGRAM Main\nVAR x : INT; END_VAR\nx := 1;\nEND_PROGRAM",
        SyntaxKind::NameRef,
    );
    let field = parsed_expression(
        "PROGRAM Main\nitem.value := 1;\nEND_PROGRAM",
        SyntaxKind::FieldExpr,
    );
    let index = parsed_expression(
        "PROGRAM Main\nitems[0] := 1;\nEND_PROGRAM",
        SyntaxKind::IndexExpr,
    );
    let deref = parsed_expression(
        "PROGRAM Main\nptr^ := 1;\nEND_PROGRAM",
        SyntaxKind::DerefExpr,
    );

    assert!(is_write_context(&direct));
    assert!(is_write_context(&field));
    assert!(is_write_context(&index));
    assert!(is_write_context(&deref));
}

#[test]
fn right_hand_and_non_assignment_references_are_reads() {
    let source = parse(
        r#"
PROGRAM Main
VAR x : INT; END_VAR
x := item.value;
IF item.ready THEN
END_IF;
END_PROGRAM
"#,
    )
    .syntax();
    let fields = source
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::FieldExpr)
        .collect::<Vec<_>>();

    assert_eq!(fields.len(), 2);
    assert!(fields.iter().all(|field| !is_write_context(field)));
}

#[test]
fn qualified_field_parts_are_returned_in_source_order() {
    let field = parsed_expression(
        "PROGRAM Main\nresult := Plant.Line.Shared;\nEND_PROGRAM",
        SyntaxKind::FieldExpr,
    );
    let outermost = field
        .ancestors()
        .take_while(|node| node.kind() == SyntaxKind::FieldExpr)
        .last()
        .unwrap_or(field);

    assert_eq!(
        qualified_name_from_field_expr(&outermost),
        Some(vec![
            SmolStr::new("Plant"),
            SmolStr::new("Line"),
            SmolStr::new("Shared")
        ])
    );
}

#[test]
fn task_identity_is_case_insensitive_and_scoped_by_configuration_and_resource() {
    let normalized = normalize_task_name("fAsT");
    let (first, first_label) = task_id_and_label(
        &normalized,
        "fAsT",
        Some(&SmolStr::new("Conf")),
        Some(&SmolStr::new("R1")),
    );
    let (same, _) = task_id_and_label(
        &normalize_task_name("FAST"),
        "FAST",
        Some(&SmolStr::new("conf")),
        Some(&SmolStr::new("r1")),
    );
    let (other_resource, _) = task_id_and_label(
        &normalized,
        "Fast",
        Some(&SmolStr::new("Conf")),
        Some(&SmolStr::new("R2")),
    );

    assert_eq!(first, same);
    assert_ne!(first, other_resource);
    assert_eq!(first_label, "Conf/R1/fAsT");
}

#[test]
fn task_list_is_sorted_and_bounded_with_remaining_count() {
    let tasks = ["Zulu", "Alpha", "Echo", "Bravo", "Delta"]
        .into_iter()
        .map(|name| TaskId(SmolStr::new(name)))
        .collect::<FxHashSet<_>>();
    let info = tasks
        .iter()
        .map(|task| {
            (
                task.clone(),
                TaskInfo {
                    label: task.0.clone(),
                    range: TextRange::default(),
                },
            )
        })
        .collect::<FxHashMap<_, _>>();

    assert_eq!(
        format_task_list(&tasks, &info),
        "Alpha, Bravo, Delta, +2 more"
    );
}

#[test]
fn read_only_access_from_multiple_tasks_is_not_a_hazard() {
    assert_no_hazard(&two_task_source(
        "observed := Shared;",
        "observed := Shared;",
    ));
}

#[test]
fn write_and_read_on_different_tasks_is_a_hazard() {
    assert_hazard(&two_task_source(
        "Shared := observed + 1;",
        "observed := Shared;",
    ));
}

#[test]
fn writes_on_two_tasks_are_a_hazard() {
    assert_hazard(&two_task_source("Shared := 1;", "Shared := 2;"));
}

#[test]
fn one_program_type_instantiated_on_two_tasks_is_a_hazard() {
    assert_hazard(
        r#"
CONFIGURATION Conf
VAR_GLOBAL Shared : INT; END_VAR
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    TASK Slow (PRIORITY := 2);
    PROGRAM First WITH Fast : Writer;
    PROGRAM Second WITH Slow : Writer;
END_RESOURCE
END_CONFIGURATION
PROGRAM Writer
    Shared := Shared + 1;
END_PROGRAM
"#,
    );
}

#[test]
fn multiple_programs_on_same_case_insensitive_task_are_one_context() {
    assert_no_hazard(
        r#"
CONFIGURATION Conf
VAR_GLOBAL Shared : INT; END_VAR
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    PROGRAM First WITH FAST : Writer;
    PROGRAM Second WITH fast : Reader;
END_RESOURCE
END_CONFIGURATION
PROGRAM Writer
    Shared := Shared + 1;
END_PROGRAM
PROGRAM Reader
    VAR observed : INT; END_VAR
    observed := Shared;
END_PROGRAM
"#,
    );
}

#[test]
fn equally_named_tasks_in_different_resources_are_distinct_contexts() {
    assert_hazard(
        r#"
CONFIGURATION Conf
VAR_GLOBAL Shared : INT; END_VAR
RESOURCE R1 ON CPU
    TASK Fast (PRIORITY := 1);
    PROGRAM First WITH Fast : Writer;
END_RESOURCE
RESOURCE R2 ON CPU
    TASK Fast (PRIORITY := 1);
    PROGRAM Second WITH Fast : Reader;
END_RESOURCE
END_CONFIGURATION
PROGRAM Writer
    Shared := 1;
END_PROGRAM
PROGRAM Reader
    VAR observed : INT; END_VAR
    observed := Shared;
END_PROGRAM
"#,
    );
}

#[test]
fn unassigned_program_access_does_not_create_a_task_hazard() {
    assert_no_hazard(
        r#"
CONFIGURATION Conf
VAR_GLOBAL Shared : INT; END_VAR
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    PROGRAM First WITH Fast : Reader;
END_RESOURCE
END_CONFIGURATION
PROGRAM Reader
    VAR observed : INT; END_VAR
    observed := Shared;
END_PROGRAM
PROGRAM UnassignedWriter
    Shared := 1;
END_PROGRAM
"#,
    );
}

#[test]
fn local_shadow_is_not_conflated_with_global_identity() {
    assert_no_hazard(&two_task_source(
        "VAR Shared : INT; END_VAR Shared := 1;",
        "observed := Shared;",
    ));
}

#[test]
fn qualified_namespaced_global_access_is_tracked_by_resolved_identity() {
    assert_hazard(
        r#"
NAMESPACE Plant
VAR_GLOBAL Shared : INT; END_VAR
END_NAMESPACE
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    TASK Slow (PRIORITY := 2);
    PROGRAM First WITH Fast : Writer;
    PROGRAM Second WITH Slow : Reader;
END_RESOURCE
END_CONFIGURATION
PROGRAM Writer
    Plant.Shared := 1;
END_PROGRAM
PROGRAM Reader
    VAR observed : INT; END_VAR
    observed := Plant.Shared;
END_PROGRAM
"#,
    );
}

#[test]
fn same_global_spelling_in_distinct_namespaces_is_not_conflated() {
    assert_no_hazard(
        r#"
NAMESPACE A
VAR_GLOBAL Shared : INT; END_VAR
END_NAMESPACE
NAMESPACE B
VAR_GLOBAL Shared : INT; END_VAR
END_NAMESPACE
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    TASK Slow (PRIORITY := 2);
    PROGRAM First WITH Fast : Writer;
    PROGRAM Second WITH Slow : Reader;
END_RESOURCE
END_CONFIGURATION
PROGRAM Writer
    A.Shared := 1;
END_PROGRAM
PROGRAM Reader
    VAR observed : INT; END_VAR
    observed := B.Shared;
END_PROGRAM
"#,
    );
}

#[test]
fn external_alias_accesses_are_linked_to_the_owning_global() {
    assert_hazard(
        r#"
CONFIGURATION Conf
VAR_GLOBAL Shared : INT; END_VAR
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    TASK Slow (PRIORITY := 2);
    PROGRAM First WITH Fast : Writer;
    PROGRAM Second WITH Slow : Reader;
END_RESOURCE
END_CONFIGURATION
PROGRAM Writer
VAR_EXTERNAL Shared : INT; END_VAR
    Shared := 1;
END_PROGRAM
PROGRAM Reader
VAR_EXTERNAL Shared : INT; END_VAR
VAR observed : INT; END_VAR
    observed := Shared;
END_PROGRAM
"#,
    );
}

#[test]
fn cross_file_programs_and_configuration_contribute_to_global_owner_warning() {
    let sources = [
        "VAR_GLOBAL Shared : INT; END_VAR",
        r#"
PROGRAM Writer
    Shared := 1;
END_PROGRAM
PROGRAM Reader
    VAR observed : INT; END_VAR
    observed := Shared;
END_PROGRAM
"#,
        r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    TASK Slow (PRIORITY := 2);
    PROGRAM First WITH Fast : Writer;
    PROGRAM Second WITH Slow : Reader;
END_RESOURCE
END_CONFIGURATION
"#,
    ];
    let owner_hazards = diagnostics_for(&sources, 0)
        .into_iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::SharedGlobalTaskHazard)
        .collect::<Vec<_>>();

    assert_eq!(owner_hazards.len(), 1);
    assert!(diagnostics_for(&sources, 1)
        .iter()
        .all(|diagnostic| diagnostic.code != DiagnosticCode::SharedGlobalTaskHazard));
}

#[test]
fn field_index_and_dereference_targets_count_as_global_writes() {
    assert_hazard(
        r#"
TYPE Record : STRUCT value : INT; END_STRUCT; END_TYPE
CONFIGURATION Conf
VAR_GLOBAL
    Item : Record;
    Items : ARRAY[0..1] OF INT;
    SharedPtr : REF_TO INT;
END_VAR
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    TASK Slow (PRIORITY := 2);
    PROGRAM First WITH Fast : Writer;
    PROGRAM Second WITH Slow : Reader;
END_RESOURCE
END_CONFIGURATION
PROGRAM Writer
    Item.value := 1;
    Items[0] := 1;
    SharedPtr^ := 1;
END_PROGRAM
PROGRAM Reader
    VAR observed : INT; END_VAR
    observed := Item.value + Items[0] + SharedPtr^;
END_PROGRAM
"#,
    );
    assert_eq!(
        hazards(
            r#"
TYPE Record : STRUCT value : INT; END_STRUCT; END_TYPE
CONFIGURATION Conf
VAR_GLOBAL
    Item : Record;
    Items : ARRAY[0..1] OF INT;
    SharedPtr : REF_TO INT;
END_VAR
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    TASK Slow (PRIORITY := 2);
    PROGRAM First WITH Fast : Writer;
    PROGRAM Second WITH Slow : Reader;
END_RESOURCE
END_CONFIGURATION
PROGRAM Writer
    Item.value := 1;
    Items[0] := 1;
    SharedPtr^ := 1;
END_PROGRAM
PROGRAM Reader
    VAR observed : INT; END_VAR
    observed := Item.value + Items[0] + SharedPtr^;
END_PROGRAM
"#
        )
        .len(),
        3
    );
}

#[test]
fn diagnostic_is_owned_by_global_and_lists_sorted_access_and_write_tasks() {
    let source = r#"
CONFIGURATION Conf
VAR_GLOBAL Shared : INT; END_VAR
RESOURCE R ON CPU
    TASK Zulu (PRIORITY := 1);
    TASK Alpha (PRIORITY := 2);
    TASK Echo (PRIORITY := 3);
    TASK Bravo (PRIORITY := 4);
    PROGRAM P1 WITH Zulu : WriterOne;
    PROGRAM P2 WITH Alpha : WriterTwo;
    PROGRAM P3 WITH Echo : ReaderOne;
    PROGRAM P4 WITH Bravo : ReaderTwo;
END_RESOURCE
END_CONFIGURATION
PROGRAM WriterOne Shared := 1; END_PROGRAM
PROGRAM WriterTwo Shared := 2; END_PROGRAM
PROGRAM ReaderOne VAR x : INT; END_VAR x := Shared; END_PROGRAM
PROGRAM ReaderTwo VAR x : INT; END_VAR x := Shared; END_PROGRAM
"#;
    let warning = hazards(source).into_iter().next().expect("hazard warning");
    let global_start = source.find("Shared : INT").expect("global declaration") as u32;

    assert_eq!(u32::from(warning.range.start()), global_start);
    assert!(warning
        .message
        .contains("multiple tasks (Conf/R/Alpha, Conf/R/Bravo, Conf/R/Echo, +1 more)"));
    assert!(warning
        .message
        .contains("writes in (Conf/R/Alpha, Conf/R/Zulu)"));
    assert_eq!(warning.related.len(), 2);
    assert!(warning.related.iter().all(|related| {
        related.message.starts_with("TASK 'Conf/R/")
            && related.message.ends_with("' participates in shared writes")
    }));
}

#[test]
fn related_write_task_locations_are_bounded_to_three() {
    let source = r#"
CONFIGURATION Conf
VAR_GLOBAL Shared : INT; END_VAR
RESOURCE R ON CPU
    TASK One (PRIORITY := 1);
    TASK Two (PRIORITY := 2);
    TASK Three (PRIORITY := 3);
    TASK Four (PRIORITY := 4);
    PROGRAM P1 WITH One : Writer;
    PROGRAM P2 WITH Two : Writer;
    PROGRAM P3 WITH Three : Writer;
    PROGRAM P4 WITH Four : Writer;
END_RESOURCE
END_CONFIGURATION
PROGRAM Writer Shared := 1; END_PROGRAM
"#;
    let warning = hazards(source).into_iter().next().expect("hazard warning");

    assert_eq!(warning.related.len(), MAX_RELATED_TASKS);
    assert!(warning.message.contains("+1 more"));
}
