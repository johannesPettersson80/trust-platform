//! API documentation generation from tagged ST comments.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;
use smol_str::SmolStr;
use trust_runtime::bundle::detect_bundle_path;
use trust_syntax::lexer::{self, Token, TokenKind};
use trust_syntax::parser;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::cli::DocsFormat;
use crate::style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApiItemKind {
    Program,
    TestProgram,
    Function,
    FunctionBlock,
    TestFunctionBlock,
    Class,
    Interface,
    Method,
    Property,
}

impl ApiItemKind {
    fn label(self) -> &'static str {
        match self {
            Self::Program => "PROGRAM",
            Self::TestProgram => "TEST_PROGRAM",
            Self::Function => "FUNCTION",
            Self::FunctionBlock => "FUNCTION_BLOCK",
            Self::TestFunctionBlock => "TEST_FUNCTION_BLOCK",
            Self::Class => "CLASS",
            Self::Interface => "INTERFACE",
            Self::Method => "METHOD",
            Self::Property => "PROPERTY",
        }
    }
}

#[derive(Debug, Clone)]
struct LoadedSource {
    path: PathBuf,
    text: String,
}

#[derive(Debug, Clone)]
struct ApiParamDoc {
    name: SmolStr,
    description: String,
}

#[derive(Debug, Clone, Default)]
struct ApiDocTags {
    brief: Option<String>,
    details: Vec<String>,
    params: Vec<ApiParamDoc>,
    returns: Option<String>,
}

#[derive(Debug, Clone)]
struct ApiItem {
    kind: ApiItemKind,
    qualified_name: SmolStr,
    file: PathBuf,
    line: usize,
    tags: ApiDocTags,
    declared_params: Vec<SmolStr>,
    has_return: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DocDiagnostic {
    file: PathBuf,
    line: usize,
    message: String,
}

#[derive(Debug, Clone)]
struct CommentBlock {
    lines: Vec<String>,
    start_line: usize,
}

enum CurrentTag {
    Brief,
    Detail,
    Param(usize),
    Return,
}

pub fn run_docs(
    project: Option<PathBuf>,
    out_dir: Option<PathBuf>,
    format: DocsFormat,
) -> anyhow::Result<()> {
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

    let sources = load_sources(&project_root, &sources_root)?;
    if sources.is_empty() {
        anyhow::bail!("no ST sources found under {}", sources_root.display());
    }

    let (items, diagnostics) = collect_api_items(&sources);
    let output_root = out_dir.unwrap_or_else(|| project_root.join("docs").join("api"));
    std::fs::create_dir_all(&output_root).with_context(|| {
        format!(
            "failed to create documentation output directory '{}'",
            output_root.display()
        )
    })?;

    let mut written = Vec::new();
    if matches!(format, DocsFormat::Markdown | DocsFormat::Both) {
        let markdown = render_markdown(&items, &diagnostics);
        let path = output_root.join("api.md");
        std::fs::write(&path, markdown)
            .with_context(|| format!("failed to write '{}'", path.display()))?;
        written.push(path);
    }

    if matches!(format, DocsFormat::Html | DocsFormat::Both) {
        let html = render_html(&items, &diagnostics);
        let path = output_root.join("api.html");
        std::fs::write(&path, html)
            .with_context(|| format!("failed to write '{}'", path.display()))?;
        written.push(path);
    }

    println!(
        "{}",
        style::success(format!(
            "Generated documentation for {} API item(s) in {}",
            items.len(),
            output_root.display()
        ))
    );
    for path in &written {
        println!(" - {}", path.display());
    }

    if diagnostics.is_empty() {
        println!("{}", style::success("No documentation tag diagnostics."));
    } else {
        println!(
            "{}",
            style::warning(format!(
                "Generated with {} documentation diagnostic(s):",
                diagnostics.len()
            ))
        );
        for diagnostic in diagnostics {
            println!(
                " - {}:{} {}",
                diagnostic.file.display(),
                diagnostic.line,
                diagnostic.message
            );
        }
    }

    Ok(())
}

fn load_sources(project_root: &Path, root: &Path) -> anyhow::Result<Vec<LoadedSource>> {
    let mut paths = BTreeSet::new();
    for pattern in ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"] {
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
        let display = path
            .strip_prefix(project_root)
            .map_or_else(|_| path.clone(), Path::to_path_buf);
        sources.push(LoadedSource {
            path: display,
            text,
        });
    }
    Ok(sources)
}

