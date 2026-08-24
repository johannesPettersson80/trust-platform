use super::*;

fn project_with_source(prefix: &str, source: &str) -> PathBuf {
    let project = contract_temp_dir(prefix);
    contract_write(&project.join("src/main.st"), source);
    project
}

fn output_snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, path: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut entries = std::fs::read_dir(path)
            .expect("read output snapshot")
            .map(|entry| entry.expect("read output entry"))
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("output below root")
                .to_string_lossy()
                .replace('\\', "/");
            let metadata = std::fs::symlink_metadata(&path).expect("output metadata");
            if metadata.file_type().is_symlink() {
                let mut value = b"symlink:".to_vec();
                value.extend_from_slice(
                    std::fs::read_link(&path)
                        .expect("read output symlink")
                        .to_string_lossy()
                        .as_bytes(),
                );
                out.push((relative, value));
            } else if metadata.is_dir() {
                out.push((relative, b"directory".to_vec()));
                walk(root, &path, out);
            } else {
                let mut value = b"file:".to_vec();
                value.extend_from_slice(&std::fs::read(&path).expect("read output file"));
                out.push((relative, value));
            }
        }
    }

    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

#[test]
fn docs_markdown_renders_diagnostics_and_complete_item_metadata() {
    let item = ApiItem {
        kind: ApiItemKind::Function,
        qualified_name: "Cell.Calc".into(),
        file: PathBuf::from("src/calc.st"),
        line: 9,
        tags: ApiDocTags {
            brief: Some("Compute the value.".to_string()),
            details: vec!["First detail.".to_string(), "Second detail.".to_string()],
            params: vec![ApiParamDoc {
                name: "Input".into(),
                description: "Source value.".to_string(),
            }],
            returns: Some("Computed result.".to_string()),
        },
        declared_params: vec!["Input".into(), "Output".into()],
        has_return: true,
    };
    let diagnostic = DocDiagnostic {
        file: PathBuf::from("src/calc.st"),
        line: 2,
        message: "sample diagnostic".to_string(),
    };

    let markdown = render_markdown(&[item], &[diagnostic]);

    for expected in [
        "# ST API Documentation",
        "## Diagnostics",
        "`src/calc.st`:2 sample diagnostic",
        "### FUNCTION `Cell.Calc`",
        "- Source: `src/calc.st`:9",
        "- Return Value: yes",
        "- Declared Parameters: `Input, Output`",
        "**Brief**: Compute the value.",
        "- `Input`: Source value.",
        "**Returns**: Computed result.",
        "- First detail.",
        "- Second detail.",
    ] {
        assert!(
            markdown.contains(expected),
            "missing {expected:?}\n{markdown}"
        );
    }
}

#[test]
fn docs_html_escapes_every_user_controlled_surface() {
    let hostile = "& < > \" '";
    let item = ApiItem {
        kind: ApiItemKind::Function,
        qualified_name: hostile.into(),
        file: PathBuf::from("src/&<>\"'.st"),
        line: 1,
        tags: ApiDocTags {
            brief: Some(hostile.to_string()),
            details: vec![hostile.to_string()],
            params: vec![ApiParamDoc {
                name: hostile.into(),
                description: hostile.to_string(),
            }],
            returns: Some(hostile.to_string()),
        },
        declared_params: vec![hostile.into()],
        has_return: true,
    };
    let diagnostic = DocDiagnostic {
        file: PathBuf::from("src/&<>\"'.st"),
        line: 1,
        message: hostile.to_string(),
    };

    let html = render_html(&[item], &[diagnostic]);

    assert!(!html.contains(hostile));
    assert!(html.contains("&amp; &lt; &gt; &quot; &#39;"));
    assert!(!html.contains("<script>"));
}

#[test]
fn docs_markdown_preserves_authored_description_markup() {
    let item = ApiItem {
        kind: ApiItemKind::Program,
        qualified_name: "Main".into(),
        file: PathBuf::from("src/main.st"),
        line: 1,
        tags: ApiDocTags {
            brief: Some("Use **bold** and [`link`](https://example.invalid).".to_string()),
            details: vec!["Inline `code` remains authored markup.".to_string()],
            ..ApiDocTags::default()
        },
        declared_params: Vec::new(),
        has_return: false,
    };

    let markdown = render_markdown(&[item], &[]);

    assert!(markdown.contains("Use **bold** and [`link`](https://example.invalid)."));
    assert!(markdown.contains("Inline `code` remains authored markup."));
}

