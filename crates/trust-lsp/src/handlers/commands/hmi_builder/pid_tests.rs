//! A page and its P&ID (specification 39, §18 phase 4 items 1 to 3): the extension's former
//! `hmi-builder-pid-link.test.ts` model cases, moved here with the rule, and the files the
//! language server now names and writes for the editor.

use serde_json::{json, Value};
use trust_runtime::hmi::catalogue::{Catalogue, ComponentCategory};
use trust_runtime::hmi::page_files::{parse_page, HmiObject, HmiPage, HmiPipe};

use super::{answer, is_process_object, linked_update};

const PID_SIZE: [u32; 2] = [2000, 1000];
const SCREEN: [u32; 2] = [1000, 500];

fn object(id: &str, kind: &str, at: [i64; 2], extra: Value) -> Value {
    let mut object =
        json!({ "id": id, "type": kind, "at": at, "size": [128, 108], "rotation": 0, "bind": {} });
    if let Value::Object(fields) = extra {
        for (key, value) in fields {
            object[key] = value;
        }
    }
    object
}

fn read(path: &str) -> Value {
    json!({ "path": path, "direction": "read" })
}

fn model(value: Value) -> HmiPage {
    serde_json::from_value(value).expect("a page model")
}

/// The P&ID of TK101, P101, XV101, PT101 and LT101, with a note of its own (§18 phase 4).
fn pid() -> HmiPage {
    model(json!({ "title": "Tank farm",
        "object": [
            object("tk101", "tank", [200, 200],
                json!({ "tag": "TK101", "bind": { "level": read("Main.TK101.Level") } })),
            object("p101", "pump", [600, 400],
                json!({ "tag": "P101", "bind": { "running": read("Main.P101.Running") } })),
            object("xv101", "valve", [1000, 400],
                json!({ "tag": "XV101", "bind": { "open": read("Main.XV101.Open") } })),
            object("pt101", "pressure-sensor", [800, 200],
                json!({ "tag": "PT101", "size": [176, 56], "attach": "pipe:0" })),
            object("lt101", "level-sensor", [100, 100],
                json!({ "tag": "LT101", "size": [176, 56], "attach": "tk101" })),
            object("note", "text", [100, 900], json!({ "text": "Tank farm, line 1" })),
        ],
        "pipe": [
            { "from": "tk101.outlet", "to": "p101.inlet", "waypoints": [[400, 450]] },
            { "from": "p101.outlet", "to": "xv101.inlet" },
        ] }))
}

/// The page linked to it, with a display of its own.
fn page() -> HmiPage {
    model(json!({ "title": "Tank farm", "pid": "tank-farm",
        "object": [
            object("tk101", "tank", [100, 100],
                json!({ "tag": "TK101", "bind": { "level": read("Main.TK101.Level") } })),
            object("p101", "pump", [300, 200],
                json!({ "tag": "P101", "bind": { "running": read("Main.P101.Running") } })),
            object("xv101", "valve", [500, 200],
                json!({ "tag": "XV101", "bind": { "open": read("Main.XV101.Open") } })),
            object("pt101", "pressure-sensor", [400, 100],
                json!({ "tag": "PT101", "size": [176, 56], "attach": "pipe:0" })),
            object("lt101", "level-sensor", [50, 50],
                json!({ "tag": "LT101", "size": [176, 56], "attach": "tk101" })),
            object("level", "value", [700, 50],
                json!({ "label": "Level", "bind": { "value": read("Main.TK101.Level") } })),
        ],
        "pipe": [
            { "from": "tk101.outlet", "to": "p101.inlet" },
            { "from": "p101.outlet", "to": "xv101.inlet", "waypoints": [[420, 260]] },
        ] }))
}

fn update(source: &HmiPage, from: [u32; 2], target: &HmiPage, to: [u32; 2]) -> Option<HmiPage> {
    linked_update(&source.objects, &source.pipes, from, target, to)
}

fn find<'a>(page: &'a HmiPage, id: &str) -> Option<&'a HmiObject> {
    page.objects.iter().find(|object| object.id == id)
}

