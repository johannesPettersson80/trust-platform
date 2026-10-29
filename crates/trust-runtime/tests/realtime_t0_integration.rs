use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use trust_runtime::realtime::{
    RealtimeRoute, T0ChannelPolicy, T0CycleScheduler, T0ErrorCode, T0ExchangePoint, T0ReadOutcome,
    T0SchedulerPolicy, T0ShmConfig, T0Transport,
};

const SHM_CHILD_MODE_ENV: &str = "TRUST_RUNTIME_T0_SHM_CHILD_MODE";
const SHM_ROOT_ENV: &str = "TRUST_RUNTIME_T0_SHM_ROOT";

fn unique_temp_path(label: &str) -> PathBuf {
    static NONCE: AtomicU64 = AtomicU64::new(0);
    let nonce = NONCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "trust-runtime-realtime-integration-{label}-{}-{nonce}",
        std::process::id()
    ))
}

fn maybe_run_shm_child_consumer() -> bool {
    if std::env::var(SHM_CHILD_MODE_ENV).ok().as_deref() != Some("consumer") {
        return false;
    }
    let root = std::env::var(SHM_ROOT_ENV).expect("child SHM root env");
    let mut transport = T0Transport::with_config(T0ShmConfig::with_root(PathBuf::from(root)));
    transport
        .register_channel(
            "packml.state",
            "sha256:packml-state",
            T0ChannelPolicy {
                slot_size: 8,
                stale_after_reads: 2,
                max_spin_retries: 3,
                max_spin_time_us: 50,
            },
        )
        .expect("register child channel");
    let sub_handle = transport
        .bind_subscriber(
            "packml.state",
            RealtimeRoute::T0HardRt,
            "sha256:packml-state",
            4,
            true,
        )
        .expect("bind child subscriber");

    let mut out = [0_u8; 4];
    for _ in 0..200 {
        match transport.read_hardrt(sub_handle, &mut out) {
            Ok(T0ReadOutcome::Fresh(_)) => {
                assert_eq!(out, [4, 3, 2, 1]);
                return true;
            }
            Ok(T0ReadOutcome::NoUpdate) => std::thread::sleep(Duration::from_millis(5)),
            Err(error) if error.code == T0ErrorCode::StaleData => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("unexpected child read error: {error}"),
        }
    }
    panic!("child subscriber did not observe fresh SHM payload");
}

#[test]
fn realtime_t0_route_does_not_fallback_to_mesh_ip_path() {
    if maybe_run_shm_child_consumer() {
        return;
    }

    let mut transport = T0Transport::new();
    transport
        .register_channel(
            "packml.state",
            "sha256:packml-state",
            T0ChannelPolicy {
                slot_size: 8,
                stale_after_reads: 2,
                max_spin_retries: 2,
                max_spin_time_us: 50,
            },
        )
        .expect("register channel");

    let err = transport
        .bind_publisher(
            "packml.state",
            RealtimeRoute::MeshIp,
            "sha256:packml-state",
            4,
            true,
        )
        .expect_err("mesh/ip fallback route must be denied for T0 binds");
    assert_eq!(err.code, T0ErrorCode::ContractViolation);
    assert_eq!(transport.fallback_denied_total(), 1);

    let pub_handle = transport
        .bind_publisher(
            "packml.state",
            RealtimeRoute::T0HardRt,
            "sha256:packml-state",
            4,
            true,
        )
        .expect("bind publisher");
    let sub_handle = transport
        .bind_subscriber(
            "packml.state",
            RealtimeRoute::T0HardRt,
            "sha256:packml-state",
            4,
            true,
        )
        .expect("bind subscriber");
    transport
        .publish_hardrt(pub_handle, &[1, 2, 3, 4])
        .expect("publish hardrt");
    let mut out = [0_u8; 4];
    let read = transport
        .read_hardrt(sub_handle, &mut out)
        .expect("read hardrt");
    assert_eq!(out, [1, 2, 3, 4]);
    assert!(
        matches!(read, T0ReadOutcome::Fresh(_)),
        "expected fresh T0 read after legal publish"
    );
}

