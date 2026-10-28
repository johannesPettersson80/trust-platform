//! Tests of the context, library and inspection answers (specification 43, AT-01 to AT-03), run through the
//! authoring command on a temporary project with a compiling program.
//!
//! Port of `editors/vscode/src/test/suite/hmi-agent-queries.test.ts`: the context and its variables, screen and
//! revisions; the inspection and its region; the library's pages, filters, composition families and definition
//! identities; and of `hmi-agent-completion.test.ts` ("selection is scoped to a project and active panel and
//! disappears on close"). The TypeScript suite runs a fake native command with 105 components; here the real
//! commands run, so the library expectations count the built-in catalogue. Not ported: the partial-discovery
//! test and the preview-state names of library definitions, which need a compilation failure and a state list
//! that the Rust library does not yet answer (see the port report).

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};
use tower_lsp::lsp_types::Url;
use trust_runtime::hmi::catalogue::CATALOGUE_JSON;

use crate::state::ServerState;

use super::super::error::{INVALID_ARGUMENT, VALIDATION_FAILED};
use super::super::service::{execute, COMPOSE, CONTEXT, EDIT, INSPECT, LIBRARY, VALIDATE};

/// The workspace the tests connect.
const WORKSPACE: &str = "workspace-queries";

/// The process page: one measured value, bound to the program's level.
const PROCESS: &str = "format = 1\ntitle = \"Process\"\n\n[[object]]\nid = \"level\"\ntype = \"value\"\nat = [40, 40]\nsize = [200, 64]\n\n[object.bind]\nvalue = \"Main.Level\"\n";

/// A plant page: a tank and a pump, joined by a pipe from the tank's outlet to the pump's inlet.
const PLANT: &str = "format = 1\ntitle = \"Plant\"\n\n[[object]]\nid = \"tank\"\ntype = \"tank\"\nat = [0, 0]\nsize = [100, 100]\n\n[object.bind]\nlevel = \"Main.Level\"\n\n[[object]]\nid = \"pump\"\ntype = \"pump\"\nat = [200, 0]\nsize = [100, 100]\n\n[[pipe]]\nfrom = \"tank.outlet\"\nto = \"pump.inlet\"\n";

/// A value turned a quarter turn: its 100 × 20 design box covers x 240 to 260 and y 160 to 260 once rotated.
const ROTATED: &str = "format = 1\ntitle = \"Rotated\"\n\n[[object]]\nid = \"turned\"\ntype = \"value\"\nat = [200, 200]\nsize = [100, 20]\nrotation = 90\n";

/// A function block with one member of each visibility: `Run` (VAR_INPUT) and `Speed` (VAR_OUTPUT) are public,
/// `Hidden` is an ordinary VAR (PROTECTED by default) and `Secret` is PRIVATE. The compiled program holds all four.
const PUMP_PROGRAM: &str = "FUNCTION_BLOCK Pump\nVAR_INPUT\n    Run : BOOL;\nEND_VAR\nVAR_OUTPUT\n    Speed : REAL;\nEND_VAR\nVAR\n    Hidden : REAL;\nEND_VAR\nVAR PRIVATE\n    Secret : REAL;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nPROGRAM Main\nVAR\n    Level : REAL;\n    P101 : Pump;\nEND_VAR\nP101(Run := TRUE);\nEND_PROGRAM\n";

/// Two value objects: one bound to the block's public output, one to the program's own variable.
const PUMP_PAGE: &str = "format = 1\ntitle = \"Pump\"\n\n[[object]]\nid = \"speed\"\ntype = \"value\"\nat = [40, 40]\nsize = [200, 64]\n\n[object.bind]\nvalue = \"Main.P101.Speed\"\n\n[[object]]\nid = \"level\"\ntype = \"value\"\nat = [40, 140]\nsize = [200, 64]\n\n[object.bind]\nvalue = \"Main.Level\"\n";

/// A project folder under the system temporary folder, removed when the test ends.
struct Project(PathBuf);

