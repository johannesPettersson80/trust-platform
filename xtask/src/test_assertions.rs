//! Syntax-aware Rust test assertion inventory for mapping review.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path};

use anyhow::{bail, Context, Result};
use proc_macro2::Span;
use quote::ToTokens;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, ExprCall, ExprMethodCall, Item, ItemFn, Macro};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssertionRecord {
    pub assertion_id: String,
    pub kind: String,
    pub function: String,
    pub line: usize,
    pub expression_digest: String,
    pub expression: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssertionInventory {
    pub schema_version: u32,
    pub path: String,
    pub test_name: String,
    pub source_digest: String,
    pub assertions: Vec<AssertionRecord>,
    pub unresolved_assertion_helpers: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchRequest {
    schema_version: u32,
    tests: Vec<TestRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TestRequest {
    path: String,
    name: String,
}

#[derive(Debug, Serialize)]
struct BatchResponse {
    schema_version: u32,
    inventories: Vec<AssertionInventory>,
}

#[derive(Clone)]
struct FunctionRecord<'ast> {
    qualified_name: String,
    item: &'ast ItemFn,
}

#[derive(Debug, Default)]
struct BodyObservations {
    sites: Vec<RawAssertion>,
    calls: BTreeSet<String>,
    unresolved_assertion_helpers: BTreeSet<String>,
}

#[derive(Debug)]
struct RawAssertion {
    kind: String,
    function: String,
    span: Span,
    expression: String,
}

pub fn run(root: &Path, arguments: &[String]) -> Result<()> {
    let (path, test_name) = parse_arguments(arguments)?;
    let source_path = root.join(&path);
    let source = fs::read_to_string(&source_path)
        .with_context(|| format!("read Rust test source {}", source_path.display()))?;
    let inventory = extract(&path, &test_name, &source)?;
    println!("{}", serde_json::to_string(&inventory)?);
    Ok(())
}

pub fn run_batch(root: &Path, arguments: &[String]) -> Result<()> {
    if !arguments.is_empty() {
        bail!("test-assertions-batch takes its closed request on stdin")
    }
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("read test-assertions-batch request")?;
    let request: BatchRequest =
        serde_json::from_str(&input).context("parse test-assertions-batch request")?;
    if request.schema_version != 1 {
        bail!("test-assertions-batch schema_version must equal 1")
    }
    let mut keys = BTreeSet::new();
    let mut inventories = Vec::new();
    for test in request.tests {
        let key = (test.path.clone(), test.name.clone());
        if !keys.insert(key) {
            bail!(
                "test-assertions-batch duplicates {}#{}",
                test.path,
                test.name
            )
        }
        let parsed = Path::new(&test.path);
        if parsed.is_absolute()
            || parsed
                .components()
                .any(|component| matches!(component, Component::ParentDir))
        {
            bail!("test-assertions-batch path must stay repository-relative")
        }
        let source_path = root.join(&test.path);
        let source = fs::read_to_string(&source_path)
            .with_context(|| format!("read Rust test source {}", source_path.display()))?;
        inventories.push(extract(&test.path, &test.name, &source)?);
    }
    inventories.sort_by(|left, right| {
        (left.path.as_str(), left.test_name.as_str())
            .cmp(&(right.path.as_str(), right.test_name.as_str()))
    });
    println!(
        "{}",
        serde_json::to_string(&BatchResponse {
            schema_version: 1,
            inventories,
        })?
    );
    Ok(())
}

fn parse_arguments(arguments: &[String]) -> Result<(String, String)> {
    let mut path = None;
    let mut test_name = None;
    let mut index = 0;
    while index < arguments.len() {
        let flag = &arguments[index];
        let value = arguments
            .get(index + 1)
            .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?;
        match flag.as_str() {
            "--path" if path.is_none() => path = Some(value.clone()),
            "--name" if test_name.is_none() => test_name = Some(value.clone()),
            "--path" | "--name" => bail!("duplicate test-assertions argument {flag}"),
            _ => bail!("unknown test-assertions argument {flag}"),
        }
        index += 2;
    }
    let path = path.ok_or_else(|| anyhow::anyhow!("test-assertions requires --path"))?;
    let parsed = Path::new(&path);
    if parsed.is_absolute()
        || parsed
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        bail!("test-assertions --path must stay repository-relative")
    }
    let test_name = test_name.ok_or_else(|| anyhow::anyhow!("test-assertions requires --name"))?;
    if test_name.trim().is_empty() {
        bail!("test-assertions --name must not be empty")
    }
    Ok((path, test_name))
}

pub fn extract(path: &str, test_name: &str, source: &str) -> Result<AssertionInventory> {
    let file = syn::parse_file(source).with_context(|| format!("parse Rust test source {path}"))?;
    let mut functions = Vec::new();
    collect_functions(&file.items, &mut Vec::new(), &mut functions);
    let candidates = functions
        .iter()
        .filter(|record| {
            record.item.sig.ident == test_name && has_test_attribute(&record.item.attrs)
        })
        .collect::<Vec<_>>();
    if candidates.len() != 1 {
        bail!(
            "Rust test {path}#{test_name} resolved to {} syntax nodes",
            candidates.len()
        )
    }
    let by_simple_name = index_functions(&functions);
    let mut queue = vec![candidates[0].qualified_name.clone()];
    let mut visited = BTreeSet::new();
    let by_qualified = functions
        .iter()
        .map(|record| (record.qualified_name.clone(), record))
        .collect::<BTreeMap<_, _>>();
    let mut sites = Vec::new();
    let mut unresolved = BTreeSet::new();
    while let Some(function_name) = queue.pop() {
        if !visited.insert(function_name.clone()) {
            continue;
        }
        let function = by_qualified
            .get(&function_name)
            .ok_or_else(|| anyhow::anyhow!("missing indexed Rust function {function_name}"))?;
        if has_should_panic_attribute(&function.item.attrs) {
            sites.push(RawAssertion {
                kind: "should_panic_attribute".to_string(),
                function: function_name.clone(),
                span: function.item.sig.ident.span(),
                expression: "#[should_panic]".to_string(),
            });
        }
        let mut visitor = AssertionVisitor {
            function: function_name.clone(),
            observations: BodyObservations::default(),
        };
        visitor.visit_block(&function.item.block);
        sites.extend(visitor.observations.sites);
        unresolved.extend(visitor.observations.unresolved_assertion_helpers);
        for called in visitor.observations.calls {
            match by_simple_name.get(&called) {
                Some(matches) if matches.len() == 1 => {
                    queue.push(matches[0].qualified_name.clone());
                }
                Some(_) if looks_like_assertion_helper(&called) => {
                    unresolved.insert(format!("{function_name}::{called}:ambiguous"));
                }
                None if looks_like_assertion_helper(&called) => {
                    unresolved.insert(format!("{function_name}::{called}:unresolved"));
                }
                _ => {}
            }
        }
    }
    let mut assertions = sites
        .into_iter()
        .map(|site| {
            let line = site.span.start().line;
            let expression = normalize_expression(&site.expression);
            let expression_digest = sha256_tag(expression.as_bytes());
            let identity = format!(
                "{path}\0{test_name}\0{}\0{}\0{line}\0{expression_digest}",
                site.function, site.kind
            );
            AssertionRecord {
                assertion_id: format!(
                    "ASSERT_{}",
                    &sha256_hex(identity.as_bytes()).to_uppercase()[..24]
                ),
                kind: site.kind,
                function: site.function,
                line,
                expression_digest,
                expression,
            }
        })
        .collect::<Vec<_>>();
    assertions.sort_by(|left, right| {
        (
            left.function.as_str(),
            left.line,
            left.kind.as_str(),
            left.assertion_id.as_str(),
        )
            .cmp(&(
                right.function.as_str(),
                right.line,
                right.kind.as_str(),
                right.assertion_id.as_str(),
            ))
    });
    assertions.dedup_by(|left, right| left.assertion_id == right.assertion_id);
    Ok(AssertionInventory {
        schema_version: 1,
        path: path.to_string(),
        test_name: test_name.to_string(),
        source_digest: sha256_tag(source.as_bytes()),
        assertions,
        unresolved_assertion_helpers: unresolved.into_iter().collect(),
    })
}

fn collect_functions<'ast>(
    items: &'ast [Item],
    modules: &mut Vec<String>,
    output: &mut Vec<FunctionRecord<'ast>>,
) {
    for item in items {
        match item {
            Item::Fn(function) => {
                let mut segments = modules.clone();
                segments.push(function.sig.ident.to_string());
                output.push(FunctionRecord {
                    qualified_name: segments.join("::"),
                    item: function,
                });
            }
            Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    modules.push(module.ident.to_string());
                    collect_functions(nested, modules, output);
                    modules.pop();
                }
            }
            _ => {}
        }
    }
}

