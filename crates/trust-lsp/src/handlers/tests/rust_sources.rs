//! Go to Definition of a Rust POU opens its Rust item (specification 48 RU-31, RU-50).

use super::*;
use tower_lsp::lsp_types::{
    request::GotoDeclarationParams, GotoDefinitionParams, GotoDefinitionResponse, Location,
    Position, Range, TextDocumentIdentifier, TextDocumentPositionParams, Url,
};
use trust_runtime::native::rust_pous::{write_declarations, Written, DECLARATIONS_FILE};

/// The manifest of the project's crate.
const MANIFEST: &str = "[package]\nname = \"line\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
                        [dependencies]\ntrust-plc = \"*\"\n";

/// The crate's root, with the module of its block.
const LIB: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub mod fill;
";

/// A block whose doc comment and budget lie around its attribute, and its method.
const FILL: &str = "use trust_plc::prelude::*;

/// Adds its rate to the level it is given.
#[trust_fb(name = \"FILL\")]
#[budget(\"1us\")]
pub struct Fill {
    #[input]
    pub rate: f64,
}

impl Fill {
    #[trust_method]
    #[budget(\"1us\")]
    pub fn reset(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        self.rate = 0.0;
        Ok(())
    }
}
";

/// The program that uses the block.
const MAIN: &str = "PROGRAM Main
VAR
    filler : FILL;
END_VAR
filler(rate := 2.0);
filler.reset();
END_PROGRAM
";

fn at(uri: &Url, position: Position) -> TextDocumentPositionParams {
    TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        position,
    }
}

fn scalar(response: Option<GotoDefinitionResponse>) -> Location {
    match response {
        Some(GotoDefinitionResponse::Scalar(location)) => location,
        other => panic!("one location, got {other:?}"),
    }
}

#[test]
fn lsp_goto_definition_of_a_rust_pou_opens_its_rust_item() {
    // RU-31, RU-50: Go to Definition of FILL in the program opens rust/src/fill.rs from its
    // `#[trust_fb]`, line 4 (3 from 0), to the end of its name `Fill`, column 15 of line 6 (5 from
    // 0), past the doc comment above them, and of its method `reset` its Rust `fn`; Go to
    // Declaration stays the generated declaration, and the definition of a Structured Text name
    // stays its own. Catches a Rust POU's definition left
    // in the generated file, one sent to its doc comment or to another item, and a Structured Text
    // definition sent away.
    let root = temp_dir("lsp-rust-sources");
    for (path, text) in [
        ("rust/Cargo.toml", MANIFEST),
        ("rust/src/lib.rs", LIB),
        ("rust/src/fill.rs", FILL),
        ("src/main.st", MAIN),
    ] {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("the folder");
        std::fs::write(file, text).expect("the file");
    }
    let written = write_declarations(&root);
    assert!(matches!(written, Ok(Written::Wrote(_))), "{written:?}");
    let generated =
        std::fs::read_to_string(root.join(DECLARATIONS_FILE)).expect("the declarations");
    let state = ServerState::new();
    let main_uri = Url::from_file_path(root.join("src/main.st")).expect("a file URI");
    let generated_uri = Url::from_file_path(root.join(DECLARATIONS_FILE)).expect("a file URI");
    state.open_document(main_uri.clone(), 1, MAIN.to_string());
    state.open_document(generated_uri.clone(), 1, generated.clone());

    let definition = scalar(goto_definition(
        &state,
        GotoDefinitionParams {
            text_document_position_params: at(&main_uri, position_at(MAIN, "FILL;")),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    ));
    assert_eq!(
        definition,
        Location {
            uri: Url::from_file_path(root.join("rust/src/fill.rs")).expect("a file URI"),
            range: Range {
                start: Position {
                    line: 3,
                    character: 0
                },
                end: Position {
                    line: 5,
                    character: 15
                },
            },
        }
    );

    let declaration = goto_declaration(
        &state,
        GotoDeclarationParams {
            text_document_position_params: at(&main_uri, position_at(MAIN, "FILL;")),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    match declaration {
        Some(GotoDefinitionResponse::Scalar(location)) => {
            assert_eq!(location.uri, generated_uri);
            assert_eq!(location.range.start, position_at(&generated, "FILL\n"));
        }
        other => panic!("the generated declaration, got {other:?}"),
    }

    let own = scalar(goto_definition(
        &state,
        GotoDefinitionParams {
            text_document_position_params: at(&main_uri, position_at(MAIN, "filler(rate")),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    ));
    assert_eq!(own.uri, main_uri);
    assert_eq!(own.range.start, position_at(MAIN, "filler : FILL"));

    // The method's definition is its Rust `fn`: `#[trust_method]` at column 4 of line 12 (11 from
    // 0) to the end of `reset`, column 16 of line 14 (13 from 0).
    let method = scalar(goto_definition(
        &state,
        GotoDefinitionParams {
            text_document_position_params: at(&main_uri, position_at(MAIN, "reset()")),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    ));
    assert_eq!(
        method,
        Location {
            uri: Url::from_file_path(root.join("rust/src/fill.rs")).expect("a file URI"),
            range: Range {
                start: Position {
                    line: 11,
                    character: 4
                },
                end: Position {
                    line: 13,
                    character: 16
                },
            },
        }
    );
}