#[test]
fn realtime_t0_determinism_holds_under_cloud_budget_pressure() {
    if maybe_run_shm_child_consumer() {
        return;
    }

    let mut transport = T0Transport::new();
    transport
        .register_channel(
            "line-a.speed",
            "sha256:line-a-speed",
            T0ChannelPolicy {
                slot_size: 8,
                stale_after_reads: 3,
                max_spin_retries: 2,
                max_spin_time_us: 50,
            },
        )
        .expect("register channel");
    let pub_handle = transport
        .bind_publisher(
            "line-a.speed",
            RealtimeRoute::T0HardRt,
            "sha256:line-a-speed",
            4,
            true,
        )
        .expect("bind publisher");
    let sub_handle = transport
        .bind_subscriber(
            "line-a.speed",
            RealtimeRoute::T0HardRt,
            "sha256:line-a-speed",
            4,
            true,
        )
        .expect("bind subscriber");

    let mut scheduler = T0CycleScheduler::new(T0SchedulerPolicy {
        max_cloud_ops_per_cycle: 1,
    });
    let mut out = [0_u8; 4];
    for cycle in 1_u64..=24 {
        scheduler.begin_cycle(cycle);
        scheduler
            .mark_exchange_point(T0ExchangePoint::PreTask)
            .expect("mark pre-task exchange");
        let _ = scheduler.consume_cloud_budget(500);
        let payload = [cycle as u8, cycle as u8, cycle as u8, cycle as u8];
        transport
            .publish_hardrt(pub_handle, &payload)
            .expect("publish t0 payload");
        let read = transport
            .read_hardrt(sub_handle, &mut out)
            .expect("read t0 payload");
        assert_eq!(out, payload);
        match read {
            T0ReadOutcome::Fresh(details) => {
                assert_eq!(details.sequence, cycle);
                assert_eq!(details.bytes, 4);
                assert_eq!(details.dropped_updates, 0);
            }
            T0ReadOutcome::NoUpdate => panic!("expected fresh read in cycle {cycle}"),
        }
        scheduler
            .mark_exchange_point(T0ExchangePoint::PostTask)
            .expect("mark post-task exchange");
    }
    assert!(
        scheduler.denied_cloud_ops_total() > 0,
        "cloud plane budget must clamp excess work"
    );
    let counters = transport
        .channel_counters("line-a.speed")
        .expect("channel counters");
    assert_eq!(counters.stale_count, 0);
    assert_eq!(counters.overrun_count, 0);
}

#[test]
fn realtime_t0_multi_process_shm_exchange_succeeds() {
    if maybe_run_shm_child_consumer() {
        return;
    }

    let root = unique_temp_path("multi-process-shm");
    let mut transport = T0Transport::with_config(T0ShmConfig::with_root(root.clone()));
    transport
        .register_channel(
            "packml.state",
            "sha256:packml-state",
            T0ChannelPolicy {
                slot_size: 8,
                stale_after_reads: 2,
                max_spin_retries: 3,
                max_spin_time_us: 50,
            },
        )
        .expect("register parent channel");
    let pub_handle = transport
        .bind_publisher(
            "packml.state",
            RealtimeRoute::T0HardRt,
            "sha256:packml-state",
            4,
            true,
        )
        .expect("bind parent publisher");

    let child = Command::new(std::env::current_exe().expect("current test binary"))
        .arg("--exact")
        .arg("realtime_t0_multi_process_shm_exchange_succeeds")
        .arg("--nocapture")
        .env(SHM_CHILD_MODE_ENV, "consumer")
        .env(SHM_ROOT_ENV, root.display().to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn child consumer");

    std::thread::sleep(Duration::from_millis(50));
    transport
        .publish_hardrt(pub_handle, &[4, 3, 2, 1])
        .expect("publish parent payload");

    let output = child.wait_with_output().expect("wait child");
    assert!(
        output.status.success(),
        "child consumer failed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Windows removes no file that is still mapped, so the transport goes first.
    drop(transport);
    let _ = std::fs::remove_dir_all(root);
}