fn collect_api_items(sources: &[LoadedSource]) -> (Vec<ApiItem>, Vec<DocDiagnostic>) {
    let mut items = Vec::new();
    let mut diagnostics = Vec::new();

    for source in sources {
        let parse = parser::parse(&source.text);
        let syntax = parse.syntax();
        let tokens = lexer::lex(&source.text);
        for node in syntax.descendants() {
            let Some(kind) = declaration_kind(&node) else {
                continue;
            };
            let Some(name) = declaration_name(&node) else {
                continue;
            };

            let declared_params = declared_param_names(&node);
            let has_return = declaration_has_return(&node, kind);
            let qualified_name = qualified_name(&node, &name);
            let Some(decl_offset) = first_non_trivia_token_start(&node) else {
                continue;
            };
            let decl_line = line_for_offset(&source.text, decl_offset);

            let mut tags = ApiDocTags::default();
            if let Some(comment) = leading_comment_block(&source.text, &tokens, decl_offset) {
                let (parsed, issues) = parse_doc_tags(
                    &comment,
                    &source.path,
                    kind,
                    qualified_name.as_str(),
                    &declared_params,
                    has_return,
                );
                tags = parsed;
                diagnostics.extend(issues);
            }

            items.push(ApiItem {
                kind,
                qualified_name,
                file: source.path.clone(),
                line: decl_line,
                tags,
                declared_params,
                has_return,
            });
        }
    }

    items.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line.cmp(&right.line))
            .then(left.qualified_name.cmp(&right.qualified_name))
    });

    (items, diagnostics)
}

fn declaration_kind(node: &SyntaxNode) -> Option<ApiItemKind> {
    match node.kind() {
        SyntaxKind::Program => {
            let first = first_non_trivia_token(node)?;
            if first == SyntaxKind::KwTestProgram {
                Some(ApiItemKind::TestProgram)
            } else {
                Some(ApiItemKind::Program)
            }
        }
        SyntaxKind::Function => Some(ApiItemKind::Function),
        SyntaxKind::FunctionBlock => {
            let first = first_non_trivia_token(node)?;
            if first == SyntaxKind::KwTestFunctionBlock {
                Some(ApiItemKind::TestFunctionBlock)
            } else {
                Some(ApiItemKind::FunctionBlock)
            }
        }
        SyntaxKind::Class => Some(ApiItemKind::Class),
        SyntaxKind::Interface => Some(ApiItemKind::Interface),
        SyntaxKind::Method => Some(ApiItemKind::Method),
        SyntaxKind::Property => Some(ApiItemKind::Property),
        _ => None,
    }
}

fn first_non_trivia_token(node: &SyntaxNode) -> Option<SyntaxKind> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
}

fn first_non_trivia_token_start(node: &SyntaxNode) -> Option<usize> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .map(|token| usize::from(token.text_range().start()))
}

fn declaration_name(node: &SyntaxNode) -> Option<SmolStr> {
    node.children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .map(|name| {
            let text = name.text().to_string();
            SmolStr::new(text.trim())
        })
}

fn qualified_name(node: &SyntaxNode, name: &SmolStr) -> SmolStr {
    let mut parts = vec![name.to_string()];
    for ancestor in node.ancestors().skip(1) {
        let include = matches!(
            ancestor.kind(),
            SyntaxKind::Namespace
                | SyntaxKind::Class
                | SyntaxKind::FunctionBlock
                | SyntaxKind::Interface
        );
        if !include {
            continue;
        }
        if let Some(ancestor_name) = declaration_name(&ancestor) {
            parts.push(ancestor_name.to_string());
        }
    }
    parts.reverse();
    SmolStr::new(parts.join("."))
}

fn declaration_has_return(node: &SyntaxNode, kind: ApiItemKind) -> bool {
    match kind {
        ApiItemKind::Function => true,
        ApiItemKind::Method | ApiItemKind::Property => node
            .children()
            .any(|child| child.kind() == SyntaxKind::TypeRef),
        _ => false,
    }
}

