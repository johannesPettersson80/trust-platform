//! Tests of the setup request and answer records (`messages.rs`): the names the setup page reads, the optional selection the
//! three-page panel sends, and the file of the loop a selection names.

use serde_json::json;

use super::{SetupDiscoverRequest, SetupDiscoverResult};
use crate::setup::catalog::CompiledCatalog;
use crate::setup::form::setup_form_model;

#[test]
fn the_discover_request_carries_an_optional_selection_and_family_in_camel_case() {
    let selecting: SetupDiscoverRequest = serde_json::from_value(json!({
        "projectRoot": "/project",
        "runtimeId": "runtime:local",
        "selection": {
            "loopId": "loop:tank",
            "controllerInstance": "Main.TankController",
            "inputIds": ["manipulatedVariable"],
            "outputIds": ["processVariable"],
        },
        "family": "PI",
    }))
    .expect("a selecting request decodes");
    assert_eq!(selecting.family.as_deref(), Some("PI"));
    assert_eq!(
        selecting.selection.map(|selection| selection.loop_id),
        Some("loop:tank".to_string())
    );

    let listing: SetupDiscoverRequest = serde_json::from_value(json!({
        "projectRoot": "/project",
        "runtimeId": "runtime:local",
    }))
    .expect("a listing decodes without a selection");
    assert!(listing.selection.is_none());
    assert!(listing.family.is_none());

    assert!(serde_json::from_value::<SetupDiscoverRequest>(json!({
        "projectRoot": "/project",
        "runtimeId": "runtime:local",
        "bogus": true,
    }))
    .is_err());
}

#[test]
fn the_answer_names_the_selected_file_only_when_the_engine_accepts_the_selection() {
    let empty = CompiledCatalog {
        symbols: Vec::new(),
        controllers: Vec::new(),
        links: Vec::new(),
        writes: Vec::new(),
    };
    let listing = SetupDiscoverResult {
        form_model: setup_form_model(&empty),
        loops: Vec::new(),
        rejected: Vec::new(),
        selected_file: None,
        estimator_models: Vec::new(),
    };
    let value = serde_json::to_value(&listing).expect("json");
    assert!(
        value.get("selectedFile").is_none(),
        "a listing names no file"
    );

    let accepted = SetupDiscoverResult {
        selected_file: Some("/project/control-loops/tank.json".to_string()),
        ..listing
    };
    let value = serde_json::to_value(&accepted).expect("json");
    assert_eq!(value["selectedFile"], "/project/control-loops/tank.json");
    assert_eq!(value["rejected"], json!([]));
}
