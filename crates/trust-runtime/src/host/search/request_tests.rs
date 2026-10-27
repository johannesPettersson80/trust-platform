use std::time::Duration;

use serde_json::json;

use super::*;

fn check(params: serde_json::Value) -> Result<ValidSearch, SearchError> {
    SearchRequest::from_params(Some(params))?.validate(SearchOrigin::Runtime)
}

#[test]
fn deadline_defaults_to_10_s_and_rejects_above_60_s() {
    let absent = SearchRequest::from_params(None)
        .and_then(|request| request.validate(SearchOrigin::ThisHost))
        .expect("a request without members");
    assert_eq!(absent.deadline, Duration::from_secs(10));
    assert_eq!(absent.sources, SourceId::ALL.to_vec());
    assert_eq!(
        check(json!({ "deadline_ms": null }))
            .expect("null deadline")
            .deadline,
        Duration::from_secs(10)
    );
    assert_eq!(
        check(json!({ "deadline_ms": 60_000 }))
            .expect("60 s deadline")
            .deadline,
        Duration::from_secs(60)
    );
    assert_eq!(
        check(json!({ "deadline_ms": 1 }))
            .expect("1 ms deadline")
            .deadline,
        Duration::from_millis(1)
    );
    for deadline_ms in [
        json!(60_001),
        json!(0),
        json!(-5),
        json!(2.5),
        json!("10000"),
    ] {
        assert!(
            matches!(
                check(json!({ "deadline_ms": deadline_ms })),
                Err(SearchError::Invalid(_))
            ),
            "deadline_ms={deadline_ms}"
        );
    }
}

#[test]
fn ads_scope_ports_default_merge_and_reject_invalid() {
    // Specification 11, "Communication discovery scope and symbol browsing": an absent ADS port
    // list is the default; duplicates merge; the list sorts; an invalid value is a request error
    // that names it, never a silent drop.
    assert_eq!(
        check(json!({})).expect("default ports").ads_ports,
        vec![301, 501, 851, 852, 853, 854]
    );
    assert_eq!(
        check(json!({ "ads_ports": [851, 301, 851] }))
            .expect("merged ports")
            .ads_ports,
        vec![301, 851]
    );
    assert_eq!(
        check(json!({ "ads_ports": [] }))
            .expect("an explicit empty list")
            .ads_ports,
        Vec::<u16>::new()
    );
    for bad in [json!(0), json!(65536), json!("852"), json!(501.5)] {
        let error = check(json!({ "ads_ports": [bad] }))
            .expect_err("an invalid ADS port must be a request error");
        assert!(matches!(error, SearchError::Invalid(_)), "bad={bad}");
        assert!(
            error.to_string().contains(&bad.to_string()),
            "the error must name {bad}: {error}"
        );
    }
}

#[test]
fn unknown_request_keys_are_rejected() {
    for params in [
        json!({ "deadline": 5000 }),
        json!({ "targets": { "host": "10.0.0.9" } }),
        json!({ "targets": { "hosts": [], "subnet": "10.0.0.0/24" } }),
        json!({ "fieldbus": [{ "bus": "ethercat", "adapter": "eth0", "force": true }] }),
        json!({ "fieldbus": [{ "bus": "profinet" }] }),
        json!({ "only": ["modbus"] }),
        json!({ "origin": "cloud" }),
        json!({ "confirm_fieldbus_scan": "true" }),
    ] {
        let error = check(params.clone()).expect_err("a misspelled member must not vanish");
        assert!(matches!(error, SearchError::Invalid(_)), "params={params}");
        assert_eq!(error.code(), None, "params={params}");
    }
    for params in [
        json!({ "search_id": "s-1", "since": 0, "cursor": 3 }),
        json!({ "search": "s-1" }),
    ] {
        assert!(
            StatusRequest::from_params(Some(params.clone())).is_err(),
            "params={params}"
        );
    }
    assert!(CancelRequest::from_params(Some(json!({ "search_id": "s-1", "all": true }))).is_err());
    assert!(StatusRequest::from_params(None).is_err());
    assert!(CancelRequest::from_params(None).is_err());
    let status = StatusRequest::from_params(Some(json!({ "search_id": "s-2", "since": 7 })))
        .expect("status parameters");
    assert_eq!((status.search_id.as_str(), status.since), ("s-2", Some(7)));
}

