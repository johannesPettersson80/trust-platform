//! Syntax-aware Rust test assertion inventory for mapping review.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use proc_macro2::Span;
use quote::ToTokens;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, ExprCall, ExprMethodCall, Item, ItemFn, Macro, UseTree};

use crate::code_spec_scan::scan_crate_root_module_graph;

const ASSERTION_ROOT_MODULE: &str = "__trust_assertion_root__";
const ASSERTION_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssertionRecord {
    pub assertion_id: String,
    pub kind: String,
    pub source_path: String,
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
    pub source_digests: BTreeMap<String, String>,
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
    source_kind: String,
}

#[derive(Debug, Serialize)]
struct BatchResponse {
    schema_version: u32,
    inventories: Vec<AssertionInventory>,
}

#[derive(Clone)]
struct FunctionRecord {
    qualified_name: String,
    source_path: String,
    module: Vec<String>,
    imports: ImportReachability,
    item: ItemFn,
}

struct ContextSource {
    path: String,
    module: Vec<String>,
    source: String,
}

struct ExtractionContext {
    source_by_path: BTreeMap<String, String>,
    source_digests: BTreeMap<String, String>,
    functions: Vec<FunctionRecord>,
}

#[derive(Clone, Default)]
struct ImportReachability {
    glob_modules: BTreeSet<Vec<String>>,
    bindings: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CallReference {
    leading_colon: bool,
    segments: Vec<String>,
}

impl CallReference {
    fn display(&self) -> String {
        let prefix = if self.leading_colon { "::" } else { "" };
        format!("{prefix}{}", self.segments.join("::"))
    }

