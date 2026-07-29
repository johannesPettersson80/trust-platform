use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};

use anyhow::{bail, Context, Result};
use proc_macro2::{TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{Expr, ExprCall, Macro};

use super::repository_source_tree::source_tree_digests;
use super::sha256_tag;

#[derive(Default)]
struct ExternalOracleVisitor {
    host_runtime_observations: Vec<Macro>,
    includes: BTreeSet<String>,
    runtime_reads: BTreeSet<String>,
    snapshot_names: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for ExternalOracleVisitor {
    fn visit_macro(&mut self, expression: &'ast Macro) {
        let name = expression
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .unwrap_or_default();
        if matches!(name.as_str(), "include_str" | "include_bytes") {
            if let Some(value) = first_string_literal(&expression.tokens) {
                self.includes.insert(value);
            }
        } else if name.contains("assert_") && name.ends_with("snapshot") {
            if let Some(value) = first_string_literal(&expression.tokens) {
                self.snapshot_names.insert(value);
            }
        } else if name == "linux_proc_self_status_read_to_string" {
            self.host_runtime_observations.push(expression.clone());
        }
        scan_nested_external_oracles(&expression.tokens, self);
        visit::visit_macro(self, expression);
    }

    fn visit_expr_call(&mut self, expression: &'ast ExprCall) {
        if runtime_fixture_reader(expression).is_some() {
            if let Some(Expr::Lit(value)) = expression.args.first() {
                if let syn::Lit::Str(path) = &value.lit {
                    self.runtime_reads.insert(path.value());
                }
            }
        }
        visit::visit_expr_call(self, expression);
    }
}

pub(super) fn validate_explicit_external_oracles(file: &syn::File) -> Result<()> {
    let mut visitor = ExternalOracleVisitor::default();
    visitor.visit_file(file);
    for observation in visitor.host_runtime_observations {
        if !observation.tokens.is_empty() {
            bail!(
                "linux_proc_self_status_read_to_string! is a fixed host observation and accepts no parameters"
            )
        }
    }
    Ok(())
}

pub(super) fn runtime_fixture_reader(expression: &ExprCall) -> Option<String> {
    let Expr::Path(path) = expression.func.as_ref() else {
        return None;
    };
    let segments = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    if segments.len() < 2
        || segments.get(segments.len() - 2).map(String::as_str) != Some("fs")
        || !matches!(
            segments.last().map(String::as_str),
            Some("read" | "read_to_string")
        )
    {
        return None;
    }
    Some(segments.join("::"))
}

fn is_cross_platform_absolute_path(literal: &str) -> bool {
    let bytes = literal.as_bytes();
    Path::new(literal).is_absolute()
        || literal.starts_with(['/', '\\'])
        || matches!(
            (bytes.first(), bytes.get(1), bytes.get(2)),
            (Some(drive), Some(b':'), Some(b'/' | b'\\'))
                if drive.is_ascii_alphabetic()
        )
}

fn first_string_literal(tokens: &TokenStream) -> Option<String> {
    tokens.clone().into_iter().find_map(|token| match token {
        TokenTree::Literal(literal) => syn::parse_str::<syn::LitStr>(&literal.to_string())
            .ok()
            .map(|value| value.value()),
        _ => None,
    })
}

fn scan_nested_external_oracles(tokens: &TokenStream, visitor: &mut ExternalOracleVisitor) {
    let trees = tokens.clone().into_iter().collect::<Vec<_>>();
    for (index, token) in trees.iter().enumerate() {
        if let TokenTree::Group(group) = token {
            scan_nested_external_oracles(&group.stream(), visitor);
        }
        let TokenTree::Ident(name) = token else {
            continue;
        };
        let Some(TokenTree::Punct(marker)) = trees.get(index + 1) else {
            continue;
        };
        let Some(TokenTree::Group(group)) = trees.get(index + 2) else {
            continue;
        };
        if marker.as_char() != '!' {
            continue;
        }
        let name = name.to_string();
        if matches!(name.as_str(), "include_str" | "include_bytes") {
            if let Some(value) = first_string_literal(&group.stream()) {
                visitor.includes.insert(value);
            }
        } else if name.contains("assert_") && name.ends_with("snapshot") {
            if let Some(value) = first_string_literal(&group.stream()) {
                visitor.snapshot_names.insert(value);
            }
        }
    }
}

pub(super) fn external_oracle_digests(
    root: &Path,
    source_path: &str,
    file: &syn::File,
) -> Result<BTreeMap<String, String>> {
    let mut visitor = ExternalOracleVisitor::default();
    visitor.visit_file(file);
    let source_parent = root
        .join(source_path)
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Rust assertion source has no parent: {source_path}"))?
        .to_path_buf();
    let mut digests = source_tree_digests(root, file)?;
    for include in visitor.includes {
        let path = source_parent.join(include);
        let relative = closed_oracle_path(root, &path)?;
        digests.insert(relative, sha256_tag(&fs::read(&path)?));
    }
    for runtime_read in visitor.runtime_reads {
        let literal_path = Path::new(&runtime_read);
        if is_cross_platform_absolute_path(&runtime_read)
            || literal_path
                .components()
                .any(|component| matches!(component, Component::ParentDir))
        {
            continue;
        }
        let relative =
            resolve_runtime_fixture_path(root, &source_parent, source_path, &runtime_read)?;
        let path = root.join(&relative);
        digests.insert(relative, sha256_tag(&fs::read(path)?));
    }
    if !visitor.snapshot_names.is_empty() {
        let snapshot_dir = source_parent.join("snapshots");
        if snapshot_dir.is_dir() {
            let entries = fs::read_dir(&snapshot_dir)
                .with_context(|| format!("read snapshot directory {}", snapshot_dir.display()))?
                .collect::<std::io::Result<Vec<_>>>()?;
            for name in visitor.snapshot_names {
                let suffix = format!("__{name}.snap");
                let matches = entries
                    .iter()
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.file_name()
                            .and_then(|value| value.to_str())
                            .is_some_and(|value| value.ends_with(&suffix))
                    })
                    .collect::<Vec<_>>();
                if matches.len() > 1 {
                    bail!(
                        "named snapshot {name} from {source_path} resolves to {} files",
                        matches.len()
                    )
                }
                if let Some(path) = matches.first() {
                    let relative = closed_oracle_path(root, path)?;
                    digests.insert(relative, sha256_tag(&fs::read(path)?));
                }
            }
        }
    }
    Ok(digests)
}