#[test]
fn request_names_its_origin_sources_and_targets_exactly() {
    let refused = check(json!({ "origin": "this_host" })).expect_err("wrong origin");
    assert!(refused.to_string().contains("origin must be runtime"));
    assert!(check(json!({ "origin": "runtime" })).is_ok());
    assert!(check(json!({ "only": [] })).is_err());

    let search = check(json!({
        "only": ["mqtt", "trust_runtime", "mqtt"],
        "targets": { "hosts": [" 10.0.0.9:502 ", "10.0.0.9:502", "plc.local"], "cidr": " 192.168.1.0/24 " },
        "confirm_fieldbus_scan": true,
        "fieldbus": [
            { "bus": "ethercat", "adapter": "eth1" },
            { "bus": "ethercat", "adapter": " eth1 " },
            { "bus": "ethercat" }
        ]
    }))
    .expect("a full request");
    assert_eq!(search.sources, vec![SourceId::TrustRuntime, SourceId::Mqtt]);
    assert_eq!(search.hosts, vec!["10.0.0.9:502", "plc.local"]);
    assert_eq!(search.cidr.as_deref(), Some("192.168.1.0/24"));
    assert!(search.fieldbus_confirmed);
    assert_eq!(
        search.ethercat_adapters,
        vec![Some("eth1".to_string()), None]
    );
    assert_eq!(
        check(json!({})).expect("empty request").ethercat_adapters,
        vec![None]
    );

    for hosts in [
        json!([""]),
        json!(["opc.tcp://10.0.0.9:4840"]),
        json!(["10.0.0.9/24"]),
        json!(["user@10.0.0.9"]),
        json!(["10.0.0 .9"]),
    ] {
        assert!(
            check(json!({ "targets": { "hosts": hosts } })).is_err(),
            "hosts={hosts}"
        );
    }
    let too_many: Vec<String> = (0..=MAX_HOSTS)
        .map(|n| format!("10.0.{}.{}", n / 250, n % 250))
        .collect();
    assert!(check(json!({ "targets": { "hosts": too_many } })).is_err());
    for cidr in ["10.0.0.0/23", "10.0.0.0", "300.0.0.0/24", ""] {
        assert!(
            check(json!({ "targets": { "cidr": cidr } })).is_err(),
            "cidr={cidr}"
        );
    }
    assert!(check(json!({ "fieldbus": [{ "bus": "ethercat", "adapter": "  " }] })).is_err());
}

#[test]
fn compare_request_names_the_search_or_the_latest_one() {
    // Specification 11, "Compare with the project" ("Request"): `{"search_id": "s-3"}`, or `{}` or no
    // parameters for the most recent search; another member, or a search ID that is not text, is a request
    // error. A wrong implementation ignores a misspelled member and compares the latest search instead.
    let named =
        CompareRequest::from_params(Some(json!({ "search_id": "s-3" }))).expect("a named search");
    assert_eq!(named.search_id.as_deref(), Some("s-3"));
    for latest in [None, Some(json!({}))] {
        let request = CompareRequest::from_params(latest).expect("the latest search");
        assert_eq!(request.search_id, None);
    }
    for bad in [
        json!({ "searchid": "s-3" }),
        json!({ "search_id": 3 }),
        json!({ "search_id": "s-3", "since": 1 }),
    ] {
        assert!(
            matches!(
                CompareRequest::from_params(Some(bad.clone())),
                Err(SearchError::Invalid(_))
            ),
            "params={bad}"
        );
    }
}
