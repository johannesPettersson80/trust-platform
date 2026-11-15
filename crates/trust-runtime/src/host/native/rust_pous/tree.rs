//! The module tree of the project's crate (specification 48 RU-22, RU-24): the files the crate is
//! built from, from its root file through its `mod` declarations as the Rust compiler finds them,
//! and the Rust POUs they declare, each with the path the program's generated code names it by.
//! A module behind `#[cfg(test)]` is the crate's tests and declares no POU.

use std::path::{Path, PathBuf};

use proc_macro2::{Span, TokenStream};
use syn::ext::IdentExt as _;
use syn::{Attribute, Ident, Item, UseTree, Visibility};
use trust_plc_decl::{Kind, Pou};

use super::{RustPou, RustPouError};

/// What the walk found: the POUs, or the errors that keep their declarations from being written.
pub(super) struct Found {
    pub(super) pous: Vec<RustPou>,
    pub(super) errors: Vec<RustPouError>,
}

/// A module: its path from the crate's root and whether every module on the way is `pub`.
#[derive(Clone)]
struct Module {
    path: Vec<String>,
    public: bool,
}

impl Module {
    fn child(&self, name: String, public: bool) -> Self {
        let mut path = self.path.clone();
        path.push(name);
        Self {
            path,
            public: self.public && public,
        }
    }
}

/// A POU as its module declares it, before the walk knows how the generated code reaches it.
struct Declared {
    pou: RustPou,
    public: bool,
    module: Vec<String>,
    item: String,
    span: Span,
}

/// Where items lie: the file's label and path, and the folder its child modules' files lie in.
struct Place<'a> {
    label: &'a str,
    file: &'a Path,
    children: PathBuf,
    /// Inside an inline `mod name { ... }`, whose `#[path]` is relative to `children`.
    inline: bool,
}

struct Walk<'a> {
    project: &'a Path,
    declared: Vec<Declared>,
    /// The `pub use` of the crate's root: the path each names without `crate::` or `self::`, and
    /// the name it exports, `*` for a glob.
    exports: Vec<(Vec<String>, String)>,
    errors: Vec<RustPouError>,
}

/// Walks the crate whose root file is `root` in the project in `project`.
pub(super) fn walk(project: &Path, root: &Path) -> Found {
    let mut walk = Walk {
        project,
        declared: Vec::new(),
        exports: Vec::new(),
        errors: Vec::new(),
    };
    let children = root.parent().map(Path::to_path_buf).unwrap_or_default();
    let module = Module {
        path: Vec::new(),
        public: true,
    };
    walk.file(root, &module, children);
    walk.finish()
}