fn resolve_runtime_fixture_path(
    root: &Path,
    source_parent: &Path,
    source_path: &str,
    literal: &str,
) -> Result<String> {
    let literal_path = Path::new(literal);
    if is_cross_platform_absolute_path(literal)
        || literal_path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        bail!("runtime fixture path from {source_path} is not closed: {literal}")
    }
    let mut matches = BTreeSet::new();
    for candidate in [root.join(literal_path), source_parent.join(literal_path)] {
        if candidate.is_file() {
            matches.insert(closed_oracle_path(root, &candidate)?);
        }
    }
    if matches.len() != 1 {
        bail!(
            "runtime fixture path from {source_path} resolved to {} files: {literal}",
            matches.len()
        )
    }
    Ok(matches
        .into_iter()
        .next()
        .expect("one runtime fixture match"))
}

fn closed_oracle_path(root: &Path, path: &Path) -> Result<String> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("inspect assertion oracle {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!(
            "assertion oracle must be a regular non-symlink file: {}",
            path.display()
        )
    }
    let canonical_root = root
        .canonicalize()
        .context("canonicalize repository root")?;
    let canonical_path = path
        .canonicalize()
        .with_context(|| format!("canonicalize assertion oracle {}", path.display()))?;
    let relative = canonical_path
        .strip_prefix(&canonical_root)
        .with_context(|| format!("assertion oracle escapes repository: {}", path.display()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}