#[test]
fn a_shared_objects_tag_type_and_bindings_changed_in_the_pid_change_on_the_page_its_place_kept() {
    // Wrong behaviour caught: the page keeping the old tag, type or binding, or taking the
    // P&ID's position for its own.
    let mut edited = pid();
    let pump = edited
        .objects
        .iter_mut()
        .find(|object| object.id == "p101")
        .unwrap();
    pump.kind = "dosing-pump".into();
    pump.tag = Some("P102".into());
    pump.bind = serde_json::from_value(json!({ "running": read("Main.P102.Running") })).unwrap();
    pump.at = [900, 900];
    let page = update(&edited, PID_SIZE, &page(), SCREEN).expect("the page changes");
    let pump = find(&page, "p101").unwrap();
    assert_eq!(pump.kind, "dosing-pump");
    assert_eq!(pump.tag.as_deref(), Some("P102"));
    assert_eq!(pump.bind["running"].path, "Main.P102.Running");
    assert_eq!(
        pump.at,
        [300, 200],
        "its place on the page is the page's own"
    );
}

#[test]
fn an_object_added_comes_scaled_one_deleted_leaves_and_displays_and_texts_stay_in_their_view() {
    // Wrong behaviour caught: a new object not scaled to the other view's size, a deleted process
    // object staying, the page's own display lost, or a P&ID-only text leaking into the page.
    let mut added = pid();
    added.objects.retain(|object| object.id != "xv101");
    added.objects.push(
        serde_json::from_value(object(
            "fv102",
            "control-valve",
            [1400, 600],
            json!({ "tag": "FV102" }),
        ))
        .unwrap(),
    );
    let page = update(&added, PID_SIZE, &page(), SCREEN).expect("the page changes");
    assert_eq!(
        find(&page, "fv102").map(|object| object.at),
        Some([700, 300])
    );
    assert!(
        find(&page, "xv101").is_none(),
        "deleted on the P&ID, gone from the page"
    );
    assert!(find(&page, "level").is_some(), "the page's own value stays");
    assert!(
        find(&page, "note").is_none(),
        "the P&ID's text stays on the P&ID"
    );
    assert_eq!(
        update(&self::page(), SCREEN, &pid(), PID_SIZE),
        None,
        "the views agree: nothing to change"
    );
    let mut grown = self::page();
    grown.objects.push(
        serde_json::from_value(object(
            "e101",
            "heat-exchanger",
            [600, 400],
            json!({ "tag": "E101" }),
        ))
        .unwrap(),
    );
    let pid = update(&grown, SCREEN, &pid(), PID_SIZE).expect("the P&ID changes");
    assert_eq!(
        find(&pid, "e101").map(|object| object.at),
        Some([1200, 800]),
        "added on the page, it comes into the P&ID"
    );
    assert!(find(&pid, "level").is_none(), "a display does not");
}

#[test]
fn the_pipes_between_shared_objects_are_one_in_both_views_each_keeping_its_own_bends() {
    // Wrong behaviour caught: the page taking the P&ID's bends, losing its own, or missing a
    // change of a shared pipe's line style.
    let mut rerouted = pid();
    rerouted.pipes = serde_json::from_value(json!([
        { "from": "tk101.outlet", "to": "p101.inlet", "waypoints": [[400, 450]] },
        { "from": "p101.outlet", "to": "xv101.inlet", "style": "signal" },
    ]))
    .unwrap();
    let page = update(&rerouted, PID_SIZE, &page(), SCREEN).expect("the page changes");
    let expected: Vec<HmiPipe> = serde_json::from_value(json!([
        { "from": "tk101.outlet", "to": "p101.inlet" },
        { "from": "p101.outlet", "to": "xv101.inlet", "style": "signal", "waypoints": [[420, 260]] },
    ]))
    .unwrap();
    assert_eq!(page.pipes, expected);
}

#[test]
fn a_new_object_lands_at_the_nearest_whole_pixel_halves_upward() {
    // A place scaled to the other view rounds as the editor's drawing does. Wrong behaviour
    // caught: positions truncated (or rounded away from zero), so an object scaled back and
    // forth creeps.
    let source = model(json!({ "title": "P&ID", "object": [
        object("a", "pump", [3, -3], json!({})), object("b", "pump", [5, -5], json!({})) ] }));
    let target = model(json!({ "title": "Page" }));
    let page = update(&source, [2, 2], &target, [1, 1]).unwrap();
    assert_eq!(find(&page, "a").map(|object| object.at), Some([2, -1]));
    assert_eq!(find(&page, "b").map(|object| object.at), Some([3, -2]));
}