impl Walk<'_> {
    /// The label of `file`: its path relative to the project, with `/`.
    fn label(&self, file: &Path) -> String {
        let relative = file.strip_prefix(self.project).unwrap_or(file);
        relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    }

    fn error(&mut self, label: &str, span: Span, message: String) {
        let start = span.start();
        self.errors.push(RustPouError {
            file: label.to_string(),
            line: start.line,
            column: start.column + 1,
            message,
        });
    }

    /// The items of the module `module` in `file`, whose child modules' files lie in `children`.
    fn file(&mut self, file: &Path, module: &Module, children: PathBuf) {
        let label = self.label(file);
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(error) => {
                self.errors.push(RustPouError {
                    file: label,
                    line: 0,
                    column: 0,
                    message: error.to_string(),
                });
                return;
            }
        };
        match syn::parse_file(&text) {
            Ok(parsed) => {
                let place = Place {
                    label: &label,
                    file,
                    children,
                    inline: false,
                };
                self.items(&parsed.items, &place, module);
            }
            Err(error) => self.error(&label, error.span(), error.to_string()),
        }
    }

    fn items(&mut self, items: &[Item], place: &Place<'_>, module: &Module) {
        for item in items {
            match item {
                Item::Mod(declared) if !is_test_only(&declared.attrs) => {
                    let name = declared.ident.unraw().to_string();
                    let public = matches!(declared.vis, Visibility::Public(_));
                    let child = module.child(name.clone(), public);
                    let folder = place.children.join(&name);
                    if let Some((_, content)) = &declared.content {
                        let inner = Place {
                            label: place.label,
                            file: place.file,
                            children: folder,
                            inline: true,
                        };
                        self.items(content, &inner, &child);
                    } else {
                        self.module_file(place, &declared.attrs, &declared.ident, &child, folder);
                    }
                }
                Item::Struct(item) if !is_test_only(&item.attrs) => {
                    self.pou(
                        place.label,
                        module,
                        &item.attrs,
                        &item.ident,
                        Kind::FunctionBlock,
                        |args| trust_plc_decl::function_block(args, item),
                    );
                }
                Item::Fn(item) if !is_test_only(&item.attrs) => {
                    self.pou(
                        place.label,
                        module,
                        &item.attrs,
                        &item.sig.ident,
                        Kind::Function,
                        |args| trust_plc_decl::function(args, item),
                    );
                }
                Item::Use(used)
                    if module.path.is_empty() && matches!(used.vis, Visibility::Public(_)) =>
                {
                    exports(&used.tree, &mut Vec::new(), &mut self.exports);
                }
                _ => {}
            }
        }
    }

    /// The file of the module `child`, declared `mod name;` in `place`: `#[path]`'s, else
    /// `name.rs` or `name/mod.rs` beside the declaring file's own modules.
    fn module_file(
        &mut self,
        place: &Place<'_>,
        attributes: &[Attribute],
        ident: &Ident,
        child: &Module,
        folder: PathBuf,
    ) {
        if let Some(path) = path_attribute(attributes) {
            let base = if place.inline {
                place.children.parent().map(Path::to_path_buf)
            } else {
                place.file.parent().map(Path::to_path_buf)
            };
            let target = base.unwrap_or_default().join(path);
            // A file named by `#[path]` keeps its child modules beside it, as `mod.rs` does.
            let children = target.parent().map(Path::to_path_buf).unwrap_or_default();
            self.file(&target, child, children);
            return;
        }
        let name = ident.unraw().to_string();
        let flat = place.children.join(format!("{name}.rs"));
        let nested = folder.join("mod.rs");
        if flat.is_file() {
            self.file(&flat, child, folder);
        } else if nested.is_file() {
            self.file(&nested, child, folder);
        } else {
            let message = format!(
                "the module `{name}` has no file: {} or {}",
                self.label(&flat),
                self.label(&nested)
            );
            self.error(place.label, ident.span(), message);
        }
    }

    /// The POU the item `ident` with the attributes `attributes` declares, when one of them is a
    /// POU's attribute: `read` reads the item with that attribute's arguments.
    fn pou(
        &mut self,
        label: &str,
        module: &Module,
        attributes: &[Attribute],
        ident: &Ident,
        item_kind: Kind,
        read: impl FnOnce(TokenStream) -> syn::Result<Pou>,
    ) {
        let Some((attribute, kind)) = attributes.iter().find_map(|attribute| {
            trust_plc_decl::declared_kind(attribute).map(|kind| (attribute, kind))
        }) else {
            return;
        };
        if kind != item_kind {
            self.error(
                label,
                ident.span(),
                "`#[trust_fb]` declares a struct and `#[trust_function]` a function".into(),
            );
            return;
        }
        let args = attribute
            .meta
            .require_list()
            .map(|list| list.tokens.clone())
            .unwrap_or_default();
        match read(args) {
            Ok(pou) => {
                let item = ident.unraw().to_string();
                let mut path = module.path.clone();
                path.push(item.clone());
                self.declared.push(Declared {
                    pou: RustPou {
                        pou,
                        file: label.to_string(),
                        line: ident.span().start().line,
                        path,
                    },
                    public: module.public,
                    module: module.path.clone(),
                    item,
                    span: ident.span(),
                });
            }
            Err(errors) => {
                for error in errors {
                    self.error(label, error.span(), error.to_string());
                }
            }
        }
    }

    /// The POUs with the paths the generated code reaches them by: through `pub` modules, or as
    /// the crate root's `pub use` of the item or of its module's items; and no name twice.
    fn finish(mut self) -> Found {
        let mut pous: Vec<(RustPou, Span)> = Vec::new();
        for declared in std::mem::take(&mut self.declared) {
            let Some(path) = self.reachable(&declared) else {
                let module = declared.module.join("::");
                let message = format!(
                    "{} lies in the module `{module}`, which is not public: declare the modules \
                     `pub mod`, or re-export it in the crate's root with `pub use {module}::{};`, so \
                     that the program's generated code can call it (specification 48 RU-22)",
                    declared.pou.pou.name, declared.item
                );
                self.error(&declared.pou.file.clone(), declared.span, message);
                continue;
            };
            if let Some((first, _)) = pous
                .iter()
                .find(|(other, _)| other.pou.name.eq_ignore_ascii_case(&declared.pou.pou.name))
            {
                let message = format!(
                    "{} is declared twice: also at {}:{}",
                    declared.pou.pou.name, first.file, first.line
                );
                self.error(&declared.pou.file.clone(), declared.span, message);
                continue;
            }
            let mut pou = declared.pou;
            pou.path = path;
            pous.push((pou, declared.span));
        }
        Found {
            pous: pous.into_iter().map(|(pou, _)| pou).collect(),
            errors: self.errors,
        }
    }

    fn reachable(&self, declared: &Declared) -> Option<Vec<String>> {
        if declared.public {
            return Some(declared.pou.path.clone());
        }
        self.exports.iter().find_map(|(used, name)| {
            if name == "*" {
                (*used == declared.module).then(|| vec![declared.item.clone()])
            } else {
                (*used == declared.pou.path).then(|| vec![name.clone()])
            }
        })
    }
}

