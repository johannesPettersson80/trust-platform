//! Specification 39, §14.5 items 4, 7 and 8: what the editor is given of a project's texts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::builtin;
use super::project::{read_project_texts, LanguageEntry, ProjectText};

fn temp(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "trust-hmi-texts-{name}-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("a folder");
    dir
}

fn write(root: &Path, file: &str, text: &str) {
    let path = root.join(file);
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("a folder");
    std::fs::write(path, text).expect("written");
}

const SCREEN: &str = r#"format = 1
size = [1280, 720]
theme = "dark"
symbols = "flat"
languages = ["en", "sv", "fi"]

[top_bar]
title = "Feed area"
clock = true

[navigation]
style = "side"
position = "left"
"#;

const OVERVIEW: &str = r#"format = 1
title = "Overview"
group = "Process"

[[object]]
id = "heading"
type = "text"
at = [40, 40]
size = [240, 40]
text = "Pumps"

[[object]]
id = "start"
type = "button"
at = [40, 96]
size = [120, 40]
text = "Start"
confirm = "Start the pump?"
action = "set"

[object.bind]
target = { path = "Plant.P101.Run", direction = "write" }

[[object]]
id = "speed"
type = "selector"
at = [40, 160]
size = [160, 40]
label = "Fan speed"
options = ["Off", "Low", "High"]

[object.bind]
value = { path = "Plant.Fan.Speed", direction = "read-write" }
"#;

const TRENDS: &str = r#"format = 1
title = "Trends"
group = "Process"

[[object]]
id = "note"
type = "text"
at = [40, 40]
size = [240, 40]
text = "Pumps"
"#;

const PUMP: &str = r#"format = 1
title = "Pump"
size = [400, 240]

[[object]]
id = "speed"
type = "text"
at = [24, 24]
size = [200, 40]
text = "Speed"
"#;

const SWEDISH: &str = "format = 1\nlanguage = \"sv\"\n\n[texts]\n\"No alarms\" = \"Inget larm\"\n\"Pumps\" = \"Pumpar\"\n";

fn project(name: &str) -> PathBuf {
    let root = temp(name);
    write(&root, "hmi/screen.toml", SCREEN);
    write(&root, "hmi/pages/overview.toml", OVERVIEW);
    write(&root, "hmi/pages/trends.toml", TRENDS);
    write(&root, "hmi/popups/pump.toml", PUMP);
    write(&root, "hmi/texts/sv.toml", SWEDISH);
    write(
        &root,
        "hmi/texts/fi.toml",
        "format = 1\nlanguage = \"fi\"\n\n[texts]\n\"Pumps\" = \"Pumput\"\n",
    );
    root
}

fn text(text: &str, places: &[&str]) -> ProjectText {
    ProjectText {
        text: text.to_string(),
        places: places.iter().map(|place| place.to_string()).collect(),
    }
}

#[test]
fn a_project_gives_every_text_its_screen_pages_and_popups_show_once_and_where() {
    let root = project("inventory");
    let texts = read_project_texts(&root, &BTreeMap::new());
    assert!(texts.errors.is_empty(), "{:?}", texts.errors);
    let language = |tag: &str, name: &str| LanguageEntry {
        tag: tag.to_string(),
        name: name.to_string(),
    };
    assert_eq!(
        texts.languages,
        [
            language("en", "English"),
            language("sv", "Svenska"),
            language("fi", "fi")
        ]
    );
    assert_eq!(
        texts.texts,
        [
            text("Feed area", &["screen"]),
            text("Overview", &["overview"]),
            text("Process", &["overview", "trends"]),
            text("Pumps", &["overview", "trends"]),
            text("Start", &["overview"]),
            text("Start the pump?", &["overview"]),
            text("Fan speed", &["overview"]),
            text("Off", &["overview"]),
            text("Low", &["overview"]),
            text("High", &["overview"]),
            text("Trends", &["trends"]),
            text("Pump", &["popup:pump"]),
            text("Speed", &["popup:pump"]),
        ]
    );
    // The project's own text files, by language; the first language needs none.
    assert_eq!(
        texts.project.get("sv"),
        Some(&BTreeMap::from([
            ("No alarms".to_string(), "Inget larm".to_string()),
            ("Pumps".to_string(), "Pumpar".to_string()),
        ]))
    );
    assert_eq!(texts.project["fi"]["Pumps"], "Pumput");
    assert!(!texts.project.contains_key("en"));
    // truST's words, and their translations in the project languages truST ships.
    let english = builtin("en").expect("English is built in");
    assert_eq!(
        texts.words,
        english.texts.keys().cloned().collect::<Vec<_>>()
    );
    assert_eq!(texts.builtin["sv"]["No alarms"], "Inga larm");
    assert_eq!(texts.builtin["en"]["No alarms"], "No alarms");
    assert!(
        !texts.builtin.contains_key("fi"),
        "truST does not ship Finnish"
    );
}

