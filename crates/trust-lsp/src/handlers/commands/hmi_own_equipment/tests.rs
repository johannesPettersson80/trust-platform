//! Specification 39, §21.9: My equipment proposes a definition for each of the project's own
//! function blocks without one, the plain device with an automatic panel or the definition of the
//! block it extends, and gives the files to write when the block is first placed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use trust_runtime::hmi::page_files::parse_faceplate;

use super::super::hmi_authoring::bundle::read_edited;
use super::super::LoadedSource;
use super::definition::{stem, title};
use super::*;

const OWN: &str = r#"
USING Plant;

INTERFACE IMixer
METHOD Start : BOOL
END_METHOD
METHOD Stop : BOOL
END_METHOD
END_INTERFACE

FUNCTION_BLOCK FB_MixingTank IMPLEMENTS IMixer
VAR_OUTPUT
    Running : BOOL;
    MotorFault : BOOL;
    Level : REAL;
    Batch : INT;
END_VAR
VAR PUBLIC RETAIN
    MixTime : TIME := T#5m;
END_VAR
VAR
    Hidden : BOOL;
END_VAR
METHOD PUBLIC Start : BOOL
    Running := TRUE;
    Start := TRUE;
END_METHOD
METHOD PUBLIC Stop : BOOL
    Running := FALSE;
    Stop := TRUE;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_FeedPump EXTENDS Plant.FB_Pump
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Defined
VAR_OUTPUT
    Done : BOOL;
END_VAR
END_FUNCTION_BLOCK
"#;

/// A temporary project folder, unique per test and process, removed afterwards.
struct Folder(PathBuf);

impl Folder {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "trust-own-equipment-{}-{unique}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("hmi/pairs")).expect("the pairs folder");
        std::fs::create_dir_all(path.join("hmi/panels")).expect("the panels folder");
        Self(path)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The project: the Plant pump's definition and panel, a definition of its own for FB_Defined,
/// its own blocks in `src/Main.st`, and the libraries' sources outside its folder.
fn project() -> (Folder, Vec<(OwnBlock, Proposal)>, HmiBundle) {
    let folder = Folder::new();
    let root = folder.0.clone();
    let plant = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../libraries/plant/hmi");
    std::fs::copy(
        plant.join("pairs/plant-pump.toml"),
        root.join("hmi/pairs/plant-pump.toml"),
    )
    .expect("the pump");
    std::fs::copy(
        plant.join("panels/plant-run.toml"),
        root.join("hmi/panels/plant-run.toml"),
    )
    .expect("its panel");
    std::fs::write(
        root.join("hmi/pairs/defined.toml"),
        "format = 1\nblock = \"FB_Defined\"\nfaceplate = \"plant-run\"\nsymbol = \"device\"\n",
    )
    .expect("a definition of the project's own");
    let mut sources = Vec::new();
    let libraries = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../libraries");
    for library in ["alarm", "control", "plant"] {
        let mut files: Vec<PathBuf> = std::fs::read_dir(libraries.join(library).join("src"))
            .expect("the library")
            .map(|entry| entry.expect("an entry").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "st"))
            .collect();
        files.sort();
        for path in files {
            let text = std::fs::read_to_string(&path).expect("a source");
            sources.push(LoadedSource { path, text });
        }
    }
    sources.push(LoadedSource {
        path: root.join("src/Main.st"),
        text: OWN.to_string(),
    });
    let index = Declarations::read(&sources);
    let edited = read_edited(&root, &BTreeMap::new()).expect("the bundle");
    let found = proposals(index.as_ref(), &root, &[], &edited);
    (folder, found, edited)
}

#[test]
fn own_blocks_without_a_definition_are_proposed_with_the_device_or_their_bases_definition() {
    let (_folder, found, edited) = project();
    let names: Vec<&str> = found.iter().map(|(block, _)| block.name.as_str()).collect();
    assert_eq!(
        names,
        ["FB_FeedPump", "FB_MixingTank"],
        "library blocks and defined blocks are not proposed"
    );
    let definitions = proposed_definitions(&found, &edited);
    let definition = |block: &str| {
        definitions
            .iter()
            .find(|definition| definition["block"] == block)
            .unwrap_or_else(|| panic!("{block} in {definitions:#?}"))
    };
    let mixer = definition("FB_MixingTank");
    assert_eq!(mixer["proposed"], true);
    assert_eq!(mixer["title"], "Mixing tank");
    assert_eq!(
        mixer["symbol"], "device",
        "the plain device until a picture is chosen"
    );
    assert_eq!(mixer["panel"], "fb-mixing-tank");
    assert_eq!(
        mixer["bindings"]["running"], "Running",
        "the device's lamp follows Running"
    );
    assert_eq!(
        mixer["settings"][0]["member"], "MixTime",
        "a public RETAIN member is a setting"
    );
    let pump = definition("FB_FeedPump");
    assert_eq!(pump["title"], "Feed pump");
    assert_eq!(
        (pump["symbol"].as_str(), pump["panel"].as_str()),
        (Some("pump"), Some("plant-run")),
        "the base pump's definition"
    );
}