fn index_functions<'ast>(
    functions: &'ast [FunctionRecord<'ast>],
) -> BTreeMap<String, Vec<&'ast FunctionRecord<'ast>>> {
    let mut indexed: BTreeMap<String, Vec<&FunctionRecord<'ast>>> = BTreeMap::new();
    for function in functions {
        if let Some(name) = function.qualified_name.rsplit("::").next() {
            indexed.entry(name.to_string()).or_default().push(function);
        }
    }
    indexed
}

struct AssertionVisitor {
    function: String,
    observations: BodyObservations,
}

impl<'ast> Visit<'ast> for AssertionVisitor {
    fn visit_macro(&mut self, expression: &'ast Macro) {
        let name = expression
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .unwrap_or_default();
        if is_assertion_macro(&name) {
            self.observations.sites.push(RawAssertion {
                kind: format!("macro:{name}"),
                function: self.function.clone(),
                span: expression.span(),
                expression: expression.to_token_stream().to_string(),
            });
        }
        visit::visit_macro(self, expression);
    }

    fn visit_expr_method_call(&mut self, expression: &'ast ExprMethodCall) {
        let method = expression.method.to_string();
        if is_assertion_method(&method) {
            self.observations.sites.push(RawAssertion {
                kind: format!("method:{method}"),
                function: self.function.clone(),
                span: expression.span(),
                expression: expression.to_token_stream().to_string(),
            });
        }
        visit::visit_expr_method_call(self, expression);
    }

