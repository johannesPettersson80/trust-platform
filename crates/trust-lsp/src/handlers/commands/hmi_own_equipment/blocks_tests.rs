//! Specification 39, §21.9: a block is the project's own when the project's folder holds its file
//! and no library folder deeper than the project's folder does (the deepest-folder rule of
//! specification 14 §7.6).

use super::*;
use crate::handlers::commands::LoadedSource;

const MIXER: &str =
    "FUNCTION_BLOCK FB_Mixer\nVAR_OUTPUT\n    Running : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\n";

/// The names of the own blocks among `sources`, for the project at `root`.
fn own_names(sources: &[(PathBuf, &str)], root: &Path, libraries: &[PathBuf]) -> Vec<String> {
    let sources: Vec<LoadedSource> = sources
        .iter()
        .map(|(path, text)| LoadedSource {
            path: path.clone(),
            text: text.to_string(),
        })
        .collect();
    let index = Declarations::read(&sources);
    own_blocks(index.as_ref(), root, libraries)
        .into_iter()
        .map(|block| block.name)
        .collect()
}

#[test]
fn a_library_folder_that_holds_the_project_leaves_the_projects_blocks_its_own() {
    // The Plant examples' layout: `Plant = { path = "../.." }` makes the library folder hold the
    // project. Catches every file of the project taken for the library's, which left My equipment
    // empty in each example.
    let library = PathBuf::from("/w/plant");
    let root = library.join("examples/equipment");
    let sources = [
        (
            library.join("src/Pump.st"),
            "FUNCTION_BLOCK FB_LibraryPump\nEND_FUNCTION_BLOCK\n",
        ),
        (root.join("src/Main.st"), MIXER),
    ];
    assert_eq!(own_names(&sources, &root, &[library]), ["FB_Mixer"]);
}

#[test]
fn a_library_inside_the_project_keeps_its_blocks_out() {
    // Catches the project's folder winning over a deeper library folder that lies inside it.
    let root = PathBuf::from("/p");
    let library = root.join("libs/vendor");
    let sources = [
        (
            library.join("src/Valve.st"),
            "FUNCTION_BLOCK FB_Valve\nEND_FUNCTION_BLOCK\n",
        ),
        (root.join("src/Main.st"), MIXER),
    ];
    assert_eq!(own_names(&sources, &root, &[library]), ["FB_Mixer"]);
}
