//! New page and panel files and the links a renamed file keeps (specification 39, §3.3 item 8,
//! §4 items 1 and 9, §8.7). Expected texts are the canonical files of §15.2, written out by hand.

use serde_json::{json, Value};
use trust_runtime::hmi::page_files::{parse_faceplate, parse_screen, HmiDockEdge, HmiDockScope};

use super::*;

fn ask(request: Value) -> Value {
    answer(request).expect("the request is answered")
}

/// The files a rename of `before` to `after` in `folder` rewrites, by path.
fn rewritten(folder: &str, before: &str, after: &str, files: Value) -> Value {
    let answered = ask(json!({
        "action": "references",
        "renames": [{ "folder": folder, "before": before, "after": after }],
        "files": files,
    }));
    let mut written = serde_json::Map::new();
    for file in answered["write"].as_array().expect("files to write") {
        written.insert(
            file["file"].as_str().expect("a path").to_string(),
            file["text"].clone(),
        );
    }
    Value::Object(written)
}

#[test]
fn a_file_name_has_1_to_64_letters_digits_hyphens_and_underscores() {
    // Catches a name the runtime cannot address as a page (a space, a dot, a letter outside
    // ASCII, a path) and the 64-character limit off by one.
    let longest = "a".repeat(64);
    for valid in ["a", "Room_heating-2", "0", longest.as_str()] {
        assert!(is_file_name(valid), "{valid}");
        assert_eq!(
            ask(json!({ "action": "name", "name": valid })),
            json!({ "ok": true })
        );
    }
    let too_long = "a".repeat(65);
    for invalid in [
        "",
        too_long.as_str(),
        "my page",
        "page.v2",
        "kök",
        "../x",
        "a/b",
    ] {
        assert!(!is_file_name(invalid), "{invalid}");
        assert_eq!(
            ask(json!({ "action": "name", "name": invalid })),
            json!({ "ok": true, "problem": "Use 1–64 letters, digits, - or _." }),
            "{invalid}"
        );
    }
}

#[test]
fn a_name_in_words_has_one_space_for_each_run_of_hyphens_and_underscores() {
    // Catches a title from `room_heating-2` other than "Room heating 2", a leading space or
    // doubled spaces from `--draft__copy-`, and a first letter left small.
    assert_eq!(in_words("room_heating-2"), "Room heating 2");
    assert_eq!(in_words("room-heating"), "Room heating");
    assert_eq!(in_words("--draft__copy-"), "Draft copy");
    assert_eq!(in_words("x"), "X");
    assert_eq!(in_words("Tank"), "Tank");
    assert_eq!(in_words("ärende-1"), "Ärende 1");
}

#[test]
fn a_new_page_is_titled_with_its_name_in_words() {
    // Catches the file in another folder, a title that keeps the hyphen, and a screen layout
    // written over the project's own.
    let created = ask(json!({
        "action": "create", "kind": "page", "name": "room_heating-2", "hasScreen": true,
    }));
    assert_eq!(
        created,
        json!({
            "ok": true,
            "file": "hmi/pages/room_heating-2.toml",
            "text": "format = 1\ntitle = \"Room heating 2\"\n",
        })
    );
}

#[test]
fn a_new_panel_is_240_by_720_without_fields_and_docked_nowhere() {
    // Catches a panel that docks on every page, demands fields, or has the page's size.
    let created =
        ask(json!({ "action": "create", "kind": "panel", "name": "nav", "hasScreen": true }));
    assert_eq!(created["file"], "hmi/panels/nav.toml");
    let text = created["text"].as_str().expect("the panel's text");
    assert_eq!(
        text,
        "format = 1\ntitle = \"Nav\"\nsize = [240, 720]\n\n[panel]\nscope = \"manual\"\ndock = \"left\"\n"
    );
    let panel = parse_faceplate("hmi/panels/nav.toml", text).expect("the panel loads");
    let dock = panel.panel.expect("a [panel] table");
    assert_eq!(
        (
            panel.popup.size,
            panel.fields.len(),
            dock.scope,
            dock.dock,
            dock.order
        ),
        ([240, 720], 0, HmiDockScope::Manual, HmiDockEdge::Left, 0)
    );
    assert!(dock.pages.is_empty());
}