    fn simple_name(&self) -> &str {
        self.segments.last().map(String::as_str).unwrap_or_default()
    }
}

#[derive(Debug, Default)]
struct BodyObservations {
    sites: Vec<RawAssertion>,
    calls: BTreeSet<CallReference>,
    unresolved_assertion_helpers: BTreeSet<String>,
}

#[derive(Debug)]
struct RawAssertion {
    kind: String,
    source_path: String,
    function: String,
    span: Span,
    expression: String,
}

pub fn run(root: &Path, arguments: &[String]) -> Result<()> {
    let (path, test_name) = parse_arguments(arguments)?;
    let source_kind = if integration_test_module_root(root, Path::new(&path)).is_some() {
        "rust_integration_test"
    } else {
        "rust_unit_test"
    };
    let inventory = extract_from_repository(root, &path, &test_name, source_kind)?;
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
    if request.schema_version != ASSERTION_SCHEMA_VERSION {
        bail!("test-assertions-batch schema_version must equal {ASSERTION_SCHEMA_VERSION}")
    }
    let mut keys = BTreeSet::new();
    let mut contexts = BTreeMap::new();
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
        let context_key = repository_context_key(root, &test.path, &test.source_kind)?;
        if !contexts.contains_key(&context_key) {
            contexts.insert(
                context_key.clone(),
                build_repository_context(root, &test.path, &test.source_kind)?,
            );
        }
        let context = contexts
            .get(&context_key)
            .ok_or_else(|| anyhow::anyhow!("missing cached assertion context"))?;
        inventories.push(extract_from_built_context(&test.path, &test.name, context)?);
    }
    inventories.sort_by(|left, right| {
        (left.path.as_str(), left.test_name.as_str())
            .cmp(&(right.path.as_str(), right.test_name.as_str()))
    });
    println!(
        "{}",
        serde_json::to_string(&BatchResponse {
            schema_version: ASSERTION_SCHEMA_VERSION,
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
    let context = build_context(
        None,
        vec![ContextSource {
            path: path.to_string(),
            module: Vec::new(),
            source: source.to_string(),
        }],
    )?;
    extract_from_built_context(path, test_name, &context)
}

fn extract_from_repository(
    root: &Path,
    path: &str,
    test_name: &str,
    source_kind: &str,
) -> Result<AssertionInventory> {
    let context = build_repository_context(root, path, source_kind)?;
    extract_from_built_context(path, test_name, &context)
}

fn repository_context_key(root: &Path, path: &str, source_kind: &str) -> Result<String> {
    if source_kind == "rust_unit_test" {
        return Ok(format!("unit:{path}"));
    }
    if source_kind != "rust_integration_test" {
        bail!("unsupported Rust assertion source kind {source_kind}")
    }
    let module_root = integration_test_module_root(root, Path::new(path))
        .ok_or_else(|| anyhow::anyhow!("cannot resolve integration-test module root for {path}"))?;
    let relative = module_root
        .strip_prefix(root)
        .with_context(|| format!("integration-test root escapes repository for {path}"))?;
    Ok(format!(
        "integration:{}",
        relative.to_string_lossy().replace('\\', "/")
    ))
}

fn build_repository_context(
    root: &Path,
    path: &str,
    source_kind: &str,
) -> Result<ExtractionContext> {
    let selected_path = root.join(path);
    if source_kind == "rust_unit_test" {
        let source = fs::read_to_string(&selected_path)
            .with_context(|| format!("read Rust test source {}", selected_path.display()))?;
        return build_context(
            Some(root),
            vec![ContextSource {
                path: path.to_string(),
                module: Vec::new(),
                source,
            }],
        );
    }
    if source_kind != "rust_integration_test" {
        bail!("unsupported Rust assertion source kind {source_kind}")
    }
    let Some(module_root) = integration_test_module_root(root, Path::new(path)) else {
        bail!("cannot resolve integration-test module root for {path}")
    };
    let graph = scan_crate_root_module_graph(root, &module_root, ASSERTION_ROOT_MODULE)?;
    if !graph.diagnostics.is_empty() {
        bail!(
            "Rust assertion context for {path} has module diagnostics: {}",
            graph.diagnostics.join("; ")
        )
    }
    let mut sources = Vec::new();
    for source in graph.sources {
        let source_path = root.join(&source.path);
        let content = fs::read_to_string(&source_path)
            .with_context(|| format!("read Rust assertion context {}", source_path.display()))?;
        sources.push(ContextSource {
            path: source.path,
            module: assertion_module_segments(&source.module)?,
            source: content,
        });
    }
    if !sources.iter().any(|source| source.path == path) {
        bail!("Rust assertion context does not contain selected test source {path}")
    }
    build_context(Some(root), sources)
}

fn extract_from_context(
    path: &str,
    test_name: &str,
    sources: Vec<ContextSource>,
) -> Result<AssertionInventory> {
    let context = build_context(None, sources)?;
    extract_from_built_context(path, test_name, &context)
}

fn build_context(root: Option<&Path>, sources: Vec<ContextSource>) -> Result<ExtractionContext> {
    let mut functions = Vec::new();
    let mut source_by_path = BTreeMap::new();
    let mut source_digests = BTreeMap::new();
    for context in sources {
        let file = syn::parse_file(&context.source)
            .with_context(|| format!("parse Rust assertion source {}", context.path))?;
        collect_functions(
            &file.items,
            &context.path,
            &mut context.module.clone(),
            &mut functions,
        );
        source_digests.insert(context.path.clone(), sha256_tag(context.source.as_bytes()));
        if let Some(root) = root {
            for (oracle_path, digest) in external_oracle_digests(root, &context.path, &file)? {
                if let Some(previous) = source_digests.insert(oracle_path.clone(), digest.clone()) {
                    if previous != digest {
                        bail!("assertion oracle digest conflicts at {oracle_path}")
                    }
                }
            }
        }
        if source_by_path
            .insert(context.path.clone(), context.source)
            .is_some()
        {
            bail!("Rust assertion context duplicates {}", context.path)
        }
    }
    Ok(ExtractionContext {
        source_by_path,
        source_digests,
        functions,
    })
}

fn extract_from_built_context(
    path: &str,
    test_name: &str,
    context: &ExtractionContext,
) -> Result<AssertionInventory> {
    let source = context
        .source_by_path
        .get(path)
        .ok_or_else(|| anyhow::anyhow!("Rust assertion context omits {path}"))?;
    let candidates = context
        .functions
        .iter()
        .filter(|record| {
            record.source_path == path
                && record.item.sig.ident == test_name
                && has_test_attribute(&record.item.attrs)
        })
        .collect::<Vec<_>>();
    if candidates.len() != 1 {
        bail!(
            "Rust test {path}#{test_name} resolved to {} syntax nodes",
            candidates.len()
        )
    }
    let by_simple_name = index_functions(&context.functions);
    let mut queue = vec![candidates[0].qualified_name.clone()];
    let mut visited = BTreeSet::new();
    let mut by_qualified = BTreeMap::new();
    for record in &context.functions {
        if by_qualified
            .insert(record.qualified_name.clone(), record)
            .is_some()
        {
            bail!(
                "Rust assertion context duplicates function {}",
                record.qualified_name
            )
        }
    }
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
                source_path: function.source_path.clone(),
                function: function_name.clone(),
                span: function.item.sig.ident.span(),
                expression: "#[should_panic]".to_string(),
            });
        }
        let mut visitor = AssertionVisitor {
            function: function_name.clone(),
            source_path: function.source_path.clone(),
            generated_output_roots: generated_output_roots(&function.item.block),
            observations: BodyObservations::default(),
        };
        visitor.visit_block(&function.item.block);
        sites.extend(visitor.observations.sites);
        unresolved.extend(visitor.observations.unresolved_assertion_helpers);
        for called in visitor.observations.calls {
            let reachable = resolve_call(function, &called, &by_qualified);
            match reachable.as_slice() {
                [matched] => {
                    queue.push(matched.qualified_name.clone());
                }
                matches if !matches.is_empty() => {
                    unresolved.insert(format!("{function_name}::{}:ambiguous", called.display()));
                }
                [] if by_simple_name.contains_key(called.simple_name())
                    || looks_like_assertion_helper(called.simple_name()) =>
                {
                    unresolved.insert(format!("{function_name}::{}:unresolved", called.display()));
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
                "{path}\0{test_name}\0{}\0{}\0{}\0{line}\0{expression_digest}",
                site.source_path, site.function, site.kind
            );
            AssertionRecord {
                assertion_id: format!(
                    "ASSERT_{}",
                    &sha256_hex(identity.as_bytes()).to_uppercase()[..24]
                ),
                kind: site.kind,
                source_path: site.source_path,
                function: site.function,
                line,
                expression_digest,
                expression,
            }
        })
        .collect::<Vec<_>>();
    assertions.sort_by(|left, right| {
        (
            left.source_path.as_str(),
            left.function.as_str(),
            left.line,
            left.kind.as_str(),
            left.assertion_id.as_str(),
        )
            .cmp(&(
                right.source_path.as_str(),
                right.function.as_str(),
                right.line,
                right.kind.as_str(),
                right.assertion_id.as_str(),
            ))
    });
    assertions.dedup_by(|left, right| left.assertion_id == right.assertion_id);
    Ok(AssertionInventory {
        schema_version: ASSERTION_SCHEMA_VERSION,
        path: path.to_string(),
        test_name: test_name.to_string(),
        source_digest: sha256_tag(source.as_bytes()),
        source_digests: context.source_digests.clone(),
        assertions,
        unresolved_assertion_helpers: unresolved.into_iter().collect(),
    })
}

