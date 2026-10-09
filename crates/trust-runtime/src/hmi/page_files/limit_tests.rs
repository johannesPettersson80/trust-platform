//! Specification 39 §5.9 item 3: a display's fallback limits, without an equipment block.
use super::{parse_page, write_page};

fn page(limits: &str) -> String {
    format!("format = 1\ntitle = \"Level\"\n\n[[object]]\nid = \"level\"\ntype = \"gauge\"\nat = [0, 0]\nsize = [160, 160]\nlimits = {{ {limits} }}\n")
}

#[test]
fn a_display_keeps_its_ordered_manual_alarm_limits() {
    let source = page("low-low = 5, low = 10, high = 90, high-high = 95");
    let model = parse_page("hmi/pages/level.toml", &source).expect("manual display limits load");
    let json = serde_json::to_value(&model).unwrap();
    assert_eq!(
        json["object"][0]["limits"],
        serde_json::json!({"low-low": 5.0, "low": 10.0, "high": 90.0, "high-high": 95.0})
    );
    let written = write_page(&model);
    assert!(
        written.contains("limits = { low-low = 5, low = 10, high = 90, high-high = 95 }"),
        "{written}"
    );
    assert_eq!(parse_page("hmi/pages/level.toml", &written).unwrap(), model);
}

#[test]
fn display_alarm_limits_refuse_nonfinite_or_reversed_values() {
    for limits in [
        "low = nan",
        "high = inf",
        "low = 90, high = 10",
        "low-low = 20, low = 10",
        "high = 90, high-high = 80",
    ] {
        let errors =
            parse_page("hmi/pages/level.toml", &page(limits)).expect_err("bad limits are refused");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("limits must be finite and ordered")),
            "{limits}: {errors:?}"
        );
    }
}
