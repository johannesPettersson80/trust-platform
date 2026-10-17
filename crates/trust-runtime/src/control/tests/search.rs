fn search_request(id: u64, kind: &str, params: serde_json::Value) -> serde_json::Value {
    json!({ "id": id, "type": kind, "params": params })
}

#[test]
fn search_requests_answer_through_the_control_connection() {
    let state = hmi_test_state("PROGRAM Main END_PROGRAM");
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
    let mut state = hmi_test_state("PROGRAM Main END_PROGRAM");
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
