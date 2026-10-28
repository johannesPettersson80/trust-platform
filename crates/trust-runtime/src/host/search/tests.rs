use std::sync::mpsc;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use serde_json::json;

use super::model::DeviceKey;
use super::runner::SourceSink;
use super::*;
use crate::connectors::DiscoveryConfidence;
use crate::discovery::DiscoveryState;

fn result(key: &str, addresses: &[&str]) -> ProtocolFind {
    ProtocolFind::new(
        SourceId::ModbusTcp,
        "tcp_connect",
        DiscoveryConfidence::PortReachable,
        Identity::Modbus(ModbusIdentity {
            unit_id: 1,
            vendor_name: None,
            product_code: None,
            revision: None,
        }),
        json!({ "address": addresses.first() }),
        1,
    )
    .keyed(
        key.to_string(),
        addresses
            .iter()
            .map(|address| DeviceKey::Address((*address).to_string()))
            .collect(),
    )
}

/// A source that runs until the search ends, then says so.
fn until_the_end(exited: Option<mpsc::Sender<SourceId>>, source: SourceId) -> SourcePlan {
    SourcePlan::Run(Box::new(move |sink: &SourceSink| {
        while !sink.ended() {
            sink.wait(Duration::from_millis(20));
        }
        if let Some(exited) = exited {
            let _ = exited.send(source);
        }
    }))
}

/// A search that ends at once on the tool's computer: GPIO runs only inside a runtime, so it is
/// skipped there.
fn quick_request() -> SearchRequest {
    SearchRequest {
        only: Some(vec![SourceId::Gpio]),
        ..SearchRequest::default()
    }
}

fn device_ids(status: &SearchStatus) -> Vec<&str> {
    status
        .devices
        .iter()
        .map(|device| device.id.as_str())
        .collect()
}

fn source_states(status: &SearchStatus) -> Vec<(SourceId, SourceState, Option<u32>)> {
    status
        .sources
        .iter()
        .map(|source| (source.id, source.state, source.count))
        .collect()
}