impl Project {
    /// A project with a compiling program and a 1280 × 800 screen. The tests add their own pages.
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "trust-hmi-queries-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&path);
        let project = Self(path);
        project.write(
            "src/main.st",
            "PROGRAM Main\nVAR Level : REAL; END_VAR\nEND_PROGRAM\n",
        );
        project.write(
            "hmi/screen.toml",
            "format = 1\nsize = [1280, 800]\ntheme = \"dark\"\nsymbols = \"flat\"\n",
        );
        project
    }

    /// Writes a file of the project, creating its folders.
    fn write(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().expect("a folder")).expect("create the folder");
        fs::write(path, text).expect("write the file");
    }

    /// The argument of a command on this project: its address, its storage and the request's input, with the
    /// extra fields the command reads (the open documents, the selections).
    fn argument(&self, input: Value, extra: Value) -> Value {
        let root_uri = Url::from_directory_path(&self.0)
            .expect("a folder address")
            .to_string();
        let mut argument = json!({
            "workspaceId": WORKSPACE,
            "root_uri": root_uri,
            "storage": self.0.join("private").display().to_string(),
            "open": {},
            "request": { "contractVersion": 1, "workspaceId": WORKSPACE, "input": input },
        });
        if let (Value::Object(fields), Value::Object(extra)) = (&mut argument, extra) {
            fields.extend(extra);
        }
        argument
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Runs an authoring command on its argument and returns its envelope.
fn call(state: &ServerState, command: &str, argument: Value) -> Value {
    execute(state, command, vec![argument]).expect("an authoring command")
}

/// The data of a successful answer.
fn ok_data(answer: &Value) -> &Value {
    assert_eq!(answer["ok"], json!(true), "{answer}");
    &answer["data"]
}

/// Every variable of a context answer, with the members of each program node after it (the variable tree is
/// nested: `Main` holds `Main.Level`).
fn all_variables(nodes: &Value) -> Vec<&Value> {
    let mut found = Vec::new();
    if let Some(list) = nodes.as_array() {
        for node in list {
            found.push(node);
            found.extend(all_variables(&node["children"]));
        }
    }
    found
}

/// The identifiers of a list of objects, in the list's order.
fn ids_of(objects: &Value) -> Vec<&str> {
    objects
        .as_array()
        .expect("a list of objects")
        .iter()
        .map(|object| object["id"].as_str().expect("an identifier"))
        .collect()
}

/// The number of components in the built-in catalogue.
fn catalogue_total() -> usize {
    let catalogue: Value = serde_json::from_str(CATALOGUE_JSON).expect("the catalogue is JSON");
    catalogue["components"]
        .as_array()
        .expect("the components")
        .len()
}

/// The inspection of a region of a page, with its answer's data.
fn inspect_region(state: &ServerState, project: &Project, file: &str, region: Value) -> Value {
    let answer = call(
        state,
        INSPECT,
        project.argument(json!({ "file": file, "region": region }), json!({})),
    );
    ok_data(&answer).clone()
}

/// Port of `hmi-agent-queries.test.ts` ("context exposes typed variables, screen and file revisions without
/// inferred topology"): the program's variables, the screen, the page revisions, and a topology that is
/// unspecified rather than inferred (specification 43, AT-01).
#[test]
fn the_context_names_the_variables_screen_and_revisions_without_inferred_topology() {
    let project = Project::new("context");
    project.write("hmi/pages/process.toml", PROCESS);
    let state = ServerState::new();
    let answer = call(&state, CONTEXT, project.argument(json!({}), json!({})));
    let context = ok_data(&answer);
    let level = all_variables(&context["variables"])
        .into_iter()
        .find(|node| node["path"] == "Main.Level")
        .expect("the declared level");
    // The native variable tree's kinds of a REAL (the HMI Builder's own picker shows the same).
    assert_eq!(level["kinds"], json!(["number", "value"]));
    assert_eq!(context["screen"]["size"], json!([1280, 800]));
    assert_eq!(context["screen"]["theme"], json!("dark"));
    assert_eq!(
        context["topology"],
        json!({ "status": "unspecified", "connections": [] })
    );
    let revision = context["revisions"]["hmi/pages/process.toml"]
        .as_str()
        .expect("a revision");
    assert!(revision.starts_with("disk:"), "{revision}");
    assert_eq!(context["compilation"]["status"], json!("ready"));
    assert_eq!(context["pageCount"], json!(1));
    assert_eq!(context["nextCursor"], Value::Null);
}

/// A context section request keeps the mandatory fields and returns only the sections it names (specification
/// 43, AT-01; the `sections` filter of `queries.ts`).
#[test]
fn a_context_section_request_returns_the_named_section_and_the_mandatory_fields() {
    let project = Project::new("sections");
    project.write("hmi/pages/process.toml", PROCESS);
    let state = ServerState::new();
    let answer = call(
        &state,
        CONTEXT,
        project.argument(json!({ "sections": ["screen"] }), json!({})),
    );
    let context = ok_data(&answer);
    assert!(context.get("screen").is_some(), "the named section");
    for key in ["variables", "pages", "catalogue", "symbols", "faceplates"] {
        assert!(context.get(key).is_none(), "{key} was not requested");
    }
    for key in [
        "compilation",
        "partial",
        "selection",
        "provenance",
        "revisions",
        "programRevision",
        "sourceState",
        "nextCursor",
        "pageCount",
        "drafts",
    ] {
        assert!(context.get(key).is_some(), "{key} is mandatory");
    }
}

/// Specification 43, review with `explore: true`: the `sources` section holds the text and unsaved state of each page and
/// popup as the documents observe them, a dirty buffer over its file; the screen settings are not a page. Only a request
/// that names the section gets it, so a full context does not carry every page's text.
#[test]
fn the_sources_section_holds_each_page_and_popup_as_the_documents_observe_them() {
    let project = Project::new("sources");
    project.write("hmi/pages/process.toml", PROCESS);
    project.write("hmi/pages/plant.toml", PROCESS);
    project.write("hmi/popups/detail.toml", ROTATED);
    let state = ServerState::new();
    let open = json!({ "open": { "hmi/pages/plant.toml": { "text": PLANT, "version": 3, "dirty": true } } });
    let answer = call(
        &state,
        CONTEXT,
        project.argument(json!({ "sections": ["sources"] }), open.clone()),
    );
    let sources = ok_data(&answer)["sources"]
        .as_array()
        .expect("the sources")
        .clone();
    let by_file: BTreeMap<&str, &Value> = sources
        .iter()
        .map(|source| (source["file"].as_str().expect("a file"), source))
        .collect();
    assert_eq!(
        by_file.keys().copied().collect::<Vec<_>>(),
        [
            "hmi/pages/plant.toml",
            "hmi/pages/process.toml",
            "hmi/popups/detail.toml"
        ],
        "every page and popup, and not the screen settings"
    );
    assert_eq!(
        by_file["hmi/pages/plant.toml"]["text"], PLANT,
        "the dirty buffer, not the file"
    );
    assert_eq!(by_file["hmi/pages/plant.toml"]["dirty"], true);
    assert_eq!(by_file["hmi/pages/process.toml"]["text"], PROCESS);
    assert_eq!(by_file["hmi/pages/process.toml"]["dirty"], false);
    assert_eq!(by_file["hmi/popups/detail.toml"]["text"], ROTATED);
    for input in [json!({ "sections": ["screen"] }), json!({})] {
        let answer = call(&state, CONTEXT, project.argument(input, open.clone()));
        assert!(
            ok_data(&answer).get("sources").is_none(),
            "only a named request gets the sources"
        );
    }
}

/// Port of `hmi-agent-queries.test.ts` ("library queries paginate ..."), for the context's pages: a window names
/// the cursor of its next page and the answer stays partial until its last page (specification 43, §4).
#[test]
fn the_context_pages_are_a_window_with_a_next_cursor() {
    let project = Project::new("pages");
    project.write("hmi/pages/a.toml", "format = 1\ntitle = \"A\"\n");
    project.write("hmi/pages/b.toml", "format = 1\ntitle = \"B\"\n");
    project.write("hmi/pages/process.toml", PROCESS);
    let state = ServerState::new();
    let first_answer = call(
        &state,
        CONTEXT,
        project.argument(json!({ "limit": 2 }), json!({})),
    );
    let first = ok_data(&first_answer);
    assert_eq!(first["pageCount"], json!(3));
    assert_eq!(first["nextCursor"], json!(2));
    assert_eq!(first["partial"], json!(true));
    let first_files: Vec<&str> = first["pages"]
        .as_array()
        .expect("the first window")
        .iter()
        .map(|page| page["file"].as_str().expect("a file"))
        .collect();
    assert_eq!(first_files, ["hmi/pages/a.toml", "hmi/pages/b.toml"]);
    let second_answer = call(
        &state,
        CONTEXT,
        project.argument(json!({ "cursor": 2, "limit": 2 }), json!({})),
    );
    let second = ok_data(&second_answer);
    assert_eq!(second["pages"][0]["file"], json!("hmi/pages/process.toml"));
    assert_eq!(second["pages"].as_array().map(Vec::len), Some(1));
    assert_eq!(second["nextCursor"], Value::Null);
    assert_eq!(second["partial"], json!(false));
}

/// Port of `hmi-agent-completion.test.ts` ("selection is scoped to a project and active panel and disappears on
/// close"): the context reports the panel that shows the named page, else the one active panel. It reports none
/// when the choice is ambiguous or the panels have closed (specification 43, AT-01 and §10.1).
#[test]
fn the_context_selection_is_the_named_page_or_the_one_active_panel() {
    let project = Project::new("selection");
    project.write("hmi/pages/process.toml", PROCESS);
    let state = ServerState::new();
    let panel = json!({
        "file": "hmi/pages/process.toml",
        "ids": ["level"],
        "documentVersion": 3,
        "active": true,
    });
    let active_answer = call(
        &state,
        CONTEXT,
        project.argument(json!({}), json!({ "selections": [panel.clone()] })),
    );
    assert_eq!(
        ok_data(&active_answer)["selection"],
        json!({ "file": "hmi/pages/process.toml", "ids": ["level"], "documentVersion": 3 })
    );
    let other_answer = call(
        &state,
        CONTEXT,
        project.argument(
            json!({ "file": "hmi/pages/other.toml" }),
            json!({ "selections": [panel.clone()] }),
        ),
    );
    assert_eq!(ok_data(&other_answer)["selection"], Value::Null);
    let named_answer = call(
        &state,
        CONTEXT,
        project.argument(
            json!({ "file": "hmi/pages/process.toml" }),
            json!({ "selections": [panel.clone()] }),
        ),
    );
    assert_eq!(ok_data(&named_answer)["selection"]["ids"], json!(["level"]));
    let closed_answer = call(
        &state,
        CONTEXT,
        project.argument(json!({}), json!({ "selections": [] })),
    );
    assert_eq!(ok_data(&closed_answer)["selection"], Value::Null);
    let other_panel = json!({
        "file": "hmi/pages/other.toml",
        "ids": [],
        "documentVersion": 1,
        "active": true,
    });
    let ambiguous_answer = call(
        &state,
        CONTEXT,
        project.argument(json!({}), json!({ "selections": [panel, other_panel] })),
    );
    assert_eq!(ok_data(&ambiguous_answer)["selection"], Value::Null);
}

/// Port of `hmi-agent-queries.test.ts` ("inspection returns selected objects and connected neighbours
/// separately"): the selected objects, the pipes they join, the connected neighbour apart from the selection, the
/// original z-order of the selection and the page's title (specification 43, AT-03).
#[test]
fn an_inspection_returns_the_selection_and_its_neighbours_apart() {
    let project = Project::new("inspect");
    project.write("hmi/pages/plant.toml", PLANT);
    let state = ServerState::new();
    let answer = call(
        &state,
        INSPECT,
        project.argument(
            json!({ "file": "hmi/pages/plant.toml", "ids": ["tank"] }),
            json!({}),
        ),
    );
    let inspection = ok_data(&answer);
    assert_eq!(ids_of(&inspection["objects"]), vec!["tank"]);
    assert_eq!(inspection["neighbours"], json!(["pump"]));
    assert_eq!(inspection["pipes"].as_array().map(Vec::len), Some(1));
    assert_eq!(inspection["zOrder"], json!({ "tank": 0 }));
    assert_eq!(inspection["title"], json!("Plant"));
    assert_eq!(inspection["origin"], json!("source"));
    assert_eq!(inspection["dirty"], json!(false));
    let revision = inspection["revision"].as_str().expect("a revision");
    assert!(revision.starts_with("disk:"), "{revision}");
}

/// Port of `hmi-agent-queries.test.ts` ("region inspection selects rotated design boxes and preserves connected
/// neighbours"): a region selects the objects whose rotated boxes overlap its interior. An empty region selects
/// nothing, and a box that only touches the region's edge is not selected (specification 43, AT-03).
#[test]
fn a_region_inspection_selects_the_rotated_boxes_that_overlap_it() {
    let project = Project::new("region");
    project.write("hmi/pages/plant.toml", PLANT);
    project.write("hmi/pages/rotated.toml", ROTATED);
    let state = ServerState::new();
    let top_left = inspect_region(
        &state,
        &project,
        "hmi/pages/plant.toml",
        json!({ "x": 0, "y": 0, "width": 120, "height": 120 }),
    );
    assert_eq!(ids_of(&top_left["objects"]), vec!["tank"]);
    assert_eq!(top_left["neighbours"], json!(["pump"]));
    assert_eq!(top_left["zOrder"], json!({ "tank": 0 }));
    let empty = inspect_region(
        &state,
        &project,
        "hmi/pages/plant.toml",
        json!({ "x": 500, "y": 500, "width": 20, "height": 20 }),
    );
    assert_eq!(empty["objects"], json!([]));
    assert_eq!(empty["pipes"], json!([]));
    let turned = inspect_region(
        &state,
        &project,
        "hmi/pages/rotated.toml",
        json!({ "x": 240, "y": 160, "width": 20, "height": 20 }),
    );
    assert_eq!(ids_of(&turned["objects"]), vec!["turned"]);
    let touching = inspect_region(
        &state,
        &project,
        "hmi/pages/rotated.toml",
        json!({ "x": 260, "y": 160, "width": 20, "height": 20 }),
    );
    assert_eq!(touching["objects"], json!([]));
}

/// An inspection selects object IDs or one region, never both, names a page, and names only objects the page
/// holds (specification 43, AT-03; the refusals of `inspect` in `queries.ts`).
#[test]
fn an_inspection_refuses_ids_with_a_region_paths_outside_the_pages_and_unknown_objects() {
    let project = Project::new("inspect-refusals");
    project.write("hmi/pages/plant.toml", PLANT);
    let state = ServerState::new();
    let both = call(
        &state,
        INSPECT,
        project.argument(
            json!({
                "file": "hmi/pages/plant.toml",
                "ids": ["tank"],
                "region": { "x": 0, "y": 0, "width": 20, "height": 20 },
            }),
            json!({}),
        ),
    );
    assert_eq!(both["ok"], json!(false));
    assert_eq!(both["code"], INVALID_ARGUMENT);
    let outside = call(
        &state,
        INSPECT,
        project.argument(json!({ "file": "../outside" }), json!({})),
    );
    assert_eq!(outside["code"], INVALID_ARGUMENT);
    let zero_width = call(
        &state,
        INSPECT,
        project.argument(
            json!({
                "file": "hmi/pages/plant.toml",
                "region": { "x": 0, "y": 0, "width": 0, "height": 20 },
            }),
            json!({}),
        ),
    );
    assert_eq!(zero_width["code"], INVALID_ARGUMENT);
    let unknown = call(
        &state,
        INSPECT,
        project.argument(
            json!({ "file": "hmi/pages/plant.toml", "ids": ["ghost"] }),
            json!({}),
        ),
    );
    assert_eq!(unknown["code"], INVALID_ARGUMENT);
}

/// Port of `hmi-agent-queries.test.ts` ("library queries paginate instead of silently dropping components"): every
/// component is reached through the next cursor, and the default page holds twelve (`hmi_browse_library` in
/// `tools.ts`). The TypeScript test counts 105 fake components; this one counts the built-in catalogue, so the
/// page sizes are those of the real count (specification 43, AT-02 and §4).
#[test]
fn the_library_pages_reach_every_component_through_the_next_cursor() {
    let total = catalogue_total();
    assert!(
        total > 50,
        "the built-in catalogue holds more than one page of fifty"
    );
    let project = Project::new("library-pages");
    let state = ServerState::new();
    let default_answer = call(&state, LIBRARY, project.argument(json!({}), json!({})));
    let default = ok_data(&default_answer);
    assert_eq!(default["components"].as_array().map(Vec::len), Some(12));
    assert_eq!(default["nextCursor"], json!(12));
    assert_eq!(default["total"], json!(total));
    let first_answer = call(
        &state,
        LIBRARY,
        project.argument(json!({ "limit": 50 }), json!({})),
    );
    let first = ok_data(&first_answer);
    assert_eq!(first["components"].as_array().map(Vec::len), Some(50));
    assert_eq!(first["nextCursor"], json!(50));
    let rest_answer = call(
        &state,
        LIBRARY,
        project.argument(json!({ "cursor": 50, "limit": 50 }), json!({})),
    );
    let rest = ok_data(&rest_answer);
    assert_eq!(
        rest["components"].as_array().map(Vec::len),
        Some(total - 50)
    );
    assert_eq!(rest["nextCursor"], Value::Null);
}

/// Port of `hmi-agent-queries.test.ts` ("library exposes versioned composition families with operator tasks and
/// honest review status"), in its filter rule: a query selects the components whose type, category or group
/// contains it, and the composition families whose ID, title or task does (specification 43, AT-02).
#[test]
fn a_library_query_filters_components_and_composition_families() {
    let project = Project::new("library-filter");
    let state = ServerState::new();
    let pumps_answer = call(
        &state,
        LIBRARY,
        project.argument(json!({ "query": "pump" }), json!({})),
    );
    let pumps = ok_data(&pumps_answer);
    let components = pumps["components"].as_array().expect("the components");
    assert!(components
        .iter()
        .any(|component| component["type"] == "pump"));
    for component in components {
        let text = format!(
            "{} {} {}",
            component["type"], component["category"], component["group"]
        )
        .to_lowercase();
        assert!(text.contains("pump"), "{text}");
    }
    assert_eq!(pumps["compositionPatterns"], json!([]));
    let commissioning_answer = call(
        &state,
        LIBRARY,
        project.argument(json!({ "query": "commissioning" }), json!({})),
    );
    let commissioning = ok_data(&commissioning_answer);
    assert_eq!(commissioning["components"], json!([]));
    let families: Vec<&str> = commissioning["compositionPatterns"]
        .as_array()
        .expect("the families")
        .iter()
        .map(|pattern| pattern["id"].as_str().expect("an identifier"))
        .collect();
    assert_eq!(families, ["settings"]);
}

/// Port of `hmi-agent-queries.test.ts` ("library exposes versioned composition families with operator tasks and
/// honest review status"): five composition families in their order, each versioned, with its task, its required
/// inputs and a pending visual review, and none with a reference until its exact ID is asked for (specification
/// 43, §6 and AT-02).
#[test]
fn the_library_lists_five_composition_families_with_pending_review() {
    let project = Project::new("families");
    let state = ServerState::new();
    let answer = call(&state, LIBRARY, project.argument(json!({}), json!({})));
    let patterns = ok_data(&answer)["compositionPatterns"]
        .as_array()
        .expect("the families")
        .clone();
    let ids: Vec<&str> = patterns
        .iter()
        .map(|pattern| pattern["id"].as_str().expect("an identifier"))
        .collect();
    assert_eq!(
        ids,
        [
            "process-overview",
            "equipment-detail",
            "multi-unit",
            "investigation",
            "settings",
        ]
    );
    for pattern in &patterns {
        assert_eq!(pattern["version"], json!(1));
        assert!(pattern["task"].as_str().is_some_and(|task| task.len() > 20));
        assert!(pattern["requiredInputs"]
            .as_array()
            .is_some_and(|inputs| !inputs.is_empty()));
        assert_eq!(pattern["visualReview"], json!("pending"));
        assert!(pattern.get("reference").is_none(), "{pattern}");
    }
}

/// Port of `hmi-agent-tools.test.ts` ("...exact pattern ID returns a browser-rendered reference image and its
/// normal builder object model"), its object-model part: an exact family ID adds the reference to that family
/// only (specification 43, AT-02). The image is drawn by the extension and is not checked here.
#[test]
fn an_exact_family_id_adds_its_reference_model_to_that_family_only() {
    let project = Project::new("reference");
    let state = ServerState::new();
    let answer = call(
        &state,
        LIBRARY,
        project.argument(json!({ "query": "process-overview" }), json!({})),
    );
    let patterns = ok_data(&answer)["compositionPatterns"]
        .as_array()
        .expect("the families")
        .clone();
    assert_eq!(patterns.len(), 1, "{patterns:?}");
    let reference = &patterns[0]["reference"];
    assert!(reference["model"].is_object(), "{reference}");
    assert_eq!(reference["size"].as_array().map(Vec::len), Some(2));
    assert!(reference["missingComponents"].is_array(), "{reference}");
}

/// Port of `hmi-agent-queries.test.ts` ("library definitions carry stable identities ..."), its identity part: the
/// same catalogue has the same revision, and each definition carries a SHA-256 revision. The preview-state names
/// of the TypeScript test are not ported (see the port report).
#[test]
fn library_definitions_carry_stable_revisions() {
    let project = Project::new("identity");
    let state = ServerState::new();
    let first_answer = call(
        &state,
        LIBRARY,
        project.argument(json!({ "query": "pump", "limit": 1 }), json!({})),
    );
    let second_answer = call(
        &state,
        LIBRARY,
        project.argument(json!({ "query": "pump", "limit": 1 }), json!({})),
    );
    let first = ok_data(&first_answer);
    let second = ok_data(&second_answer);
    assert_eq!(first["catalogueRevision"], second["catalogueRevision"]);
    let revision = first["components"][0]["definitionRevision"]
        .as_str()
        .expect("a definition revision");
    assert_eq!(revision.len(), 64, "{revision}");
    assert!(
        revision.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{revision}"
    );
}

/// The context offers the program's variables and the public members of its function block, and not the block's
/// protected or private members. The binding checks refuse those members, so a page bound to an offered path has no
/// binding finding, and an edit that binds a protected member is refused (specification 39, §7.1; specification 43,
/// AT-01 and AT-06).
#[test]
fn the_context_offers_only_the_members_the_binding_checks_resolve() {
    let project = Project::new("protected-members");
    project.write("src/main.st", PUMP_PROGRAM);
    project.write("hmi/pages/pump.toml", PUMP_PAGE);
    let state = ServerState::new();
    let answer = call(&state, CONTEXT, project.argument(json!({}), json!({})));
    let context = ok_data(&answer);
    assert_eq!(context["compilation"]["status"], json!("ready"), "{answer}");
    let offered: Vec<&str> = all_variables(&context["variables"])
        .into_iter()
        .filter_map(|node| node["path"].as_str())
        .collect();
    for path in [
        "Main.Level",
        "Main.P101",
        "Main.P101.Run",
        "Main.P101.Speed",
    ] {
        assert!(offered.contains(&path), "{path} is offered: {offered:?}");
    }
    for path in ["Main.P101.Hidden", "Main.P101.Secret"] {
        assert!(
            !offered.contains(&path),
            "{path} is not offered: {offered:?}"
        );
    }
    let composed = call(
        &state,
        COMPOSE,
        project.argument(
            json!({
                "key": "pump-copy",
                "file": "hmi/pages/pump.toml",
                "sourceFile": "hmi/pages/pump.toml",
            }),
            json!({}),
        ),
    );
    let draft = ok_data(&composed).clone();
    let checked = call(
        &state,
        VALIDATE,
        project.argument(
            json!({ "draftId": draft["draftId"], "revision": draft["revision"] }),
            json!({}),
        ),
    );
    let findings = ok_data(&checked)["findings"]
        .as_array()
        .expect("the findings");
    assert!(
        findings
            .iter()
            .all(|finding| finding["rule"] != json!("binding")),
        "{checked}"
    );
    let refused = call(
        &state,
        EDIT,
        project.argument(
            json!({
                "draftId": draft["draftId"],
                "revision": draft["revision"],
                "key": "bind-hidden",
                "operations": [{
                    "op": "bind",
                    "id": "speed",
                    "property": "value",
                    "binding": { "path": "Main.P101.Hidden", "direction": "read" },
                }],
            }),
            json!({}),
        ),
    );
    assert_eq!(refused["code"], json!(VALIDATION_FAILED), "{refused}");
}
