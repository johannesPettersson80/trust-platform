use super::*;

#[test]
fn hmi_dashboard_routes_render_without_manual_layout() {
    let state = hmi_control_state(hmi_fixture_source());
    let base = start_test_server(state);

    let hmi_html = ureq::get(&format!("{base}/hmi"))
        .call()
        .expect("get /hmi")
        .body_mut()
        .read_to_string()
        .expect("read /hmi body");
    assert!(hmi_html.contains("truST HMI"));
    assert!(hmi_html.contains("id=\"hmiGroups\""));
    assert!(hmi_html.contains("id=\"pageSidebar\""));
    assert!(hmi_html.contains("class=\"pill status-chip\""));
    assert!(hmi_html.contains("id=\"connectorSummaryState\""));
    assert!(!hmi_html.contains("id=\"connectorProofState\""));
    assert!(hmi_html.contains("class=\"action-link hmi-action\""));
    assert!(
        hmi_html
            .find("id=\"themeLabel\"")
            .is_some_and(|theme_index| {
                hmi_html
                    .find("class=\"actions\"")
                    .is_some_and(|actions_index| actions_index < theme_index)
            }),
        "theme toggle must be rendered with actions, not passive status chips"
    );

    let hmi_js = load_hmi_runtime_script_bundle(&base);
    assert!(hmi_js.contains("hmi.schema.get"));
    assert!(hmi_js.contains("hmi.values.get"));
    assert!(hmi_js.contains("hmi.trends.get"));
    assert!(hmi_js.contains("hmi.alarms.get"));
    assert!(hmi_js.contains("hmi.alarm.ack"));
    assert!(hmi_js.contains("connectors.status"));
    assert!(hmi_js.contains("updateConnectorStatusSummary"));
    assert!(hmi_js.contains("connectWebSocketTransport"));
    assert!(hmi_js.contains("/ws/hmi"));
    assert!(hmi_js.contains("hmi.values.delta"));
    assert!(hmi_js.contains("hmi.schema.revision"));
    assert!(hmi_js.contains("renderProcessPage"));
    assert!(hmi_js.contains("/hmi/assets/"));
    assert!(hmi_js.contains("section-grid"));
    assert!(hmi_js.contains("section-widget-grid"));
    assert!(hmi_js.contains("createGaugeRenderer"));
    assert!(hmi_js.contains("createSparklineRenderer"));
    assert!(hmi_js.contains("createBarRenderer"));
    assert!(hmi_js.contains("createTankRenderer"));
    assert!(hmi_js.contains("createIndicatorRenderer"));
    assert!(hmi_js.contains("createToggleRenderer"));
    assert!(hmi_js.contains("createSliderRenderer"));

    let hmi_css = ureq::get(&format!("{base}/hmi/styles.css"))
        .call()
        .expect("get /hmi/styles.css")
        .body_mut()
        .read_to_string()
        .expect("read /hmi/styles.css body");
    assert!(hmi_css.contains(".card"));
    assert!(hmi_css.contains(".section-grid"));
    assert!(hmi_css.contains(".hmi-section"));
    assert!(hmi_css.contains(".section-widget-grid"));
    assert!(hmi_css.contains(".widget-gauge"));
    assert!(hmi_css.contains(".widget-sparkline"));
    assert!(hmi_css.contains(".widget-bar"));
    assert!(hmi_css.contains(".widget-tank"));
    assert!(hmi_css.contains(".widget-indicator"));
    assert!(hmi_css.contains(".widget-toggle-control"));
    assert!(hmi_css.contains(".widget-slider-control"));
    assert!(hmi_css.contains("viewport-kiosk"));
    assert!(hmi_css.contains("prefers-color-scheme: dark"));
    assert!(hmi_css.contains("@media (max-width: 680px)"));
    assert!(hmi_css.contains("@media (max-width: 1024px)"));
    assert!(hmi_css.contains(".status-chip"));
    assert!(hmi_css.contains(".connector-chip"));
    assert!(hmi_css.contains("cursor: default"));
    assert!(hmi_css.contains(".action-link:focus-visible"));
    assert!(hmi_css.contains("box-shadow: var(--shadow-sm)"));

    let schema = post_control(&base, "hmi.schema.get", None);
    assert_eq!(schema.get("ok").and_then(|v| v.as_bool()), Some(true));
    let widgets = schema
        .get("result")
        .and_then(|v| v.get("widgets"))
        .and_then(|v| v.as_array())
        .expect("schema widgets");
    assert!(
        !widgets.is_empty(),
        "schema should return discovered widgets"
    );
    assert!(schema
        .get("result")
        .and_then(|v| v.get("theme"))
        .and_then(|v| v.get("style"))
        .and_then(|v| v.as_str())
        .is_some());
    assert!(schema
        .get("result")
        .and_then(|v| v.get("pages"))
        .and_then(|v| v.as_array())
        .is_some());
    assert_eq!(
        schema
            .get("result")
            .and_then(|v| v.get("responsive"))
            .and_then(|v| v.get("mode"))
            .and_then(|v| v.as_str()),
        Some("auto")
    );
    assert_eq!(
        schema
            .get("result")
            .and_then(|v| v.get("export"))
            .and_then(|v| v.get("enabled"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert!(schema
        .get("result")
        .and_then(|v| v.get("pages"))
        .and_then(|v| v.as_array())
        .is_some_and(|pages| pages.iter().any(|page| {
            page.get("kind")
                .and_then(|v| v.as_str())
                .is_some_and(|kind| kind == "trend" || kind == "alarm")
        })));
    let ids = widgets
        .iter()
        .filter_map(|widget| widget.get("id").and_then(|v| v.as_str()))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let values = post_control(&base, "hmi.values.get", Some(json!({ "ids": ids })));
    assert_eq!(values.get("ok").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        values
            .get("result")
            .and_then(|v| v.get("connected"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );

    let trends = post_control(
        &base,
        "hmi.trends.get",
        Some(json!({ "duration_ms": 60_000, "buckets": 32 })),
    );
    assert_eq!(trends.get("ok").and_then(|v| v.as_bool()), Some(true));
    assert!(trends
        .get("result")
        .and_then(|v| v.get("series"))
        .and_then(|v| v.as_array())
        .is_some_and(|series| !series.is_empty()));

    let alarms = post_control(&base, "hmi.alarms.get", Some(json!({ "limit": 10 })));
    assert_eq!(alarms.get("ok").and_then(|v| v.as_bool()), Some(true));
}

#[test]
fn hmi_schema_exposes_section_spans_and_widget_spans_for_web_layout() {
    let root = temp_dir("section-layout");
    write_file(
        &root.join("hmi/overview.toml"),
        r##"
title = "Overview"
kind = "dashboard"

[[section]]
title = "Drive Controls"
span = 8

[[section.widget]]
type = "gauge"
bind = "Main.speed"
label = "Speed"
span = 6
min = 0
max = 100

[[section.widget]]
type = "indicator"
bind = "Main.run"
label = "Running"
span = 3
on_color = "#22c55e"
off_color = "#94a3b8"
"##,
    );

    let state = hmi_control_state_with_root(hmi_fixture_source(), Some(root.clone()));
    let base = start_test_server(state);
    let schema = post_control(&base, "hmi.schema.get", None);
    assert_eq!(schema.get("ok").and_then(|v| v.as_bool()), Some(true));

    let result = schema.get("result").expect("schema result");
    let overview = result
        .get("pages")
        .and_then(|v| v.as_array())
        .and_then(|pages| {
            pages
                .iter()
                .find(|page| page.get("id").and_then(|v| v.as_str()) == Some("overview"))
        })
        .expect("overview page");
    let first_section = overview
        .get("sections")
        .and_then(|v| v.as_array())
        .and_then(|sections| sections.first())
        .expect("overview section");
    assert_eq!(
        first_section.get("title").and_then(|v| v.as_str()),
        Some("Drive Controls")
    );
    assert_eq!(first_section.get("span").and_then(|v| v.as_u64()), Some(8));
    assert!(first_section
        .get("widget_ids")
        .and_then(|v| v.as_array())
        .is_some_and(|ids| ids.len() == 2));

    let speed_widget = result
        .get("widgets")
        .and_then(|v| v.as_array())
        .and_then(|widgets| {
            widgets
                .iter()
                .find(|widget| widget.get("path").and_then(|v| v.as_str()) == Some("Main.speed"))
        })
        .expect("speed widget");
    assert_eq!(
        speed_widget.get("section_title").and_then(|v| v.as_str()),
        Some("Drive Controls")
    );
    assert_eq!(
        speed_widget.get("widget_span").and_then(|v| v.as_u64()),
        Some(6)
    );

    fs::remove_dir_all(root).ok();
}