#[test]
fn a_project_without_a_screen_layout_gets_the_default_one() {
    // §3.4 and §4 item 1: 1920 × 1080, dark, flat symbols; no navigation. Catches the 1280 × 720
    // New HMI Page wrote before, and a layout that does not load.
    let created = ask(json!({
        "action": "create", "kind": "page", "name": "overview", "hasScreen": false,
    }));
    let screen = created["screen"].as_str().expect("a screen layout");
    assert_eq!(
        screen,
        "format = 1\nsize = [1920, 1080]\ntheme = \"dark\"\nsymbols = \"flat\"\n\n[navigation]\nstyle = \"none\"\n"
    );
    assert_eq!(
        parse_screen("hmi/screen.toml", screen)
            .expect("it loads")
            .size,
        [1920, 1080]
    );
}

#[test]
fn an_invalid_name_is_refused_before_any_file_is_made() {
    // Catches a file created under a name the input box would have refused.
    for name in ["my page", ""] {
        let refused =
            answer(json!({ "action": "create", "kind": "page", "name": name, "hasScreen": false }));
        assert_eq!(
            refused,
            Err("Invalid HMI file name.".to_string()),
            "{name:?}"
        );
    }
}

#[test]
fn renames_of_pages_panels_and_popups_are_grouped_by_project() {
    // Catches a rename of a source file, a text file or a move to another folder taken as a
    // page rename, and two projects' renames applied to one project's files.
    let answered = ask(json!({
        "action": "renames",
        "renames": [
            { "from": "/work/plant/hmi/pages/overview.toml", "to": "/work/plant/hmi/pages/process.toml" },
            { "from": "/work/plant/src/main.st", "to": "/work/plant/src/plant.st" },
            { "from": "/work/plant/hmi/pages/pumps.toml", "to": "/work/plant/hmi/popups/pumps.toml" },
            { "from": "/work/plant/hmi/texts/sv.toml", "to": "/work/plant/hmi/texts/de.toml" },
            { "from": "/work/mill/hmi/panels/nav.toml", "to": "/work/mill/hmi/panels/side.toml" },
            { "from": "/work/plant/hmi/popups/pump.toml", "to": "/work/plant/hmi/popups/pump-detail.toml" },
        ],
    }));
    assert_eq!(
        answered,
        json!({
            "ok": true,
            "projects": [
                {
                    "path": "/work/plant",
                    "index": 0,
                    "renames": [
                        { "folder": "pages", "before": "overview", "after": "process" },
                        { "folder": "popups", "before": "pump", "after": "pump-detail" },
                    ],
                },
                {
                    "path": "/work/mill",
                    "index": 4,
                    "renames": [{ "folder": "panels", "before": "nav", "after": "side" }],
                },
            ],
            "pattern": "hmi/{screen.toml,pages/*.toml,panels/*.toml,popups/*.toml,pairs/*.toml}",
        })
    );
}

#[test]
fn a_page_rename_updates_the_start_page_docked_panels_and_page_buttons_only() {
    // The assertions of the extension's former renamedReferences test (hmi-shared-panels): the
    // start page, a docked panel's page list and a page button follow the page; the button's
    // text "overview" and the panel's title "overview" are words, not links, and stay. Catches
    // a link left on the old name, and words rewritten as if they were links.
    let screen = "format = 1\nsize = [1280, 720]\ntheme = \"dark\"\nstart_page = \"overview\"\nsymbols = \"flat\"\n\n[navigation]\nstyle = \"side\"\n";
    let panel = "format = 1\ntitle = \"overview\"\nsize = [240, 720]\n\n[panel]\nscope = \"selected\"\npages = [\"overview\", \"alarms\"]\ndock = \"left\"\n\n[[object]]\nid = \"nav\"\ntype = \"page-button\"\nat = [16, 32]\nsize = [180, 48]\ntext = \"overview\"\n\n[object.click]\npage = \"overview\"\n";
    let written = rewritten(
        "pages",
        "overview",
        "process",
        json!({ "/p/hmi/screen.toml": screen, "/p/hmi/panels/navigation.toml": panel }),
    );
    assert_eq!(
        written,
        json!({
            "/p/hmi/screen.toml": screen.replace("start_page = \"overview\"", "start_page = \"process\""),
            "/p/hmi/panels/navigation.toml": panel
                .replace("pages = [\"overview\"", "pages = [\"process\"")
                .replace("page = \"overview\"", "page = \"process\""),
        })
    );
}

