use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn read_workspace_file(relative: &str) -> String {
    fs::read_to_string(workspace_root().join(relative))
        .unwrap_or_else(|err| panic!("read {relative}: {err}"))
        .replace("\r\n", "\n")
}

fn rust_files_under(relative: &str) -> Vec<PathBuf> {
    fn visit(path: &Path, files: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(path).unwrap_or_else(|err| panic!("read dir {path:?}: {err}")) {
            let entry = entry.expect("directory entry");
            let path = entry.path();
            if path.is_dir() {
                visit(&path, files);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }

    let mut files = Vec::new();
    visit(&workspace_root().join(relative), &mut files);
    files
}

#[test]
fn syntax_classifier_helpers_delegate_to_central_api() {
    let delegates = [
        (
            "crates/trust-ide/src/var_decl.rs",
            "fn is_expression_kind(kind: SyntaxKind) -> bool {\n    kind.is_initializer_expression_node()\n}",
        ),
        (
            "crates/trust-ide/src/refactor/operations/inline_and_namespace_helpers.rs",
            "fn is_expression_kind(kind: SyntaxKind) -> bool {\n    kind.is_initializer_expression_node()\n}",
        ),
        (
            "crates/trust-ide/src/refactor/operations/convert_callsite_updates.rs",
            "fn is_statement_kind(kind: SyntaxKind) -> bool {\n    kind.is_statement_node()\n}",
        ),
        (
            "crates/trust-hir/src/db/diagnostics/unreachable.rs",
            "fn is_expression_kind(kind: SyntaxKind) -> bool {\n    kind.is_initializer_expression_node()\n}",
        ),
        (
            "crates/trust-hir/src/db/diagnostics/unreachable.rs",
            "fn is_statement_kind(kind: SyntaxKind) -> bool {\n    kind.is_statement_node()\n}",
        ),
        (
            "crates/trust-hir/src/db/diagnostics/expression.rs",
            "pub(in crate::db) fn is_expression_kind(kind: SyntaxKind) -> bool {\n    kind.is_initializer_expression_node()\n}",
        ),
        (
            "crates/trust-hir/src/type_check/mod.rs",
            "fn is_expression_kind(kind: SyntaxKind) -> bool {\n    kind.is_initializer_expression_node()\n}",
        ),
        (
            "crates/trust-hir/src/type_check/mod.rs",
            "fn is_statement_kind(kind: SyntaxKind) -> bool {\n    kind.is_statement_node()\n}",
        ),
        (
            "crates/trust-lsp/src/handlers/features/core_impl/helpers/syntax_utils.rs",
            "pub(in super::super) fn is_expression_kind(kind: SyntaxKind) -> bool {\n    kind.is_initializer_expression_node()\n}",
        ),
        (
            "crates/trust-runtime/src/host/harness/util.rs",
            "pub(super) fn is_expression_kind(kind: SyntaxKind) -> bool {\n    kind.is_initializer_expression_node()\n}",
        ),
        (
            "crates/trust-runtime/src/host/harness/util.rs",
            "pub(super) fn is_statement_kind(kind: SyntaxKind) -> bool {\n    kind.is_statement_node()\n}",
        ),
    ];

    for (file, expected) in delegates {
        let source = read_workspace_file(file);
        assert!(
            source.contains(expected),
            "{file} must delegate to the central trust-syntax classifier"
        );
    }
}

#[test]
fn hir_collection_and_import_do_not_drop_member_initializers() {
    for file in [
        "crates/trust-hir/src/db/queries/collector/types.rs",
        "crates/trust-hir/src/db/symbol_import.rs",
    ] {
        let source = read_workspace_file(file);
        assert!(
            !source.contains("default_initializer: None"),
            "{file} must translate or register initializer ids instead of dropping them"
        );
    }

    let table = read_workspace_file("crates/trust-hir/src/symbols/table.rs");
    assert!(
        table.contains("initializer_catalog: InitializerCatalog"),
        "initializer catalog must be owned by the Salsa-tracked SymbolTable"
    );
    assert!(
        table.contains("pub fn initializer(&self, id: InitializerId)"),
        "SymbolTable must expose narrow initializer lookup accessors"
    );
}

#[test]
fn runtime_initializer_service_is_the_source_level_funnel() {
    let root = workspace_root();
    let allowed = [
        root.join("crates/trust-runtime/src/host/harness/coerce.rs"),
        root.join("crates/trust-runtime/src/host/harness/initializer.rs"),
    ];

    for path in rust_files_under("crates/trust-runtime/src") {
        let source = fs::read_to_string(&path).expect("read rust source");
        if !source.contains("coerce_initializer_value_to_type(") {
            continue;
        }
        assert!(
            allowed.iter().any(|allowed_path| allowed_path == &path),
            "{} calls coerce_initializer_value_to_type outside the initializer funnel",
            path.display()
        );
    }
}

#[test]
fn vm_local_init_does_not_create_runtime_storage_frames() {
    let source = read_workspace_file("crates/trust-runtime/src/runtime/vm/local_init.rs");
    assert!(
        !source.contains("runtime\n            .storage_mut()\n            .push_frame_with_instance")
            && !source.contains("runtime.storage_mut().push_frame("),
        "VM local initialization must populate VM frame slots directly instead of creating temporary runtime storage frames"
    );
}

#[test]
fn dynamic_ref_partial_index_does_not_clone_entire_value_ref() {
    let source = read_workspace_file("crates/trust-runtime/src/runtime/vm/dispatch_refs.rs");
    let body = source
        .split_once("pub(super) fn dynamic_ref_index(")
        .and_then(|(_, rest)| rest.split_once("pub(super) fn peek_dynamic_ref"))
        .map(|(body, _)| body)
        .expect("dynamic_ref_index body");

    assert!(
        !body.contains("reference.path.last().cloned()"),
        "partial multidimensional index handling must borrow the trailing index segment"
    );
    assert!(
        !body.contains("reference.clone()"),
        "partial multidimensional index handling must borrow the base path instead of cloning the whole ValueRef"
    );
}

#[test]
fn vm_function_block_ref_execution_reads_reference_without_clone() {
    let source = read_workspace_file("crates/trust-runtime/src/runtime/vm/dispatch.rs");
    let body = source
        .split_once("pub(super) fn execute_function_block_ref(")
        .and_then(|(_, rest)| rest.split_once("fn execute_pou("))
        .map(|(body, _)| body)
        .expect("execute_function_block_ref body");

    assert!(
        !body.contains("read_by_ref(reference.clone())"),
        "VM function-block ref execution must borrow ValueRef for storage reads"
    );
}

#[test]
fn tier1_dynamic_ref_field_borrows_reference_registers() {
    let source =
        read_workspace_file("crates/trust-runtime/src/runtime/vm/register_ir/tier1/execute.rs");
    let body = source
        .split_once("Tier1CompiledInstr::RefField { base, field, dest } => {")
        .and_then(|(_, rest)| rest.split_once("Tier1CompiledInstr::RefIndex"))
        .map(|(body, _)| body)
        .expect("tier1 RefField body");

    assert!(
        body.contains("dynamic_ref_field_borrowed(runtime, frames, reference, field.clone())"),
        "tier-1 RefField must use the borrowed dynamic-ref helper"
    );
    assert!(
        !body.contains("reference.clone()"),
        "tier-1 RefField must not clone the whole ValueRef before resolving the field"
    );
}

#[test]
fn register_ir_decode_uses_inline_operand_storage() {
    let source =
        read_workspace_file("crates/trust-runtime/src/runtime/vm/register_ir/lower/decode.rs");
    let decode_body = source
        .split_once("fn decode_pou(")
        .and_then(|(_, rest)| rest.split_once("fn opcode_operand_len_for_lowering"))
        .map(|(body, _)| body)
        .expect("decode_pou body");

    assert!(
        !source.contains("operands: Vec<u8>"),
        "register-IR decoded instructions must not allocate operand Vecs"
    );
    assert!(
        !decode_body.contains(".to_vec()"),
        "register-IR decode must copy operand bytes into inline storage"
    );
}

#[test]
fn runtime_var_decl_parts_are_structural_not_positional_tuples() {
    let vars = read_workspace_file("crates/trust-runtime/src/host/harness/compiler/vars.rs");
    assert!(
        vars.contains("pub(super) struct VarDeclParts"),
        "runtime declaration lowering must expose named VarDeclParts"
    );

    for file in rust_files_under("crates/trust-runtime/src/host/harness") {
        let source = fs::read_to_string(&file).expect("read harness source");
        assert!(
            !source.contains("let (names, type_ref, initializer, address) = parse_var_decl"),
            "{} still destructures parse_var_decl positionally",
            file.display()
        );
    }
}

#[test]
fn dependency_boundaries_for_initializer_metadata_hold() {
    for file in rust_files_under("crates/trust-hir/src") {
        let source = fs::read_to_string(&file).expect("read HIR source");
        assert!(
            !source.contains("trust_runtime"),
            "{} must not depend on trust-runtime",
            file.display()
        );
        assert!(
            !source.contains("crate::value::Value"),
            "{} must not import runtime Value",
            file.display()
        );
    }

    for file in [
        "crates/trust-runtime/src/host/harness/initializer.rs",
        "crates/trust-runtime/src/host/harness/initializer/defaults.rs",
    ] {
        let source = read_workspace_file(file);
        assert!(
            !source.contains("SyntaxNode") && !source.contains("trust_syntax"),
            "{file} must consume lowered Expr/catalog data, not raw CST"
        );
    }
}

#[test]
fn runtime_pou_registration_is_hir_catalog_driven() {
    let entry_points =
        read_workspace_file("crates/trust-runtime/src/host/harness/compiler/pou/entry_points.rs");
    assert!(
        entry_points.contains("DeclarationCatalog"),
        "runtime POU registration must take the HIR declaration catalog as input"
    );
    assert!(
        !entry_points.contains(
            ".descendants()\n        .filter(|child| child.kind() == SyntaxKind::Program)"
        ),
        "PROGRAM registration must be driven by HIR catalog entries"
    );
    assert!(
        !entry_points.contains(
            ".descendants()\n        .filter(|child| child.kind() == SyntaxKind::Function)"
        ),
        "FUNCTION registration must be driven by HIR catalog entries"
    );
    assert!(
        !entry_points.contains(
            ".descendants()\n        .filter(|child| child.kind() == SyntaxKind::FunctionBlock)"
        ),
        "FUNCTION_BLOCK registration must be driven by HIR catalog entries"
    );
    assert!(
        !entry_points
            .contains(".descendants()\n        .filter(|child| child.kind() == SyntaxKind::Class)"),
        "CLASS registration must be driven by HIR catalog entries"
    );
    assert!(
        !entry_points.contains(
            ".descendants()\n        .filter(|child| child.kind() == SyntaxKind::Interface)"
        ),
        "INTERFACE registration must be driven by HIR catalog entries"
    );
    assert!(
        entry_points.contains("HIR declaration catalog/lowering mismatch"),
        "catalog-driven registration must fail explicitly when a catalog entry cannot be matched for body lowering"
    );
}

#[test]
fn initializer_service_size_caps_hold() {
    for file in [
        "crates/trust-runtime/src/host/harness/initializer.rs",
        "crates/trust-runtime/src/host/harness/initializer/defaults.rs",
    ] {
        let source = read_workspace_file(file);
        let line_count = source.lines().count();
        assert!(
            line_count <= 400,
            "{file} has {line_count} lines; initializer service modules are capped at 400"
        );

        let lines: Vec<_> = source.lines().collect();
        let function_starts: Vec<_> = lines
            .iter()
            .enumerate()
            .filter_map(|(idx, line)| {
                let trimmed = line.trim_start();
                (trimmed.starts_with("fn ")
                    || trimmed.starts_with("pub(crate) fn ")
                    || trimmed.starts_with("pub(super) fn "))
                .then_some(idx)
            })
            .collect();

        for (position, start) in function_starts.iter().copied().enumerate() {
            let end = function_starts
                .get(position + 1)
                .copied()
                .unwrap_or(lines.len());
            let len = end.saturating_sub(start);
            assert!(
                len <= 60,
                "{file}: function starting at line {} has {len} lines; cap is 60",
                start + 1
            );
        }
    }
}

#[test]
fn init_benchmark_cli_and_fixture_are_reproducible() {
    let cli = read_workspace_file("crates/trust-runtime/src/bin/trust-runtime/cli/bench.rs");
    assert!(cli.contains("Init"), "bench CLI must expose an init action");
    assert!(
        cli.contains("warmup_cycles"),
        "init bench CLI must keep the locked warmup_cycles field"
    );

    let bench = read_workspace_file("crates/trust-runtime/src/bin/trust-runtime/bench.rs");
    assert!(
        bench.contains("include!(\"bench/init.rs\")"),
        "bench/init.rs must be included from the existing bench harness"
    );

    let fixture =
        workspace_root().join("crates/trust-runtime/tests/fixtures/init_bench/runtime.toml");
    assert!(fixture.exists(), "init bench fixture must be checked in");
    let runtime_toml = fs::read_to_string(fixture).expect("read init bench runtime.toml");
    assert!(runtime_toml.contains("[runtime]"));
    assert!(runtime_toml.contains("execution_backend = \"vm\""));
    assert!(runtime_toml.contains("auth_token = \"trust-init-bench-fixture-token\""));
}
