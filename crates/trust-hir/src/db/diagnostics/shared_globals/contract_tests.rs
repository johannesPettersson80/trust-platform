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

/// The fixtures share an `LINT`: 64-bit data, which W012 reports with a single writing task too
/// (specification 28 section 6), so these tests keep proving the task and identity resolution
/// that every case of the rule rests on.
fn two_task_source(writer_body: &str, reader_body: &str) -> String {
    format!(
        r#"
CONFIGURATION Conf
VAR_GLOBAL
    Shared : LINT;
END_VAR
RESOURCE R ON CPU
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    TASK Slow (INTERVAL := T#20ms, PRIORITY := 2);
    PROGRAM WriterInstance WITH Fast : Writer;
    PROGRAM ReaderInstance WITH Slow : Reader;
END_RESOURCE
END_CONFIGURATION

PROGRAM Writer
    VAR observed : LINT; END_VAR
    {writer_body}
END_PROGRAM

PROGRAM Reader
    VAR observed : LINT; END_VAR
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

    assert!(writes(&direct, &FxHashSet::default()));
    assert!(writes(&field, &FxHashSet::default()));
    assert!(writes(&index, &FxHashSet::default()));
    assert!(writes(&deref, &FxHashSet::default()));
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
    assert!(fields
        .iter()
        .all(|field| !writes(field, &FxHashSet::default())));
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
                    name: task.0.clone(),
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
VAR_GLOBAL Shared : LINT; END_VAR
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
VAR_GLOBAL Shared : LINT; END_VAR
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
    VAR observed : LINT; END_VAR
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
VAR_GLOBAL Shared : LINT; END_VAR
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
    VAR observed : LINT; END_VAR
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
VAR_GLOBAL Shared : LINT; END_VAR
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    PROGRAM First WITH Fast : Reader;
END_RESOURCE
END_CONFIGURATION
PROGRAM Reader
    VAR observed : LINT; END_VAR
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
        "VAR Shared : LINT; END_VAR Shared := 1;",
        "observed := Shared;",
    ));
}

#[test]
fn qualified_namespaced_global_access_is_tracked_by_resolved_identity() {
    assert_hazard(
        r#"
NAMESPACE Plant
VAR_GLOBAL Shared : LINT; END_VAR
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
    VAR observed : LINT; END_VAR
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
VAR_GLOBAL Shared : LINT; END_VAR
END_NAMESPACE
NAMESPACE B
VAR_GLOBAL Shared : LINT; END_VAR
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
    VAR observed : LINT; END_VAR
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
VAR_GLOBAL Shared : LINT; END_VAR
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    TASK Slow (PRIORITY := 2);
    PROGRAM First WITH Fast : Writer;
    PROGRAM Second WITH Slow : Reader;
END_RESOURCE
END_CONFIGURATION
PROGRAM Writer
VAR_EXTERNAL Shared : LINT; END_VAR
    Shared := 1;
END_PROGRAM
PROGRAM Reader
VAR_EXTERNAL Shared : LINT; END_VAR
VAR observed : LINT; END_VAR
    observed := Shared;
END_PROGRAM
"#,
    );
}

