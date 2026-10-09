//! Real authenticated TLS peers, native owner policy and runtime storage. This is host/core
//! integration evidence; it is not physical hardware or an operator-browser acceptance run.
use crate::config::{MeshConfig, MeshRole};
use crate::hmi::access::{
    config::{HmiAccessConfig, HmiUser, WriteRule},
    password::{hash_password, PasswordHash},
    sessions::{SessionInfo, SessionStore},
};
use crate::hmi::commands::write::WriteAction;
use crate::hmi::member_paths::MemberPath;
use crate::scheduler::{ResourceCommand, ResourceControl, StdClock};
use crate::security::AccessRole;
use indexmap::IndexMap;
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

fn config(address: String, connect: Vec<String>, publish: Vec<&str>) -> MeshConfig {
    MeshConfig {
        enabled: true,
        role: MeshRole::Peer,
        listen: address.into(),
        connect: connect.into_iter().map(Into::into).collect(),
        tls: true,
        auth_token: Some("isolated-hmi-mesh-fixture".into()),
        publish: publish.into_iter().map(Into::into).collect(),
        subscribe: IndexMap::new(),
        zenohd_version: super::ZENOHD_BASELINE_VERSION.into(),
        plugin_versions: IndexMap::new(),
    }
}
struct Worker {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn address() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().to_string()
}
#[test]
fn two_tls_peers_read_and_write_only_published_owner_authorized_variables() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    let certificate = std::fs::read(root.join("server-cert.pem")).unwrap();
    let tls = crate::security::TlsMaterials {
        cert_path: root.join("server-cert.pem"),
        key_path: root.join("server-key.pem"),
        ca_path: None,
        certificate_pem: certificate.clone(),
        private_key_pem: std::fs::read(root.join("server-key.pem")).unwrap(),
        ca_pem: certificate,
    };
    let mut plant = crate::harness::TestHarness::from_source(
        "PROGRAM Main VAR Setpoint : REAL := 4.0; END_VAR END_PROGRAM",
    )
    .unwrap();
    plant.cycle();
    let (owner_resource, commands) = ResourceControl::stub(StdClock::new());
    let owner_exchange = owner_resource.hmi_exchange();
    let access = Arc::new(HmiAccessConfig {
        users: vec![HmiUser {
            name: "operator".into(),
            role: AccessRole::Operator,
            password: PasswordHash::parse(&hash_password("secret", 10_000, b"mesh-hmi-test-salt"))
                .unwrap(),
        }],
        write: vec![WriteRule {
            path: MemberPath::parse("Main.Setpoint").unwrap(),
            below: false,
            role: AccessRole::Operator,
            min: Some(0.0),
            max: Some(10.0),
        }],
        ..Default::default()
    });
    let (audit_tx, audit_rx) = std::sync::mpsc::channel();
    crate::hmi::remote_operations::configure_owner(
        &owner_exchange,
        Arc::new(SessionStore::new((*access).clone())),
        access,
        Some(audit_tx),
    );
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let exchange = owner_exchange.clone();
    let worker = std::thread::spawn(move || {
        while !thread_stop.load(Ordering::Relaxed) {
            plant.cycle();
            exchange.serve(plant.runtime_mut(), &[]);
            for command in commands.try_iter() {
                match command {
                    ResourceCommand::MeshCatalogue { names, respond_to } => {
                        let _ = respond_to.send(crate::hmi::variable_tree::published_variables(
                            &plant.runtime().metadata_snapshot(),
                            plant.runtime().storage(),
                            &names,
                        ));
                    }
                    ResourceCommand::MeshSnapshot { names, respond_to } => {
                        let _ = respond_to.send(plant.runtime().snapshot_globals(&names));
                    }
                    _ => {}
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    let worker = Worker {
        stop: stop.clone(),
        thread: Some(worker),
    };
    let owner_name = format!("Owner-{}", std::process::id());
    let origin_name = format!("Origin-{}", std::process::id());
    let owner_address = address();
    let owner = super::start_mesh_with_tls(
        &config(owner_address.clone(), vec![], vec!["Main.Setpoint"]),
        owner_name.clone().into(),
        owner_resource,
        Some(&tls),
    )
    .unwrap()
    .unwrap();
    let (origin_resource, _origin_commands) = ResourceControl::stub(StdClock::new());
    let origin_exchange = origin_resource.hmi_exchange();
    let origin = super::start_mesh_with_tls(
        &config(
            address(),
            vec![format!(
                "tls/localhost:{}",
                owner_address.rsplit(':').next().unwrap()
            )],
            vec![],
        ),
        origin_name.clone().into(),
        origin_resource,
        Some(&tls),
    )
    .unwrap()
    .unwrap();
    let connected = Instant::now() + Duration::from_secs(10);
    while !origin
        .topology_evidence()
        .liveliness_snapshot()
        .peers
        .contains(&owner_name)
        && Instant::now() < connected
    {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        origin
            .topology_evidence()
            .liveliness_snapshot()
            .peers
            .contains(&owner_name),
        "authenticated TLS peer discovered"
    );
    assert!(
        !origin
            .topology_evidence()
            .liveliness_snapshot()
            .peers
            .contains(&origin_name),
        "the local runtime is not its own remote peer"
    );
    let local = SessionInfo {
        id: "origin-session".into(),
        user: "local-operator".into(),
        role: AccessRole::Operator,
        idle_expires_in: Duration::from_secs(60),
        expires_in: Duration::from_secs(600),
    };
    let target = MemberPath::parse(&format!("{owner_name}:Main.Setpoint")).unwrap();
    assert!(origin_exchange
        .remote_operations
        .write(
            &local,
            &target,
            &WriteAction::Set(json!(5)),
            "before-login",
            None
        )
        .unwrap()
        .is_err());
    assert!(origin_exchange
        .remote_operations
        .login(&local, &owner_name, "operator", "wrong")
        .is_err());
    let login = origin_exchange
        .remote_operations
        .login(&local, &owner_name, "operator", "secret")
        .unwrap();
    assert!(login.get("token").is_none());
    assert!(origin_exchange
        .remote_operations
        .write(
            &local,
            &target,
            &WriteAction::Set(json!(11)),
            "over-limit",
            None
        )
        .unwrap()
        .is_err());
    let written = origin_exchange
        .remote_operations
        .write(
            &local,
            &target,
            &WriteAction::Set(json!(5)),
            "accepted",
            None,
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        crate::hmi::write_value::json_of(&written.readback),
        Some(json!(5.0))
    );
    assert_eq!(
        origin_exchange.remote_operations.observe(&local, &target),
        Some(json!(5.0))
    );
    let replay = origin_exchange
        .remote_operations
        .write(
            &local,
            &target,
            &WriteAction::Set(json!(5)),
            "accepted",
            None,
        )
        .unwrap()
        .unwrap();
    assert_eq!(replay, written);
    let unlisted = MemberPath::parse(&format!("{owner_name}:Main.Unpublished")).unwrap();
    assert!(origin_exchange
        .remote_operations
        .write(
            &local,
            &unlisted,
            &WriteAction::Set(json!(5)),
            "unlisted",
            None
        )
        .unwrap()
        .is_err());
    assert!(
        audit_rx.try_iter().filter(|event| event.ok).count() == 1,
        "the owner's successful write is audited once"
    );
    origin_exchange.remote_operations.logout(&local);
    drop(origin);
    drop(owner);
    drop(worker);
}
