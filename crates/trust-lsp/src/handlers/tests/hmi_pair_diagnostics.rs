//! Specification 39, §8.6: when a faceplate field refers to a member that the function block no
//! longer has, or one holding a kind of value the field does not take, it is an error in both
//! files: the pair file's check reports it, and the block's ST file shows it at the block's name,
//! in the same words with the pair file named.

use super::*;

const FACEPLATE: &str = "format = 1\ntitle = \"Pump\"\nsize = [400, 300]\n\n[fields.Running]\naccepts = [\"boolean\"]\n\n[fields.Speed]\naccepts = [\"number\"]\n";
const PAIR: &str = "format = 1\nblock = \"Pump\"\nfaceplate = \"pump\"\nsymbol = \"pump\"\n";

/// The pair diagnostics of an ST file in a project with the pump pair and faceplate.
fn pair_diagnostics(name: &str, source: &str) -> Vec<tower_lsp::lsp_types::Diagnostic> {
    let root = temp_dir(name);
    for (path, text) in [
        ("hmi/panels/pump.toml", FACEPLATE),
        ("hmi/pairs/pump.toml", PAIR),
        ("src/pump.st", source),
    ] {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("folders");
        std::fs::write(file, text).expect("write");
    }
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::from_directory_path(&root).expect("root uri");
    state.set_workspace_folders(vec![root_uri.clone()]);
    state.set_workspace_config(root_uri, ProjectConfig::load(&root));
    let uri =
        tower_lsp::lsp_types::Url::from_file_path(root.join("src/pump.st")).expect("source uri");
    state.open_document(uri.clone(), 1, source.to_string());
    let file_id = state.get_document(&uri).expect("the document").file_id;
    let ticket = state.begin_semantic_request();
    let diagnostics = super::diagnostics::collect_diagnostics_with_ticket_for_tests(
        &state, &uri, source, file_id, ticket,
    );
    std::fs::remove_dir_all(&root).ok();
    diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.message.contains("hmi/pairs/"))
        .collect()
}

#[test]
fn a_block_without_a_member_its_pair_needs_shows_the_problem_at_its_name() {
    let source =
        "FUNCTION_BLOCK Pump\nVAR_OUTPUT\n    Running : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\n";
    let found = pair_diagnostics("trustlsp-pair-missing", source);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].message, "the pair hmi/pairs/pump.toml: `Pump` has no member `Speed`, which the faceplate's field `Speed` needs");
    assert_eq!(
        (found[0].range.start.line, found[0].range.start.character),
        (0, 15),
        "at the block's name"
    );
    assert_eq!(found[0].range.end.character, 19);
}

#[test]
fn a_member_of_a_kind_its_field_does_not_take_shows_the_problem_at_the_blocks_name() {
    let source = "FUNCTION_BLOCK Pump\nVAR_OUTPUT\n    Running : BOOL;\n    Speed : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\n";
    let found = pair_diagnostics("trustlsp-pair-kind", source);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].message, "the pair hmi/pairs/pump.toml: the member `Speed` of `Pump` holds a boolean; the field `Speed` takes a number");
    assert_eq!(
        (found[0].range.start.line, found[0].range.start.character),
        (0, 15),
        "at the block's name"
    );
}

#[test]
fn a_member_a_block_inherits_satisfies_its_pair() {
    let source = "FUNCTION_BLOCK Motor\nVAR_OUTPUT\n    Speed : REAL;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nFUNCTION_BLOCK Pump EXTENDS Motor\nVAR_OUTPUT\n    Running : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\n";
    assert_eq!(pair_diagnostics("trustlsp-pair-inherited", source), vec![]);
}
