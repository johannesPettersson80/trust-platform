//! ST test runner command.

use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::json;
use smol_str::SmolStr;
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::error::RuntimeError;
use trust_runtime::eval::call_function_block;
use trust_runtime::harness::{CompileSession, SourceFile as HarnessSourceFile};
use trust_runtime::instance::create_fb_instance;
use trust_runtime::Runtime;
use trust_syntax::parser;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::cli::TestOutput;
use crate::style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestKind {
    Program,
    FunctionBlock,
}

impl TestKind {
    fn label(self) -> &'static str {
        match self {
            Self::Program => "TEST_PROGRAM",
            Self::FunctionBlock => "TEST_FUNCTION_BLOCK",
        }
    }
}

#[derive(Debug, Clone)]
struct LoadedSource {
    path: PathBuf,
    text: String,
}

#[derive(Debug, Clone)]
struct DiscoveredTest {
    kind: TestKind,
    name: SmolStr,
    file: PathBuf,
    byte_offset: u32,
    line: usize,
    source_line: Option<String>,
}

#[derive(Debug, Default, Clone, Copy)]
struct TestSummary {
    passed: usize,
    failed: usize,
    errors: usize,
}

impl TestSummary {
    fn total(self) -> usize {
        self.passed + self.failed + self.errors
    }

    fn has_failures(self) -> bool {
        self.failed > 0 || self.errors > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestOutcome {
    Passed,
    Failed,
    Error,
}

impl TestOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone)]
struct ExecutedTest {
    case: DiscoveredTest,
    outcome: TestOutcome,
    message: Option<String>,
}

pub fn run_test(
    project: Option<PathBuf>,
    filter: Option<String>,
    output: TestOutput,
    ci: bool,
) -> anyhow::Result<()> {
    let output = effective_output(output, ci);
    let project_root = match project {
        Some(path) => path,
        None => match detect_bundle_path(None) {
            Ok(path) => path,
            Err(_) => std::env::current_dir().context("failed to resolve current directory")?,
        },
    };
    let sources_root = project_root.join("sources");
    if !sources_root.is_dir() {
        anyhow::bail!(
            "invalid project folder '{}': missing sources/ directory",
            project_root.display()
        );
    }

    let sources = load_sources(&sources_root)?;
    if sources.is_empty() {
        anyhow::bail!("no ST sources found under {}", sources_root.display());
    }

    let mut tests = discover_tests(&sources);
    if let Some(filter) = filter.as_deref() {
        let needle = filter.to_ascii_lowercase();
        tests.retain(|case| case.name.as_str().to_ascii_lowercase().contains(&needle));
    }

    if tests.is_empty() {
        let rendered = render_output(output, &project_root, &[], TestSummary::default())?;
        print!("{rendered}");
        return Ok(());
    }

    let compile_sources = sources
        .iter()
        .map(|source| {
            HarnessSourceFile::with_path(
                source.path.to_string_lossy().into_owned(),
                source.text.clone(),
            )
        })
        .collect::<Vec<_>>();
    let session = CompileSession::from_sources(compile_sources);
    let _ = session.build_runtime()?;

    let mut results = Vec::with_capacity(tests.len());
    for case in &tests {
        let result = match execute_test_case(&session, case) {
            Ok(()) => ExecutedTest {
                case: case.clone(),
                outcome: TestOutcome::Passed,
                message: None,
            },
            Err(RuntimeError::AssertionFailed(message)) => ExecutedTest {
                case: case.clone(),
                outcome: TestOutcome::Failed,
                message: Some(message.to_string()),
            },
            Err(err) => ExecutedTest {
                case: case.clone(),
                outcome: TestOutcome::Error,
                message: Some(err.to_string()),
            },
        };
        results.push(result);
    }

    let summary = summarize_results(&results);
    let rendered = render_output(output, &project_root, &results, summary)?;
    print!("{rendered}");

    if summary.has_failures() {
        anyhow::bail!("{} ST test(s) failed", summary.failed + summary.errors);
    }

    Ok(())
}

fn summarize_results(results: &[ExecutedTest]) -> TestSummary {
    let mut summary = TestSummary::default();
    for result in results {
        match result.outcome {
            TestOutcome::Passed => summary.passed += 1,
            TestOutcome::Failed => summary.failed += 1,
            TestOutcome::Error => summary.errors += 1,
        }
    }
    summary
}