#[test]
fn an_unsaved_file_is_read_as_the_editor_holds_it() {
    let root = project("unsaved");
    let open = BTreeMap::from([
        (
            "hmi/pages/overview.toml".to_string(),
            OVERVIEW.replace("text = \"Pumps\"", "text = \"Feed pumps\""),
        ),
        (
            "hmi/texts/sv.toml".to_string(),
            SWEDISH.replace("Pumpar", "Matarpumpar"),
        ),
    ]);
    let texts = read_project_texts(&root, &open);
    assert!(
        texts.texts.contains(&text("Feed pumps", &["overview"])),
        "{:?}",
        texts.texts
    );
    assert!(
        texts.texts.contains(&text("Pumps", &["trends"])),
        "{:?}",
        texts.texts
    );
    assert_eq!(texts.project["sv"]["Pumps"], "Matarpumpar");
}

#[test]
fn a_file_that_does_not_load_is_reported_with_its_line_and_its_texts_are_left_out() {
    let root = project("broken");
    write(
        &root,
        "hmi/texts/sv.toml",
        "format = 1\nlanguage = \"sv\"\n\n[texts]\n\"Pumps\" = 3\n",
    );
    write(
        &root,
        "hmi/pages/trends.toml",
        &TRENDS.replace("size = [240, 40]", "size = [240]"),
    );
    let texts = read_project_texts(&root, &BTreeMap::new());
    let located: Vec<(&str, usize)> = texts
        .errors
        .iter()
        .map(|error| (error.file.as_str(), error.line))
        .collect();
    assert!(located.contains(&("hmi/texts/sv.toml", 5)), "{located:?}");
    assert!(
        located
            .iter()
            .any(|(file, _)| *file == "hmi/pages/trends.toml"),
        "{located:?}"
    );
    assert!(!texts.project.contains_key("sv"));
    assert_eq!(
        texts.texts.iter().find(|shown| shown.text == "Trends"),
        None
    );
    assert!(
        texts.texts.contains(&text("Pumps", &["overview"])),
        "{:?}",
        texts.texts
    );
}

#[test]
fn a_panel_the_pages_show_gives_its_labels_and_title_as_texts_too() {
    let root = project("panels");
    write(
        &root,
        "hmi/panels/meter.toml",
        "format = 1\ntitle = \"Meter\"\nsize = [300, 120]\n\n[fields.Value]\naccepts = [\"number\"]\n\n[[object]]\nid = \"reading\"\ntype = \"value\"\nlabel = \"Reading\"\nat = [10, 10]\nsize = [200, 48]\n\n[object.bind]\nvalue = \"$Value\"\n",
    );
    write(
        &root,
        "hmi/pages/meters.toml",
        "format = 1\ntitle = \"Meters\"\n\n[[object]]\nid = \"m1\"\ntype = \"faceplate\"\nfaceplate = \"meter\"\ninstance = \"Main.M1\"\nat = [0, 0]\nsize = [300, 164]\n",
    );
    let texts = read_project_texts(&root, &BTreeMap::new());
    assert!(
        texts.texts.contains(&text("Reading", &["panel:meter"])),
        "{:?}",
        texts.texts
    );
    assert!(
        texts.texts.contains(&text("Meter", &["panel:meter"])),
        "the panel's title: {:?}",
        texts.texts
    );
}