#[test]
fn the_process_objects_are_the_drawn_equipment_junctions_and_the_projects_own_symbols() {
    // §18 phase 4 item 1: process objects are equipment, instruments, junctions and the
    // project's own symbols; texts, lines, displays and controls are each view's own. The rule
    // reads the shipped symbol table, which the editor's drawing reads too; it agrees with the
    // catalogue's equipment category. Wrong behaviour caught: a symbol type added to the
    // catalogue but not shared between the views, or a display shared.
    for component in &Catalogue::shipped().components {
        assert_eq!(
            is_process_object(&component.kind),
            component.category == ComponentCategory::Equipment,
            "{}",
            component.kind
        );
    }
    assert!(is_process_object("my:tank3"));
    assert!(!is_process_object("text"));
    assert!(!is_process_object("line"));
}

fn ask(request: Value) -> Value {
    let reply = answer(request).expect("an answer");
    assert_eq!(reply["ok"], json!(true), "{reply}");
    reply
}

#[test]
fn a_page_is_linked_to_the_pid_it_names_and_a_pid_to_the_pages() {
    // Wrong behaviour caught: a page without `pid` linked to something, a P&ID not reaching its
    // pages, or a popup's model refused.
    let linked = ask(
        json!({ "action": "linked", "file": "hmi/pages/tank-farm.toml",
        "model": { "title": "Tank farm", "pid": "tank-farm", "object": [], "pipe": [] } }),
    );
    assert_eq!(
        (linked["pid"].clone(), linked["files"].clone()),
        (json!(false), json!(["hmi/pid/tank-farm.toml"]))
    );
    let alone = ask(json!({ "action": "linked", "file": "hmi/popups/pump.toml",
        "model": { "title": "Pump", "size": [400, 300], "object": [], "pipe": [] } }));
    assert_eq!(alone["files"], json!([]));
    let pid = ask(json!({ "action": "linked", "file": "hmi/pid/tank-farm.toml" }));
    assert_eq!(
        (
            pid["pid"].clone(),
            pid["files"].clone(),
            pid["pattern"].clone()
        ),
        (json!(true), json!([]), json!("hmi/pages/*.toml"))
    );
}

const PID_TOML: &str = "format = 1\ntitle = \"Tank farm\"\n\n[[object]]\nid = \"tk101\"\ntag = \"TK101\"\ntype = \"tank\"\nat = [200, 200]\nsize = [128, 172]\n\n[[object]]\nid = \"p101\"\ntag = \"P101\"\ntype = \"pump\"\nat = [600, 400]\nsize = [128, 108]\n\n[object.bind]\nrunning = \"Main.P101.Running\"\n\n[[pipe]]\nfrom = \"tk101.outlet\"\nto = \"p101.inlet\"\n";
const PAGE_TOML: &str = "format = 1\ntitle = \"Tank farm\"\npid = \"tank-farm\"\n\n[[object]]\nid = \"tk101\"\ntag = \"TK101\"\ntype = \"tank\"\nat = [100, 100]\nsize = [128, 172]\n\n[[object]]\nid = \"p101\"\ntag = \"P101\"\ntype = \"pump\"\nat = [300, 200]\nsize = [128, 108]\n\n[object.bind]\nrunning = \"Main.P101.Running\"\n\n[[object]]\nid = \"speed\"\ntype = \"value\"\nat = [600, 100]\nsize = [160, 48]\n\n[[pipe]]\nfrom = \"tk101.outlet\"\nto = \"p101.inlet\"\n";

fn file(name: &str, text: &str) -> Value {
    json!({ "file": name, "text": text })
}

fn writes(reply: &Value) -> Vec<(String, HmiPage)> {
    reply["writes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|write| {
            let name = write["file"].as_str().unwrap().to_string();
            let page = parse_page(&name, write["text"].as_str().unwrap()).expect("it loads");
            (name, page)
        })
        .collect()
}

#[test]
fn an_edit_of_a_pid_reaches_the_pages_that_name_it_and_no_other() {
    // Wrong behaviour caught: a page naming another P&ID changed, a page that does not load
    // overwritten, the P&ID written onto itself, or the page's own value lost.
    let mut edited = parse_page("hmi/pid/tank-farm.toml", PID_TOML).unwrap();
    edited.objects[1].tag = Some("P102".into());
    let other = PAGE_TOML.replace("pid = \"tank-farm\"", "pid = \"other\"");
    let reply = ask(json!({ "action": "sync", "file": "hmi/pid/tank-farm.toml",
        "model": edited, "size": [1280, 720],
        "linked": [file("hmi/pages/tank-farm.toml", PAGE_TOML), file("hmi/pages/other.toml", &other),
            file("hmi/pages/broken.toml", "format = 1\n"),
            file("hmi/pid/tank-farm.toml", PID_TOML)] }));
    let written = writes(&reply);
    assert_eq!(written.len(), 1, "{reply}");
    let (name, page) = &written[0];
    assert_eq!(name, "hmi/pages/tank-farm.toml");
    let pump = find(page, "p101").unwrap();
    assert_eq!((pump.tag.as_deref(), pump.at), (Some("P102"), [300, 200]));
    assert!(find(page, "speed").is_some(), "the page's own value stays");
}