fn wait_for_end(registry: &SearchRegistry, search_id: &str) -> SearchStatus {
    let give_up = Instant::now() + Duration::from_secs(20);
    loop {
        let status = registry.status(search_id, None).expect("kept search");
        if status.state != SearchState::Running || Instant::now() > give_up {
            return status;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn search_runs_sources_in_parallel_and_ends_at_the_deadline() {
    // Each of the three sources waits for the other two: only sources that run at the same time
    // get past the barrier.
    let barrier = Arc::new(Barrier::new(3));
    let mut plans = Vec::new();
    for (index, source) in [SourceId::Ads, SourceId::ModbusTcp, SourceId::Mqtt]
        .into_iter()
        .enumerate()
    {
        let barrier = Arc::clone(&barrier);
        plans.push((
            source,
            SourcePlan::Run(Box::new(move |sink: &SourceSink| {
                barrier.wait();
                let address = format!("10.0.0.{index}");
                sink.found(result(&format!("k{index}"), &[address.as_str()]));
            })),
        ));
    }
    plans.push((
        SourceId::TrustRuntime,
        until_the_end(None, SourceId::TrustRuntime),
    ));
    let deadline = Duration::from_millis(1_500);
    let started = Instant::now();
    let registry = SearchRegistry::new();
    let search_id = registry
        .start_plans(SearchOrigin::ThisHost, deadline, plans)
        .expect("start");

    let status = wait_for_end(&registry, &search_id);
    assert!(started.elapsed() >= deadline, "it ran until its deadline");
    assert_eq!(status.state, SearchState::Done);
    assert_eq!(status.deadline_ms, 1_500);
    assert_eq!(status.elapsed_ms, 1_500);
    assert_eq!(
        source_states(&status),
        vec![
            (SourceId::Ads, SourceState::Found, Some(1)),
            (SourceId::ModbusTcp, SourceState::Found, Some(1)),
            (SourceId::Mqtt, SourceState::Found, Some(1)),
            (SourceId::TrustRuntime, SourceState::TimedOut, Some(0)),
        ]
    );
    assert_eq!(status.devices.len(), 3);
}

#[test]
fn running_source_reports_timed_out_with_its_results() {
    let (saw_end, ended) = mpsc::channel();
    let plans = vec![(
        SourceId::ModbusTcp,
        SourcePlan::Run(Box::new(move |sink: &SourceSink| {
            sink.found(result("a", &["10.0.0.1"]));
            sink.found(result("b", &["10.0.0.2"]));
            while !sink.ended() {
                sink.wait(Duration::from_millis(10));
            }
            // Too late: the search has ended.
            sink.found(result("late", &["10.0.0.3"]));
            sink.warn("late warning");
            let _ = saw_end.send(());
        })),
    )];
    let registry = SearchRegistry::new();
    let search_id = registry
        .start_plans(SearchOrigin::Runtime, Duration::from_millis(1_000), plans)
        .expect("start");
    ended
        .recv_timeout(Duration::from_secs(20))
        .expect("the source saw the end");

    let status = registry.status(&search_id, None).expect("kept search");
    assert_eq!(status.state, SearchState::Done);
    assert_eq!(status.origin, SearchOrigin::Runtime);
    assert_eq!(
        source_states(&status),
        vec![(SourceId::ModbusTcp, SourceState::TimedOut, Some(2))]
    );
    assert!(status.sources[0].warnings.is_empty());
    assert_eq!(
        device_ids(&status),
        vec!["dev:ip:10.0.0.1", "dev:ip:10.0.0.2"]
    );
}

#[test]
fn status_since_returns_only_changed_devices() {
    let (send, results) = mpsc::channel::<ProtocolFind>();
    let (kept, acknowledged) = mpsc::channel::<()>();
    let plans = vec![(
        SourceId::ModbusTcp,
        SourcePlan::Run(Box::new(move |sink: &SourceSink| {
            while let Ok(find) = results.recv() {
                sink.found(find);
                let _ = kept.send(());
            }
        })),
    )];
    let registry = SearchRegistry::new();
    let search_id = registry
        .start_plans(SearchOrigin::ThisHost, Duration::from_secs(30), plans)
        .expect("start");
    let report = |find: ProtocolFind| {
        send.send(find).expect("source runs");
        acknowledged
            .recv_timeout(Duration::from_secs(20))
            .expect("result kept");
    };

    report(result("a", &["10.0.0.1"]));
    report(result("b", &["10.0.0.2"]));
    let first = registry.status(&search_id, None).expect("status");
    assert_eq!(
        device_ids(&first),
        vec!["dev:ip:10.0.0.1", "dev:ip:10.0.0.2"]
    );
    let unchanged = registry
        .status(&search_id, Some(first.cursor))
        .expect("status");
    assert!(unchanged.devices.is_empty());
    assert_eq!(unchanged.cursor, first.cursor);

    report(result("c", &["10.0.0.2"]));
    let second = registry
        .status(&search_id, Some(first.cursor))
        .expect("status");
    assert_eq!(device_ids(&second), vec!["dev:ip:10.0.0.2"]);
    assert_eq!(second.devices[0].finds.len(), 2);
    assert!(second.merged.is_empty());

    // A result on both addresses joins the second device into the first.
    report(result("d", &["10.0.0.1", "10.0.0.2"]));
    let third = registry
        .status(&search_id, Some(second.cursor))
        .expect("status");
    assert_eq!(device_ids(&third), vec!["dev:ip:10.0.0.1"]);
    assert_eq!(
        third.merged,
        vec![MergedDevice {
            id: "dev:ip:10.0.0.2".to_string(),
            into: "dev:ip:10.0.0.1".to_string(),
        }]
    );
    let everything = registry.status(&search_id, None).expect("status");
    assert_eq!(device_ids(&everything), vec!["dev:ip:10.0.0.1"]);
    assert_eq!(everything.devices[0].finds.len(), 4);
    assert!(everything.merged.is_empty());

    drop(send);
    assert_eq!(wait_for_end(&registry, &search_id).state, SearchState::Done);
}

#[test]
fn cancel_ends_every_source() {
    let (exited, exits) = mpsc::channel();
    let plans = vec![
        (
            SourceId::Ads,
            until_the_end(Some(exited.clone()), SourceId::Ads),
        ),
        (
            SourceId::ModbusTcp,
            until_the_end(Some(exited), SourceId::ModbusTcp),
        ),
        (
            SourceId::Usb,
            SourcePlan::Skip("USB search is not available in this truST version yet.".into()),
        ),
    ];
    let registry = SearchRegistry::new();
    let search_id = registry
        .start_plans(SearchOrigin::ThisHost, Duration::from_secs(60), plans)
        .expect("start");
    assert_eq!(
        registry.status(&search_id, None).expect("status").state,
        SearchState::Running
    );

    assert_eq!(
        registry.cancel(&search_id).expect("cancel"),
        SearchState::Cancelled
    );
    let status = registry.status(&search_id, None).expect("status");
    assert_eq!(status.state, SearchState::Cancelled);
    assert!(status.elapsed_ms < 60_000);
    assert_eq!(
        source_states(&status),
        vec![
            (SourceId::Ads, SourceState::Cancelled, Some(0)),
            (SourceId::ModbusTcp, SourceState::Cancelled, Some(0)),
            (SourceId::Usb, SourceState::Skipped, None),
        ]
    );
    let mut stopped = [
        exits
            .recv_timeout(Duration::from_secs(20))
            .expect("a source stops"),
        exits
            .recv_timeout(Duration::from_secs(20))
            .expect("a source stops"),
    ];
    stopped.sort();
    assert_eq!(stopped, [SourceId::Ads, SourceId::ModbusTcp]);
    assert_eq!(
        registry.cancel(&search_id).expect("cancel again"),
        SearchState::Cancelled
    );
}

#[test]
fn a_search_is_compared_only_after_it_ran_to_its_end() {
    // Specification 11, "Compare with the project" ("Request"): `comm.compare` takes the named search, or
    // the most recent one the registry keeps, and only a search that ended on its own: a search that
    // still runs or was cancelled has not looked everywhere. A wrong implementation compares a running
    // search, so that a device not yet found is called missing, or takes the oldest search for the latest.
    let registry = SearchRegistry::new();
    let none = registry
        .finished_status(None)
        .expect_err("no search is kept");
    assert_eq!(none, SearchError::NoSearch);
    assert_eq!(none.code(), Some("search_not_found"));
    assert_eq!(
        registry.finished_status(Some("s-1")).expect_err("unknown"),
        SearchError::NotFound("s-1".to_string())
    );

    let ended = registry
        .start(quick_request(), SearchHost::ThisHost)
        .expect("start");
    assert_eq!(ended, "s-1");
    assert_eq!(
        registry
            .finished_status(None)
            .expect("the latest has ended")
            .search_id,
        "s-1"
    );

    let running = registry
        .start_plans(
            SearchOrigin::ThisHost,
            Duration::from_secs(60),
            vec![(SourceId::Ads, until_the_end(None, SourceId::Ads))],
        )
        .expect("start");
    assert_eq!(running, "s-2");
    assert_eq!(registry.latest().as_deref(), Some("s-2"));
    let refused = registry
        .finished_status(None)
        .expect_err("the latest still runs");
    assert_eq!(
        refused,
        SearchError::NotFinished {
            search_id: "s-2".to_string(),
            state: SearchState::Running
        }
    );
    assert_eq!(refused.code(), Some("search_not_finished"));
    assert_eq!(
        registry
            .finished_status(Some("s-1"))
            .expect("an older search that ended")
            .search_id,
        "s-1"
    );

    registry.cancel(&running).expect("cancel");
    assert_eq!(
        registry
            .finished_status(Some("s-2"))
            .expect_err("cancelled"),
        SearchError::NotFinished {
            search_id: "s-2".to_string(),
            state: SearchState::Cancelled
        }
    );
}

#[test]
fn registry_keeps_at_most_8_searches() {
    let registry = SearchRegistry::new();
    let first: Vec<String> = (0..KEPT_SEARCHES)
        .map(|_| {
            registry
                .start(quick_request(), SearchHost::ThisHost)
                .expect("start")
        })
        .collect();
    assert_eq!(first[0], "s-1");
    for search_id in &first {
        let status = registry.status(search_id, None).expect("kept search");
        assert_eq!(
            status.state,
            SearchState::Done,
            "a skipped source ends it at once"
        );
    }
    let ninth = registry
        .start(quick_request(), SearchHost::ThisHost)
        .expect("a ninth search replaces the oldest ended one");
    assert_eq!(ninth, "s-9");
    let forgotten = registry
        .status("s-1", None)
        .expect_err("the oldest is gone");
    assert_eq!(forgotten, SearchError::NotFound("s-1".to_string()));
    assert_eq!(forgotten.code(), Some("search_not_found"));
    for search_id in first.iter().skip(1).chain([&ninth]) {
        assert!(registry.status(search_id, None).is_ok(), "{search_id}");
    }

    // While eight searches still run, a ninth is refused and none is dropped.
    let busy = SearchRegistry::new();
    let running: Vec<String> = (0..KEPT_SEARCHES)
        .map(|_| {
            busy.start_plans(
                SearchOrigin::ThisHost,
                Duration::from_secs(60),
                vec![(SourceId::Ads, until_the_end(None, SourceId::Ads))],
            )
            .expect("start")
        })
        .collect();
    let refused = busy
        .start_plans(
            SearchOrigin::ThisHost,
            Duration::from_secs(60),
            vec![(SourceId::Ads, until_the_end(None, SourceId::Ads))],
        )
        .expect_err("eight searches run");
    assert_eq!(refused, SearchError::Busy(KEPT_SEARCHES));
    assert_eq!(refused.code(), Some("search_busy"));
    for search_id in &running {
        assert_eq!(
            busy.cancel(search_id).expect("kept"),
            SearchState::Cancelled
        );
    }
    let next = busy
        .start(quick_request(), SearchHost::ThisHost)
        .expect("an ended search makes room");
    assert_eq!(next, "s-9");
    assert!(busy.status(&running[0], None).is_err());
}

#[test]
fn ended_search_is_forgotten_after_its_retention() {
    let registry = SearchRegistry::with_limits(KEPT_SEARCHES, Duration::from_millis(100));
    let ended = registry
        .start(quick_request(), SearchHost::ThisHost)
        .expect("start");
    let running = registry
        .start_plans(
            SearchOrigin::ThisHost,
            Duration::from_secs(60),
            vec![(SourceId::Ads, until_the_end(None, SourceId::Ads))],
        )
        .expect("start");
    assert!(registry.status(&ended, None).is_ok());
    std::thread::sleep(Duration::from_millis(400));
    assert!(
        matches!(registry.status(&ended, None), Err(SearchError::NotFound(_))),
        "an ended search is kept only for its retention"
    );
    assert!(
        registry.status(&running, None).is_ok(),
        "a running search is kept until it ends"
    );
    assert_eq!(
        registry.cancel(&running).expect("cancel"),
        SearchState::Cancelled
    );
}

#[test]
fn fieldbus_sources_skipped_without_confirmation() {
    let runtime = SearchHost::Runtime(RuntimeFacts {
        project_root: None,
        discovery: Arc::new(DiscoveryState::new()),
        discovery_enabled: true,
        discovery_interfaces: Vec::new(),
        runtime_id: smol_str::SmolStr::new("test-runtime-id"),
        owned_adapters: Vec::new(),
        mesh: None,
    });
    let request = |confirm: bool| {
        SearchRequest::from_params(Some(json!({
            "only": ["ethercat"],
            "confirm_fieldbus_scan": confirm,
            "fieldbus": [{ "bus": "ethercat", "adapter": "mock" }]
        })))
        .expect("request")
    };
    let registry = SearchRegistry::new();

    let unconfirmed = registry
        .start(request(false), runtime.clone())
        .expect("start");
    let status = wait_for_end(&registry, &unconfirmed);
    assert_eq!(status.state, SearchState::Done);
    assert_eq!(status.sources[0].state, SourceState::Skipped);
    assert!(status.sources[0]
        .reason
        .as_deref()
        .is_some_and(|reason| reason.contains("confirm_fieldbus_scan")));
    assert!(status.devices.is_empty());

    // Confirmed on the tool's computer: EtherCAT is still searched only inside a runtime.
    let elsewhere = registry
        .start(request(true), SearchHost::ThisHost)
        .expect("start");
    let status = wait_for_end(&registry, &elsewhere);
    assert_eq!(status.sources[0].state, SourceState::Skipped);
    assert!(status.sources[0]
        .reason
        .as_deref()
        .is_some_and(|reason| reason.contains("inside a runtime")));

    // Confirmed inside a runtime: the simulated bus is scanned, and its results say so.
    let confirmed = registry.start(request(true), runtime).expect("start");
    let status = wait_for_end(&registry, &confirmed);
    assert_eq!(status.state, SearchState::Done);
    assert_eq!(status.sources[0].state, SourceState::Found);
    assert!(status.sources[0].count.is_some_and(|count| count > 0));
    let finds: Vec<&ProtocolFind> = status
        .devices
        .iter()
        .flat_map(|device| &device.finds)
        .collect();
    assert!(finds.iter().all(|find| find.protocol == SourceId::Ethercat
        && find.simulated
        && find.confidence == DiscoveryConfidence::Confirmed));
    assert!(finds.iter().any(|find| matches!(
        &find.identity,
        Identity::Ethercat(EthercatIdentity { model, adapter, .. })
            if model == "EL1008" && adapter == "mock"
    )));
    assert!(status
        .devices
        .iter()
        .all(|device| device.id.starts_with("dev:ethercat:mock:")));
}
