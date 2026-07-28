use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{bracketed, Expr, Ident, LitStr, Macro, Token};

use super::sha256_tag;

const MACRO_NAME: &str = "repository_source_tree_read_to_string";

struct SourceTreeArguments {
    _path: Expr,
    roots: Vec<LitStr>,
    extension: LitStr,
}

impl Parse for SourceTreeArguments {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let path = input.parse()?;
        input.parse::<Token![,]>()?;
        let roots_name: Ident = input.parse()?;
        if roots_name != "roots" {
            return Err(syn::Error::new(roots_name.span(), "expected roots = [...]"));
        }
        input.parse::<Token![=]>()?;
        let content;
        bracketed!(content in input);
        let roots = Punctuated::<LitStr, Token![,]>::parse_terminated(&content)?
            .into_iter()
            .collect();
        input.parse::<Token![,]>()?;
        let extension_name: Ident = input.parse()?;
        if extension_name != "extension" {
            return Err(syn::Error::new(
                extension_name.span(),
                "expected extension = \"...\"",
            ));
        }
        input.parse::<Token![=]>()?;
        let extension = input.parse()?;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
        if !input.is_empty() {
            return Err(input.error("unexpected repository source-tree arguments"));
        }
        Ok(Self {
            _path: path,
            roots,
            extension,
        })
    }
}

#[derive(Default)]
struct SourceTreeVisitor {
    invocations: Vec<Macro>,
}

impl<'ast> Visit<'ast> for SourceTreeVisitor {
    fn visit_macro(&mut self, expression: &'ast Macro) {
        if expression
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == MACRO_NAME)
        {
            self.invocations.push(expression.clone());
        }
        visit::visit_macro(self, expression);
    }
}

pub(super) fn source_tree_digests(
    root: &Path,
    file: &syn::File,
) -> Result<BTreeMap<String, String>> {
    let mut visitor = SourceTreeVisitor::default();
    visitor.visit_file(file);
    let mut digests = BTreeMap::new();
    for invocation in visitor.invocations {
        let arguments = syn::parse2::<SourceTreeArguments>(invocation.tokens.clone())
            .with_context(|| format!("parse {MACRO_NAME}! invocation"))?;
        let extension = arguments.extension.value();
        if extension.is_empty()
            || extension.contains(['/', '\\'])
            || extension == "."
            || extension == ".."
        {
            bail!("{MACRO_NAME}! extension must be one file extension")
        }
        if arguments.roots.is_empty() {
            bail!("{MACRO_NAME}! requires at least one root")
        }
        for literal in arguments.roots {
            let relative = closed_relative_root(&literal.value())?;
            collect_tree(root, &root.join(&relative), &extension, &mut digests)?;
        }
    }
    Ok(digests)
}

fn closed_relative_root(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if value.contains('\\')
        || path.is_absolute()
        || path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("{MACRO_NAME}! root must be a canonical repository-relative path: {value}")
    }
    Ok(path.to_path_buf())
}

fn collect_tree(
    repository_root: &Path,
    directory: &Path,
    extension: &str,
    digests: &mut BTreeMap<String, String>,
) -> Result<()> {
    let metadata = fs::symlink_metadata(directory).with_context(|| {
        format!(
            "{MACRO_NAME}! inspect source-tree root {}",
            directory.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!(
            "{MACRO_NAME}! root must be a regular directory: {}",
            directory.display()
        )
    }
    let mut entries = fs::read_dir(directory)
        .with_context(|| {
            format!(
                "{MACRO_NAME}! read source-tree directory {}",
                directory.display()
            )
        })?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            bail!(
                "{MACRO_NAME}! source tree contains symlink: {}",
                path.display()
            )
        }
        if metadata.is_dir() {
            collect_tree(repository_root, &path, extension, digests)?;
            continue;
        }
        if !metadata.is_file()
            || path.extension().and_then(|value| value.to_str()) != Some(extension)
        {
            continue;
        }
        let relative = path
            .strip_prefix(repository_root)
            .with_context(|| {
                format!(
                    "{MACRO_NAME}! source escaped repository: {}",
                    path.display()
                )
            })?
            .to_string_lossy()
            .replace('\\', "/");
        digests.insert(relative, sha256_tag(&fs::read(&path)?));
    }
    Ok(())
}
