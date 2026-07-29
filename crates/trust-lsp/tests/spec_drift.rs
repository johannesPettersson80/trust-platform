use std::path::PathBuf;

include!("../../../tests/support/repository_source_oracle.rs");

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn technical_spec_path() -> PathBuf {
    repository_root().join("docs/specs/14-lsp.md")
}

fn technical_spec() -> String {
    repository_source_tree_read_to_string!(
        (technical_spec_path(), repository_root()),
        roots = ["docs/specs"],
        extension = "md",
    )
    .expect("read docs/specs/14-lsp.md")
}

#[test]
fn technical_spec_lists_lsp_capabilities() {
    let spec = technical_spec();
    let expected_methods = [
        "textDocument/didOpen",
        "textDocument/publishDiagnostics",
        "textDocument/diagnostic",
        "workspace/diagnostic",
        "workspace/diagnostic/refresh",
        "textDocument/completion",
        "textDocument/hover",
        "textDocument/signatureHelp",
        "textDocument/definition",
        "textDocument/declaration",
        "textDocument/typeDefinition",
        "textDocument/implementation",
        "textDocument/references",
        "textDocument/documentHighlight",
        "textDocument/documentSymbol",
        "workspace/symbol",
        "workspace/willRenameFiles",
        "textDocument/rename",
        "textDocument/semanticTokens",
        "workspace/semanticTokens/refresh",
        "textDocument/foldingRange",
        "textDocument/selectionRange",
        "textDocument/linkedEditingRange",
        "textDocument/documentLink",
        "textDocument/inlayHint",
        "textDocument/inlineValue",
        "textDocument/codeLens",
        "textDocument/prepareCallHierarchy",
        "textDocument/prepareTypeHierarchy",
        "textDocument/formatting",
        "textDocument/rangeFormatting",
        "textDocument/onTypeFormatting",
        "workspace/didChangeConfiguration",
        "textDocument/codeAction",
        "workspace/executeCommand",
    ];

    let missing: Vec<&str> = expected_methods
        .iter()
        .copied()
        .filter(|method| !spec.contains(method))
        .collect();
    assert!(
        missing.is_empty(),
        "Platform spec missing LSP methods: {missing:?}"
    );
}

#[test]
fn technical_spec_mentions_index_cache_and_diagnostics_toggles() {
    let spec = technical_spec();
    let expected_fragments = [
        "[indexing]",
        "cache",
        "cache_dir",
        "[diagnostics]",
        "warn_missing_else",
        "warn_implicit_conversion",
    ];
    let missing: Vec<&str> = expected_fragments
        .iter()
        .copied()
        .filter(|fragment| !spec.contains(fragment))
        .collect();
    assert!(
        missing.is_empty(),
        "Platform spec missing config fragments: {missing:?}"
    );
}