fn render_output(
    output: TestOutput,
    project_root: &Path,
    results: &[ExecutedTest],
    summary: TestSummary,
) -> anyhow::Result<String> {
    match output {
        TestOutput::Human => Ok(render_human_output(project_root, results, summary)),
        TestOutput::Json => render_json_output(project_root, results, summary),
        TestOutput::Tap => Ok(render_tap_output(results)),
        TestOutput::Junit => Ok(render_junit_output(results, summary)),
    }
}

fn effective_output(output: TestOutput, ci: bool) -> TestOutput {
    if ci && matches!(output, TestOutput::Human) {
        TestOutput::Junit
    } else {
        output
    }
}

fn render_human_output(
    project_root: &Path,
    results: &[ExecutedTest],
    summary: TestSummary,
) -> String {
    let mut output = String::new();
    let mut failed_results = Vec::new();
    let _ = writeln!(
        output,
        "{}",
        style::accent(format!(
            "Running {} ST test(s) in {}",
            summary.total(),
            project_root.display()
        ))
    );
    if results.is_empty() {
        let _ = writeln!(output, "{}", style::warning("No ST tests discovered."));
    }
    for (idx, result) in results.iter().enumerate() {
        let prefix = format!("[{}/{}]", idx + 1, results.len());
        let test_id = format!("{}::{}", result.case.kind.label(), result.case.name);
        match result.outcome {
            TestOutcome::Passed => {
                let _ = writeln!(
                    output,
                    "{} {} {} ({})",
                    style::success("PASS"),
                    prefix,
                    test_id,
                    result.case.file.display()
                );
            }
            TestOutcome::Failed => {
                let _ = writeln!(
                    output,
                    "{} {} {} {}:{}",
                    style::error("FAIL"),
                    prefix,
                    test_id,
                    result.case.file.display(),
                    result.case.line
                );
                let _ = writeln!(
                    output,
                    "    reason   : {}",
                    result.message.as_deref().unwrap_or("assertion failed")
                );
                if let Some(source_line) = result.case.source_line.as_deref() {
                    let _ = writeln!(output, "    source   : {source_line}");
                }
                failed_results.push(result);
            }
            TestOutcome::Error => {
                let _ = writeln!(
                    output,
                    "{} {} {} {}:{}",
                    style::error("ERROR"),
                    prefix,
                    test_id,
                    result.case.file.display(),
                    result.case.line
                );
                let _ = writeln!(
                    output,
                    "    reason   : {}",
                    result.message.as_deref().unwrap_or("runtime error")
                );
                if let Some(source_line) = result.case.source_line.as_deref() {
                    let _ = writeln!(output, "    source   : {source_line}");
                }
                failed_results.push(result);
            }
        }
    }
    if !failed_results.is_empty() {
        let _ = writeln!(output);
        let _ = writeln!(output, "{}", style::warning("Failure summary:"));
        for (idx, result) in failed_results.iter().enumerate() {
            let _ = writeln!(
                output,
                "  {}. {}::{} @ {}:{}",
                idx + 1,
                result.case.kind.label(),
                result.case.name,
                result.case.file.display(),
                result.case.line
            );
            let _ = writeln!(
                output,
                "     {}",
                result.message.as_deref().unwrap_or(match result.outcome {
                    TestOutcome::Failed => "assertion failed",
                    TestOutcome::Error => "runtime error",
                    TestOutcome::Passed => "passed",
                })
            );
            if let Some(source_line) = result.case.source_line.as_deref() {
                let _ = writeln!(output, "     source: {source_line}");
            }
        }
    }
    let _ = writeln!(
        output,
        "{} passed, {} failed, {} errors",
        summary.passed, summary.failed, summary.errors
    );
    output
}

