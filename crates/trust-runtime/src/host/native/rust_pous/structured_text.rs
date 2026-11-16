//! What a project's crate calls of Structured Text (specification 48 RU-38 to RU-40): the names it
//! writes under `st::`, the library of the generated bindings (`trust_st`, RU-39), which tell the
//! build what layer 1 holds (RU-42). The crate names each one, `st::SCALE(cx, x)` or
//! `use st::{SCALE, HYSTERESIS};`; `use st::*` would hide them, and is refused.

use proc_macro2::{Delimiter, Spacing, Span, TokenStream, TokenTree};
use syn::ext::IdentExt;

use super::RustPouError;

/// A name of Structured Text that the crate writes, where it writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredTextUse {
    /// The name as written: `SCALE` of `st::SCALE`.
    pub name: String,
    /// The file, relative to the project with `/`.
    pub file: String,
    /// The line, from 1.
    pub line: usize,
    /// The column, from 1.
    pub column: usize,
}

/// What the crate's files write under `st::`.
#[derive(Debug, Default)]
pub(super) struct Uses {
    /// The names, in the order the files write them.
    names: Vec<StructuredTextUse>,
    /// Each `use st::*`, as a use named `*`.
    globs: Vec<StructuredTextUse>,
    /// The crate's own `st`, a `mod st` or an `… as st`, the first, as a use named `st`: `st::`
    /// then names it, not the bindings.
    own: Option<StructuredTextUse>,
}

/// The use named `name` at `span` of the file `file`.
fn used(name: String, file: &str, span: Span) -> StructuredTextUse {
    let start = span.start();
    StructuredTextUse {
        name,
        file: file.to_string(),
        line: start.line,
        column: start.column + 1,
    }
}

/// Adds to `uses` what `tokens`, the text of the file `file`, writes under `st::` as a path's first
/// segment, and where it declares an `st` of its own.
pub(super) fn scan(tokens: TokenStream, file: &str, uses: &mut Uses) {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    for (index, token) in tokens.iter().enumerate() {
        if let TokenTree::Group(group) = token {
            scan(group.stream(), file, uses);
            continue;
        }
        let TokenTree::Ident(ident) = token else {
            continue;
        };
        if ident == "mod" || ident == "as" {
            if let Some(TokenTree::Ident(own)) = tokens.get(index + 1)
                && own == "st"
                && uses.own.is_none()
            {
                uses.own = Some(used("st".to_string(), file, own.span()));
            }
            continue;
        }
        let after_path = index >= 2 && is_path_separator(&tokens[index - 2], &tokens[index - 1]);
        if ident != "st" || after_path {
            continue;
        }
        if let (Some(first), Some(second)) = (tokens.get(index + 1), tokens.get(index + 2))
            && is_path_separator(first, second)
        {
            tree(
                tokens.get(index + 3..).unwrap_or_default(),
                ("", None),
                file,
                uses,
            );
        }
    }
}

/// Adds the names of the tree that `tokens` begin with, which follows `st::` and the path before it,
/// its names joined with `.` and where its first name lies (IEC §6.9): a path, its names joined with
/// `.` (`Motion::MOVE_ABS` as `Motion.MOVE_ABS`, `MODE::RUN` as `MODE.RUN`, which the build resolves
/// to what it names), at its first name, `*`, or a group of trees, `{SCALE as Scale, Motion::AXIS}`,
/// each a tree of its own after the path.
fn tree(tokens: &[TokenTree], before: (&str, Option<Span>), file: &str, uses: &mut Uses) {
    let (prefix, start) = before;
    match tokens.first() {
        Some(TokenTree::Ident(name)) if name != "self" => {
            let path = format!("{prefix}{}", name.unraw());
            let start = start.unwrap_or_else(|| name.span());
            let further = match (tokens.get(1), tokens.get(2), tokens.get(3)) {
                (Some(first), Some(second), Some(next)) if is_path_separator(first, second) => {
                    match next {
                        TokenTree::Ident(next) => next != "self",
                        TokenTree::Group(group) => group.delimiter() == Delimiter::Brace,
                        TokenTree::Punct(star) => star.as_char() == '*',
                        TokenTree::Literal(_) => false,
                    }
                }
                _ => false,
            };
            if further {
                tree(&tokens[3..], (&format!("{path}."), Some(start)), file, uses);
            } else {
                uses.names.push(used(path, file, start));
            }
        }
        Some(TokenTree::Punct(star)) if star.as_char() == '*' => {
            uses.globs.push(used(
                format!("{prefix}*"),
                file,
                start.unwrap_or_else(|| star.span()),
            ));
        }
        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace => {
            let inner: Vec<TokenTree> = group.stream().into_iter().collect();
            for item in inner
                .split(|token| matches!(token, TokenTree::Punct(comma) if comma.as_char() == ','))
            {
                tree(item, (prefix, start), file, uses);
            }
        }
        _ => {}
    }
}

/// Whether `first` and `second` are `::`.
fn is_path_separator(first: &TokenTree, second: &TokenTree) -> bool {
    matches!(
        (first, second),
        (TokenTree::Punct(first), TokenTree::Punct(second))
            if first.as_char() == ':' && first.spacing() == Spacing::Joint && second.as_char() == ':'
    )
}

/// The Structured Text that a crate in the project's folder `crate_folder`, whose manifest
/// `manifest` names the bindings or not (`named`), calls by what its files write (`uses`); what
/// refuses the crate goes to `errors` (RU-39).
pub(super) fn calls(
    uses: Uses,
    named: bool,
    crate_folder: &str,
    manifest: &str,
    errors: &mut Vec<RustPouError>,
) -> Vec<StructuredTextUse> {
    let refusal = |at: &StructuredTextUse, message: String| RustPouError {
        file: at.file.clone(),
        line: at.line,
        column: at.column,
        message,
    };
    if let Some(own) = &uses.own {
        // `st::` names the crate's own `st`, which Rust would not tell from the bindings.
        if named {
            errors.push(refusal(
                own,
                "`st` names the Structured Text bindings that the crate's [dependencies] name: \
                 give the crate's own `st` another name (specification 48 RU-39)"
                    .to_string(),
            ));
        }
        return Vec::new();
    }
    for glob in &uses.globs {
        errors.push(refusal(
            glob,
            "`use st::*` hides which Structured Text the crate calls: name each, \
             `use st::{SCALE, HYSTERESIS};` (specification 48 RU-39)"
                .to_string(),
        ));
    }
    if let (Some(first), false) = (uses.names.first(), named) {
        errors.push(RustPouError {
            file: manifest.to_string(),
            line: 0,
            column: 0,
            message: format!(
                "the crate calls Structured Text (st::{} at {}:{}): name its bindings in \
                 [dependencies], `{}` (specification 48 RU-39)",
                first.name,
                first.file,
                first.line,
                dependency_line(crate_folder)
            ),
        });
    }
    uses.names
}

/// The dependency line of the bindings for a crate in `crate_folder`, relative to the project:
/// the path from it to the project's `build/trust_st` (RU-39).
fn dependency_line(crate_folder: &str) -> String {
    let up: String = std::path::Path::new(crate_folder)
        .components()
        .filter(|component| matches!(component, std::path::Component::Normal(_)))
        .map(|_| "../")
        .collect();
    format!("trust_st = {{ path = \"{up}{}\" }}", super::BINDINGS_FOLDER)
}

#[cfg(test)]
#[path = "structured_text_tests.rs"]
mod tests;