fn declared_param_names(node: &SyntaxNode) -> Vec<SmolStr> {
    let mut names = Vec::new();
    for block in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarBlock)
    {
        if !is_parameter_var_block(&block) {
            continue;
        }
        for decl in block
            .children()
            .filter(|child| child.kind() == SyntaxKind::VarDecl)
        {
            for name in decl
                .children()
                .filter(|child| child.kind() == SyntaxKind::Name)
            {
                let text = name.text().to_string();
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    names.push(SmolStr::new(trimmed));
                }
            }
        }
    }
    names
}

fn is_parameter_var_block(block: &SyntaxNode) -> bool {
    let Some(token) = block
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
    else {
        return false;
    };
    matches!(
        token.kind(),
        SyntaxKind::KwVarInput | SyntaxKind::KwVarOutput | SyntaxKind::KwVarInOut
    )
}

fn leading_comment_block(
    source: &str,
    tokens: &[Token],
    declaration_start: usize,
) -> Option<CommentBlock> {
    let token_pos =
        tokens.partition_point(|token| usize::from(token.range.start()) < declaration_start);
    if token_pos == 0 {
        return None;
    }

    let mut idx = token_pos;
    let mut collected = Vec::new();
    let mut seen_comment = false;
    while idx > 0 {
        idx -= 1;
        let token = tokens[idx];
        match token.kind {
            TokenKind::Whitespace => {
                let ws = token_text(source, token);
                let newlines = ws.bytes().filter(|byte| *byte == b'\n').count();
                if newlines > 1 {
                    break;
                }
            }
            TokenKind::LineComment | TokenKind::BlockComment => {
                seen_comment = true;
                collected.push(token);
            }
            _ => break,
        }
    }

    if !seen_comment {
        return None;
    }

    collected.reverse();
    let mut lines = Vec::new();
    for token in &collected {
        let raw = token_text(source, *token);
        lines.extend(normalize_comment_lines(token.kind, raw));
    }

    while matches!(lines.first(), Some(line) if line.trim().is_empty()) {
        lines.remove(0);
    }
    while matches!(lines.last(), Some(line) if line.trim().is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        return None;
    }

    let start_line = line_for_offset(source, usize::from(collected[0].range.start()));
    Some(CommentBlock { lines, start_line })
}

