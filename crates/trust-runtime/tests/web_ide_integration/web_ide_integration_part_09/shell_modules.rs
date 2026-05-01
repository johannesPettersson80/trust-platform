use super::super::*;

#[test]
fn unified_shell_entry_routes_redirect_to_ide() {
    let project = make_project("root-redirect");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    for path in ["", "/setup"] {
        let response = ureq::get(&format!("{base}{path}"))
            .config()
            .http_status_as_error(false)
            .max_redirects(0)
            .build()
            .call()
            .unwrap_or_else(|err| panic!("fetch {path} without redirect failed: {err}"));
        assert_eq!(
            response.status().as_u16(),
            302,
            "{path} must issue 302 redirect"
        );
        let location = response
            .headers()
            .get("location")
            .expect("redirect must have Location header");
        assert_eq!(location, "/ide", "{path} must redirect to /ide");
    }

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn unified_shell_tab_deep_links_serve_ide_html() {
    let project = make_project("tab-deep-links");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    for path in [
        "/ide",
        "/ide/code",
        "/ide/hardware",
        "/ide/settings",
        "/ide/logs",
    ] {
        let shell = ureq::get(&format!("{base}{path}"))
            .call()
            .unwrap_or_else(|_| panic!("fetch {path} failed"))
            .body_mut()
            .read_to_string()
            .unwrap_or_else(|_| panic!("read {path} failed"));
        assert!(
            shell.contains("id=\"ideTabNav\""),
            "{path} must contain tab navigation"
        );
        assert!(
            shell.contains("id=\"editorMount\""),
            "{path} must contain editor mount"
        );
    }

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn unified_shell_header_uses_compact_toolbar_with_overflow_menu() {
    let project = make_project("compact-toolbar");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let mut response = ureq::get(&format!("{base}/ide"))
        .call()
        .expect("fetch /ide");
    let body = response.body_mut().read_to_string().expect("read /ide");

    for id in [
        "id=\"openProjectBtn\"",
        "id=\"saveBtn\"",
        "id=\"buildBtn\"",
        "id=\"deployBtn\"",
        "id=\"moreActionsBtn\"",
        "id=\"moreActionsMenu\"",
        "id=\"quickOpenBtn\"",
        "id=\"cmdPaletteBtn\"",
        "id=\"saveAllBtn\"",
        "id=\"validateBtn\"",
        "id=\"testBtn\"",
    ] {
        assert!(body.contains(id), "toolbar html must contain {id}");
    }

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn unified_shell_serves_all_ide_tab_modules() {
    let project = make_project("tab-modules");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    for module in [
        "ide-tabs.js",
        "ide-hardware.js",
        "ide-online.js",
        "ide-debug.js",
        "ide-settings.js",
        "ide-logs.js",
    ] {
        let mut response = ureq::get(&format!("{base}/ide/modules/{module}"))
            .call()
            .unwrap_or_else(|err| panic!("fetch {module} failed: {err}"));
        let body = response
            .body_mut()
            .read_to_string()
            .unwrap_or_else(|_| panic!("read {module} failed"));
        assert!(
            body.len() > 100,
            "{module} must contain substantial content (got {} bytes)",
            body.len()
        );
    }

    let cytoscape = ureq::get(&format!("{base}/ide/modules/cytoscape.min.js"))
        .call()
        .expect("fetch cytoscape under IDE namespace")
        .body_mut()
        .read_to_string()
        .expect("read cytoscape");
    assert!(
        cytoscape.len() > 1000,
        "cytoscape.min.js must be a large library"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn unified_shell_online_module_defaults_connection_to_same_origin_and_auto_connects() {
    let project = make_project("online-default-connect");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let mut response = ureq::get(&format!("{base}/ide/modules/ide-online.js"))
        .call()
        .expect("fetch ide-online.js");
    let body = response
        .body_mut()
        .read_to_string()
        .expect("read ide-online.js");

    assert!(
        body.contains("function onlineDefaultConnectPort()"),
        "online module must expose derived default port helper"
    );
    assert!(
        body.contains("window.location.port"),
        "online module must derive connection defaults from current page origin"
    );
    assert!(
        body.contains("function onlineSeedConnectionDefaults()"),
        "online module must seed connection dialog defaults from current origin"
    );
    assert!(
        body.contains("if (!currentPort || currentPort === \"18080\")"),
        "online module must migrate stale legacy 18080 default to current origin port"
    );
    assert!(
        body.contains("void onlineConnect(withPort, null, { silent: true });"),
        "online module must auto-connect silently at startup in same-origin runtime mode"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn unified_shell_tab_module_enforces_tab_aria_contract() {
    let project = make_project("tab-aria-contract");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let mut response = ureq::get(&format!("{base}/ide/modules/ide-tabs.js"))
        .call()
        .expect("fetch ide-tabs.js");
    let body = response
        .body_mut()
        .read_to_string()
        .expect("read ide-tabs.js");

    assert!(
        body.contains("nav.setAttribute('role', 'tablist')"),
        "tab module must set tablist role"
    );
    assert!(
        body.contains("btn.setAttribute('role', 'tab')"),
        "tab module must set tab role on tab buttons"
    );
    assert!(
        body.contains("panel.setAttribute('role', 'tabpanel')"),
        "tab module must set tabpanel role on tab panels"
    );
    assert!(
        body.contains("btn.setAttribute('tabindex', isActive ? '0' : '-1')"),
        "tab module must keep keyboard tab order aligned with active tab"
    );
    assert!(
        body.contains("panel.classList.toggle('active', isActive)"),
        "tab module must keep active class on tab panels in sync with active tab"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn unified_shell_base_css_enforces_hidden_attribute_contract() {
    let project = make_project("base-css-hidden-contract");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let mut response = ureq::get(&format!("{base}/ide/base.css"))
        .call()
        .expect("fetch ide base.css");
    let body = response
        .body_mut()
        .read_to_string()
        .expect("read ide base.css");

    assert!(
        body.contains("[hidden] { display: none !important; }"),
        "base.css must enforce hidden attribute display contract"
    );

    let _ = std::fs::remove_dir_all(project);
}