fn render_json_output(
    project_root: &Path,
    results: &[ExecutedTest],
    summary: TestSummary,
) -> anyhow::Result<String> {
    let tests = results
        .iter()
        .map(|result| {
            json!({
                "name": result.case.name.as_str(),
                "kind": result.case.kind.label(),
                "status": result.outcome.as_str(),
                "file": result.case.file.display().to_string(),
                "line": result.case.line,
                "source": result.case.source_line.as_deref(),
                "message": result.message.as_deref(),
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "version": 1,
        "project": project_root.display().to_string(),
        "summary": {
            "total": summary.total(),
            "passed": summary.passed,
            "failed": summary.failed,
            "errors": summary.errors,
        },
        "tests": tests,
    });
    let mut text = serde_json::to_string_pretty(&payload)?;
    text.push('\n');
    Ok(text)
}

fn render_tap_output(results: &[ExecutedTest]) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "TAP version 13");
    let _ = writeln!(output, "1..{}", results.len());
    for (idx, result) in results.iter().enumerate() {
        let name = tap_escape(&format!(
            "{}::{}",
            result.case.kind.label(),
            result.case.name
        ));
        match result.outcome {
            TestOutcome::Passed => {
                let _ = writeln!(output, "ok {} - {}", idx + 1, name);
            }
            TestOutcome::Failed | TestOutcome::Error => {
                let _ = writeln!(output, "not ok {} - {}", idx + 1, name);
                let _ = writeln!(output, "# file: {}", result.case.file.display());
                let _ = writeln!(output, "# line: {}", result.case.line);
                if let Some(source_line) = result.case.source_line.as_deref() {
                    let _ = writeln!(output, "# source: {}", tap_escape(source_line));
                }
                if let Some(message) = &result.message {
                    for line in message.lines() {
                        let _ = writeln!(output, "# {}", tap_escape(line));
                    }
                }
            }
        }
    }
    output
}

fn render_junit_output(results: &[ExecutedTest], summary: TestSummary) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(
        output,
        "<testsuite name=\"trust-runtime\" tests=\"{}\" failures=\"{}\" errors=\"{}\" skipped=\"0\">",
        summary.total(),
        summary.failed,
        summary.errors
    );
    for result in results {
        let name = xml_escape(&format!(
            "{}::{}",
            result.case.kind.label(),
            result.case.name
        ));
        let file = xml_escape(&result.case.file.display().to_string());
        let _ = writeln!(
            output,
            "  <testcase name=\"{}\" classname=\"st\" file=\"{}\" line=\"{}\">",
            name, file, result.case.line
        );
        match result.outcome {
            TestOutcome::Passed => {}
            TestOutcome::Failed => {
                let message_text = result.message.as_deref().unwrap_or("assertion failed");
                let message = xml_escape(message_text);
                let mut details = String::from(message_text);
                if let Some(source_line) = result.case.source_line.as_deref() {
                    let _ = write!(details, "\nsource: {source_line}");
                }
                let details = xml_escape(&details);
                let _ = writeln!(
                    output,
                    "    <failure message=\"{}\">{}</failure>",
                    message, details
                );
            }
            TestOutcome::Error => {
                let message_text = result.message.as_deref().unwrap_or("runtime error");
                let message = xml_escape(message_text);
                let mut details = String::from(message_text);
                if let Some(source_line) = result.case.source_line.as_deref() {
                    let _ = write!(details, "\nsource: {source_line}");
                }
                let details = xml_escape(&details);
                let _ = writeln!(
                    output,
                    "    <error message=\"{}\">{}</error>",
                    message, details
                );
            }
        }
        let _ = writeln!(output, "  </testcase>");
    }
    let _ = writeln!(output, "</testsuite>");
    output
}

fn tap_escape(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
}

fn xml_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn execute_test_case(session: &CompileSession, case: &DiscoveredTest) -> Result<(), RuntimeError> {
    let mut runtime = session
        .build_runtime()
        .map_err(|err| RuntimeError::ControlError(err.to_string().into()))?;
    match case.kind {
        TestKind::Program => execute_test_program(&mut runtime, case.name.as_str()),
        TestKind::FunctionBlock => execute_test_function_block(&mut runtime, case.name.as_str()),
    }
}

fn execute_test_program(runtime: &mut Runtime, name: &str) -> Result<(), RuntimeError> {
    let program = runtime
        .programs()
        .values()
        .find(|program| program.name.eq_ignore_ascii_case(name))
        .cloned()
        .ok_or_else(|| RuntimeError::UndefinedProgram(name.into()))?;
    runtime.execute_program(&program)
}

fn execute_test_function_block(runtime: &mut Runtime, name: &str) -> Result<(), RuntimeError> {
    runtime.with_eval_context(None, None, |ctx| {
        let function_blocks = ctx.function_blocks.ok_or(RuntimeError::TypeMismatch)?;
        let functions = ctx.functions.ok_or(RuntimeError::TypeMismatch)?;
        let stdlib = ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?;
        let classes = ctx.classes.ok_or(RuntimeError::TypeMismatch)?;

        let key = SmolStr::new(name.to_ascii_uppercase());
        let fb = function_blocks
            .get(&key)
            .ok_or_else(|| RuntimeError::UndefinedFunctionBlock(name.into()))?;
        let instance_id = create_fb_instance(
            ctx.storage,
            ctx.registry,
            &ctx.profile,
            classes,
            function_blocks,
            functions,
            stdlib,
            fb,
        )?;
        call_function_block(ctx, fb, instance_id, &[])
    })
}