#[test]
fn equipment_added_on_a_page_comes_into_its_pid_and_nothing_changed_writes_nothing() {
    // Wrong behaviour caught: the page's P&ID left behind, another P&ID changed, the page's
    // value carried into the P&ID, or an unchanged P&ID written again.
    let mut grown = parse_page("hmi/pages/tank-farm.toml", PAGE_TOML).unwrap();
    grown.objects.push(
        serde_json::from_value(object(
            "e101",
            "heat-exchanger",
            [500, 300],
            json!({ "tag": "E101" }),
        ))
        .unwrap(),
    );
    let reply = ask(
        json!({ "action": "sync", "file": "hmi/pages/tank-farm.toml",
        "model": grown, "size": [1280, 720],
        "linked": [file("hmi/pid/tank-farm.toml", PID_TOML), file("hmi/pid/other.toml", PID_TOML)] }),
    );
    let written = writes(&reply);
    assert_eq!(written.len(), 1, "{reply}");
    let (name, pid) = &written[0];
    assert_eq!(name, "hmi/pid/tank-farm.toml");
    assert_eq!(find(pid, "e101").map(|object| object.at), Some([500, 300]));
    assert!(
        find(pid, "speed").is_none(),
        "the page's value stays on the page"
    );
    let unchanged = parse_page("hmi/pages/tank-farm.toml", PAGE_TOML).unwrap();
    let reply = ask(
        json!({ "action": "sync", "file": "hmi/pages/tank-farm.toml",
        "model": unchanged, "size": [1280, 720],
        "linked": [file("hmi/pid/tank-farm.toml", PID_TOML)] }),
    );
    assert_eq!(reply["writes"], json!([]));
}

#[test]
fn generate_process_screen_writes_the_page_linked_to_the_pid_bound_as_on_it() {
    // §18 phase 4 item 3. Wrong behaviour caught: a page not linked to its P&ID, without its
    // title, its objects unbound, or a page written for a file that is not a P&ID.
    let reply = ask(
        json!({ "action": "processScreen", "file": "hmi/pid/tank-farm.toml",
        "text": PID_TOML, "size": [1280, 720] }),
    );
    assert_eq!(
        (reply["name"].clone(), reply["file"].clone()),
        (json!("tank-farm"), json!("hmi/pages/tank-farm.toml"))
    );
    let text = reply["text"].as_str().unwrap();
    assert!(
        text.starts_with("format = 1\ntitle = \"Tank farm\"\npid = \"tank-farm\"\n"),
        "{text}"
    );
    let page = parse_page("hmi/pages/tank-farm.toml", text).unwrap();
    assert_eq!(
        page.objects
            .iter()
            .map(|object| (object.id.as_str(), object.at))
            .collect::<Vec<_>>(),
        [("tk101", [200, 200]), ("p101", [600, 400])]
    );
    assert_eq!(page.objects[1].bind["running"].path, "Main.P101.Running");
    assert_eq!(page.pipes.len(), 1);
    let refused = answer(
        json!({ "action": "processScreen", "file": "hmi/pages/tank-farm.toml",
        "text": PAGE_TOML, "size": [1280, 720] }),
    );
    assert_eq!(
        refused,
        Err("Open a P&ID (hmi/pid/<name>.toml) to generate its process screen.".to_string())
    );
    let broken = answer(
        json!({ "action": "processScreen", "file": "hmi/pid/tank-farm.toml",
        "text": "format = 1\n", "size": [1280, 720] }),
    );
    assert!(
        broken
            .unwrap_err()
            .starts_with("The P&ID does not load: line "),
        "a P&ID that does not load says where"
    );
}

#[test]
fn a_screen_without_area_scales_nothing() {
    // Wrong behaviour caught: a division by a zero size placing objects nowhere.
    let reply = answer(
        json!({ "action": "processScreen", "file": "hmi/pid/tank-farm.toml",
        "text": PID_TOML, "size": [0, 720] }),
    );
    assert_eq!(
        reply,
        Err("The screen's size 0 × 720 has no area.".to_string())
    );
}