fn collect_functions(
    items: &[Item],
    source_path: &str,
    modules: &mut Vec<String>,
    output: &mut Vec<FunctionRecord>,
) {
    let imports = imports_from_items(items, modules);
    for item in items {
        match item {
            Item::Fn(function) => {
                let mut segments = modules.clone();
                segments.push(function.sig.ident.to_string());
                output.push(FunctionRecord {
                    qualified_name: segments.join("::"),
                    source_path: source_path.to_string(),
                    module: modules.clone(),
                    imports: imports.clone(),
                    item: function.clone(),
                });
            }
            Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    modules.push(module.ident.to_string());
                    collect_functions(nested, source_path, modules, output);
                    modules.pop();
                }
            }
            _ => {}
        }
    }
}

fn imports_from_items(items: &[Item], current_module: &[String]) -> ImportReachability {
    let mut imports = ImportReachability::default();
    for item in items {
        let Item::Use(item_use) = item else {
            continue;
        };
        collect_use_tree(&item_use.tree, current_module, Vec::new(), &mut imports);
    }
    imports
}

fn collect_use_tree(
    tree: &UseTree,
    current_module: &[String],
    mut prefix: Vec<String>,
    imports: &mut ImportReachability,
) {
    match tree {
        UseTree::Path(path) => {
            let segment = path.ident.to_string();
            if prefix.is_empty() {
                match segment.as_str() {
                    "crate" => {}
                    "self" => prefix.extend_from_slice(current_module),
                    "super" => {
                        prefix.extend_from_slice(
                            &current_module[..current_module.len().saturating_sub(1)],
                        );
                    }
                    _ => prefix.push(segment),
                }
            } else if segment != "self" {
                prefix.push(segment);
            }
            collect_use_tree(&path.tree, current_module, prefix, imports);
        }
        UseTree::Name(name) => {
            let imported = name.ident.to_string();
            prefix.push(imported.clone());
            imports.bindings.insert(imported, prefix);
        }
        UseTree::Rename(rename) => {
            prefix.push(rename.ident.to_string());
            imports.bindings.insert(rename.rename.to_string(), prefix);
        }
        UseTree::Glob(_) => {
            imports.glob_modules.insert(prefix);
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_tree(item, current_module, prefix.clone(), imports);
            }
        }
    }
}