/// A page whose objects link to a page, a popup and a panel, all named `pump`.
const LINKS: &str = "format = 1\ntitle = \"Process\"\n\n[[object]]\nid = \"p-101\"\ntype = \"pump\"\nfaceplate = \"pump\"\ninstance = \"Main.P101\"\nat = [40, 40]\nsize = [80, 80]\n\n[[object]]\nid = \"details\"\ntype = \"button\"\nat = [200, 40]\nsize = [120, 48]\n\n[object.click]\npopup = \"pump\"\n\n[[object]]\nid = \"to-pump\"\ntype = \"page-button\"\nat = [200, 120]\nsize = [120, 48]\n\n[object.click]\npage = \"pump\"\n\n[[object]]\nid = \"open\"\ntype = \"value\"\nat = [200, 200]\nsize = [120, 48]\n\n[object.click]\nfaceplate = \"pump\"\ninstance = \"Main.P102\"\n";

/// A component definition whose panel is `pump`.
const PAIR: &str = "format = 1\nblock = \"Pump\"\nfaceplate = \"pump\"\nsymbol = \"pump\"\n";

#[test]
fn a_page_rename_leaves_links_to_a_panel_or_popup_of_the_same_name() {
    // Catches a panel's own name or a popup's rewritten by a rename in the pages folder.
    let files = json!({ "/p/hmi/pages/process.toml": LINKS, "/p/hmi/pairs/pump.toml": PAIR });
    let written = rewritten("pages", "pump", "pumps", files);
    assert_eq!(
        written,
        json!({
            "/p/hmi/pages/process.toml": LINKS.replace("page = \"pump\"", "page = \"pumps\""),
        })
    );
}

#[test]
fn a_panel_rename_updates_equipment_panel_clicks_and_definitions_only() {
    // Catches a definition still opening the old panel, a click's instance lost, and a popup or
    // page of the same name relinked to the panel (as the extension's rule did before).
    let files = json!({ "/p/hmi/pages/process.toml": LINKS, "/p/hmi/pairs/pump.toml": PAIR });
    let written = rewritten("panels", "pump", "pump-2", files);
    assert_eq!(
        written,
        json!({
            "/p/hmi/pages/process.toml": LINKS
                .replace("faceplate = \"pump\"\ninstance = \"Main.P101\"", "faceplate = \"pump-2\"\ninstance = \"Main.P101\"")
                .replace("faceplate = \"pump\"\ninstance = \"Main.P102\"", "faceplate = \"pump-2\"\ninstance = \"Main.P102\""),
            "/p/hmi/pairs/pump.toml": PAIR.replace("faceplate = \"pump\"", "faceplate = \"pump-2\""),
        })
    );
}

#[test]
fn a_popup_rename_updates_popup_clicks_only() {
    // Catches an equipment object's panel, or a panel click, relinked to a popup of its name.
    let files = json!({ "/p/hmi/pages/process.toml": LINKS, "/p/hmi/pairs/pump.toml": PAIR });
    let written = rewritten("popups", "pump", "pump-detail", files);
    assert_eq!(
        written,
        json!({
            "/p/hmi/pages/process.toml": LINKS.replace("popup = \"pump\"", "popup = \"pump-detail\""),
        })
    );
}

#[test]
fn several_renames_apply_in_turn_and_a_file_is_written_once() {
    // Catches the second rename computed from the file's old text, losing the first.
    let answered = ask(json!({
        "action": "references",
        "renames": [
            { "folder": "pages", "before": "pump", "after": "pumps" },
            { "folder": "popups", "before": "pump", "after": "pump-detail" },
        ],
        "files": { "/p/hmi/pages/process.toml": LINKS },
    }));
    assert_eq!(
        answered["write"],
        json!([{
            "file": "/p/hmi/pages/process.toml",
            "text": LINKS
                .replace("page = \"pump\"", "page = \"pumps\"")
                .replace("popup = \"pump\"", "popup = \"pump-detail\""),
        }])
    );
}

#[test]
fn a_file_that_does_not_load_or_holds_no_links_is_left_as_it_is() {
    // Catches a broken file overwritten, and a text file or symbol rewritten as a page.
    let files = json!({
        "/p/hmi/pages/broken.toml": "format = 1\ntitle =\n[object.click]\npage = \"pump\"\n",
        "/p/hmi/texts/sv.toml": "format = 1\nlanguage = \"sv\"\n\n[texts]\n\"pump\" = \"pump\"\n",
        "/p/hmi/pages/other.toml": "format = 1\ntitle = \"Other\"\n",
    });
    assert_eq!(rewritten("pages", "pump", "pumps", files), json!({}));
}
