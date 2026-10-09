//! The regions Create PLC code owns in a program (specification 39, §10.2, §10.3): the
//! declarations, inside a `VAR` block, the retained declarations of its loops (§10.5), inside a
//! `VAR RETAIN` block after it, and the calls, at the end of the body. Each begin marker carries
//! the SHA-256 of the region's text, so a region changed after it was written is found; the
//! generator then accepts it only when it is what it would write itself (§10.3 item 1). It also
//! adds the `USING` directives its loops' code needs, beside the file's own before the program:
//! the only lines it writes outside a region.

use sha2::{Digest, Sha256};
use trust_syntax::{parser::parse, SyntaxKind, SyntaxNode};

/// A region's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Declarations,
    Retained,
    Calls,
}

impl Kind {
    fn begin(self) -> &'static str {
        match self {
            Self::Declarations => "(* truST plant: declarations",
            Self::Retained => "(* truST plant: retained declarations",
            Self::Calls => "(* truST plant: calls",
        }
    }

    fn end(self) -> &'static str {
        match self {
            Self::Declarations => "(* end of truST plant declarations *)",
            Self::Retained => "(* end of truST plant retained declarations *)",
            Self::Calls => "(* end of truST plant calls *)",
        }
    }

    fn words(self) -> &'static str {
        match self {
            Self::Declarations => "declarations",
            Self::Retained => "retained declarations",
            Self::Calls => "calls",
        }
    }
}

/// What the generator writes into a program: its three regions' texts and the namespaces its
/// code names without their prefix.
pub(super) struct Generated<'a> {
    pub declarations: &'a str,
    pub retained: &'a str,
    pub calls: &'a str,
    pub usings: &'a [&'a str],
}

const INDENT: &str = "    ";

/// The SHA-256 of a region's text, in lowercase hexadecimal.
fn checksum(content: &str) -> String {
    format!("{:x}", Sha256::digest(content.as_bytes()))
}

/// A region with its markers, as the program text holds it.
fn render(kind: Kind, content: &str) -> String {
    format!(
        "{INDENT}{} sha256:{} *)\n{content}{INDENT}{}\n",
        kind.begin(),
        checksum(content),
        kind.end()
    )
}

/// A region found in a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Region {
    /// The byte offset where the begin marker's line starts.
    start: usize,
    /// The byte offset after the end marker's line.
    end: usize,
    /// The 1-based line of the begin marker.
    pub line: usize,
    /// The text between the markers.
    pub content: String,
    /// Whether the text still has the checksum its begin marker records.
    pub intact: bool,
}

/// The refusal of a region changed by hand.
pub(super) fn changed_by_hand(file: &str, line: usize, kind: Kind) -> String {
    format!(
        "{file}:{line}: the truST plant {} region was changed by hand. Move your own code out of \
         the region and delete its two marker lines; Create PLC code then writes the region again",
        kind.words()
    )
}

/// The line starting at each byte offset of `text`, with its end (after its newline).
fn lines(text: &str) -> impl Iterator<Item = (usize, usize, &str)> {
    let mut offset = 0;
    text.split_inclusive('\n').map(move |line| {
        let start = offset;
        offset += line.len();
        (start, offset, line)
    })
}

/// The 1-based line of a byte offset.
fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].matches('\n').count() + 1
}