#[test]
fn cross_file_programs_and_configuration_contribute_to_global_owner_warning() {
    let sources = [
        "VAR_GLOBAL Shared : LINT; END_VAR",
        r#"
PROGRAM Writer
    Shared := 1;
END_PROGRAM
PROGRAM Reader
    VAR observed : LINT; END_VAR
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
    assert!(diagnostics_for(&sources, 2)
        .iter()
        .all(|diagnostic| diagnostic.code != DiagnosticCode::SharedGlobalTaskHazard));
    assert!(diagnostics_for(&sources, 1)
        .iter()
        .all(|diagnostic| diagnostic.code != DiagnosticCode::SharedGlobalTaskHazard));
}

/// A project of two programs, each with an instance `block` of a function block that writes the `INT` `Counter` in its body and in
/// its action `Reset`, and each of whose bodies is given. The first program runs in `Fast`, the second in `second_task`.
fn call_tree_source(first_body: &str, second_body: &str, second_task: &str) -> String {
    format!(
        r#"
VAR_GLOBAL Counter : INT; END_VAR
FUNCTION_BLOCK FB_Count
Counter := 1;
ACTION Reset:
Counter := 0;
END_ACTION
END_FUNCTION_BLOCK
PROGRAM First
VAR block : FB_Count; END_VAR
{first_body}
END_PROGRAM
PROGRAM Second
VAR block : FB_Count; END_VAR
{second_body}
END_PROGRAM
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    TASK Slow (PRIORITY := 2);
    PROGRAM P1 WITH Fast : First;
    PROGRAM P2 WITH {second_task} : Second;
END_RESOURCE
END_CONFIGURATION
"#
    )
}

/// Specification 28 §6: a program accesses what the POUs its calls run access, and an action's statements are part of the POU that
/// declares it. Worked by hand, with an `INT`, which a single writing task does not make a hazard, named by neither program's own
/// body: the programs of `Fast` and `Slow` call the block, its action, or one each, so two tasks write `Counter`; when both programs
/// run in `Fast`, one task does. Catches W012 missed for a global that a block or its action writes from two tasks, and W012 reported
/// for a block that one task runs.
#[test]
fn a_function_block_or_its_action_that_two_tasks_call_writes_its_global_from_both() {
    assert_hazard(&call_tree_source("block();", "block();", "Slow"));
    assert_hazard(&call_tree_source(
        "block.Reset();",
        "block.Reset();",
        "Slow",
    ));
    assert_hazard(&call_tree_source("block();", "block.Reset();", "Slow"));
    assert_no_hazard(&call_tree_source("block();", "block.Reset();", "Fast"));
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
    SharedPtr : REF_TO LINT;
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
    VAR observed : LINT; END_VAR
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
    SharedPtr : REF_TO LINT;
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
    VAR observed : LINT; END_VAR
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
VAR_GLOBAL Shared : LINT; END_VAR
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
    let global_start = source.find("Shared : LINT").expect("global declaration") as u32;

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
VAR_GLOBAL Shared : LINT; END_VAR
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

/// W012 on the global list `gvl`, in a file of its own as a CODESYS global variable list is, when
/// the task `Fast` runs `FastProgram` (`fast_body`) and the task `Slow` runs `SlowProgram`
/// (`slow_body`): the names of the globals it reports, sorted.
fn gvl_hazard_names(gvl: &str, fast_body: &str, slow_body: &str) -> Vec<String> {
    let programs = format!(
        "PROGRAM FastProgram\n{fast_body}\nEND_PROGRAM\nPROGRAM SlowProgram\n{slow_body}\nEND_PROGRAM\n"
    );
    let configuration = r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    TASK Slow (PRIORITY := 2);
    PROGRAM FastInstance WITH Fast : FastProgram;
    PROGRAM SlowInstance WITH Slow : SlowProgram;
END_RESOURCE
END_CONFIGURATION
"#;
    let mut names: Vec<String> = diagnostics_for(&[gvl, &programs, configuration], 0)
        .into_iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::SharedGlobalTaskHazard)
        .filter_map(|diagnostic| {
            let (_, rest) = diagnostic.message.split_once("shared global '")?;
            rest.split_once('\'').map(|(name, _)| name.to_owned())
        })
        .collect();
    names.sort();
    names
}

#[test]
fn an_atomic_value_that_one_task_writes_and_others_read_is_silent() {
    // A one-word read cannot tear, and how several such values stay consistent with each other is
    // the program's protocol (specification 28 section 6; CODESYS has no rule for it). A reference
    // counts as its target, here one DINT. Catches the old W012, which warned for every write that
    // another task could see.
    let names = gvl_hazard_names(
        "VAR_GLOBAL\n    Count : DINT;\n    Ready : BOOL;\n    Level : REAL;\n    Mask : DWORD;\n    Cell : REF_TO DINT;\nEND_VAR\n",
        "Count := Count + 1;\nReady := TRUE;\nLevel := 1.5;\nMask := 16#FF;\nCell^ := 1;",
        "VAR c : DINT; r : BOOL; l : REAL; m : DWORD; d : DINT; END_VAR\nc := Count;\nr := Ready;\nl := Level;\nm := Mask;\nd := Cell^;",
    );
    assert_eq!(names, Vec::<String>::new());
}

#[test]
fn non_atomic_data_that_one_task_writes_and_another_reads_warns() {
    // One access does not cover a string, an array, a structure, a function block instance or a
    // 64-bit value, so a reader can see half of a write (CODESYS SA0103). TIME is 64-bit data in
    // truST, which stores it in eight bytes (specification 02), and a reference counts as its
    // target, here a structure. Catches a single writer of non-atomic data left silent with the
    // atomic ones, a 64-bit value taken for one word, and a reference judged by its own size.
    let names = gvl_hazard_names(
        r#"
TYPE Pair : STRUCT
    a : INT;
    b : INT;
END_STRUCT
END_TYPE
VAR_GLOBAL
    Text : STRING[20];
    Table : ARRAY[0..3] OF INT;
    Point : Pair;
    Total : LINT;
    Elapsed : TIME;
    Timer : TON;
    Block : REF_TO Pair;
END_VAR
"#,
        "VAR spare : Pair; END_VAR\n\
         Text := 'a';\nTable[0] := 1;\nPoint.a := 1;\nTotal := 1;\nElapsed := T#1s;\nTimer.IN := TRUE;\nBlock^ := spare;",
        "VAR t : STRING[20]; i : INT; p : INT; n : LINT; e : TIME; q : BOOL; w : Pair; END_VAR\n\
         t := Text;\ni := Table[0];\np := Point.a;\nn := Total;\ne := Elapsed;\nq := Timer.Q;\nw := Block^;",
    );
    assert_eq!(
        names,
        ["Block", "Elapsed", "Point", "Table", "Text", "Timer", "Total"]
    );
}

#[test]
fn outputs_in_outs_loops_and_calls_write_the_global_they_reach() {
    // Specification 28 §6, the owner's rule of 7 November 2026: an output binding (`=>`), an in-out
    // argument by name or by position, a FOR loop's control variable, and a call of a global
    // instance or of its method write the global. Each task writes each global here only in that
    // way, so W012 reports a global only when both tasks write it. Catches the old rule, which
    // counted only `:=` targets, under which two tasks writing these globals stayed silent.
    let gvl = "FUNCTION Bump : BOOL\nVAR_IN_OUT\n    v : INT;\nEND_VAR\nv := v + 1;\nBump := TRUE;\nEND_FUNCTION\n\
               FUNCTION_BLOCK FB_Tank\nMETHOD PUBLIC Start\nEND_METHOD\nEND_FUNCTION_BLOCK\n\
               VAR_GLOBAL\n    Done : BOOL;\n    Named : INT;\n    Positional : INT;\n    Counter : INT;\n    Timer : TON;\n    Tank : FB_Tank;\nEND_VAR\n";
    let body = "VAR t : TON; ok : BOOL; END_VAR\n\
                t(IN := TRUE, PT := T#1s, Q => Done);\nok := Bump(v := Named);\nok := Bump(Positional);\n\
                FOR Counter := 1 TO 3 DO\n    ok := TRUE;\nEND_FOR;\nTimer(IN := TRUE, PT := T#1s);\nTank.Start();";
    assert_eq!(
        gvl_hazard_names(gvl, body, body),
        ["Counter", "Done", "Named", "Positional", "Tank", "Timer"]
    );
}

#[test]
fn input_arguments_and_indexes_are_reads() {
    // Specification 28 §6: an argument bound to an input is read, and so is an index where its
    // array is written. Catches the binder's record taken for every argument, and the old rule,
    // which climbed from the index `Slot` to the assignment and counted it written by both tasks.
    let gvl = "FUNCTION Twice : INT\nVAR_INPUT\n    a : INT;\nEND_VAR\nTwice := a * 2;\nEND_FUNCTION\n\
               VAR_GLOBAL\n    Level : INT;\n    Slot : INT;\n    Table : ARRAY[0..3] OF INT;\nEND_VAR\n";
    let body =
        "VAR n : INT; END_VAR\nn := Twice(a := Level);\nn := Twice(Level);\nTable[Slot] := n;";
    assert_eq!(gvl_hazard_names(gvl, body, body), ["Table"]);
}

#[test]
fn several_writing_tasks_warn_even_for_an_atomic_value() {
    // Two tasks writing one global race whatever its size (CODESYS SA0006). Catches the atomic
    // exemption applied to more than one writer.
    let names = gvl_hazard_names(
        "VAR_GLOBAL\n    Count : DINT;\nEND_VAR\n",
        "Count := 1;",
        "Count := 2;",
    );
    assert_eq!(names, ["Count"]);
}

#[test]
fn cross_task_before_a_global_list_covers_every_global_of_it() {
    // `{attribute 'cross_task'}` before `VAR_GLOBAL` declares the whole list shared by design: its
    // non-atomic single-written global and its global with two writers are both silent. Catches
    // the attribute unread before the list keyword.
    let names = gvl_hazard_names(
        "{attribute 'cross_task'}\nVAR_GLOBAL\n    Total : LINT;\n    Count : DINT;\nEND_VAR\n",
        "Total := 1;\nCount := 1;",
        "VAR n : LINT; END_VAR\nn := Total;\nCount := 2;",
    );
    assert_eq!(names, Vec::<String>::new());
}

#[test]
fn cross_task_before_one_global_covers_only_that_global() {
    // Before a single global the attribute declares that global alone: a hand-off flag that both
    // tasks write is silent, while the undeclared globals beside it keep warning, one with two
    // writers and one non-atomic. Catches a variable's attribute read as the list's, and an
    // undeclared global silenced by its neighbour.
    let names = gvl_hazard_names(
        "VAR_GLOBAL\n    {attribute 'cross_task'}\n    Handoff : BOOL;\n    Pending : BOOL;\n    Total : LINT;\nEND_VAR\n",
        "Handoff := TRUE;\nPending := TRUE;\nTotal := 1;",
        "VAR n : LINT; END_VAR\nHandoff := FALSE;\nPending := FALSE;\nn := Total;",
    );
    assert_eq!(names, ["Pending", "Total"]);
}

#[test]
fn cross_task_before_a_configuration_global_list_covers_it() {
    // The global list of a configuration takes the attribute the same way. Catches a check that
    // finds the attribute only on a list at file level.
    assert_no_hazard(
        r#"
CONFIGURATION Conf
{attribute 'cross_task'}
VAR_GLOBAL
    Shared : LINT;
END_VAR
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    TASK Slow (PRIORITY := 2);
    PROGRAM WriterInstance WITH Fast : Writer;
    PROGRAM ReaderInstance WITH Slow : Reader;
END_RESOURCE
END_CONFIGURATION
PROGRAM Writer
    Shared := 1;
END_PROGRAM
PROGRAM Reader
    VAR observed : LINT; END_VAR
    observed := Shared;
END_PROGRAM
"#,
    );
}
