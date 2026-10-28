use std::time::Instant;

fn search_request(id: u64, kind: &str, params: serde_json::Value) -> serde_json::Value {
    json!({ "id": id, "type": kind, "params": params })
}

#[test]
fn search_requests_answer_through_the_control_connection() {
    let state = control_test_state("PROGRAM Main END_PROGRAM");
    // An EtherCAT search without a confirmed fieldbus scan is skipped, so it ends at once.
    let started = handle_request_value(
        search_request(1, "comm.search.start", json!({ "only": ["ethercat"] })),
        &state,
        None,
    );
    assert!(started.ok, "start: {:?}", started.error);
    assert_eq!(started.result, Some(json!({ "search_id": "s-1" })));

    let status = handle_request_value(
        search_request(2, "comm.search.status", json!({ "search_id": "s-1" })),
        &state,
        None,
    );
    assert!(status.ok, "status: {:?}", status.error);
    let result = status.result.expect("status result");
    assert_eq!(result["schema_version"], json!(1));
    assert_eq!(result["origin"], json!("runtime"));
    assert_eq!(result["state"], json!("done"));
    assert_eq!(result["deadline_ms"], json!(10_000));
    assert_eq!(
        result["sources"],
        json!([{
            "id": "ethercat",
            "state": "skipped",
            "reason": "An EtherCAT scan runs only when the search confirms it (confirm_fieldbus_scan)."
        }])
    );
    assert_eq!(result["devices"], json!([]));
    assert!(result.get("merged").is_none());

    let cancelled = handle_request_value(
        search_request(3, "comm.search.cancel", json!({ "search_id": "s-1" })),
        &state,
        None,
    );
    assert_eq!(
        cancelled.result,
        Some(json!({ "search_id": "s-1", "state": "done" }))
    );

    let missing = handle_request_value(
        search_request(4, "comm.search.status", json!({ "search_id": "s-99" })),
        &state,
        None,
    );
    assert!(!missing.ok);
    assert_eq!(missing.error_code.as_deref(), Some("search_not_found"));

    for (id, params) in [
        (5, json!({ "origin": "this_host" })),
        (6, json!({ "deadline_ms": 60_001 })),
        (7, json!({ "only": ["usb"], "scope": {} })),
    ] {
        let refused = handle_request_value(
            search_request(id, "comm.search.start", params.clone()),
            &state,
            None,
        );
        assert!(!refused.ok, "params={params}");
        assert!(
            refused
                .error
                .as_deref()
                .is_some_and(|error| error.starts_with("invalid comm.search.start payload:")),
            "params={params}: {:?}",
            refused.error
        );
    }
    let next = handle_request_value(
        search_request(8, "comm.search.start", json!({ "only": ["ethercat"] })),
        &state,
        None,
    );
    assert_eq!(
        next.result,
        Some(json!({ "search_id": "s-2" })),
        "a refused request starts nothing"
    );
}

#[test]
fn confirmed_fieldbus_search_needs_the_engineer_role() {
    let mut state = control_test_state("PROGRAM Main END_PROGRAM");
    state.auth_token = Arc::new(Mutex::new(Some(SmolStr::new("admin-token"))));
    state.control_requires_auth = true;
    let path = pairing_file("search-roles");
    let store = Arc::new(PairingStore::load(path.clone()));
    state.pairing = Some(store.clone());
    let code = store.start_pairing();
    let viewer = store
        .claim(&code.code, Some(AccessRole::Viewer))
        .expect("viewer token");

    let confirmed = json!({
        "id": 1,
        "type": "comm.search.start",
        "auth": viewer,
        "params": { "only": ["ethercat"], "confirm_fieldbus_scan": true }
    });
    let refused = handle_request_value(confirmed, &state, None);
    assert!(!refused.ok);
    assert_eq!(refused.error_code.as_deref(), Some("insufficient_role"));

    let unconfirmed = json!({
        "id": 2,
        "type": "comm.search.start",
        "auth": viewer,
        "params": { "only": ["ethercat"] }
    });
    let started = handle_request_value(unconfirmed, &state, None);
    assert!(started.ok, "viewer search: {:?}", started.error);
    assert_eq!(
        started.result,
        Some(json!({ "search_id": "s-1" })),
        "the refused fieldbus scan started nothing"
    );

    let confirmed = json!({
        "id": 3,
        "type": "comm.search.start",
        "auth": "admin-token",
        "params": { "only": ["ethercat"], "confirm_fieldbus_scan": true }
    });
    assert!(handle_request_value(confirmed, &state, None).ok);
    let _ = fs::remove_file(path);
}

/// A project folder whose `io.toml` has an EtherCAT driver on the simulated bus with the modules of the
/// simulator's chain, the EL2008 at position 3 where the simulated bus has it at position 2.
fn compare_project(name: &str) -> PathBuf {
    let folder = temp_dir(name);
    write_file(
        &folder.join("runtime.toml"),
        &crate::bundle_template::render_runtime_toml(&SmolStr::new("cell"), 100),
    );
    write_file(
        &folder.join("io.toml"),
        "[[io.drivers]]\nname = \"ethercat\"\nparams = { adapter = \"mock\", modules = [{ model = \"EK1100\", slot = 0 }, { model = \"EL1008\", slot = 1 }, { model = \"EL2008\", slot = 3 }] }\n",
    );
    folder
}

