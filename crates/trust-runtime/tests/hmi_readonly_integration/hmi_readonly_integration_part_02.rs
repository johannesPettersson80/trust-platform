use super::*;

#[test]
fn hmi_standalone_export_bundle_contains_assets_routes_and_config() {
    let state = hmi_control_state(hmi_fixture_source());
    let base = start_test_server(state);

    let export = ureq::get(&format!("{base}/hmi/export.json"))
        .call()
        .expect("get /hmi/export.json")
        .body_mut()
        .read_to_string()
        .expect("read export body");
    let payload: serde_json::Value = serde_json::from_str(&export).expect("parse export body");

    assert_eq!(payload.get("version").and_then(|v| v.as_u64()), Some(2));
    assert_eq!(
        payload.get("entrypoint").and_then(|v| v.as_str()),
        Some("hmi/index.html")
    );
    assert!(payload
        .get("routes")
        .and_then(|v| v.as_array())
        .is_some_and(|routes| {
            routes.iter().any(|route| route.as_str() == Some("/hmi"))
                && routes
                    .iter()
                    .any(|route| route.as_str() == Some("/hmi/app.js"))
                && routes
                    .iter()
                    .any(|route| route.as_str() == Some("/hmi/modules/hmi-renderers.js"))
                && routes.iter().any(|route| route.as_str() == Some("/ws/hmi"))
        }));
    assert_eq!(
        payload
            .get("config")
            .and_then(|v| v.get("ws_route"))
            .and_then(|v| v.as_str()),
        Some("/ws/hmi")
    );
    assert!(payload
        .get("config")
        .and_then(|v| v.get("descriptor"))
        .is_some_and(serde_json::Value::is_null));
    assert!(payload
        .get("assets")
        .and_then(|v| v.as_object())
        .is_some_and(|assets| {
            assets.contains_key("hmi/index.html")
                && assets.contains_key("hmi/styles.css")
                && assets.contains_key("hmi/app.js")
                && assets.contains_key("hmi/modules/hmi-model-descriptor.js")
                && assets.contains_key("hmi/modules/hmi-model-layout.js")
                && assets.contains_key("hmi/modules/hmi-model-navigation.js")
                && assets.contains_key("hmi/modules/hmi-model.js")
                && assets.contains_key("hmi/modules/hmi-renderers.js")
                && assets.contains_key("hmi/modules/hmi-widgets.js")
                && assets.contains_key("hmi/modules/hmi-trends-alarms.js")
                && assets.contains_key("hmi/modules/hmi-process-view.js")
                && assets.contains_key("hmi/modules/hmi-transport.js")
                && assets.contains_key("hmi/modules/hmi-pages.js")
        }));
    let assets = payload
        .get("assets")
        .and_then(serde_json::Value::as_object)
        .expect("hmi assets object");
    let mut app_js = assets
        .get("hmi/app.js")
        .and_then(serde_json::Value::as_str)
        .expect("hmi app js")
        .to_string();
    for module_path in [
        "hmi/modules/hmi-model-descriptor.js",
        "hmi/modules/hmi-model-layout.js",
        "hmi/modules/hmi-model-navigation.js",
        "hmi/modules/hmi-model.js",
        "hmi/modules/hmi-renderers.js",
        "hmi/modules/hmi-widgets.js",
        "hmi/modules/hmi-trends-alarms.js",
        "hmi/modules/hmi-process-view.js",
        "hmi/modules/hmi-transport.js",
        "hmi/modules/hmi-pages.js",
    ] {
        let module_js = assets
            .get(module_path)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("missing exported {module_path}"));
        app_js.push('\n');
        app_js.push_str(module_js);
    }
    assert!(
        app_js.contains("function renderProcessPage")
            && app_js.contains("createGaugeRenderer")
            && app_js.contains("kind === 'sparkline'")
            && app_js.contains("function widgetWritePolicy")
            && app_js.contains("This value can be watched but not changed from HMI."),
        "exported hmi bundle should include process-page and rich-widget renderers"
    );
    let styles = assets
        .get("hmi/styles.css")
        .and_then(serde_json::Value::as_str)
        .expect("hmi styles css");
    assert!(
        styles.contains(".widget-policy-badge") && styles.contains(".card-policy-note"),
        "exported hmi styles should make read-only widget policy visible"
    );
    assert!(payload
        .get("config")
        .and_then(|v| v.get("schema"))
        .and_then(|v| v.get("widgets"))
        .and_then(|v| v.as_array())
        .is_some_and(|widgets| !widgets.is_empty()));
}

#[test]
fn hmi_standalone_export_bundle_includes_resolved_descriptor_when_hmi_dir_present() {
    let root = temp_dir("export-descriptor");
    write_file(
        &root.join("hmi/_config.toml"),
        r##"
[theme]
style = "industrial"
accent = "#ff6b00"

[write]
enabled = true
allow = ["Main.run"]
"##,
    );
    write_file(
        &root.join("hmi/overview.toml"),
        r##"
title = "Overview"
kind = "dashboard"
order = 0

[[section]]
title = "Drive"
span = 8

[[section.widget]]
type = "gauge"
bind = "Main.speed"
label = "Speed"
min = 0
max = 100
"##,
    );

    let state = hmi_control_state_with_root(hmi_fixture_source(), Some(root.clone()));
    let base = start_test_server(state);

    let export = ureq::get(&format!("{base}/hmi/export.json"))
        .call()
        .expect("get /hmi/export.json")
        .body_mut()
        .read_to_string()
        .expect("read export body");
    let payload: serde_json::Value = serde_json::from_str(&export).expect("parse export body");

    assert_eq!(payload.get("version").and_then(|v| v.as_u64()), Some(2));
    let descriptor = payload
        .get("config")
        .and_then(|v| v.get("descriptor"))
        .expect("descriptor field");
    assert!(descriptor.is_object(), "descriptor should be object");

    assert_eq!(
        descriptor
            .get("config")
            .and_then(|v| v.get("theme"))
            .and_then(|v| v.get("style"))
            .and_then(|v| v.as_str()),
        Some("industrial")
    );
    assert!(descriptor
        .get("config")
        .and_then(|v| v.get("write"))
        .and_then(|v| v.get("allow"))
        .and_then(|v| v.as_array())
        .is_some_and(|allow| allow.iter().any(|entry| entry.as_str() == Some("Main.run"))));
    assert!(descriptor
        .get("pages")
        .and_then(|v| v.as_array())
        .is_some_and(|pages| pages.iter().any(|page| {
            page.get("id").and_then(|v| v.as_str()) == Some("overview")
                && page.get("kind").and_then(|v| v.as_str()) == Some("dashboard")
                && page
                    .get("sections")
                    .and_then(|v| v.as_array())
                    .is_some_and(|sections| {
                        sections.iter().any(|section| {
                            section.get("title").and_then(|v| v.as_str()) == Some("Drive")
                        })
                    })
        })));

    fs::remove_dir_all(root).ok();
}
