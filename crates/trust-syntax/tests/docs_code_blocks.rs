//! The Structured Text in the documentation follows the product (checklist G16): every code block fenced as Structured
//! Text in `docs/public`, the README and the examples' READMEs either quotes the example's own sources or parses as
//! Structured Text, as written, as the statements of a program or as the declarations of a program's `VAR` block. A block
//! quotes its example when each of its lines, without comments and with its whitespace collapsed, is found in the
//! example's sources; a line or a part of a line written as `...` stands for code the quote leaves out. A README that
//! quotes a lesson's program fails once the program changes without it; a block that no parser accepts fails at once. A
//! block whose fence says `ignore` is left out (` ```iecst ignore`), visibly.

use std::fs;
use std::path::{Path, PathBuf};

use trust_syntax::parser::parse;

/// The fence languages that mark Structured Text.
const STRUCTURED_TEXT: [&str; 5] = [
    "iecst",
    "st",
    "structured-text",
    "structured_text",
    "iec61131",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The Markdown files the check reads: every page of `docs/public`, the README, and every README of `examples`.
fn pages(root: &Path) -> Vec<PathBuf> {
    let mut found = vec![root.join("README.md")];
    collect(&root.join("docs/public"), &mut found, &|path| {
        path.extension().is_some_and(|extension| extension == "md")
    });
    collect(&root.join("examples"), &mut found, &|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("README") && name.ends_with(".md"))
    });
    found.sort();
    found
}

fn collect(folder: &Path, found: &mut Vec<PathBuf>, wanted: &dyn Fn(&Path) -> bool) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if !matches!(name.to_str(), Some("node_modules" | "target" | ".git")) {
                collect(&path, found, wanted);
            }
        } else if wanted(&path) {
            found.push(path);
        }
    }
}

/// One fenced block: its first line in the page, its fence's info string and its text.
struct Block {
    line: usize,
    info: String,
    text: String,
}

/// The fenced blocks of `page`: a fence opens and closes with three backticks, at any indentation, and the block's lines
/// lose the opening fence's indentation.
fn blocks(page: &str) -> Vec<Block> {
    let mut found = Vec::new();
    let mut lines = page.lines().enumerate();
    while let Some((number, line)) = lines.next() {
        let trimmed = line.trim_start();
        let Some(info) = trimmed.strip_prefix("```") else {
            continue;
        };
        let indent = line.len() - trimmed.len();
        let mut text = Vec::new();
        for (_, body) in lines.by_ref() {
            if body.trim_start().starts_with("```") {
                break;
            }
            let cut = body.len() - body.trim_start().len();
            text.push(&body[cut.min(indent)..]);
        }
        found.push(Block {
            line: number + 1,
            info: info.trim().to_owned(),
            text: text.join("\n"),
        });
    }
    found
}

/// Whether a fence's info string names Structured Text, and whether it asks to be left out.
fn structured_text(info: &str) -> (bool, bool) {
    let words: Vec<String> = info
        .split(|character: char| character.is_whitespace() || character == ',')
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let language = words
        .first()
        .is_some_and(|word| STRUCTURED_TEXT.contains(&word.as_str()));
    (language, words.iter().any(|word| word == "ignore"))
}

/// The Structured Text sources that `page` may quote, without comments and with their whitespace collapsed: those of the
/// example the page belongs to and of every example it links to with a relative link (a tutorial quotes the example it walks
/// through). An example's sources are the `.st` files below the linked or own folder and below the nearest folder above it,
/// up to `examples`, that has a `src` folder. Empty for a page that neither lies in nor links to `examples`.
fn example_sources(root: &Path, page: &Path, text: &str) -> Vec<String> {
    let Ok(examples) = fs::canonicalize(root.join("examples")) else {
        return Vec::new();
    };
    let Some(folder) = page.parent() else {
        return Vec::new();
    };
    let mut places = vec![folder.to_path_buf()];
    places.extend(
        relative_links(text)
            .into_iter()
            .map(|link| folder.join(link)),
    );
    let is_source = |path: &Path| path.extension().is_some_and(|extension| extension == "st");
    let mut files = Vec::new();
    for place in places {
        let Ok(place) = fs::canonicalize(&place) else {
            continue;
        };
        let place = if place.is_file() {
            place.parent().map(Path::to_path_buf).unwrap_or_default()
        } else {
            place
        };
        if !place.starts_with(&examples) || place == examples {
            continue;
        }
        collect(&place, &mut files, &is_source);
        let mut above = place.parent();
        while let Some(current) =
            above.filter(|current| current.starts_with(&examples) && *current != examples)
        {
            if current.join("src").is_dir() {
                collect(current, &mut files, &is_source);
                break;
            }
            above = current.parent();
        }
    }
    files.sort();
    files.dedup();
    files
        .iter()
        .filter_map(|file| fs::read_to_string(file).ok())
        .map(|source| collapsed(&without_comments(&source)))
        .collect()
}

/// The targets of the relative Markdown links in `text`, without their `#` anchors: `](../../hmi_tank_level/README.md)`.
fn relative_links(text: &str) -> Vec<String> {
    text.match_indices("](")
        .filter_map(|(at, _)| {
            let rest = &text[at + 2..];
            let target = rest[..rest.find(')')?].split('#').next()?.trim();
            (!target.is_empty() && !target.contains("://") && !target.starts_with("mailto:"))
                .then(|| target.to_owned())
        })
        .collect()
}