/// The paths a `pub use` tree names, each with the name it exports (`*` for a glob), without a
/// leading `crate` or `self`.
fn exports(tree: &UseTree, prefix: &mut Vec<String>, found: &mut Vec<(Vec<String>, String)>) {
    let full = |prefix: &Vec<String>, last: Option<String>| {
        let mut path: Vec<String> = prefix
            .iter()
            .skip_while(|segment| *segment == "crate" || *segment == "self")
            .cloned()
            .collect();
        path.extend(last);
        path
    };
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.unraw().to_string());
            exports(&path.tree, prefix, found);
            prefix.pop();
        }
        UseTree::Name(name) => {
            let name = name.ident.unraw().to_string();
            found.push((full(prefix, Some(name.clone())), name));
        }
        UseTree::Rename(rename) => found.push((
            full(prefix, Some(rename.ident.unraw().to_string())),
            rename.rename.unraw().to_string(),
        )),
        UseTree::Glob(_) => found.push((full(prefix, None), "*".into())),
        UseTree::Group(group) => {
            for tree in &group.items {
                exports(tree, prefix, found);
            }
        }
    }
}

/// Whether the attributes hold `#[cfg(test)]`: the crate's tests, compiled only by `cargo test`.
fn is_test_only(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            && attribute
                .parse_args::<Ident>()
                .is_ok_and(|ident| ident == "test")
    })
}

/// The file `#[path = "..."]` names.
fn path_attribute(attributes: &[Attribute]) -> Option<String> {
    attributes.iter().find_map(|attribute| {
        let syn::Meta::NameValue(pair) = &attribute.meta else {
            return None;
        };
        if !pair.path.is_ident("path") {
            return None;
        }
        match &pair.value {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(text),
                ..
            }) => Some(text.value()),
            _ => None,
        }
    })
}
