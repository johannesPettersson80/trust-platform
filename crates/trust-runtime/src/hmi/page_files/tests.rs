//! Specification 39, §5.1 and §15: loading and validating page files.

use super::fixtures::{FILE, SPEC_EXAMPLE};
use super::*;

fn errors(text: &str) -> Vec<PageFileError> {
    parse_page(FILE, text).expect_err("the page should be rejected")
}

fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .expect("the needle is in the text")
        + 1
}

#[test]
fn a_trend_pen_keeps_its_own_fixed_range() {
    let text = "format = 1\ntitle = \"Trend\"\n\n[[object]]\nid = \"trend\"\ntype = \"trend\"\nat = [0, 0]\nsize = [480, 240]\n\n[object.bind]\nTemperature = { path = \"Main.Temperature\", range = [-20, 120] }\n";
    let page = parse_page(FILE, text).expect("a numeric trend pen accepts its own range");
    let model = serde_json::to_value(&page).unwrap();
    assert_eq!(
        model["object"][0]["bind"]["Temperature"]["range"],
        serde_json::json!([-20.0, 120.0])
    );
    let written = write_page(&page);
    assert!(written.contains("range = [-20, 120]"), "{written}");
    assert_eq!(parse_page(FILE, &written).unwrap(), page);
}

#[test]
fn a_trend_pen_refuses_a_nonfinite_or_reversed_range() {
    for range in ["[10, 0]", "[2, 2]", "[0, inf]", "[nan, 10]"] {
        let text = format!("format = 1\ntitle = \"Trend\"\n\n[[object]]\nid = \"trend\"\ntype = \"trend\"\nat = [0, 0]\nsize = [480, 240]\n\n[object.bind]\nTemperature = {{ path = \"Main.Temperature\", range = {range} }}\n");
        let problems = parse_page(FILE, &text).unwrap_err();
        assert!(
            problems.iter().any(|error| error
                .message
                .contains("range must have finite, increasing bounds")),
            "{range}: {problems:?}"
        );
    }
}

#[test]
fn spec_example_page_loads_with_objects_bindings_click_and_pipe() {
    let page = parse_page(FILE, SPEC_EXAMPLE).expect("the specification example loads");
    assert_eq!(page.title, "Overview");
    assert_eq!(page.objects.len(), 2);

    let tank = &page.objects[0];
    assert_eq!(tank.id, "tank-1");
    assert_eq!(tank.tag.as_deref(), Some("TK101"));
    assert_eq!(tank.kind, "tank");
    assert_eq!(
        (tank.at, tank.size, tank.rotation),
        ([400, 300], [160, 260], 0)
    );
    assert_eq!(
        tank.bind["level"],
        HmiBinding {
            quality: None,
            path: "FeedArea.TK101.Level".to_string(),
            unit: Some("%".to_string()),
            range: None,
            direction: HmiBindingDirection::Read,
        }
    );

    let pump = &page.objects[1];
    assert_eq!(pump.variant.as_deref(), Some("horizontal"));
    assert_eq!(
        pump.bind.keys().map(String::as_str).collect::<Vec<_>>(),
        ["fault", "running", "speed"]
    );
    assert_eq!(pump.bind["running"].path, "FeedArea.P101.Running");
    assert_eq!(pump.bind["running"].unit, None);
    assert_eq!(pump.bind["speed"].unit.as_deref(), Some("rpm"));
    assert_eq!(
        pump.click,
        Some(HmiClick::Faceplate {
            name: "pump".to_string(),
            instance: None
        })
    );

    assert_eq!(
        page.pipes,
        vec![HmiPipe {
            from: "tank-1.outlet".to_string(),
            to: "pump-1.inlet".to_string(),
            style: HmiPipeStyle::Process,
            waypoints: Vec::new(),
            bind: std::collections::BTreeMap::new(),
        }]
    );
}

#[test]
fn unknown_key_is_reported_with_file_line_and_key() {
    let text = SPEC_EXAMPLE.replace(
        "variant = \"horizontal\"",
        "variant = \"horizontal\"\ncolour = \"red\"",
    );
    let errors = errors(&text);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].file, FILE);
    assert_eq!(errors[0].line, line_of(&text, "colour"));
    assert!(
        errors[0].message.contains("colour"),
        "{}",
        errors[0].message
    );
}

#[test]
fn missing_format_is_reported() {
    let text = SPEC_EXAMPLE.replacen("format = 1\n", "", 1);
    assert_eq!(
        errors(&text),
        vec![PageFileError {
            file: FILE.to_string(),
            line: 1,
            message: "the file has no `format`; add `format = 1` at the top".to_string(),
        }]
    );
}

#[test]
fn unsupported_format_is_reported_on_its_line() {
    let text = SPEC_EXAMPLE.replacen("format = 1", "\nformat = 2", 1);
    assert_eq!(
        errors(&text),
        vec![PageFileError {
            file: FILE.to_string(),
            line: 2,
            message: "format 2 is not supported; this truST reads format 1".to_string(),
        }]
    );
}