fn load_sources(root: &Path) -> anyhow::Result<Vec<LoadedSource>> {
    let mut paths = BTreeSet::new();
    let patterns = ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"];
    for pattern in patterns {
        for entry in glob::glob(&format!("{}/{}", root.display(), pattern))
            .with_context(|| format!("invalid glob pattern for '{}'", root.display()))?
        {
            paths.insert(entry?);
        }
    }

    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read source '{}'", path.display()))?;
        sources.push(LoadedSource { path, text });
    }
    Ok(sources)
}

fn discover_tests(sources: &[LoadedSource]) -> Vec<DiscoveredTest> {
    let mut tests = Vec::new();
    for source in sources {
        let parse = parser::parse(&source.text);
        let syntax = parse.syntax();
        for node in syntax.descendants() {
            let kind = match node.kind() {
                SyntaxKind::Program | SyntaxKind::FunctionBlock => test_kind_for_node(&node),
                _ => None,
            };
            let Some(kind) = kind else {
                continue;
            };
            let Some(name) = qualified_pou_name(&node) else {
                continue;
            };
            let byte_offset = node
                .children_with_tokens()
                .filter_map(|element| element.into_token())
                .find(|token| !token.kind().is_trivia())
                .map(|token| u32::from(token.text_range().start()))
                .unwrap_or_else(|| u32::from(node.text_range().start()));
            let line = line_for_offset(&source.text, byte_offset as usize);
            tests.push(DiscoveredTest {
                kind,
                name,
                file: source.path.clone(),
                byte_offset,
                line,
                source_line: source_line_for_offset(&source.text, byte_offset as usize),
            });
        }
    }
    tests.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.byte_offset.cmp(&right.byte_offset))
            .then(left.name.cmp(&right.name))
    });
    tests
}

fn test_kind_for_node(node: &SyntaxNode) -> Option<TestKind> {
    let first_token = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())?;
    match first_token.kind() {
        SyntaxKind::KwTestProgram => Some(TestKind::Program),
        SyntaxKind::KwTestFunctionBlock => Some(TestKind::FunctionBlock),
        _ => None,
    }
}

fn qualified_pou_name(node: &SyntaxNode) -> Option<SmolStr> {
    let mut parts = Vec::new();
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    parts.push(name_part_from_name_node(&name_node)?);

    for ancestor in node.ancestors() {
        if ancestor.kind() != SyntaxKind::Namespace {
            continue;
        }
        if let Some(ns_name) = ancestor
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)
            .and_then(|name_node| name_part_from_name_node(&name_node))
        {
            parts.push(ns_name);
        }
    }

    parts.reverse();
    Some(parts.join(".").into())
}

fn name_part_from_name_node(node: &SyntaxNode) -> Option<String> {
    let text = first_ident_token(node)?.text().trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn first_ident_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| {
            matches!(
                token.kind(),
                SyntaxKind::Ident | SyntaxKind::KwEn | SyntaxKind::KwEno
            )
        })
}

fn line_for_offset(text: &str, byte_offset: usize) -> usize {
    let offset = byte_offset.min(text.len());
    text[..offset].bytes().filter(|byte| *byte == b'\n').count() + 1
}