fn normalize_comment_lines(kind: TokenKind, raw: &str) -> Vec<String> {
    match kind {
        TokenKind::LineComment => vec![raw
            .strip_prefix("//")
            .unwrap_or(raw)
            .trim_start()
            .trim_end()
            .to_string()],
        TokenKind::BlockComment => {
            let mut body = raw.trim_end();
            if let Some(stripped) = body
                .strip_prefix("(*")
                .and_then(|text| text.strip_suffix("*)"))
            {
                body = stripped;
            } else if let Some(stripped) = body
                .strip_prefix("/*")
                .and_then(|text| text.strip_suffix("*/"))
            {
                body = stripped;
            }
            body.lines()
                .map(|line| {
                    let trimmed = line.trim_start();
                    let without_star = trimmed.strip_prefix('*').map_or(trimmed, str::trim_start);
                    without_star.trim_end().to_string()
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn parse_doc_tags(
    comment: &CommentBlock,
    file: &Path,
    kind: ApiItemKind,
    symbol_name: &str,
    declared_params: &[SmolStr],
    has_return: bool,
) -> (ApiDocTags, Vec<DocDiagnostic>) {
    let mut tags = ApiDocTags::default();
    let mut diagnostics = Vec::new();
    let mut current: Option<CurrentTag> = None;

    for line in &comment.lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(tag_line) = trimmed.strip_prefix('@') {
            let mut parts = tag_line.splitn(2, char::is_whitespace);
            let tag = parts.next().unwrap_or_default().to_ascii_lowercase();
            let remainder = parts.next().map_or("", str::trim_start);
            match tag.as_str() {
                "brief" => {
                    if tags.brief.is_some() {
                        diagnostics.push(DocDiagnostic {
                            file: file.to_path_buf(),
                            line: comment.start_line,
                            message: format!(
                                "duplicate @brief tag for {} `{}`",
                                kind.label(),
                                symbol_name
                            ),
                        });
                    }
                    if remainder.is_empty() {
                        diagnostics.push(DocDiagnostic {
                            file: file.to_path_buf(),
                            line: comment.start_line,
                            message: format!(
                                "missing description for @brief on {} `{}`",
                                kind.label(),
                                symbol_name
                            ),
                        });
                    }
                    tags.brief = if remainder.is_empty() {
                        None
                    } else {
                        Some(remainder.to_string())
                    };
                    current = Some(CurrentTag::Brief);
                }
                "param" => {
                    let mut param_parts = remainder.splitn(2, char::is_whitespace);
                    let Some(name) = param_parts.next().filter(|text| !text.trim().is_empty())
                    else {
                        diagnostics.push(DocDiagnostic {
                            file: file.to_path_buf(),
                            line: comment.start_line,
                            message: format!(
                                "malformed @param tag on {} `{}` (expected: @param <name> <description>)",
                                kind.label(),
                                symbol_name
                            ),
                        });
                        current = None;
                        continue;
                    };
                    let description = param_parts.next().map_or("", str::trim_start);
                    if description.is_empty() {
                        diagnostics.push(DocDiagnostic {
                            file: file.to_path_buf(),
                            line: comment.start_line,
                            message: format!(
                                "missing description for @param `{}` on {} `{}`",
                                name,
                                kind.label(),
                                symbol_name
                            ),
                        });
                    }
                    tags.params.push(ApiParamDoc {
                        name: SmolStr::new(name),
                        description: description.to_string(),
                    });
                    current = Some(CurrentTag::Param(tags.params.len() - 1));
                }
                "return" => {
                    if tags.returns.is_some() {
                        diagnostics.push(DocDiagnostic {
                            file: file.to_path_buf(),
                            line: comment.start_line,
                            message: format!(
                                "duplicate @return tag for {} `{}`",
                                kind.label(),
                                symbol_name
                            ),
                        });
                    }
                    if remainder.is_empty() {
                        diagnostics.push(DocDiagnostic {
                            file: file.to_path_buf(),
                            line: comment.start_line,
                            message: format!(
                                "missing description for @return on {} `{}`",
                                kind.label(),
                                symbol_name
                            ),
                        });
                    }
                    tags.returns = if remainder.is_empty() {
                        None
                    } else {
                        Some(remainder.to_string())
                    };
                    current = Some(CurrentTag::Return);
                }
                other => {
                    diagnostics.push(DocDiagnostic {
                        file: file.to_path_buf(),
                        line: comment.start_line,
                        message: format!(
                            "unknown documentation tag `@{}` on {} `{}`",
                            other,
                            kind.label(),
                            symbol_name
                        ),
                    });
                    current = None;
                }
            }
            continue;
        }

        match current {
            Some(CurrentTag::Brief) => append_with_space(&mut tags.brief, trimmed),
            Some(CurrentTag::Param(index)) => {
                if let Some(param) = tags.params.get_mut(index) {
                    append_string_with_space(&mut param.description, trimmed);
                }
            }
            Some(CurrentTag::Return) => append_with_space(&mut tags.returns, trimmed),
            Some(CurrentTag::Detail) | None => {
                tags.details.push(trimmed.to_string());
                current = Some(CurrentTag::Detail);
            }
        }
    }

    let mut seen_params = HashSet::new();
    let declared: HashMap<String, &SmolStr> = declared_params
        .iter()
        .map(|name| (name.as_str().to_ascii_uppercase(), name))
        .collect();

    for param in &tags.params {
        let normalized = param.name.as_str().to_ascii_uppercase();
        if !seen_params.insert(normalized.clone()) {
            diagnostics.push(DocDiagnostic {
                file: file.to_path_buf(),
                line: comment.start_line,
                message: format!(
                    "duplicate @param entry for `{}` on {} `{}`",
                    param.name,
                    kind.label(),
                    symbol_name
                ),
            });
        }
        if !declared.contains_key(&normalized) {
            diagnostics.push(DocDiagnostic {
                file: file.to_path_buf(),
                line: comment.start_line,
                message: format!(
                    "@param `{}` does not match any declared parameter on {} `{}`",
                    param.name,
                    kind.label(),
                    symbol_name
                ),
            });
        }
    }

    if tags.returns.is_some() && !has_return {
        diagnostics.push(DocDiagnostic {
            file: file.to_path_buf(),
            line: comment.start_line,
            message: format!(
                "@return used on non-returning {} `{}`",
                kind.label(),
                symbol_name
            ),
        });
    }

    (tags, diagnostics)
}

fn append_with_space(target: &mut Option<String>, value: &str) {
    if let Some(existing) = target {
        append_string_with_space(existing, value);
    } else {
        *target = Some(value.to_string());
    }
}

fn append_string_with_space(target: &mut String, value: &str) {
    if target.is_empty() {
        target.push_str(value);
    } else {
        target.push(' ');
        target.push_str(value);
    }
}

fn render_markdown(items: &[ApiItem], diagnostics: &[DocDiagnostic]) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "# ST API Documentation");
    let _ = writeln!(output);
    let _ = writeln!(
        output,
        "Generated by `trust-runtime docs` from tagged ST comments."
    );
    let _ = writeln!(output);

    if !diagnostics.is_empty() {
        let _ = writeln!(output, "## Diagnostics");
        let _ = writeln!(output);
        for diagnostic in diagnostics {
            let _ = writeln!(
                output,
                "- `{}`:{} {}",
                diagnostic.file.display(),
                diagnostic.line,
                diagnostic.message
            );
        }
        let _ = writeln!(output);
    }

    let _ = writeln!(output, "## API");
    let _ = writeln!(output);
    for item in items {
        let _ = writeln!(
            output,
            "### {} `{}`",
            item.kind.label(),
            item.qualified_name
        );
        let _ = writeln!(output, "- Source: `{}`:{}", item.file.display(), item.line);
        let _ = writeln!(
            output,
            "- Return Value: {}",
            if item.has_return { "yes" } else { "no" }
        );
        if !item.declared_params.is_empty() {
            let names = item
                .declared_params
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(output, "- Declared Parameters: `{names}`");
        }
        if let Some(brief) = &item.tags.brief {
            let _ = writeln!(output);
            let _ = writeln!(output, "**Brief**: {brief}");
        }
        if !item.tags.params.is_empty() {
            let _ = writeln!(output);
            let _ = writeln!(output, "**Parameters**:");
            for param in &item.tags.params {
                if param.description.is_empty() {
                    let _ = writeln!(output, "- `{}`", param.name);
                } else {
                    let _ = writeln!(output, "- `{}`: {}", param.name, param.description);
                }
            }
        }
        if let Some(returns) = &item.tags.returns {
            let _ = writeln!(output);
            let _ = writeln!(output, "**Returns**: {returns}");
        }
        if !item.tags.details.is_empty() {
            let _ = writeln!(output);
            let _ = writeln!(output, "**Details**:");
            for detail in &item.tags.details {
                let _ = writeln!(output, "- {detail}");
            }
        }
        let _ = writeln!(output);
    }

    output
}

fn render_html(items: &[ApiItem], diagnostics: &[DocDiagnostic]) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "<!doctype html>");
    let _ = writeln!(output, "<html lang=\"en\">");
    let _ = writeln!(output, "<head>");
    let _ = writeln!(output, "  <meta charset=\"utf-8\">");
    let _ = writeln!(output, "  <title>ST API Documentation</title>");
    let _ = writeln!(
        output,
        "  <style>body{{font-family:ui-sans-serif,system-ui,sans-serif;max-width:960px;margin:2rem auto;padding:0 1rem;line-height:1.5}}article{{border:1px solid #d5d5d5;border-radius:8px;padding:1rem;margin:1rem 0}}code{{background:#f5f5f5;padding:0.1rem 0.3rem;border-radius:4px}}.diag{{color:#8a4b00}}</style>"
    );
    let _ = writeln!(output, "</head>");
    let _ = writeln!(output, "<body>");
    let _ = writeln!(output, "  <h1>ST API Documentation</h1>");
    let _ = writeln!(
        output,
        "  <p>Generated by <code>trust-runtime docs</code> from tagged ST comments.</p>"
    );

    if !diagnostics.is_empty() {
        let _ = writeln!(output, "  <section>");
        let _ = writeln!(output, "    <h2>Diagnostics</h2>");
        let _ = writeln!(output, "    <ul>");
        for diagnostic in diagnostics {
            let _ = writeln!(
                output,
                "      <li class=\"diag\"><code>{}</code>:{} {}</li>",
                html_escape(&diagnostic.file.display().to_string()),
                diagnostic.line,
                html_escape(&diagnostic.message)
            );
        }
        let _ = writeln!(output, "    </ul>");
        let _ = writeln!(output, "  </section>");
    }

    let _ = writeln!(output, "  <section>");
    let _ = writeln!(output, "    <h2>API</h2>");
    for item in items {
        let _ = writeln!(output, "    <article>");
        let _ = writeln!(
            output,
            "      <h3>{} <code>{}</code></h3>",
            item.kind.label(),
            html_escape(item.qualified_name.as_str())
        );
        let _ = writeln!(
            output,
            "      <p><strong>Source:</strong> <code>{}</code>:{}<br><strong>Return Value:</strong> {}</p>",
            html_escape(&item.file.display().to_string()),
            item.line,
            if item.has_return { "yes" } else { "no" }
        );
        if !item.declared_params.is_empty() {
            let params = item
                .declared_params
                .iter()
                .map(|name| html_escape(name.as_str()))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                output,
                "      <p><strong>Declared Parameters:</strong> <code>{params}</code></p>"
            );
        }
        if let Some(brief) = &item.tags.brief {
            let _ = writeln!(
                output,
                "      <p><strong>Brief:</strong> {}</p>",
                html_escape(brief)
            );
        }
        if !item.tags.params.is_empty() {
            let _ = writeln!(output, "      <h4>Parameters</h4>");
            let _ = writeln!(output, "      <ul>");
            for param in &item.tags.params {
                let _ = writeln!(
                    output,
                    "        <li><code>{}</code>: {}</li>",
                    html_escape(param.name.as_str()),
                    html_escape(&param.description)
                );
            }
            let _ = writeln!(output, "      </ul>");
        }
        if let Some(returns) = &item.tags.returns {
            let _ = writeln!(
                output,
                "      <p><strong>Returns:</strong> {}</p>",
                html_escape(returns)
            );
        }
        if !item.tags.details.is_empty() {
            let _ = writeln!(output, "      <h4>Details</h4>");
            let _ = writeln!(output, "      <ul>");
            for detail in &item.tags.details {
                let _ = writeln!(output, "        <li>{}</li>", html_escape(detail));
            }
            let _ = writeln!(output, "      </ul>");
        }
        let _ = writeln!(output, "    </article>");
    }
    let _ = writeln!(output, "  </section>");
    let _ = writeln!(output, "</body>");
    let _ = writeln!(output, "</html>");
    output
}