#[test]
fn duplicate_object_id_is_reported_on_the_second_object() {
    let text = SPEC_EXAMPLE.replace("id = \"pump-1\"", "id = \"tank-1\"");
    let second = text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("id = \"tank-1\""))
        .nth(1)
        .expect("two objects use the id")
        .0
        + 1;
    assert_eq!(
        errors(&text),
        vec![PageFileError {
            file: FILE.to_string(),
            line: second,
            message: "object id \"tank-1\" is used twice on this page".to_string(),
        }]
    );
}

#[test]
fn invalid_value_is_reported_on_its_line() {
    let text = SPEC_EXAMPLE.replace("at = [640, 480]", "at = \"left\"");
    let errors = errors(&text);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].line, line_of(&text, "at = \"left\""));
}

#[test]
fn click_with_two_targets_is_reported_within_the_click_table() {
    let text = SPEC_EXAMPLE.replace(
        "faceplate = \"pump\"",
        "faceplate = \"pump\"\npage = \"overview\"",
    );
    let errors = errors(&text);
    assert_eq!(errors.len(), 1, "{errors:?}");
    let header = line_of(&text, "[object.click]");
    assert!(
        (header..=header + 2).contains(&errors[0].line),
        "line {} is not in the click table starting at {header}",
        errors[0].line
    );
    assert!(
        errors[0].message.contains("exactly one"),
        "{}",
        errors[0].message
    );
}

/// Specification 39, §15.2: a display range is a finite number. Values such as `nan` or `inf`
/// cannot travel to the editor and are refused when the file is read.
#[test]
fn non_finite_display_range_is_reported_on_its_line() {
    for (key, value) in [("min", "nan"), ("max", "inf"), ("max", "-inf")] {
        let text = SPEC_EXAMPLE.replace(
            "size = [160, 260]",
            &format!("size = [160, 260]\n{key} = {value}"),
        );
        assert_eq!(
            errors(&text),
            vec![PageFileError {
                file: FILE.to_string(),
                line: line_of(&text, &format!("{key} = {value}")),
                message: format!("{key} must be a finite number"),
            }],
            "{key} = {value}"
        );
    }
}

/// Specification 39, §5.1: an image object names a project asset.
#[test]
fn an_image_object_keeps_its_asset_path() {
    let text = "format = 1\ntitle = \"Logo\"\n\n[[object]]\nid = \"logo\"\ntype = \"image\"\nat = [0, 0]\nsize = [160, 80]\nimage = \"hmi/assets/logo.svg\"\n";
    let page = parse_page(FILE, text).expect("the page loads");
    assert_eq!(
        page.objects[0].image.as_deref(),
        Some("hmi/assets/logo.svg")
    );
    assert_eq!(write_page(&page), text);
}

/// Specification 39, §5.1: an image cannot point outside the project's assets.
#[test]
fn an_image_outside_the_project_assets_is_reported_on_its_line() {
    for path in [
        "/etc/passwd",
        "hmi/assets/../secret.svg",
        "../logo.svg",
        "C:\\\\logo.png",
        "hmi\\\\assets\\\\x.png",
        "hmi/pages/overview.toml",
        "hmi/assets/",
    ] {
        let text = format!("format = 1\ntitle = \"Logo\"\n\n[[object]]\nid = \"logo\"\ntype = \"image\"\nat = [0, 0]\nsize = [160, 80]\nimage = \"{path}\"\n");
        let errors = errors(&text);
        assert_eq!(errors.len(), 1, "{path}: {errors:?}");
        assert_eq!(errors[0].line, 9, "{path}");
        assert!(
            errors[0].message.contains("hmi/assets/"),
            "{path}: {}",
            errors[0].message
        );
    }
}

/// Specification 39, §5.1: a trend keeps the time it shows, written after its range.
#[test]
fn a_trend_keeps_its_span() {
    let text = "format = 1\ntitle = \"Trends\"\n\n[[object]]\nid = \"loop\"\ntype = \"trend\"\nat = [0, 0]\nsize = [480, 240]\nmin = 0\nmax = 100\nspan = 600\n";
    let page = parse_page(FILE, text).expect("the page loads");
    assert_eq!(page.objects[0].span, Some(600));
    assert_eq!(write_page(&page), text);
}

/// Specification 39, §5.1: a span is at least one second.
#[test]
fn a_span_of_zero_is_reported_on_its_line() {
    let text = "format = 1\ntitle = \"Trends\"\n\n[[object]]\nid = \"loop\"\ntype = \"trend\"\nat = [0, 0]\nsize = [480, 240]\nspan = 0\n";
    let errors = errors(text);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].line, 9);
    assert!(errors[0].message.contains("span"), "{}", errors[0].message);
}
