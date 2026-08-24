use super::*;

fn collect_one(text: &str) -> (ApiItem, Vec<DocDiagnostic>) {
    let (mut items, diagnostics) = collect_api_items(&[contract_source("src/contract.st", text)]);
    assert_eq!(items.len(), 1, "expected one API item: {items:#?}");
    (items.remove(0), diagnostics)
}

fn diagnostic_messages(diagnostics: &[DocDiagnostic]) -> Vec<&str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect()
}

#[test]
fn docs_discovery_accepts_every_ascii_case_source_extension() {
    let project = contract_temp_dir("extension-case");
    for (name, declaration) in [
        ("a.st", "PROGRAM A\nEND_PROGRAM\n"),
        ("b.ST", "PROGRAM B\nEND_PROGRAM\n"),
        ("c.St", "PROGRAM C\nEND_PROGRAM\n"),
        ("d.sT", "PROGRAM D\nEND_PROGRAM\n"),
        ("e.pou", "PROGRAM E\nEND_PROGRAM\n"),
        ("f.POU", "PROGRAM F\nEND_PROGRAM\n"),
        ("g.Pou", "PROGRAM G\nEND_PROGRAM\n"),
        ("h.pOu", "PROGRAM H\nEND_PROGRAM\n"),
    ] {
        contract_write(&project.join("src").join(name), declaration);
    }

    let sources = load_sources(&project, &project.join("src")).expect("load all sources");

    assert_eq!(sources.len(), 8);
    assert_eq!(
        sources
            .iter()
            .map(|source| source.path.to_string_lossy().replace('\\', "/"))
            .collect::<Vec<_>>(),
        [
            "src/a.st",
            "src/b.ST",
            "src/c.St",
            "src/d.sT",
            "src/e.pou",
            "src/f.POU",
            "src/g.Pou",
            "src/h.pOu",
        ]
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
#[cfg(not(windows))]
fn docs_discovery_is_recursive_literal_and_deterministic() {
    let project = contract_temp_dir("literal-order");
    for (name, declaration) in [
        ("z last/Ωmega.st", "PROGRAM Omega\nEND_PROGRAM\n"),
        ("a[first]/two.pou", "PROGRAM Two\nEND_PROGRAM\n"),
        ("a[first]/one.st", "PROGRAM One\nEND_PROGRAM\n"),
        ("literal*/three.ST", "PROGRAM Three\nEND_PROGRAM\n"),
    ] {
        contract_write(&project.join("src").join(name), declaration);
    }
    contract_write(
        &project.join("src/ignored.txt"),
        "PROGRAM Ignored\nEND_PROGRAM\n",
    );

    let first = load_sources(&project, &project.join("src")).expect("first source load");
    let second = load_sources(&project, &project.join("src")).expect("second source load");
    let first_paths = first
        .iter()
        .map(|source| source.path.clone())
        .collect::<Vec<_>>();
    let second_paths = second
        .iter()
        .map(|source| source.path.clone())
        .collect::<Vec<_>>();

    assert_eq!(first_paths, second_paths);
    assert_eq!(first_paths.len(), 4);
    assert!(first_paths.windows(2).all(|pair| pair[0] < pair[1]));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
#[cfg(not(windows))]
fn docs_discovery_treats_glob_metacharacters_in_root_literally() {
    let outer = contract_temp_dir("root-glob");
    let project = outer.join("project[one]*");
    contract_write(&project.join("src/main.st"), "PROGRAM Main\nEND_PROGRAM\n");

    let sources = load_sources(&project, &project.join("src"))
        .expect("literal root with glob metacharacters");

    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].path, PathBuf::from("src/main.st"));
    let _ = std::fs::remove_dir_all(outer);
}

#[test]
fn docs_discovery_rejects_matching_directory() {
    let project = contract_temp_dir("matching-directory");
    std::fs::create_dir_all(project.join("src/not-a-source.st"))
        .expect("create matching directory");

    let error = load_sources(&project, &project.join("src"))
        .expect_err("matching directory must fail discovery");

    assert!(error.to_string().contains("source"));
    let _ = std::fs::remove_dir_all(project);
}

#[cfg(unix)]
#[test]
fn docs_discovery_rejects_source_symlink() {
    use std::os::unix::fs::symlink;

    let project = contract_temp_dir("source-symlink");
    contract_write(
        &project.join("outside.st"),
        "PROGRAM Outside\nEND_PROGRAM\n",
    );
    std::fs::create_dir_all(project.join("src")).expect("create source root");
    symlink("../outside.st", project.join("src/link.st")).expect("create source symlink");

    let error = load_sources(&project, &project.join("src"))
        .expect_err("source symlink must fail discovery");

    assert!(error.to_string().to_ascii_lowercase().contains("symbolic"));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn docs_discovery_rejects_non_utf8_source_bytes() {
    let project = contract_temp_dir("non-utf8");
    contract_write(&project.join("src/main.st"), [0xff, 0xfe, 0xfd]);

    let error = load_sources(&project, &project.join("src"))
        .expect_err("non-UTF-8 source must fail discovery");

    assert!(error.to_string().contains("failed to read source"));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn docs_extraction_covers_every_supported_declaration_kind() {
    let source = contract_source(
        "src/kinds.st",
        r#"
PROGRAM ProgramItem
END_PROGRAM

TEST_PROGRAM TestProgramItem
END_PROGRAM

FUNCTION FunctionItem : INT
FunctionItem := 1;
END_FUNCTION

FUNCTION_BLOCK FunctionBlockItem
END_FUNCTION_BLOCK

TEST_FUNCTION_BLOCK TestFunctionBlockItem
END_FUNCTION_BLOCK

CLASS ClassItem
METHOD PUBLIC MethodItem : INT
MethodItem := 1;
END_METHOD
PUBLIC PROPERTY PropertyItem : INT
GET
    PropertyItem := 1;
END_GET
END_PROPERTY
END_CLASS

INTERFACE InterfaceItem
END_INTERFACE
"#,
    );

    let (items, diagnostics) = collect_api_items(&[source]);

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert_eq!(
        items.iter().map(|item| item.kind).collect::<Vec<_>>(),
        [
            ApiItemKind::Program,
            ApiItemKind::TestProgram,
            ApiItemKind::Function,
            ApiItemKind::FunctionBlock,
            ApiItemKind::TestFunctionBlock,
            ApiItemKind::Class,
            ApiItemKind::Method,
            ApiItemKind::Property,
            ApiItemKind::Interface,
        ]
    );
}

#[test]
fn docs_extraction_builds_nested_qualified_names() {
    let source = contract_source(
        "src/qualified.st",
        r#"
NAMESPACE Cell.Tools
CLASS Motor
METHOD PUBLIC Start : BOOL
Start := TRUE;
END_METHOD
PUBLIC PROPERTY Speed : INT
GET
    Speed := 1;
END_GET
END_PROPERTY
END_CLASS
END_NAMESPACE
"#,
    );

    let (items, diagnostics) = collect_api_items(&[source]);

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert_eq!(
        items
            .iter()
            .map(|item| item.qualified_name.as_str())
            .collect::<Vec<_>>(),
        [
            "Cell.Tools.Motor",
            "Cell.Tools.Motor.Start",
            "Cell.Tools.Motor.Speed"
        ]
    );
}

#[test]
fn docs_extraction_reports_only_parameter_variable_sections() {
    let (item, diagnostics) = collect_one(
        r#"
FUNCTION_BLOCK Contract
VAR_INPUT
    InputA, InputB : INT;
END_VAR
VAR_OUTPUT
    OutputA : BOOL;
END_VAR
VAR_IN_OUT
    Shared : DINT;
END_VAR
VAR
    Local : INT;
END_VAR
VAR_TEMP
    Scratch : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert_eq!(
        item.declared_params
            .iter()
            .map(SmolStr::as_str)
            .collect::<Vec<_>>(),
        ["InputA", "InputB", "OutputA", "Shared"]
    );
}

#[test]
fn docs_extraction_return_matrix_is_exact() {
    let source = contract_source(
        "src/returns.st",
        r#"
PROGRAM ProgramItem
END_PROGRAM
FUNCTION FunctionItem : INT
FunctionItem := 1;
END_FUNCTION
FUNCTION_BLOCK BlockItem
METHOD VoidMethod
END_METHOD
METHOD TypedMethod : BOOL
TypedMethod := TRUE;
END_METHOD
PROPERTY VoidProperty
END_PROPERTY
PROPERTY TypedProperty : INT
GET
    TypedProperty := 1;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
"#,
    );

    let (items, diagnostics) = collect_api_items(&[source]);

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert_eq!(
        items
            .iter()
            .map(|item| (item.qualified_name.as_str(), item.has_return))
            .collect::<Vec<_>>(),
        [
            ("ProgramItem", false),
            ("FunctionItem", true),
            ("BlockItem", false),
            ("BlockItem.VoidMethod", false),
            ("BlockItem.TypedMethod", true),
            ("BlockItem.VoidProperty", false),
            ("BlockItem.TypedProperty", true),
        ]
    );
}

#[test]
fn docs_comment_association_stops_at_blank_line() {
    let (item, diagnostics) = collect_one(
        r#"
// @brief Must not attach.

PROGRAM Main
END_PROGRAM
"#,
    );

    assert!(diagnostics.is_empty());
    assert!(item.tags.brief.is_none());
    assert!(item.tags.details.is_empty());
}

#[test]
fn docs_comment_association_stops_at_intervening_token() {
    let source = contract_source(
        "src/intervening.st",
        r#"
// @brief Belongs to First.
PROGRAM First
END_PROGRAM

// @brief Must not cross the pragma.
{attribute 'qualified_only'}
PROGRAM Second
END_PROGRAM
"#,
    );

    let (items, diagnostics) = collect_api_items(&[source]);

    assert!(diagnostics.is_empty());
    assert_eq!(items[0].tags.brief.as_deref(), Some("Belongs to First."));
    assert!(items[1].tags.brief.is_none());
}

#[test]
fn docs_comment_normalization_preserves_line_and_block_order() {
    let (item, diagnostics) = collect_one(
        r#"
// First detail.
(*
 * Second detail.
 * @brief Block brief.
 * continuation.
 *)
PROGRAM Main
END_PROGRAM
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert_eq!(item.tags.details, ["First detail.", "Second detail."]);
    assert_eq!(
        item.tags.brief.as_deref(),
        Some("Block brief. continuation.")
    );
}

#[test]
fn docs_tags_are_case_insensitive_and_continuations_are_joined() {
    let (item, diagnostics) = collect_one(
        r#"
// @BrIeF First line
// second line
// @PaRaM input Value line
// continued value
// @ReTuRn Result line
// continued result
FUNCTION Calc : INT
VAR_INPUT
    Input : INT;
END_VAR
Calc := Input;
END_FUNCTION
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert_eq!(item.tags.brief.as_deref(), Some("First line second line"));
    assert_eq!(item.tags.params.len(), 1);
    assert_eq!(item.tags.params[0].name, "input");
    assert_eq!(
        item.tags.params[0].description,
        "Value line continued value"
    );
    assert_eq!(
        item.tags.returns.as_deref(),
        Some("Result line continued result")
    );
}

#[test]
fn docs_duplicate_brief_keeps_first_and_reports_second_line() {
    let (item, diagnostics) = collect_one(
        r#"
// @brief First authority.
// @brief Conflicting authority.
PROGRAM Main
END_PROGRAM
"#,
    );

    assert_eq!(item.tags.brief.as_deref(), Some("First authority."));
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.contains("duplicate @brief"));
}

#[test]
fn docs_duplicate_return_keeps_first_and_reports_second_line() {
    let (item, diagnostics) = collect_one(
        r#"
// @return First authority.
// @return Conflicting authority.
FUNCTION Main : INT
Main := 1;
END_FUNCTION
"#,
    );

    assert_eq!(item.tags.returns.as_deref(), Some("First authority."));
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.contains("duplicate @return"));
}

#[test]
fn docs_duplicate_parameter_keeps_first_case_insensitively() {
    let (item, diagnostics) = collect_one(
        r#"
// @param Input First authority.
// @param input Conflicting authority.
FUNCTION Main : INT
VAR_INPUT
    INPUT : INT;
END_VAR
Main := INPUT;
END_FUNCTION
"#,
    );

    assert_eq!(item.tags.params.len(), 1);
    assert_eq!(item.tags.params[0].description, "First authority.");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.contains("duplicate @param"));
}

#[test]
fn docs_tag_diagnostics_use_each_physical_tag_line() {
    let (_item, diagnostics) = collect_one(
        r#"
// @brief
// detail after empty brief
// @param
// @param Missing
// @param Unknown described
// @return invalid
// @mystery value
PROGRAM Main
VAR_INPUT
    Present : INT;
END_VAR
END_PROGRAM
"#,
    );

    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        [2, 4, 5, 5, 6, 7, 8]
    );
    let messages = diagnostic_messages(&diagnostics);
    assert!(messages
        .iter()
        .any(|message| message.contains("malformed @param")));
    assert!(messages
        .iter()
        .any(|message| message.contains("missing description")));
    assert!(messages
        .iter()
        .any(|message| message.contains("does not match any declared parameter")));
    assert!(messages
        .iter()
        .any(|message| message.contains("@return used on non-returning")));
    assert!(messages
        .iter()
        .any(|message| message.contains("unknown documentation tag")));
}

#[test]
fn docs_parameter_matching_is_ascii_case_insensitive() {
    let (item, diagnostics) = collect_one(
        r#"
// @param input documented
FUNCTION Main : INT
VAR_INPUT
    INPUT : INT;
END_VAR
Main := INPUT;
END_FUNCTION
"#,
    );

    assert_eq!(item.tags.params.len(), 1);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn docs_return_on_nonreturning_declaration_is_diagnostic() {
    let (_item, diagnostics) = collect_one(
        r#"
// @return impossible
FUNCTION_BLOCK Main
END_FUNCTION_BLOCK
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("@return used on non-returning FUNCTION_BLOCK `Main`"));
}

#[test]
fn docs_untagged_lines_remain_ordered_details() {
    let (item, diagnostics) = collect_one(
        r#"
// First detail.
// Second detail.
// @brief Summary.
// Third line continues summary.
PROGRAM Main
END_PROGRAM
"#,
    );

    assert!(diagnostics.is_empty());
    assert_eq!(item.tags.details, ["First detail.", "Second detail."]);
    assert_eq!(
        item.tags.brief.as_deref(),
        Some("Summary. Third line continues summary.")
    );
}

#[test]
fn docs_items_are_sorted_by_path_line_and_name() {
    let sources = vec![
        contract_source(
            "src/z.st",
            "PROGRAM Zed\nEND_PROGRAM\nPROGRAM Alpha\nEND_PROGRAM\n",
        ),
        contract_source(
            "src/a.st",
            "\n\nPROGRAM Later\nEND_PROGRAM\nPROGRAM Last\nEND_PROGRAM\n",
        ),
    ];

    let (items, diagnostics) = collect_api_items(&sources);

    assert!(diagnostics.is_empty());
    assert_eq!(
        items
            .iter()
            .map(|item| {
                (
                    item.file.to_string_lossy().to_string(),
                    item.line,
                    item.qualified_name.to_string(),
                )
            })
            .collect::<Vec<_>>(),
        [
            ("src/a.st".to_string(), 3, "Later".to_string()),
            ("src/a.st".to_string(), 5, "Last".to_string()),
            ("src/z.st".to_string(), 1, "Zed".to_string()),
            ("src/z.st".to_string(), 3, "Alpha".to_string()),
        ]
    );
}

#[test]
fn docs_duplicate_qualified_names_remain_source_distinct() {
    let sources = vec![
        contract_source("src/a.st", "PROGRAM Main\nEND_PROGRAM\n"),
        contract_source("src/b.st", "PROGRAM Main\nEND_PROGRAM\n"),
    ];

    let (items, diagnostics) = collect_api_items(&sources);

    assert!(diagnostics.is_empty());
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].qualified_name, "Main");
    assert_eq!(items[1].qualified_name, "Main");
    assert_ne!(items[0].file, items[1].file);
}