fn source_line_for_offset(text: &str, byte_offset: usize) -> Option<String> {
    let offset = byte_offset.min(text.len());
    let line_start = text[..offset].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let line_end = text[offset..]
        .find('\n')
        .map(|rel| offset + rel)
        .unwrap_or(text.len());
    let line = text[line_start..line_end].trim();
    if line.is_empty() {
        None
    } else {
        Some(line.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip_ansi(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch != '\u{1b}' {
                out.push(ch);
                continue;
            }

            if chars.next_if_eq(&'[').is_none() {
                continue;
            }

            for control in chars.by_ref() {
                if control.is_ascii_alphabetic() {
                    break;
                }
            }
        }
        out
    }

    #[test]
    fn discovery_finds_test_pous_with_namespace_qualification() {
        let sources = vec![
            LoadedSource {
                path: PathBuf::from("b.st"),
                text: r#"
TEST_PROGRAM Plain
END_TEST_PROGRAM
"#
                .to_string(),
            },
            LoadedSource {
                path: PathBuf::from("a.st"),
                text: r#"
NAMESPACE NS.Core
TEST_FUNCTION_BLOCK CaseOne
END_TEST_FUNCTION_BLOCK
END_NAMESPACE
"#
                .to_string(),
            },
        ];

        let discovered = discover_tests(&sources);
        assert_eq!(discovered.len(), 2);
        assert_eq!(discovered[0].name, "CaseOne");
        assert_eq!(discovered[0].kind, TestKind::FunctionBlock);
        assert_eq!(
            discovered[0].source_line.as_deref(),
            Some("TEST_FUNCTION_BLOCK CaseOne")
        );
        assert_eq!(discovered[1].name, "Plain");
        assert_eq!(discovered[1].kind, TestKind::Program);
        assert_eq!(
            discovered[1].source_line.as_deref(),
            Some("TEST_PROGRAM Plain")
        );
    }

    #[test]
    fn discovery_ignores_comments_after_test_name() {
        let sources = vec![LoadedSource {
            path: PathBuf::from("comments.st"),
            text: r#"
TEST_PROGRAM InlineComment (* inline comment *)
END_TEST_PROGRAM

TEST_PROGRAM NextLineComment
(* line comment right after declaration *)
END_TEST_PROGRAM
"#
            .to_string(),
        }];

        let discovered = discover_tests(&sources);
        assert_eq!(discovered.len(), 2);
        assert_eq!(discovered[0].name, "InlineComment");
        assert_eq!(discovered[1].name, "NextLineComment");
    }

    #[test]
    fn execution_reports_assertion_failure_for_test_program() {
        let sources = vec![LoadedSource {
            path: PathBuf::from("tests.st"),
            text: r#"
TEST_PROGRAM FailCase
ASSERT_TRUE(FALSE);
END_TEST_PROGRAM
"#
            .to_string(),
        }];
        let tests = discover_tests(&sources);
        assert_eq!(tests.len(), 1);

        let session = CompileSession::from_sources(vec![HarnessSourceFile::with_path(
            "tests.st",
            sources[0].text.clone(),
        )]);
        let err = execute_test_case(&session, &tests[0]).unwrap_err();
        assert!(matches!(err, RuntimeError::AssertionFailed(_)));
    }

    #[test]
    fn execution_runs_test_function_block() {
        let sources = vec![LoadedSource {
            path: PathBuf::from("tests_fb.st"),
            text: r#"
TEST_FUNCTION_BLOCK FbPass
ASSERT_FALSE(FALSE);
END_TEST_FUNCTION_BLOCK

PROGRAM Main
END_PROGRAM
"#
            .to_string(),
        }];
        let tests = discover_tests(&sources);
        assert_eq!(tests.len(), 1);

        let session = CompileSession::from_sources(vec![HarnessSourceFile::with_path(
            "tests_fb.st",
            sources[0].text.clone(),
        )]);
        execute_test_case(&session, &tests[0]).unwrap();
    }

    #[test]
    fn execution_isolated_per_test_case() {
        let sources = vec![LoadedSource {
            path: PathBuf::from("isolation.st"),
            text: r#"
TEST_PROGRAM Isolated
VAR
    X : INT := INT#0;
END_VAR
X := X + INT#1;
ASSERT_EQUAL(INT#1, X);
END_TEST_PROGRAM
"#
            .to_string(),
        }];
        let tests = discover_tests(&sources);
        assert_eq!(tests.len(), 1);

        let session = CompileSession::from_sources(vec![HarnessSourceFile::with_path(
            "isolation.st",
            sources[0].text.clone(),
        )]);
        execute_test_case(&session, &tests[0]).unwrap();
        execute_test_case(&session, &tests[0]).unwrap();
    }

    #[test]
    fn json_output_contract() {
        let results = sample_results();
        let summary = summarize_results(&results);
        let output = render_output(
            TestOutput::Json,
            Path::new("/tmp/project"),
            &results,
            summary,
        )
        .expect("json output");
        let value: serde_json::Value = serde_json::from_str(&output).expect("valid json");

        assert_eq!(value["version"], 1);
        assert_eq!(value["summary"]["total"], 3);
        assert_eq!(value["summary"]["passed"], 1);
        assert_eq!(value["summary"]["failed"], 1);
        assert_eq!(value["summary"]["errors"], 1);
        assert_eq!(value["tests"][0]["status"], "passed");
        assert_eq!(value["tests"][1]["status"], "failed");
        assert_eq!(value["tests"][2]["status"], "error");
        assert_eq!(value["tests"][1]["source"], "ASSERT_EQUAL(INT#2, X);");
    }

    #[test]
    fn tap_output_contract() {
        let results = sample_results();
        let summary = summarize_results(&results);
        let output = render_output(
            TestOutput::Tap,
            Path::new("/tmp/project"),
            &results,
            summary,
        )
        .unwrap();

        assert!(output.starts_with("TAP version 13\n1..3\n"));
        assert!(output.contains("ok 1 - TEST_PROGRAM::PassCase"));
        assert!(output.contains("not ok 2 - TEST_PROGRAM::FailCase"));
        assert!(output.contains("not ok 3 - TEST_FUNCTION_BLOCK::ErrCase"));
        assert!(output.contains("# file: tests.st"));
        assert!(output.contains("# line: 12"));
        assert!(output.contains("# source: ASSERT_EQUAL(INT#2, X);"));
    }

    #[test]
    fn junit_output_contract() {
        let results = sample_results();
        let summary = summarize_results(&results);
        let output = render_output(
            TestOutput::Junit,
            Path::new("/tmp/project"),
            &results,
            summary,
        )
        .unwrap();

        assert!(output.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(
            output.contains(
                "<testsuite name=\"trust-runtime\" tests=\"3\" failures=\"1\" errors=\"1\" skipped=\"0\">"
            )
        );
        assert!(output.contains("<testcase name=\"TEST_PROGRAM::PassCase\""));
        assert!(output
            .contains("<failure message=\"ASSERT_EQUAL failed: expected &lt;2&gt; &amp; got 3\">"));
        assert!(output.contains("<error message=\"runtime &lt;panic&gt;\">"));
    }

    fn sample_results() -> Vec<ExecutedTest> {
        vec![
            ExecutedTest {
                case: DiscoveredTest {
                    kind: TestKind::Program,
                    name: "PassCase".into(),
                    file: PathBuf::from("tests.st"),
                    byte_offset: 0,
                    line: 4,
                    source_line: Some("ASSERT_TRUE(TRUE);".to_string()),
                },
                outcome: TestOutcome::Passed,
                message: None,
            },
            ExecutedTest {
                case: DiscoveredTest {
                    kind: TestKind::Program,
                    name: "FailCase".into(),
                    file: PathBuf::from("tests.st"),
                    byte_offset: 10,
                    line: 12,
                    source_line: Some("ASSERT_EQUAL(INT#2, X);".to_string()),
                },
                outcome: TestOutcome::Failed,
                message: Some("ASSERT_EQUAL failed: expected <2> & got 3".to_string()),
            },
            ExecutedTest {
                case: DiscoveredTest {
                    kind: TestKind::FunctionBlock,
                    name: "ErrCase".into(),
                    file: PathBuf::from("fb_tests.st"),
                    byte_offset: 20,
                    line: 20,
                    source_line: Some("ASSERT_TRUE(FALSE);".to_string()),
                },
                outcome: TestOutcome::Error,
                message: Some("runtime <panic>".to_string()),
            },
        ]
    }

    #[test]
    fn human_output_shows_failure_summary_with_source_context() {
        let results = sample_results();
        let summary = summarize_results(&results);
        let output = render_output(
            TestOutput::Human,
            Path::new("/tmp/project"),
            &results,
            summary,
        )
        .expect("human output");

        let plain = strip_ansi(&output);
        assert!(plain.contains("FAIL [2/3] TEST_PROGRAM::FailCase tests.st:12"));
        assert!(plain.contains("reason   : ASSERT_EQUAL failed: expected <2> & got 3"));
        assert!(plain.contains("source   : ASSERT_EQUAL(INT#2, X);"));
        assert!(plain.contains("Failure summary:"));
        assert!(plain.contains("1. TEST_PROGRAM::FailCase @ tests.st:12"));
        assert!(plain.contains("2. TEST_FUNCTION_BLOCK::ErrCase @ fb_tests.st:20"));
    }

    #[test]
    fn ci_mode_defaults_human_output_to_junit() {
        assert_eq!(effective_output(TestOutput::Human, true), TestOutput::Junit);
        assert_eq!(effective_output(TestOutput::Json, true), TestOutput::Json);
        assert_eq!(effective_output(TestOutput::Tap, true), TestOutput::Tap);
        assert_eq!(effective_output(TestOutput::Junit, true), TestOutput::Junit);
        assert_eq!(
            effective_output(TestOutput::Human, false),
            TestOutput::Human
        );
    }
}
