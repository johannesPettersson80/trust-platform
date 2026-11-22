//! The examples' PLC code is what Create PLC code writes from their designs (specification 39,
//! §10): an example whose generated code differs was not created again after a change of the
//! generator, of its definitions or of its design. `TRUST_WRITE_EXAMPLE_PLC_CODE=1` writes what
//! Create PLC code writes into the example's files instead of failing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::super::{LoadedSource, ProjectInputs};
use super::*;
use trust_runtime::stdlib::conversions::ConversionProfile;

/// The `.st` files below `folder`, in name order.
fn st_files(folder: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut folders = vec![folder.to_path_buf()];
    while let Some(next) = folders.pop() {
        for entry in std::fs::read_dir(&next).expect("a folder of the example") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|extension| extension == "st") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// The example's project as Create PLC code reads it: its sources and its libraries' copies.
fn inputs(example: &str) -> ProjectInputs {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(example)
        .canonicalize()
        .expect("the example");
    let mut sources = Vec::new();
    for library in ["alarm", "control", "hmi", "plant"] {
        for path in st_files(&root.join("libraries").join(library).join("src")) {
            let text = std::fs::read_to_string(&path).expect("a library source");
            sources.push(LoadedSource { path, text });
        }
    }
    for path in st_files(&root.join("src")) {
        let text = std::fs::read_to_string(&path).expect("a source");
        sources.push(LoadedSource { path, text });
    }
    ProjectInputs {
        sources_root: root.join("src"),
        root,
        sources,
        open: BTreeMap::new(),
        profile: ConversionProfile::Iec,
    }
}

/// Catches an example left behind by a change of the generator (G252: the loops' tracking), of
/// its definitions or of its design: Create PLC code would change its files.
#[test]
fn room_heatings_plc_code_is_what_create_plc_code_writes() {
    let inputs = inputs("room_heating");
    let shown = shown_equipment(&inputs).expect("the example's pages show its equipment");
    let request = Request {
        selection: HmiBindingsCommandArgs {
            open: BTreeMap::new(),
            root_uri: None,
            text_document: None,
        },
        screen_task: None,
        cycle_ms: None,
    };
    let answer = create_from(&inputs, &shown, &request).expect("PLC code is created");
    let changes: Vec<(String, String)> = answer["edit"]["changes"]
        .as_object()
        .map(|changes| {
            changes
                .iter()
                .map(|(uri, edits)| {
                    let text = edits[0]["newText"].as_str().expect("a whole file");
                    (uri.clone(), text.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    if changes.is_empty() {
        return;
    }
    if std::env::var_os("TRUST_WRITE_EXAMPLE_PLC_CODE").is_some() {
        for (uri, text) in &changes {
            let path = tower_lsp::lsp_types::Url::parse(uri)
                .ok()
                .and_then(|url| url.to_file_path().ok())
                .expect("a file of the example");
            std::fs::write(&path, text).expect("the example's file is written");
        }
        return;
    }
    let files: Vec<&str> = changes.iter().map(|(uri, _)| uri.as_str()).collect();
    panic!(
        "Create PLC code would change examples/room_heating: {files:?}; run this test with \
         TRUST_WRITE_EXAMPLE_PLC_CODE=1 to write what it writes"
    );
}
