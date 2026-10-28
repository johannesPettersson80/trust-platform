//! The write rules a page needs and their place in `runtime.toml` (specification 39, §12.2 item 3,
//! §12.3, §12.8).

use serde_json::json;

use super::{add, answer, page_rules, place, RuleRole, WriteRule};
use trust_runtime::hmi::page_files::parse_page;

/// A page with these objects (each a TOML `[[object]]` table).
fn objects(tables: &str) -> Vec<trust_runtime::hmi::page_files::HmiObject> {
    parse_page(
        "hmi/pages/test.toml",
        &format!("format = 1\ntitle = \"Test\"\n\n{tables}"),
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"))
    .objects
}

fn rule(path: &str, role: RuleRole, min: Option<f64>, max: Option<f64>) -> WriteRule {
    WriteRule {
        path: path.to_owned(),
        role,
        min,
        max,
    }
}

const SETPOINT: &str =
    "[[object]]\nid = \"setpoint-1\"\ntype = \"setpoint\"\nat = [0, 0]\nsize = [240, 96]\n";

#[test]
fn a_control_without_a_role_needs_an_engineer_rule_with_its_limits() {
    // §12.3: a control that names no role is an Engineer's; a rule for Operators would widen what
    // the page asked for.
    let page = objects(&format!(
        "{SETPOINT}min = 0.5\nmax = 100\n\n[object.bind]\nvalue = {{ path = \"Main.FanSpeed\", direction = \"read-write\" }}\n"
    ));
    assert_eq!(
        page_rules(&page, None),
        vec![rule(
            "Main.FanSpeed",
            RuleRole::Engineer,
            Some(0.5),
            Some(100.0)
        )]
    );
}

#[test]
fn several_controls_on_one_point_give_the_lowest_role_and_the_widest_limits() {
    // §12.2 item 3: a narrower rule would refuse one of the controls; a limit is kept only when
    // every control has one, else the rule would bound a control that asked for none.
    let page = objects(&format!(
        "{SETPOINT}role = \"operator\"\nmin = 10\nmax = 50\n\n[object.bind]\nvalue = {{ path = \"Main.Speed\", direction = \"write\" }}\n\n\
         [[object]]\nid = \"setpoint-2\"\ntype = \"setpoint\"\nat = [0, 100]\nsize = [240, 96]\nmin = 0\nmax = 80\n\n[object.bind]\nvalue = {{ path = \"main.SPEED\", direction = \"read-write\" }}\n\n\
         [[object]]\nid = \"setpoint-3\"\ntype = \"setpoint\"\nat = [0, 200]\nsize = [240, 96]\nmin = 5\n\n[object.bind]\nvalue = {{ path = \"Main.Speed\", direction = \"write\" }}\n"
    ));
    assert_eq!(
        page_rules(&page, None),
        vec![rule("Main.Speed", RuleRole::Operator, Some(0.0), None)]
    );
}

#[test]
fn reading_bindings_need_no_rule_and_a_named_point_is_the_only_one() {
    let page = objects(&format!(
        "{SETPOINT}\n[object.bind]\nvalue = {{ path = \"Main.A\", direction = \"write\" }}\n\n\
         [[object]]\nid = \"value-1\"\ntype = \"value\"\nat = [0, 100]\nsize = [240, 96]\n\n[object.bind]\nvalue = \"Main.B\"\n\n\
         [[object]]\nid = \"setpoint-2\"\ntype = \"setpoint\"\nat = [0, 200]\nsize = [240, 96]\n\n[object.bind]\nvalue = {{ path = \"Main.C\", direction = \"write\" }}\n"
    ));
    assert_eq!(
        page_rules(&page, None),
        vec![
            rule("Main.A", RuleRole::Engineer, None, None),
            rule("Main.C", RuleRole::Engineer, None, None),
        ],
        "Main.B is only read"
    );
    assert_eq!(
        page_rules(&page, Some("main.c")),
        vec![rule("Main.C", RuleRole::Engineer, None, None)],
        "the point compares without its case"
    );
    assert_eq!(
        page_rules(&page, Some("Main.Elsewhere")),
        vec![rule("Main.Elsewhere", RuleRole::Engineer, None, None)],
        "a point no control writes gets an Engineer rule"
    );
}

#[test]
fn a_point_runtime_toml_names_already_is_left_and_each_point_is_added_once() {
    let text = "[runtime.web.hmi]\nidle_minutes = 5\n\n[[runtime.web.hmi.write]]\npath = \"Main.Level\"\nrole = \"engineer\"\n";
    let (next, added) = add(
        text,
        &[
            rule("MAIN.LEVEL", RuleRole::Operator, None, None),
            rule("Main.FanSpeed", RuleRole::Engineer, Some(0.5), Some(100.0)),
            rule("main.fanspeed", RuleRole::Operator, None, None),
        ],
    )
    .unwrap();
    assert_eq!(added, vec!["Main.FanSpeed".to_owned()]);
    assert_eq!(
        next,
        format!(
            "{text}\n[[runtime.web.hmi.write]]\npath = \"Main.FanSpeed\"\nrole = \"engineer\"\nmin = 0.5\nmax = 100.0\n"
        ),
        "a whole number is written as a TOML float, as the runtime reads limits"
    );
    let (same, none) = add(&next, &[rule("main.level", RuleRole::Engineer, None, None)]).unwrap();
    assert_eq!((same.as_str(), none.len()), (next.as_str(), 0));
}

#[test]
fn the_page_file_names_its_project_and_kind() {
    assert_eq!(
        place("/home/anna/plant/hmi/pages/overview.toml"),
        Some(("/home/anna/plant".to_owned(), "page"))
    );
    assert_eq!(
        place("/c:/plant/hmi/popups/pump.toml"),
        Some(("/c:/plant".to_owned(), "popup"))
    );
    assert_eq!(
        place("C:\\plant\\hmi\\panels\\pump.toml"),
        Some(("C:/plant".to_owned(), "panel"))
    );
    assert_eq!(place("/home/anna/plant/hmi/screen.toml"), None);
    assert_eq!(place("hmi/pages/overview.toml"), None, "no project folder");
}

#[test]
fn the_command_answers_the_rules_of_a_page_and_the_new_file() {
    let page = format!(
        "format = 1\ntitle = \"Test\"\n\n{SETPOINT}role = \"operator\"\n\n[object.bind]\nvalue = {{ path = \"Main.A\", direction = \"write\" }}\n"
    );
    assert_eq!(
        answer(json!({ "action": "forPage", "file": "/p/hmi/pages/test.toml", "text": page }))
            .unwrap(),
        json!({ "ok": true, "root": "/p", "rules": [{ "path": "Main.A", "role": "operator" }] })
    );
    assert_eq!(
        answer(json!({ "action": "forPage", "file": "/p/src/test.toml", "text": "" })).unwrap_err(),
        "The page is not in a project's hmi folder."
    );
    assert_eq!(
        answer(json!({
            "action": "add",
            "runtimeToml": "",
            "rules": [{ "path": "Main.A", "role": "operator", "max": 2.5 }]
        }))
        .unwrap(),
        json!({
            "ok": true,
            "text": "\n[[runtime.web.hmi.write]]\npath = \"Main.A\"\nrole = \"operator\"\nmax = 2.5\n",
            "added": ["Main.A"]
        })
    );
}