#[test]
fn a_placed_blocks_files_are_its_pair_and_its_automatic_panel() {
    let (_folder, found, edited) = project();
    let answer = creates(&found, &edited, "FB_MixingTank").expect("the files");
    let files: Vec<(&str, &str)> = answer["creates"]
        .as_array()
        .expect("files")
        .iter()
        .map(|file| {
            (
                file["path"].as_str().expect("a path"),
                file["text"].as_str().expect("a text"),
            )
        })
        .collect();
    assert_eq!(
        files.iter().map(|(path, _)| *path).collect::<Vec<_>>(),
        [
            "hmi/pairs/fb-mixing-tank.toml",
            "hmi/panels/fb-mixing-tank.toml"
        ]
    );
    let pair = parse_pair(files[0].0, files[0].1).expect("the pair reads");
    assert_eq!(
        (
            pair.block.as_str(),
            pair.faceplate.as_str(),
            pair.symbol.as_str()
        ),
        ("FB_MixingTank", "fb-mixing-tank", "device")
    );
    let panel = parse_faceplate(files[1].0, files[1].1).expect("the panel reads");
    let object = |id: &str| {
        panel
            .popup
            .page
            .objects
            .iter()
            .find(|object| object.id == id)
            .unwrap_or_else(|| panic!("{id}: {}", files[1].1))
    };
    assert_eq!(object("running").kind, "status");
    assert_eq!(
        object("motor-fault").variant.as_deref(),
        Some("alarm"),
        "a fault lamp in the alarm colour"
    );
    assert_eq!(
        (object("level").kind.as_str(), object("level").decimals),
        ("value", Some(2))
    );
    assert_eq!(
        object("batch").decimals,
        Some(0),
        "an integer without decimals"
    );
    assert_eq!(object("start-command").kind, "button");
    assert_eq!(object("stop-command").variant.as_deref(), Some("stop"));
    assert!(
        panel
            .popup
            .page
            .objects
            .iter()
            .all(|object| object.id != "hidden" && object.id != "mix-time"),
        "neither a private member nor a setting is on the panel"
    );
    assert!(
        panel.settings,
        "its setting is offered in the Settings dialog"
    );
    let pump = creates(&found, &edited, "FB_FeedPump").expect("the pump's files");
    assert_eq!(
        pump["creates"].as_array().map(Vec::len),
        Some(1),
        "the base's panel serves it: {pump}"
    );
    assert!(
        creates(&found, &edited, "FB_Defined").is_err(),
        "a block with a definition is not proposed"
    );
}

#[test]
fn names_become_words_and_file_names() {
    assert_eq!(title("FB_MixingTank"), "Mixing tank");
    assert_eq!(title("PIDOutput"), "PID output");
    assert_eq!(title("OutOfService"), "Out of service");
    assert_eq!(stem("Customer.FB_Mixer"), "fb-mixer");
    assert_eq!(stem("FB_MixingTank"), "fb-mixing-tank");
}

#[test]
fn choosing_a_picture_rewrites_the_definition_and_every_placed_object_of_the_block() {
    let (folder, found, edited) = project();
    let root = folder.0.clone();
    for file in creates(&found, &edited, "FB_MixingTank").expect("the files")["creates"]
        .as_array()
        .expect("files")
    {
        let path = root.join(file["path"].as_str().expect("a path"));
        std::fs::write(path, file["text"].as_str().expect("a text")).expect("written");
    }
    std::fs::create_dir_all(root.join("hmi/pages")).expect("the pages folder");
    std::fs::write(
        root.join("hmi/pages/overview.toml"),
        "format = 1\ntitle = \"Overview\"\n\n[[object]]\nid = \"m-101\"\ntype = \"device\"\nblock = \"FB_MixingTank\"\ninstance = \"Main.M101\"\nfaceplate = \"fb-mixing-tank\"\nat = [40, 40]\nsize = [128, 108]\n",
    )
    .expect("a page");
    let sources = vec![LoadedSource {
        path: root.join("src/Main.st"),
        text: OWN.to_string(),
    }];
    let index = Declarations::read(&sources);
    let catalogue = Catalogue::shipped();
    let open = BTreeMap::new();
    let before = picture::describe(&root, &open, index.as_ref(), catalogue, "FB_MixingTank");
    assert_eq!(
        (before["editable"].as_bool(), before["symbol"].as_str()),
        (Some(true), Some("device"))
    );
    let answer = picture::change(
        &root,
        &open,
        index.as_ref(),
        catalogue,
        "FB_MixingTank",
        "pump",
        &BTreeMap::new(),
    )
    .expect("changed");
    let edits = answer["edits"].as_array().expect("edits");
    let text = |path: &str| {
        edits
            .iter()
            .find(|edit| edit["path"] == path)
            .and_then(|edit| edit["text"].as_str())
            .unwrap_or_else(|| panic!("{path}: {answer}"))
    };
    let pair = parse_pair(
        "hmi/pairs/fb-mixing-tank.toml",
        text("hmi/pairs/fb-mixing-tank.toml"),
    )
    .expect("the pair reads");
    assert_eq!(pair.symbol, "pump");
    assert_eq!(
        pair.bind.get("running").map(String::as_str),
        Some("Running"),
        "an input finds the member of its name"
    );
    let page = trust_runtime::hmi::page_files::parse_page(
        "hmi/pages/overview.toml",
        text("hmi/pages/overview.toml"),
    )
    .expect("the page reads");
    assert_eq!(
        page.objects[0].kind, "pump",
        "the placed mixer shows the new picture"
    );
    assert_eq!(
        page.objects[0]
            .bind
            .get("running")
            .map(|binding| binding.path.as_str()),
        Some("Main.M101.Running")
    );
    let chosen = BTreeMap::from([("running".to_string(), "MotorFault".to_string())]);
    let rebound = picture::change(
        &root,
        &open,
        index.as_ref(),
        catalogue,
        "FB_MixingTank",
        "device",
        &chosen,
    )
    .expect("rebound");
    assert!(
        rebound.to_string().contains("running = \\\"MotorFault\\\""),
        "a chosen member drives its input: {rebound}"
    );
    let library = picture::describe(&root, &open, index.as_ref(), catalogue, "Plant.FB_Motor");
    assert_eq!(
        library["editable"], false,
        "a definition the project does not own is not changed here"
    );
}