#[test]
fn docs_command_no_sources_fails_without_output() {
    let project = contract_temp_dir("no-sources");
    std::fs::create_dir_all(project.join("src")).expect("create empty source root");
    let output = project.join("generated");

    let error = run_docs(
        Some(project.clone()),
        Some(output.clone()),
        DocsFormat::Both,
    )
    .expect_err("empty source project must fail");

    assert!(error.to_string().contains("no ST sources"));
    assert!(!output.exists());
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn docs_command_parser_error_fails_before_output_mutation() {
    let project = project_with_source(
        "parse-error",
        "PROGRAM Main\nVAR\n  Broken : ;\nEND_VAR\nEND_PROGRAM\n",
    );
    let output = project.join("generated");
    std::fs::create_dir_all(&output).expect("create output");
    contract_write(&output.join("api.md"), "old markdown");
    contract_write(&output.join("api.html"), "old html");
    let before = output_snapshot(&output);

    let error = run_docs(
        Some(project.clone()),
        Some(output.clone()),
        DocsFormat::Both,
    )
    .expect_err("parse diagnostics must fail generation");

    assert!(error.to_string().to_ascii_lowercase().contains("parse"));
    assert_eq!(output_snapshot(&output), before);
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn docs_command_tag_diagnostics_are_embedded_but_nonfatal() {
    let project = project_with_source(
        "tag-diagnostics",
        r#"
// @brief
// @unknown value
PROGRAM Main
END_PROGRAM
"#,
    );
    let output = project.join("generated");

    run_docs(
        Some(project.clone()),
        Some(output.clone()),
        DocsFormat::Both,
    )
    .expect("tag diagnostics are nonfatal");

    let markdown = std::fs::read_to_string(output.join("api.md")).expect("read markdown");
    let html = std::fs::read_to_string(output.join("api.html")).expect("read html");
    for document in [&markdown, &html] {
        assert!(document.contains("Diagnostics"));
        assert!(document.contains("missing description for @brief"));
        assert!(document.contains("unknown documentation tag"));
    }
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn docs_command_markdown_format_writes_only_markdown() {
    let project = project_with_source("markdown-only", "PROGRAM Main\nEND_PROGRAM\n");
    let output = project.join("generated");
    std::fs::create_dir_all(&output).expect("create output");
    contract_write(&output.join("api.html"), "keep html");

    run_docs(
        Some(project.clone()),
        Some(output.clone()),
        DocsFormat::Markdown,
    )
    .expect("generate markdown");

    assert!(output.join("api.md").is_file());
    assert_eq!(
        std::fs::read_to_string(output.join("api.html")).expect("read unselected html"),
        "keep html"
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn docs_command_html_format_writes_only_html() {
    let project = project_with_source("html-only", "PROGRAM Main\nEND_PROGRAM\n");
    let output = project.join("generated");
    std::fs::create_dir_all(&output).expect("create output");
    contract_write(&output.join("api.md"), "keep markdown");

    run_docs(
        Some(project.clone()),
        Some(output.clone()),
        DocsFormat::Html,
    )
    .expect("generate HTML");

    assert!(output.join("api.html").is_file());
    assert_eq!(
        std::fs::read_to_string(output.join("api.md")).expect("read unselected markdown"),
        "keep markdown"
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn docs_command_both_rolls_back_when_second_artifact_cannot_publish() {
    let project = project_with_source("both-rollback", "PROGRAM Main\nEND_PROGRAM\n");
    let output = project.join("generated");
    std::fs::create_dir_all(&output).expect("create output");
    contract_write(&output.join("api.md"), "old markdown");
    std::fs::create_dir_all(output.join("api.html")).expect("create HTML collision");
    contract_write(&output.join("api.html/keep.txt"), "keep directory");
    let before = output_snapshot(&output);

    run_docs(
        Some(project.clone()),
        Some(output.clone()),
        DocsFormat::Both,
    )
    .expect_err("HTML collision must fail complete publication");

    assert_eq!(output_snapshot(&output), before);
    let _ = std::fs::remove_dir_all(project);
}

#[cfg(unix)]
#[test]
fn docs_command_rejects_output_symlink_without_following() {
    use std::os::unix::fs::symlink;

    let project = project_with_source("output-symlink", "PROGRAM Main\nEND_PROGRAM\n");
    let output = project.join("generated");
    std::fs::create_dir_all(&output).expect("create output");
    let outside = project.join("outside.md");
    contract_write(&outside, "outside sentinel");
    symlink("../outside.md", output.join("api.md")).expect("create output symlink");

    let result = run_docs(
        Some(project.clone()),
        Some(output.clone()),
        DocsFormat::Markdown,
    );

    assert_eq!(
        std::fs::read_to_string(&outside).expect("read outside output"),
        "outside sentinel"
    );
    let error = result.expect_err("output symlink must fail");
    assert!(error.to_string().to_ascii_lowercase().contains("symbolic"));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn docs_command_valid_empty_api_index_is_deterministic() {
    let project = project_with_source("empty-api", "CONFIGURATION Plant\nEND_CONFIGURATION\n");
    let first = project.join("first");
    let second = project.join("second");

    run_docs(Some(project.clone()), Some(first.clone()), DocsFormat::Both)
        .expect("first empty API generation");
    run_docs(
        Some(project.clone()),
        Some(second.clone()),
        DocsFormat::Both,
    )
    .expect("second empty API generation");

    assert_eq!(
        std::fs::read(first.join("api.md")).expect("read first markdown"),
        std::fs::read(second.join("api.md")).expect("read second markdown")
    );
    assert_eq!(
        std::fs::read(first.join("api.html")).expect("read first html"),
        std::fs::read(second.join("api.html")).expect("read second html")
    );
    let markdown = std::fs::read_to_string(first.join("api.md")).expect("read empty index");
    assert!(markdown.contains("## API"));
    assert!(!markdown.contains("### PROGRAM"));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn docs_command_successful_both_publishes_exact_complete_set() {
    let project = project_with_source("complete-set", "PROGRAM Main\nEND_PROGRAM\n");
    let output = project.join("deep/generated");

    run_docs(
        Some(project.clone()),
        Some(output.clone()),
        DocsFormat::Both,
    )
    .expect("generate both documents");

    assert_eq!(
        output_snapshot(&output)
            .iter()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>(),
        ["api.html", "api.md"]
    );
    assert!(std::fs::read_to_string(output.join("api.md"))
        .expect("read markdown")
        .contains("### PROGRAM `Main`"));
    assert!(std::fs::read_to_string(output.join("api.html"))
        .expect("read HTML")
        .contains("PROGRAM <code>Main</code>"));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn docs_command_repeated_generation_is_byte_deterministic() {
    let project = project_with_source(
        "repeat-determinism",
        r#"
// @brief Main documentation.
PROGRAM Main
END_PROGRAM
"#,
    );
    let output = project.join("generated");

    run_docs(
        Some(project.clone()),
        Some(output.clone()),
        DocsFormat::Both,
    )
    .expect("first generation");
    let first_markdown = std::fs::read(output.join("api.md")).expect("read first markdown");
    let first_html = std::fs::read(output.join("api.html")).expect("read first html");
    run_docs(
        Some(project.clone()),
        Some(output.clone()),
        DocsFormat::Both,
    )
    .expect("second generation");

    assert_eq!(
        std::fs::read(output.join("api.md")).expect("read second markdown"),
        first_markdown
    );
    assert_eq!(
        std::fs::read(output.join("api.html")).expect("read second html"),
        first_html
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn docs_command_output_root_file_fails_without_changing_it() {
    let project = project_with_source("output-root-file", "PROGRAM Main\nEND_PROGRAM\n");
    let output = project.join("generated");
    contract_write(&output, "output root sentinel");

    run_docs(
        Some(project.clone()),
        Some(output.clone()),
        DocsFormat::Both,
    )
    .expect_err("ordinary file cannot be output directory");

    assert_eq!(
        std::fs::read_to_string(&output).expect("read output root"),
        "output root sentinel"
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn docs_command_explicit_missing_project_does_not_fallback() {
    let outer = contract_temp_dir("missing-project");
    let missing = outer.join("missing");
    let output = outer.join("generated");

    run_docs(Some(missing), Some(output.clone()), DocsFormat::Both)
        .expect_err("missing explicit project must fail");

    assert!(!output.exists());
    let _ = std::fs::remove_dir_all(outer);
}

#[test]
fn docs_command_uses_project_relative_source_identity() {
    let project = contract_temp_dir("relative-source");
    contract_write(
        &project.join("src/nested/cell.st"),
        "PROGRAM Cell\nEND_PROGRAM\n",
    );
    let output = project.join("generated");

    run_docs(
        Some(project.clone()),
        Some(output.clone()),
        DocsFormat::Markdown,
    )
    .expect("generate documentation");

    let markdown = std::fs::read_to_string(output.join("api.md")).expect("read markdown");
    let portable_markdown = markdown.replace('\\', "/");
    assert!(portable_markdown.contains("- Source: `src/nested/cell.st`:1"));
    assert!(!markdown.contains(&project.to_string_lossy().to_string()));
    let _ = std::fs::remove_dir_all(project);
}
