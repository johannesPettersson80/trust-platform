//! Specification 39, §12.2.3: a control that needs more than the write policy allows is an error
//! in the editor.

use super::config::hmi_access_from_runtime_toml;
use super::page_policy::check_page_policy;
use crate::hmi::catalogue::Catalogue;
use crate::hmi::page_files::parse_page;

const RUNTIME_TOML: &str = r#"
[runtime.web]
enabled = true

[[runtime.web.hmi.write]]
path = "Loop.Setpoint"
role = "operator"
min = 0.0
max = 120.0

[[runtime.web.hmi.write]]
path = "Loop.Tuning.*"
role = "engineer"
"#;

fn problems(objects: &str) -> Vec<String> {
    let policy = hmi_access_from_runtime_toml(RUNTIME_TOML)
        .expect("the policy loads")
        .write;
    let page = parse_page(
        "hmi/pages/p.toml",
        &format!("format = 1\ntitle = \"P\"\n{objects}"),
    )
    .expect("the page loads");
    check_page_policy(&page, Catalogue::shipped(), &policy)
        .into_iter()
        .map(|problem| {
            format!(
                "{}.{}: {}",
                problem.object,
                problem.property.unwrap_or_default(),
                problem.message
            )
        })
        .collect()
}

fn setpoint(id: &str, keys: &str, path: &str) -> String {
    format!("\n[[object]]\nid = \"{id}\"\ntype = \"setpoint\"\nat = [0, 0]\nsize = [200, 48]\n{keys}\n\n[object.bind]\nvalue = {{ path = \"{path}\", direction = \"read-write\" }}\n")
}

#[test]
fn a_control_within_the_policy_has_no_problem() {
    assert_eq!(
        problems(&setpoint("sp", "min = 10.0\nmax = 90.0", "Loop.Setpoint")),
        Vec::<String>::new()
    );
    assert_eq!(
        problems(&setpoint("gain", "role = \"engineer\"", "Loop.Tuning.Gain")),
        Vec::<String>::new()
    );
}

#[test]
fn writing_a_point_the_policy_does_not_cover_is_an_error() {
    let found = problems(&setpoint("valve", "", "Plant.Valve"));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].starts_with("valve.value:") && found[0].contains("write policy"),
        "{found:?}"
    );
}

#[test]
fn asking_a_lower_role_than_the_policy_is_an_error() {
    let found = problems(&setpoint("gain", "role = \"operator\"", "Loop.Tuning.Gain"));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("Engineer"), "{found:?}");
}

#[test]
fn wider_limits_than_the_policy_are_an_error() {
    let found = problems(&setpoint("sp", "min = -10.0\nmax = 200.0", "Loop.Setpoint"));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("limits") && found[0].contains("120"),
        "{found:?}"
    );
}

#[test]
fn displays_and_reading_bindings_are_not_checked() {
    let objects = "\n[[object]]\nid = \"pv\"\ntype = \"value\"\nat = [0, 0]\nsize = [160, 48]\n\n[object.bind]\nvalue = \"Plant.Level\"\n\n[[object]]\nid = \"sp\"\ntype = \"setpoint\"\nat = [0, 60]\nsize = [200, 48]\n\n[object.bind]\nhigh-limit = \"Plant.High\"\n";
    assert_eq!(problems(objects), Vec::<String>::new());
}

#[test]
fn a_project_without_a_policy_writes_nothing() {
    let none = hmi_access_from_runtime_toml("[resource]\nname = \"main\"\n")
        .expect("no section is no policy");
    assert!(none.write.is_empty());
    assert!(hmi_access_from_runtime_toml("[runtime.web.hmi]\nidle_minutes = 0\n").is_err());
}

/// Specification 39, §21.2 item 1: a method command needs no write rule; a rule that covers its
/// instance only raises the role it needs.
#[test]
fn a_method_command_needs_no_write_rule() {
    let command = "\n[[object]]\nid = \"start\"\ntype = \"button\"\nat = [0, 0]\nsize = [120, 48]\n\n[object.command]\ninterface = \"Plant.IRunDevice\"\nmethod = \"Start\"\ninstance = \"Main.P101\"\n";
    assert_eq!(problems(command), Vec::<String>::new());
    let lower = "\n[[object]]\nid = \"tune\"\ntype = \"button\"\nat = [0, 0]\nsize = [120, 48]\nrole = \"operator\"\n\n[object.command]\ninterface = \"Plant.ISetpoint\"\nmethod = \"SetSetpoint\"\ninstance = \"Loop.Tuning.Pid\"\n";
    let found = problems(lower);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("Engineer"), "{found:?}");
}

/// Specification 39, §12.2.3: the problem of a point no rule covers names that point, so that the
/// editor's quick fix allows exactly it without reading the message; a problem with a rule that
/// covers the point (a lower role asked) names none, since allowing the point would not fix it.
/// Catches a quick fix offered for a role or limit problem, and a point recovered from the text.
#[test]
fn only_the_problem_of_an_uncovered_point_names_the_point() {
    let policy = hmi_access_from_runtime_toml(RUNTIME_TOML)
        .expect("the policy loads")
        .write;
    let objects = format!(
        "{}{}",
        setpoint("valve", "", "Plant.Valve"),
        setpoint("gain", "role = \"operator\"", "Loop.Tuning.Gain")
    );
    let page = parse_page(
        "hmi/pages/p.toml",
        &format!("format = 1\ntitle = \"P\"\n{objects}"),
    )
    .expect("the page loads");
    let points: Vec<(String, Option<String>)> =
        check_page_policy(&page, Catalogue::shipped(), &policy)
            .into_iter()
            .map(|problem| (problem.object, problem.point))
            .collect();
    assert_eq!(
        points,
        [
            ("valve".to_string(), Some("Plant.Valve".to_string())),
            ("gain".to_string(), None)
        ]
    );
}