    fn visit_expr_call(&mut self, expression: &'ast ExprCall) {
        if let Expr::Path(path) = expression.func.as_ref() {
            if let Some(segment) = path.path.segments.last() {
                self.observations.calls.insert(segment.ident.to_string());
            }
        }
        visit::visit_expr_call(self, expression);
    }
}

fn has_test_attribute(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "test")
    })
}

fn has_should_panic_attribute(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "should_panic")
    })
}

fn is_assertion_macro(name: &str) -> bool {
    name.starts_with("assert")
        || name.starts_with("debug_assert")
        || matches!(name, "panic" | "unreachable")
}

fn is_assertion_method(name: &str) -> bool {
    matches!(name, "expect" | "expect_err" | "unwrap" | "unwrap_err")
}

fn looks_like_assertion_helper(name: &str) -> bool {
    ["assert", "expect", "verify", "check"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

fn normalize_expression(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sha256_tag(value: &[u8]) -> String {
    format!("sha256:{}", sha256_hex(value))
}

fn sha256_hex(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

#[cfg(test)]
mod tests {
    use super::extract;

    #[test]
    fn follows_unique_local_helper_and_records_exact_assertion_sites() {
        let source = r#"
fn expect_rejected(value: Result<(), String>) {
    let error = value.expect_err("must reject");
    assert!(error.contains("invalid"));
}

#[test]
fn rejects_invalid() {
    expect_rejected(Err("invalid".into()));
}
"#;
        let inventory = extract("crates/example/tests/reject.rs", "rejects_invalid", source)
            .expect("extract assertions");

        assert_eq!(2, inventory.assertions.len());
        assert!(inventory
            .assertions
            .iter()
            .any(|site| site.kind == "method:expect_err"));
        assert!(inventory
            .assertions
            .iter()
            .any(|site| site.kind == "macro:assert"));
        assert!(inventory.unresolved_assertion_helpers.is_empty());
    }

    #[test]
    fn unresolved_assertion_helper_is_explicit() {
        let source = r#"
#[test]
fn delegates() {
    expect_external_contract();
}
"#;
        let inventory = extract("crates/example/tests/delegate.rs", "delegates", source)
            .expect("extract assertions");

        assert_eq!(
            vec!["delegates::expect_external_contract:unresolved"],
            inventory.unresolved_assertion_helpers
        );
    }
}