fn html_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn token_text(source: &str, token: Token) -> &str {
    &source[usize::from(token.range.start())..usize::from(token.range.end())]
}

fn line_for_offset(source: &str, offset: usize) -> usize {
    let capped = offset.min(source.len());
    source[..capped]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_extraction_for_tagged_comments() {
        let sources = vec![LoadedSource {
            path: PathBuf::from("sources/math.st"),
            text: r#"
// @brief Adds two numbers.
// @param A Left-hand value.
// @param B Right-hand value.
// @return Sum value.
FUNCTION Add : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
Add := A + B;
END_FUNCTION
"#
            .to_string(),
        }];

        let (items, diagnostics) = collect_api_items(&sources);
        assert_eq!(diagnostics.len(), 0);
        assert_eq!(items.len(), 1);

        let function = &items[0];
        assert_eq!(function.kind, ApiItemKind::Function);
        assert_eq!(function.qualified_name, "Add");
        assert_eq!(function.tags.brief.as_deref(), Some("Adds two numbers."));
        assert_eq!(function.tags.params.len(), 2);
        assert_eq!(function.tags.params[0].name, "A");
        assert_eq!(function.tags.params[1].name, "B");
        assert_eq!(function.tags.returns.as_deref(), Some("Sum value."));
    }

    #[test]
    fn broken_tag_diagnostics_are_reported() {
        let sources = vec![LoadedSource {
            path: PathBuf::from("sources/broken.st"),
            text: r#"
// @brief
// @param MissingDescription
// @param Z Unknown
// @return Not valid for program.
// @mystery unknown tag
PROGRAM Main
VAR_INPUT
    A : INT;
END_VAR
END_PROGRAM
"#
            .to_string(),
        }];

        let (_items, diagnostics) = collect_api_items(&sources);
        let joined = diagnostics
            .iter()
            .map(|diag| diag.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("missing description for @brief"));
        assert!(joined.contains("missing description for @param `MissingDescription`"));
        assert!(joined.contains("@param `Z` does not match any declared parameter"));
        assert!(joined.contains("@return used on non-returning PROGRAM `Main`"));
        assert!(joined.contains("unknown documentation tag `@mystery`"));
    }

    #[test]
    fn markdown_output_snapshot() {
        let sources = vec![LoadedSource {
            path: PathBuf::from("sources/lib.st"),
            text: r#"
// @brief Enable output when input is true.
// @param IN Input value.
// @param OUT Output value.
FUNCTION_BLOCK Gate
VAR_INPUT
    IN : BOOL;
END_VAR
VAR_OUTPUT
    OUT : BOOL;
END_VAR
OUT := IN;
END_FUNCTION_BLOCK
"#
            .to_string(),
        }];
        let (items, diagnostics) = collect_api_items(&sources);
        let actual = render_markdown(&items, &diagnostics);
        let expected = r#"# ST API Documentation

Generated by `trust-runtime docs` from tagged ST comments.

## API

### FUNCTION_BLOCK `Gate`
- Source: `sources/lib.st`:5
- Return Value: no
- Declared Parameters: `IN, OUT`

**Brief**: Enable output when input is true.

**Parameters**:
- `IN`: Input value.
- `OUT`: Output value.

"#;
        assert_eq!(actual, expected);
    }

    #[test]
    fn html_output_snapshot() {
        let item = ApiItem {
            kind: ApiItemKind::Function,
            qualified_name: "Calc.Add".into(),
            file: PathBuf::from("sources/math.st"),
            line: 7,
            tags: ApiDocTags {
                brief: Some("Adds two INT values.".to_string()),
                details: vec!["Overflow behavior follows IEC arithmetic.".to_string()],
                params: vec![
                    ApiParamDoc {
                        name: "A".into(),
                        description: "First operand.".to_string(),
                    },
                    ApiParamDoc {
                        name: "B".into(),
                        description: "Second operand.".to_string(),
                    },
                ],
                returns: Some("INT sum.".to_string()),
            },
            declared_params: vec!["A".into(), "B".into()],
            has_return: true,
        };
        let actual = render_html(&[item], &[]);
        let expected = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>ST API Documentation</title>
  <style>body{font-family:ui-sans-serif,system-ui,sans-serif;max-width:960px;margin:2rem auto;padding:0 1rem;line-height:1.5}article{border:1px solid #d5d5d5;border-radius:8px;padding:1rem;margin:1rem 0}code{background:#f5f5f5;padding:0.1rem 0.3rem;border-radius:4px}.diag{color:#8a4b00}</style>
</head>
<body>
  <h1>ST API Documentation</h1>
  <p>Generated by <code>trust-runtime docs</code> from tagged ST comments.</p>
  <section>
    <h2>API</h2>
    <article>
      <h3>FUNCTION <code>Calc.Add</code></h3>
      <p><strong>Source:</strong> <code>sources/math.st</code>:7<br><strong>Return Value:</strong> yes</p>
      <p><strong>Declared Parameters:</strong> <code>A, B</code></p>
      <p><strong>Brief:</strong> Adds two INT values.</p>
      <h4>Parameters</h4>
      <ul>
        <li><code>A</code>: First operand.</li>
        <li><code>B</code>: Second operand.</li>
      </ul>
      <p><strong>Returns:</strong> INT sum.</p>
      <h4>Details</h4>
      <ul>
        <li>Overflow behavior follows IEC arithmetic.</li>
      </ul>
    </article>
  </section>
</body>
</html>
"#;
        assert_eq!(actual, expected);
    }
}