/// `text` without its `(* ... *)` and `// ...` comments.
fn without_comments(text: &str) -> String {
    let mut kept = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("(*") {
            rest = after.find("*)").map_or("", |end| &after[end + 2..]);
            kept.push(' ');
        } else if rest.starts_with("//") {
            rest = rest.find('\n').map_or("", |end| &rest[end..]);
        } else {
            let next = rest.chars().next().unwrap_or_default();
            kept.push(next);
            rest = &rest[next.len_utf8()..];
        }
    }
    kept
}

/// `text` with every run of whitespace made one space, trimmed.
fn collapsed(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The first line of `block` that its example's `sources` do not hold, without comments and with its whitespace collapsed;
/// `None` when the block quotes them. A line or a part of a line written as `...` stands for code the quote leaves out.
fn unquoted_line(block: &str, sources: &[String]) -> Option<String> {
    let text = without_comments(block);
    text.lines()
        .map(collapsed)
        .filter(|line| !line.is_empty())
        .find(|line| {
            line.split(['…'])
                .flat_map(|part| part.split("..."))
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .any(|part| !sources.iter().any(|source| source.contains(part)))
        })
}

/// The first parse error of `text` as written, as the statements of a program, and as the declarations of a program's `VAR`
/// block, each named; `None` once one of them parses.
fn parse_error(text: &str) -> Option<String> {
    let mut errors = Vec::new();
    for (form, candidate) in [
        ("as written", text.to_owned()),
        (
            "as statements",
            format!("PROGRAM DocBlock\n{text}\nEND_PROGRAM\n"),
        ),
        (
            "as declarations",
            format!("PROGRAM DocBlock\nVAR\n{text}\nEND_VAR\nEND_PROGRAM\n"),
        ),
    ] {
        let parsed = parse(&candidate);
        if parsed.ok() {
            return None;
        }
        let first = parsed
            .errors()
            .first()
            .map_or_else(String::new, ToString::to_string);
        errors.push(format!("{form}: {first}"));
    }
    Some(errors.join("; "))
}

#[test]
fn every_structured_text_block_quotes_its_example_or_parses() {
    let root = root();
    let mut checked = 0;
    let mut failures = Vec::new();
    for page in pages(&root) {
        let Ok(text) = fs::read_to_string(&page) else {
            continue;
        };
        let sources = example_sources(&root, &page, &text);
        for block in blocks(&text) {
            let (is_structured_text, ignored) = structured_text(&block.info);
            if !is_structured_text || ignored || block.text.trim().is_empty() {
                continue;
            }
            checked += 1;
            let unquoted = if sources.is_empty() {
                Some(String::new())
            } else {
                unquoted_line(&block.text, &sources)
            };
            let Some(unquoted) = unquoted else {
                continue;
            };
            if let Some(error) = parse_error(&block.text) {
                let shown = page
                    .strip_prefix(&root)
                    .unwrap_or(&page)
                    .display()
                    .to_string();
                let quote = if unquoted.is_empty() {
                    "no example sources".to_owned()
                } else {
                    format!("not in the example's sources: {unquoted}")
                };
                failures.push(format!("{shown}:{}: {quote}; {error}", block.line));
            }
        }
    }
    // The documentation holds about 200 such blocks (30 October 2026); far fewer means the pages were not found.
    assert!(
        checked >= 150,
        "only {checked} Structured Text blocks were found"
    );
    assert!(
        failures.is_empty(),
        "{} of {checked} Structured Text blocks follow neither their example nor the language:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn a_quote_must_match_its_example_and_other_text_must_parse() {
    // The rules the check applies, on text written here: a lesson's line quoted with other spacing is a quote; a changed
    // number is not, and a line that no parser accepts fails.
    let sources = [collapsed(&without_comments(
        "IF Error > LREAL#0.0 THEN (* heat *)\n    Output := Limit(In := Error, Max := LREAL#5.0);\n    Count := Count + 1;\nEND_IF;",
    ))];
    assert_eq!(
        unquoted_line(
            "IF Error > LREAL#0.0 THEN\n    Count := Count + 1;\nEND_IF;",
            &sources
        ),
        None
    );
    assert_eq!(
        unquoted_line(
            "Output := Limit(In := Error, ...);  (* elided *)\n...",
            &sources
        ),
        None
    );
    assert_eq!(
        unquoted_line(
            "IF Error > LREAL#0.0 THEN\n    Count := Count + 2;",
            &sources
        ),
        Some("Count := Count + 2;".to_owned())
    );
    assert_eq!(parse_error("Output := LREAL#6.0;"), None);
    assert_eq!(parse_error("Pressure : REAL := 1.0;"), None);
    assert_eq!(
        parse_error("FUNCTION_BLOCK Fb\nVAR_INPUT x : INT; END_VAR\nEND_FUNCTION_BLOCK"),
        None
    );
    assert!(
        parse_error("IF x THEN y := 1;").is_some(),
        "an IF without END_IF is refused"
    );
    assert_eq!(structured_text("iecst"), (true, false));
    assert_eq!(structured_text("st ignore"), (true, true));
    assert_eq!(structured_text("bash"), (false, false));
}