/// The region of a kind inside `range` of `text`, with whether it still has its checksum.
pub(super) fn find(
    text: &str,
    file: &str,
    range: (usize, usize),
    kind: Kind,
) -> Result<Option<Region>, String> {
    let mut begin: Option<(usize, usize, &str)> = None;
    for (start, end, line) in lines(text) {
        if start < range.0 || start >= range.1 {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.starts_with(kind.begin()) {
            if begin.is_some() {
                return Err(format!(
                    "{file}:{}: a program has two truST plant {} regions; keep one",
                    line_of(text, start),
                    kind.words()
                ));
            }
            begin = Some((start, end, trimmed));
        } else if trimmed == kind.end() {
            let Some((first, content_start, marker)) = begin else {
                return Err(format!(
                    "{file}:{}: the end of a truST plant {} region has no beginning",
                    line_of(text, start),
                    kind.words()
                ));
            };
            let content = &text[content_start..start];
            let recorded = marker
                .strip_prefix(kind.begin())
                .and_then(|rest| rest.trim().strip_prefix("sha256:"))
                .and_then(|rest| rest.strip_suffix("*)"))
                .map(str::trim);
            return Ok(Some(Region {
                start: first,
                end,
                line: line_of(text, first),
                content: content.to_string(),
                intact: recorded == Some(checksum(content).as_str()),
            }));
        }
    }
    match begin {
        Some((start, _, _)) => Err(format!(
            "{file}:{}: a truST plant {} region has no end marker",
            line_of(text, start),
            kind.words()
        )),
        None => Ok(None),
    }
}

/// The names and declared types of a declarations region.
pub(super) fn declared(content: &str) -> Vec<(String, String)> {
    let probe = format!("PROGRAM RegionProbe\nVAR\n{content}END_VAR\nEND_PROGRAM\n");
    parse(&probe)
        .syntax()
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
        .filter_map(|declaration| {
            let ty = declaration
                .children()
                .find(|node| node.kind() == SyntaxKind::TypeRef)?
                .text()
                .to_string()
                .split_whitespace()
                .collect::<String>();
            let names: Vec<String> = declaration
                .children()
                .filter(|node| node.kind() == SyntaxKind::Name)
                .map(|node| super::super::control_links::code_text(&node))
                .collect();
            Some(names.into_iter().map(move |name| (name, ty.clone())))
        })
        .flatten()
        .collect()
}

/// The node of `PROGRAM name` in a parsed source.
pub(super) fn program_node(root: &SyntaxNode, name: &str) -> Option<SyntaxNode> {
    root.descendants()
        .filter(|node| node.kind() == SyntaxKind::Program)
        .find(|node| {
            node.children()
                .find(|child| child.kind() == SyntaxKind::Name)
                .is_some_and(|child| {
                    super::super::control_links::code_text(&child).eq_ignore_ascii_case(name)
                })
        })
}

/// The byte range of a node, its trailing trivia included.
pub(super) fn range_of(node: &SyntaxNode) -> (usize, usize) {
    let range = node.text_range();
    (usize::from(range.start()), usize::from(range.end()))
}

/// The byte offset after a node's last token that is not trivia: a node's range also holds the
/// whitespace and comments up to the next token, which may stand on a later line.
fn end_of_code(node: &SyntaxNode) -> usize {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .last()
        .map_or_else(
            || range_of(node).1,
            |token| usize::from(token.text_range().end()),
        )
}

fn line_start(text: &str, offset: usize) -> usize {
    text[..offset].rfind('\n').map_or(0, |at| at + 1)
}

fn line_end(text: &str, offset: usize) -> usize {
    text[offset..]
        .find('\n')
        .map_or(text.len(), |at| offset + at + 1)
}

/// The namespaces a program sees through `USING` directives: its own and those of the file and
/// namespaces around it, in upper case.
fn namespaces_in_use(node: &SyntaxNode) -> Vec<String> {
    let mut scopes = vec![node.clone()];
    scopes.extend(node.ancestors().skip(1));
    scopes
        .iter()
        .flat_map(|scope| scope.children())
        .filter(|child| child.kind() == SyntaxKind::UsingDirective)
        .flat_map(|directive| {
            // The directive's own tokens: a comment before it belongs to its node too.
            let text = super::super::control_links::code_text(&directive);
            let body = text
                .get(5..)
                .unwrap_or("")
                .trim_end_matches(';')
                .to_string();
            body.split(',')
                .map(|name| name.trim().to_ascii_uppercase())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The program's text with its regions holding the generated texts: an existing region is
/// replaced in place, a missing one is added (the declarations in a new `VAR` block after the
/// program's last one, the retained declarations in a `VAR RETAIN` block after the declarations',
/// the calls before `END_PROGRAM`), and each namespace in `usings` the program does not see yet
/// gets its `USING` directive before the program, after the file's own.
pub(super) fn write(
    text: &str,
    file: &str,
    program: &str,
    generated: &Generated<'_>,
) -> Result<String, String> {
    let Generated {
        declarations,
        retained,
        calls,
        usings,
    } = *generated;
    let root = parse(text).syntax();
    let node = program_node(&root, program)
        .ok_or_else(|| format!("{file}: PROGRAM {program} was not found"))?;
    let range = range_of(&node);
    let found_declarations = find(text, file, range, Kind::Declarations)?;
    let found_retained = find(text, file, range, Kind::Retained)?;
    let found_calls = find(text, file, range, Kind::Calls)?;
    // Edits from the end of the text to its start keep earlier offsets valid.
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    match found_calls {
        Some(region) => edits.push((region.start, region.end, render(Kind::Calls, calls))),
        None => {
            let end = node
                .children_with_tokens()
                .filter_map(|element| element.into_token())
                .find(|token| token.kind() == SyntaxKind::KwEndProgram)
                .ok_or_else(|| format!("{file}: PROGRAM {program} has no END_PROGRAM"))?;
            let at = line_start(text, usize::from(end.text_range().start()));
            edits.push((at, at, render(Kind::Calls, calls)));
        }
    }
    let retained_block = if retained.is_empty() {
        String::new()
    } else {
        format!("VAR RETAIN\n{}END_VAR\n", render(Kind::Retained, retained))
    };
    match (&found_retained, &found_declarations) {
        (Some(region), _) => {
            edits.push((region.start, region.end, render(Kind::Retained, retained)));
        }
        (None, Some(declared)) if !retained.is_empty() => {
            // After the `VAR` block that holds the declarations region.
            let block = node
                .children()
                .filter(|child| child.kind() == SyntaxKind::VarBlock)
                .find(|child| {
                    let (start, end) = range_of(child);
                    start <= declared.start && declared.start < end
                })
                .ok_or_else(|| {
                    format!("{file}: the truST plant declarations region is not inside a VAR block")
                })?;
            let at = line_end(text, end_of_code(&block));
            edits.push((at, at, retained_block.clone()));
        }
        _ => {}
    }
    match found_declarations {
        Some(region) => edits.push((
            region.start,
            region.end,
            render(Kind::Declarations, declarations),
        )),
        None => {
            // After the last VAR block, else after the name and its USING directives.
            let after = node
                .children()
                .filter(|child| child.kind() == SyntaxKind::VarBlock)
                .last()
                .or_else(|| {
                    node.children()
                        .filter(|child| {
                            matches!(child.kind(), SyntaxKind::Name | SyntaxKind::UsingDirective)
                        })
                        .last()
                })
                .ok_or_else(|| format!("{file}: PROGRAM {program} has no name"))?;
            let at = line_end(text, end_of_code(&after));
            let calls_at = edits[0].0;
            if at > calls_at {
                return Err(format!(
                    "{file}: put the declarations and END_PROGRAM of PROGRAM {program} on \
                     separate lines before creating PLC code"
                ));
            }
            let retained_block = if found_retained.is_none() {
                retained_block
            } else {
                String::new()
            };
            edits.push((
                at,
                at,
                format!(
                    "VAR\n{}END_VAR\n{retained_block}",
                    render(Kind::Declarations, declarations)
                ),
            ));
        }
    }
    let seen = namespaces_in_use(&node);
    let missing: String = usings
        .iter()
        .filter(|namespace| !seen.contains(&namespace.to_ascii_uppercase()))
        .map(|namespace| format!("USING {namespace};\n"))
        .collect();
    if !missing.is_empty() {
        // Beside the file's own USING directives before the program, else just before it.
        let file_usings = node.parent().and_then(|scope| {
            scope
                .children()
                .filter(|child| {
                    child.kind() == SyntaxKind::UsingDirective && range_of(child).1 <= range.0
                })
                .last()
        });
        let at = match file_usings {
            Some(using) => line_end(text, end_of_code(&using)),
            None => line_start(text, range.0),
        };
        edits.push((at, at, missing));
    }
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
    let mut result = text.to_string();
    for (start, end, replacement) in edits {
        result.replace_range(start..end, &replacement);
    }
    Ok(result)
}

/// A new source file declaring a program that holds only the regions.
pub(super) fn new_program(program: &str, generated: &Generated<'_>) -> String {
    let usings: String = generated
        .usings
        .iter()
        .map(|namespace| format!("USING {namespace};\n"))
        .collect();
    let retained = if generated.retained.is_empty() {
        String::new()
    } else {
        format!(
            "VAR RETAIN\n{}END_VAR\n",
            render(Kind::Retained, generated.retained)
        )
    };
    format!(
        "PROGRAM {program}\n{usings}VAR\n{}END_VAR\n{retained}{}END_PROGRAM\n",
        render(Kind::Declarations, generated.declarations),
        render(Kind::Calls, generated.calls)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regions without loops: no retained declarations, no `USING` directives.
    fn only<'a>(declarations: &'a str, calls: &'a str) -> Generated<'a> {
        Generated {
            declarations,
            retained: "",
            calls,
            usings: &[],
        }
    }

    const SOURCE: &str = "USING Plant;\n\nPROGRAM Main\nVAR\n    Level : REAL;\nEND_VAR\n    Level := Level + 1.0;\nEND_PROGRAM\n";

    #[test]
    fn regions_are_added_once_and_then_replaced_in_place() {
        let first = write(
            SOURCE,
            "src/Main.st",
            "Main",
            &only("    P101 : Plant.FB_Pump;\n", "    P101();\n"),
        )
        .expect("the regions are added");
        assert!(first.contains("END_VAR\nVAR\n    (* truST plant: declarations sha256:"));
        assert!(first.contains("    Level := Level + 1.0;\n    (* truST plant: calls sha256:"));
        assert!(first.ends_with("    (* end of truST plant calls *)\nEND_PROGRAM\n"));
        let again = write(
            &first,
            "src/Main.st",
            "Main",
            &only("    P101 : Plant.FB_Pump;\n", "    P101();\n"),
        )
        .expect("the regions are replaced");
        assert_eq!(
            again, first,
            "writing the same regions again changes nothing"
        );
        let more = write(
            &first,
            "src/Main.st",
            "Main",
            &only(
                "    P101 : Plant.FB_Pump;\n    P102 : Plant.FB_Pump;\n",
                "    P101();\n    P102();\n",
            ),
        )
        .expect("the regions grow");
        assert_eq!(more.matches("truST plant: declarations").count(), 1);
        assert!(more.contains("    P102();\n"));
        let root = parse(&more).syntax();
        let node = program_node(&root, "Main").expect("the program");
        let region = find(&more, "src/Main.st", range_of(&node), Kind::Declarations)
            .expect("the region is well formed")
            .expect("the region");
        assert!(region.intact);
        assert_eq!(
            declared(&region.content),
            vec![
                ("P101".to_string(), "Plant.FB_Pump".to_string()),
                ("P102".to_string(), "Plant.FB_Pump".to_string())
            ]
        );
    }

    #[test]
    fn a_region_changed_after_it_was_written_is_found_with_its_line() {
        let generated = write(
            SOURCE,
            "src/Main.st",
            "Main",
            &only("    P101 : Plant.FB_Pump;\n", "    P101();\n"),
        )
        .expect("the regions are added");
        let edited = generated.replace("    P101();\n", "    P101(RunningFeedback := TRUE);\n");
        let root = parse(&edited).syntax();
        let node = program_node(&root, "Main").expect("the program");
        let calls = find(&edited, "src/Main.st", range_of(&node), Kind::Calls)
            .expect("the region is well formed")
            .expect("the region");
        assert!(!calls.intact, "its checksum no longer matches");
        assert_eq!(
            calls.line,
            generated
                .lines()
                .position(|line| line.contains("truST plant: calls"))
                .unwrap()
                + 1
        );
        let declarations = find(&edited, "src/Main.st", range_of(&node), Kind::Declarations)
            .expect("the region is well formed")
            .expect("the region");
        assert!(declarations.intact);
        assert!(changed_by_hand("src/Main.st", calls.line, Kind::Calls)
            .contains("calls region was changed by hand"));
    }

    #[test]
    fn a_program_without_declarations_gets_its_var_block_after_the_name() {
        let source = "PROGRAM Main\n    ;\nEND_PROGRAM\n";
        let text = write(
            source,
            "src/Main.st",
            "Main",
            &only("    P101 : Plant.FB_Pump;\n", "    P101();\n"),
        )
        .expect("the regions are added");
        assert!(
            text.starts_with("PROGRAM Main\nVAR\n    (* truST plant: declarations sha256:"),
            "{text}"
        );
        assert_eq!(
            write(
                &text,
                "src/Main.st",
                "Main",
                &only("    P101 : Plant.FB_Pump;\n", "    P101();\n"),
            )
            .as_deref(),
            Ok(text.as_str()),
            "the written program parses with its regions in place"
        );
    }

    #[test]
    fn a_loop_gets_its_retained_region_and_the_using_directives_it_needs() {
        let source = "USING Plant;\n\nPROGRAM Main\nVAR\n    Level : REAL;\nEND_VAR\n    Level := Level + 1.0;\nEND_PROGRAM\n";
        let regions = Generated {
            declarations: "    TIC101 : Control.PID;\n",
            retained: "    TIC101_Setpoint : LREAL;\n",
            calls: "    TIC101_Output := TIC101.Update(Input := TIC101_Input);\n",
            usings: &["Control", "Plant"],
        };
        let text = write(source, "src/Main.st", "Main", &regions).expect("the regions are added");
        assert!(
            text.starts_with("USING Plant;\nUSING Control;\n\nPROGRAM Main\nVAR\n"),
            "{text}"
        );
        assert_eq!(
            text.matches("USING Plant;").count(),
            1,
            "the file's own USING is seen: {text}"
        );
        assert!(
            text.contains(
                "(* end of truST plant declarations *)\nEND_VAR\nVAR RETAIN\n    (* truST plant: retained declarations sha256:"
            ),
            "{text}"
        );
        assert_eq!(
            write(&text, "src/Main.st", "Main", &regions).as_deref(),
            Ok(text.as_str()),
            "written again it changes nothing"
        );
        // A program that has its regions already gets the retained one after their VAR block.
        let plain = write(
            source,
            "src/Main.st",
            "Main",
            &only("    P101 : Plant.FB_Pump;\n", "    P101();\n"),
        )
        .expect("the regions are added");
        let grown =
            write(&plain, "src/Main.st", "Main", &regions).expect("the retained region is added");
        assert!(
            grown.contains("(* end of truST plant declarations *)\nEND_VAR\nVAR RETAIN\n"),
            "{grown}"
        );
        let root = parse(&grown).syntax();
        let node = program_node(&root, "Main").expect("the program parses");
        assert!(find(&grown, "src/Main.st", range_of(&node), Kind::Retained)
            .expect("the region is well formed")
            .is_some_and(|region| region.intact));
    }

    #[test]
    fn a_using_directive_after_a_comment_is_seen() {
        let source = "// The plant.\n// Its program.\nUSING Plant;\nUSING Alarm;\n\nPROGRAM Main\nVAR\n    Level : REAL;\nEND_VAR\n    Level := Level + 1.0;\nEND_PROGRAM\n";
        let regions = Generated {
            declarations: "    TIC101 : Control.PID;\n",
            retained: "    TIC101_Setpoint : LREAL;\n",
            calls: "    TIC101_Output := TIC101.Update(Input := TIC101_Input);\n",
            usings: &["Control", "Plant"],
        };
        let text = write(source, "src/Main.st", "Main", &regions).expect("the regions are added");
        assert!(
            text.starts_with(
                "// The plant.\n// Its program.\nUSING Plant;\nUSING Alarm;\nUSING Control;\n\nPROGRAM Main\n"
            ),
            "{text}"
        );
    }

    #[test]
    fn a_new_program_holds_only_its_regions() {
        let text = new_program("Line2", &only("    M1 : Plant.FB_Motor;\n", "    M1();\n"));
        let root = parse(&text).syntax();
        let node = program_node(&root, "Line2").expect("the program parses");
        assert!(find(&text, "src/Line2.st", range_of(&node), Kind::Calls)
            .expect("the region is well formed")
            .is_some_and(|region| region.intact));
    }
}
