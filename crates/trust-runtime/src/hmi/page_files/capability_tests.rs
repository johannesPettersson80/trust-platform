//! Specification 39, §21.2: method commands survive native page serialization and never become
//! scalar writes.
use super::{parse_page, write_page};

const PAGE: &str = r#"format = 1
title = "Pump"
[[object]]
id = "start"
type = "button"
at = [0, 0]
size = [100, 40]
[object.command]
interface = "Plant.IDevice"
method = "SetMode"
instance = "Main.Pumps[2]"
[object.command.arguments]
Value = "Manual"
"#;

#[test]
fn capability_round_trip_preserves_request_declaration() {
    let first = parse_page("hmi/pages/pump.toml", PAGE).unwrap();
    let text = write_page(&first);
    let second = parse_page("hmi/pages/pump.toml", &text).unwrap();
    assert_eq!(first.objects[0].command, second.objects[0].command);
    assert_eq!(write_page(&second), text);
}

#[test]
fn capability_never_falls_back_to_a_scalar_write() {
    let mut page = parse_page("hmi/pages/pump.toml", PAGE).unwrap();
    page.objects[0].bind.insert(
        "value".into(),
        super::HmiBinding {
            quality: None,
            path: "Main.UnrelatedOutput".into(),
            unit: None,
            range: None,
            direction: super::HmiBindingDirection::Write,
        },
    );
    let catalogue = crate::hmi::catalogue::Catalogue::shipped();
    let component = catalogue.component("button").unwrap();
    assert!(
        crate::hmi::commands::plan::plan(&page.objects[0], component, "set", None, None).is_err()
    );
}

#[test]
fn a_command_with_a_role_source_or_a_scalar_action_is_rejected() {
    let source = PAGE.replace(
        "[object.command]\n",
        "[object.command]\nsource = \"maintenance\"\n",
    );
    assert!(
        parse_page("hmi/pages/pump.toml", &source).is_err(),
        "the control's role is the role a command needs"
    );
    let mixed = PAGE.replace("size = [100, 40]", "size = [100, 40]\naction = \"set\"");
    assert!(parse_page("hmi/pages/pump.toml", &mixed).is_err());
    let panel = PAGE.replace(
        "instance = \"Main.Pumps[2]\"",
        "instance = \"$instance.Outer\"",
    );
    assert!(
        parse_page("hmi/pages/pump.toml", &panel).is_ok(),
        "a block inside a panel's instance"
    );
}

#[test]
fn quality_and_regions_survive_native_page_round_trip() {
    let source = r#"format=1
title="Process"
[[regions]]
at=[200,0]
size=[1080,720]
objects=["flow"]
pipes=[]
[[object]]
id="flow"
type="value"
at=[20,20]
size=[160,60]
[object.bind]
value={path="Main.FT101.Value",quality="Main.FT101.Signal.Quality"}
"#;
    let page = parse_page("hmi/pages/main.toml", source).unwrap();
    assert_eq!(
        page.objects[0].bind["value"].quality.as_deref(),
        Some("Main.FT101.Signal.Quality")
    );
    let written = write_page(&page);
    assert_eq!(parse_page("hmi/pages/main.toml", &written).unwrap(), page);
    let invalid = source.replace("objects=[\"flow\"]", "objects=[\"missing\"]");
    assert!(parse_page("hmi/pages/main.toml", &invalid).is_err());
}

#[test]
fn generic_panel_placement_is_allowed_but_equipment_panel_requires_instance() {
    let page = parse_page(
        "hmi/pages/main.toml",
        r#"format=1
title="Process"
[[object]]
id="panel"
type="faceplate"
faceplate="shared"
at=[0,0]
size=[200,720]
"#,
    )
    .unwrap();
    let mut bundle = crate::hmi::bundle::HmiBundle::default();
    let file = "hmi/panels/shared.toml";
    bundle.files.insert(
        file.into(),
        b"format=1
title='Navigation'
size=[200,720]
"
        .to_vec(),
    );
    let storage = crate::memory::VariableStorage::default();
    assert!(crate::hmi::binding_checks::check_placements(&page, &bundle, &storage).is_empty());
    bundle.files.insert(
        file.into(),
        b"format=1
title='Pump'
size=[200,720]
settings=true
"
        .to_vec(),
    );
    assert_eq!(
        crate::hmi::binding_checks::check_placements(&page, &bundle, &storage).len(),
        1
    );
}