/// Waits until the search `search_id` of the runtime has ended.
fn wait_until_search_done(state: &ControlState, auth: Option<&str>, search_id: &str) {
    let give_up = Instant::now() + Duration::from_secs(20);
    loop {
        let status = handle_request_value(
            json!({
                "id": 90, "type": "comm.search.status", "auth": auth,
                "params": { "search_id": search_id }
            }),
            state,
            None,
        );
        if status
            .result
            .as_ref()
            .is_some_and(|result| result["state"] == json!("done"))
        {
            return;
        }
        assert!(Instant::now() < give_up, "the search did not end in time");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn compare_needs_the_viewer_role() {
    // Specification 11, "Compare with the project" and the role table (§6.9.5): `comm.compare` is a Viewer
    // operation. An admin runs a confirmed EtherCAT scan of the simulated bus, which is an Engineer
    // operation, and a viewer compares that search with the project's files: the project has the EL2008 at
    // position 3 and the bus at position 2. A wrong implementation leaves `comm.compare` out of the
    // registry, where the Admin fail-safe refuses the viewer, or sets a higher role. The search named, and
    // the latest one, give the same answer.
    let mut state = control_test_state("PROGRAM Main END_PROGRAM");
    state.auth_token = Arc::new(Mutex::new(Some(SmolStr::new("admin-token"))));
    state.control_requires_auth = true;
    let folder = compare_project("compare-viewer");
    state.project_root = Some(folder.clone());
    let path = pairing_file("compare-roles");
    let store = Arc::new(PairingStore::load(path.clone()));
    state.pairing = Some(store.clone());
    let code = store.start_pairing();
    let viewer = store
        .claim(&code.code, Some(AccessRole::Viewer))
        .expect("viewer token");

    let started = handle_request_value(
        json!({
            "id": 1, "type": "comm.search.start", "auth": "admin-token",
            "params": {
                "only": ["ethercat"], "confirm_fieldbus_scan": true,
                "fieldbus": [{ "bus": "ethercat", "adapter": "mock" }]
            }
        }),
        &state,
        None,
    );
    assert_eq!(
        started.result,
        Some(json!({ "search_id": "s-1" })),
        "{:?}",
        started.error
    );
    wait_until_search_done(&state, Some("admin-token"), "s-1");

    let expected = json!({
        "schema_version": 1,
        "search_id": "s-1",
        "missing": [],
        "unused": [],
        "differs": [{
            "device_id": "dev:ethercat:mock:2",
            "protocol": "ethercat",
            "configured": { "adapter": "mock", "position": 3, "model": "EL2008" },
            "found": { "adapter": "mock", "position": 2, "model": "EL2008" },
            "fields": ["position"]
        }],
        "not_compared": []
    });
    for params in [json!({ "search_id": "s-1" }), json!({})] {
        let compared = handle_request_value(
            json!({ "id": 2, "type": "comm.compare", "auth": viewer, "params": params }),
            &state,
            None,
        );
        assert!(compared.ok, "params={params}: {:?}", compared.error);
        assert_eq!(compared.result, Some(expected.clone()), "params={params}");
    }
    let _ = fs::remove_file(path);
    let _ = fs::remove_dir_all(folder);
}

#[test]
fn compare_refuses_with_stable_codes_and_changes_nothing() {
    // Specification 11, "Compare with the project" ("Request"): a search the runtime does not keep, or no
    // search for `{}`, is `search_not_found`; a runtime without a project folder is `compare_no_project`; a
    // folder that is not a project is `compare_project_unreadable`; a misspelled member is a request
    // error without a code. A wrong implementation compares against nothing and answers an empty
    // comparison, which would call every found device unused.
    let mut state = control_test_state("PROGRAM Main END_PROGRAM");
    let compare = |state: &ControlState, id: u64, params: serde_json::Value| {
        handle_request_value(
            json!({ "id": id, "type": "comm.compare", "params": params }),
            state,
            None,
        )
    };

    let none = compare(&state, 1, json!({}));
    assert!(!none.ok);
    assert_eq!(none.error_code.as_deref(), Some("search_not_found"));
    let unknown = compare(&state, 2, json!({ "search_id": "s-99" }));
    assert_eq!(unknown.error_code.as_deref(), Some("search_not_found"));
    let misspelt = compare(&state, 3, json!({ "searchid": "s-1" }));
    assert!(!misspelt.ok && misspelt.error_code.is_none());
    assert!(misspelt
        .error
        .as_deref()
        .is_some_and(|error| error.starts_with("invalid comm.compare payload:")));

    // An EtherCAT search without a confirmed fieldbus scan is skipped, so it ends at once.
    let started = handle_request_value(
        search_request(4, "comm.search.start", json!({ "only": ["ethercat"] })),
        &state,
        None,
    );
    assert_eq!(started.result, Some(json!({ "search_id": "s-1" })));
    wait_until_search_done(&state, None, "s-1");
    let without_project = compare(&state, 5, json!({}));
    assert_eq!(
        without_project.error_code.as_deref(),
        Some("compare_no_project")
    );

    let folder = temp_dir("compare-not-a-project");
    state.project_root = Some(folder.clone());
    let unreadable = compare(&state, 6, json!({ "search_id": "s-1" }));
    assert_eq!(
        unreadable.error_code.as_deref(),
        Some("compare_project_unreadable")
    );
    assert!(unreadable
        .error
        .as_deref()
        .is_some_and(|error| error.contains("has no runtime.toml")));
    let _ = fs::remove_dir_all(folder);
}
