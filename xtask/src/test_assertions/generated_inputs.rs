use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use syn::visit::{self, Visit};
use syn::{
    Block, Expr, ExprAssign, ExprCall, ExprField, ExprMacro, ExprMethodCall, ExprStruct, FnArg,
    Local, Member, Pat, Signature, Token,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct GeneratedPath {
    root: String,
    fields: Vec<String>,
}

impl GeneratedPath {
    fn new(root: String, fields: Vec<String>) -> Self {
        Self { root, fields }
    }
}

pub(super) type GeneratedPaths = BTreeSet<GeneratedPath>;
pub(super) type GeneratedArgumentPaths = BTreeMap<usize, BTreeSet<Vec<String>>>;

pub(super) fn generated_paths(block: &Block, inherited: &GeneratedPaths) -> GeneratedPaths {
    struct GeneratedPathVisitor {
        paths: GeneratedPaths,
        unsafe_bindings: BTreeSet<String>,
        inherited_roots: BTreeSet<String>,
        shadowed_inherited: BTreeSet<String>,
    }

    impl<'ast> Visit<'ast> for GeneratedPathVisitor {
        fn visit_local(&mut self, local: &'ast Local) {
            let Some(initializer) = &local.init else {
                visit::visit_local(self, local);
                return;
            };
            let Pat::Ident(binding) = &local.pat else {
                visit::visit_local(self, local);
                return;
            };
            let name = binding.ident.to_string();
            let derived = expression_paths(initializer.expr.as_ref(), &self.paths);
            let safe_inherited_shadow = self.inherited_roots.contains(&name)
                && !self.shadowed_inherited.contains(&name)
                && !derived.is_empty();
            remove_root(&mut self.paths, &name);
            if !self.unsafe_bindings.insert(name.clone()) && !safe_inherited_shadow {
                visit::visit_local(self, local);
                return;
            }
            if safe_inherited_shadow {
                self.shadowed_inherited.insert(name.clone());
            }
            for fields in derived {
                self.paths.insert(GeneratedPath::new(name.clone(), fields));
            }
            visit::visit_local(self, local);
        }

        fn visit_expr_assign(&mut self, assignment: &'ast ExprAssign) {
            if let Some(root) = access_path(assignment.left.as_ref())
                .and_then(|(root, fields)| fields.is_empty().then_some(root))
            {
                remove_root(&mut self.paths, &root);
                self.unsafe_bindings.insert(root);
            }
            visit::visit_expr_assign(self, assignment);
        }
    }

    let inherited_roots = inherited
        .iter()
        .map(|path| path.root.clone())
        .collect::<BTreeSet<_>>();
    let mut visitor = GeneratedPathVisitor {
        paths: inherited.clone(),
        unsafe_bindings: inherited_roots.clone(),
        inherited_roots,
        shadowed_inherited: BTreeSet::new(),
    };
    visitor.visit_block(block);
    visitor.paths
}

pub(super) fn generated_argument_paths(
    expression: &ExprCall,
    paths: &GeneratedPaths,
) -> GeneratedArgumentPaths {
    expression
        .args
        .iter()
        .enumerate()
        .filter_map(|(index, argument)| {
            let generated = expression_paths(argument, paths);
            (!generated.is_empty()).then_some((index, generated))
        })
        .collect()
}

pub(super) fn inherited_parameter_paths(
    signature: &Signature,
    argument_paths: &GeneratedArgumentPaths,
) -> GeneratedPaths {
    let parameters = signature
        .inputs
        .iter()
        .filter_map(|argument| match argument {
            FnArg::Typed(argument) => match argument.pat.as_ref() {
                Pat::Ident(binding) => Some(binding.ident.to_string()),
                _ => None,
            },
            FnArg::Receiver(_) => None,
        })
        .collect::<Vec<_>>();
    let mut inherited = GeneratedPaths::new();
    for (index, fields_set) in argument_paths {
        let Some(parameter) = parameters.get(*index) else {
            continue;
        };
        for fields in fields_set {
            inherited.insert(GeneratedPath::new(parameter.clone(), fields.clone()));
        }
    }
    inherited
}

pub(super) fn is_generated_read(expression: &ExprCall, paths: &GeneratedPaths) -> bool {
    expression
        .args
        .first()
        .is_some_and(|argument| expression_paths(argument, paths).contains(&Vec::new()))
}

fn expression_paths(expression: &Expr, paths: &GeneratedPaths) -> BTreeSet<Vec<String>> {
    let expression = strip_path_wrappers(expression);
    if let Some((root, fields)) = access_path(expression) {
        return paths
            .iter()
            .filter_map(|candidate| {
                (candidate.root == root && candidate.fields.starts_with(&fields))
                    .then(|| candidate.fields[fields.len()..].to_vec())
            })
            .collect();
    }
    match expression {
        Expr::Call(call) if is_generated_factory_call(call) => BTreeSet::from([Vec::new()]),
        Expr::Macro(value) => generated_output_macro_paths(value, paths),
        Expr::MethodCall(call) => method_call_paths(call, paths),
        Expr::Struct(value) => struct_paths(value, paths),
        _ => BTreeSet::new(),
    }
}

fn method_call_paths(expression: &ExprMethodCall, paths: &GeneratedPaths) -> BTreeSet<Vec<String>> {
    let receiver = expression_paths(expression.receiver.as_ref(), paths);
    if expression.method == "join" && expression.args.len() == 1 {
        return if receiver.contains(&Vec::new())
            && expression
                .args
                .first()
                .is_some_and(is_closed_relative_literal)
        {
            BTreeSet::from([Vec::new()])
        } else {
            BTreeSet::new()
        };
    }
    if matches!(
        expression.method.to_string().as_str(),
        "clone" | "as_ref" | "as_path" | "to_path_buf"
    ) && expression.args.is_empty()
    {
        return receiver;
    }
    BTreeSet::new()
}

fn struct_paths(expression: &ExprStruct, paths: &GeneratedPaths) -> BTreeSet<Vec<String>> {
    let mut output = BTreeSet::new();
    for field in &expression.fields {
        let Member::Named(member) = &field.member else {
            continue;
        };
        for mut nested in expression_paths(&field.expr, paths) {
            nested.insert(0, member.to_string());
            output.insert(nested);
        }
    }
    output
}

fn generated_output_macro_paths(
    expression: &ExprMacro,
    paths: &GeneratedPaths,
) -> BTreeSet<Vec<String>> {
    let macro_name = expression
        .mac
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
        .unwrap_or_default();
    if !matches!(
        macro_name.as_str(),
        "generated_output_path" | "generated_test_root"
    ) {
        return BTreeSet::new();
    }
    let Ok(arguments) = expression
        .mac
        .parse_body_with(syn::punctuated::Punctuated::<Expr, Token![,]>::parse_terminated)
    else {
        return BTreeSet::new();
    };
    if macro_name == "generated_test_root" {
        return (arguments.len() == 1)
            .then(|| BTreeSet::from([Vec::new()]))
            .unwrap_or_default();
    }
    if arguments.len() != 2 {
        return BTreeSet::new();
    }
    let generated_root = arguments
        .iter()
        .nth(1)
        .is_some_and(|root| expression_paths(root, paths).contains(&Vec::new()));
    generated_root
        .then(|| BTreeSet::from([Vec::new()]))
        .unwrap_or_default()
}

fn is_generated_factory_call(expression: &ExprCall) -> bool {
    let Expr::Path(path) = expression.func.as_ref() else {
        return false;
    };
    path.path.segments.last().is_some_and(|segment| {
        let name = segment.ident.to_string();
        name == "temp_dir"
            || name == "tempdir"
            || name.starts_with("temp_")
            || name.starts_with("unique_temp_")
            || matches!(name.as_str(), "copy_fixture" | "make_project" | "test_path")
    })
}

fn is_closed_relative_literal(expression: &Expr) -> bool {
    let Expr::Lit(value) = strip_path_wrappers(expression) else {
        return false;
    };
    let syn::Lit::Str(literal) = &value.lit else {
        return false;
    };
    let value = literal.value();
    let path = Path::new(&value);
    !path.is_absolute()
        && !path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}

fn access_path(expression: &Expr) -> Option<(String, Vec<String>)> {
    match strip_path_wrappers(expression) {
        Expr::Path(path) if path.path.leading_colon.is_none() && path.path.segments.len() == 1 => {
            Some((path.path.segments.first()?.ident.to_string(), Vec::new()))
        }
        Expr::Field(field) => field_access_path(field),
        _ => None,
    }
}

fn field_access_path(field: &ExprField) -> Option<(String, Vec<String>)> {
    let (root, mut fields) = access_path(field.base.as_ref())?;
    let Member::Named(member) = &field.member else {
        return None;
    };
    fields.push(member.to_string());
    Some((root, fields))
}

fn strip_path_wrappers(mut expression: &Expr) -> &Expr {
    loop {
        expression = match expression {
            Expr::Reference(reference) => reference.expr.as_ref(),
            Expr::Paren(paren) => paren.expr.as_ref(),
            Expr::Group(group) => group.expr.as_ref(),
            _ => return expression,
        };
    }
}

fn remove_root(paths: &mut GeneratedPaths, root: &str) {
    paths.retain(|path| path.root != root);
}