fn resolve_call<'a>(
    function: &FunctionRecord,
    call: &CallReference,
    by_qualified: &'a BTreeMap<String, &'a FunctionRecord>,
) -> Vec<&'a FunctionRecord> {
    if call.segments.is_empty() {
        return Vec::new();
    }
    let mut candidates = BTreeSet::new();
    if call.leading_colon {
        let mut absolute = call.segments.clone();
        if absolute.first().map(String::as_str) == Some("crate") {
            absolute.remove(0);
        }
        candidates.insert(absolute);
    } else {
        match call.segments[0].as_str() {
            "crate" => {
                candidates.insert(call.segments[1..].to_vec());
            }
            "self" => {
                let mut path = function.module.clone();
                path.extend_from_slice(&call.segments[1..]);
                candidates.insert(path);
            }
            "super" => {
                let mut path = function.module.clone();
                let mut index = 0;
                while call.segments.get(index).map(String::as_str) == Some("super") {
                    path.pop();
                    index += 1;
                }
                path.extend_from_slice(&call.segments[index..]);
                candidates.insert(path);
            }
            first => {
                if let Some(binding) = function.imports.bindings.get(first) {
                    let mut path = binding.clone();
                    path.extend_from_slice(&call.segments[1..]);
                    candidates.insert(path);
                }
                if call.segments.len() == 1 {
                    let mut local = function.module.clone();
                    local.push(first.to_string());
                    candidates.insert(local);
                    for module in &function.imports.glob_modules {
                        let mut path = module.clone();
                        path.push(first.to_string());
                        candidates.insert(path);
                    }
                } else {
                    let mut relative = function.module.clone();
                    relative.extend_from_slice(&call.segments);
                    candidates.insert(relative);
                    candidates.insert(call.segments.clone());
                }
            }
        }
    }
    candidates
        .into_iter()
        .filter_map(|segments| by_qualified.get(&segments.join("::")).copied())
        .collect()
}

fn index_functions(functions: &[FunctionRecord]) -> BTreeMap<String, Vec<&FunctionRecord>> {
    let mut indexed: BTreeMap<String, Vec<&FunctionRecord>> = BTreeMap::new();
    for function in functions {
        if let Some(name) = function.qualified_name.rsplit("::").next() {
            indexed.entry(name.to_string()).or_default().push(function);
        }
    }
    indexed
}

fn integration_test_module_root(root: &Path, path: &Path) -> Option<PathBuf> {
    let components = path
        .components()
        .map(|component| match component {
            Component::Normal(value) => value.to_string_lossy().into_owned(),
            _ => String::new(),
        })
        .collect::<Vec<_>>();
    let tests_index = components.iter().position(|part| part == "tests")?;
    if components.first().map(String::as_str) != Some("crates") || tests_index != 2 {
        return None;
    }
    let target_part = components.get(tests_index + 1)?;
    let mut root_path = PathBuf::new();
    for component in &components[..=tests_index] {
        root_path.push(component);
    }
    if components.len() == tests_index + 2 {
        return Some(root.join(path));
    }
    root_path.push(Path::new(target_part).with_extension("rs"));
    let candidate = root.join(root_path);
    candidate.is_file().then_some(candidate)
}

fn assertion_module_segments(module: &str) -> Result<Vec<String>> {
    if module == ASSERTION_ROOT_MODULE {
        return Ok(Vec::new());
    }
    let prefix = format!("{ASSERTION_ROOT_MODULE}::");
    let Some(relative) = module.strip_prefix(&prefix) else {
        bail!("Rust assertion context module escaped root: {module}")
    };
    Ok(relative.split("::").map(str::to_string).collect())
}

struct AssertionVisitor {
    function: String,
    source_path: String,
    generated_output_roots: BTreeSet<String>,
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
                source_path: self.source_path.clone(),
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
                source_path: self.source_path.clone(),
                function: self.function.clone(),
                span: expression.span(),
                expression: expression.to_token_stream().to_string(),
            });
        } else if looks_like_assertion_helper(&method) {
            self.observations
                .unresolved_assertion_helpers
                .insert(format!("{}::{method}:unresolved-method", self.function));
        }
        visit::visit_expr_method_call(self, expression);
    }

    fn visit_expr_call(&mut self, expression: &'ast ExprCall) {
        if let Some(reader) = runtime_fixture_reader(expression) {
            let static_literal = expression
                .args
                .first()
                .is_some_and(|argument| matches!(argument, Expr::Lit(value) if matches!(value.lit, syn::Lit::Str(_))));
            if !static_literal
                && !is_generated_output_read(expression, &self.generated_output_roots)
            {
                self.observations
                    .unresolved_assertion_helpers
                    .insert(format!("{}::{reader}:unresolved-oracle", self.function));
            }
        }
        if let Expr::Path(path) = expression.func.as_ref() {
            self.observations.calls.insert(CallReference {
                leading_colon: path.path.leading_colon.is_some(),
                segments: path
                    .path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect(),
            });
        }
        visit::visit_expr_call(self, expression);
    }
}

mod external_oracles;

use external_oracles::{
    external_oracle_digests, generated_output_roots, is_generated_output_read,
    runtime_fixture_reader,
};
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
mod tests;
