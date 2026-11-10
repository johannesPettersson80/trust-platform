//! Native peer authentication: a configured token must reject unauthenticated/wrong peers.
use crate::config::{MeshConfig, MeshRole};
use crate::scheduler::{ResourceCommand, ResourceControl, StdClock};
use indexmap::IndexMap;
use std::net::{SocketAddr, TcpListener};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};
use zenoh::Wait;

/// The stable runtime ID the started meshes announce: sixteen random bytes in lowercase hex, as
/// `runtime_id` generates them.
const STABLE_ID: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f0";

fn config(listen: String, connect: Vec<String>, token: Option<&str>) -> MeshConfig {
    MeshConfig {
        enabled: true,
        role: MeshRole::Peer,
        listen: listen.into(),
        connect: connect.into_iter().map(Into::into).collect(),
        tls: false,
        auth_token: token.map(Into::into),
        publish: Vec::new(),
        subscribe: IndexMap::new(),
        zenohd_version: super::ZENOHD_BASELINE_VERSION.into(),
        plugin_versions: IndexMap::new(),
    }
}

/// The owner's mesh that `start` starts on a loopback port that was free a moment ago, with its
/// resource's command receiver, and the port's address. The owner takes a concrete port: its peers
/// and the test connect to the address, and Zenoh reports the endpoints it bound only through its
/// unstable API. Another socket can take the port first; the mesh then starts without a session,
/// degraded with the system's "address in use" error, and it is started again on another port, ten
/// times at most, and after no other failure (specification 22).
// port-retry-ok: starts the mesh again when its session found the port taken, and only then.
fn mesh_on_a_free_port(
    start: impl Fn(SocketAddr, ResourceControl<StdClock>) -> super::MeshService,
) -> (super::MeshService, Receiver<ResourceCommand>, SocketAddr) {
    let mut taken = Vec::new();
    for _ in 0..10 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let (resource, commands) = ResourceControl::stub(StdClock::new());
        let mesh = start(address, resource);
        let port_taken = mesh.session.is_none()
            && mesh
                .degraded_reason
                .as_deref()
                .is_some_and(trust_test_support::is_a_taken_port_error);
        if !port_taken {
            return (mesh, commands, address);
        }
        taken.push(address);
    }
    panic!("another socket took each chosen port before the mesh bound it: {taken:?}");
}

#[test]
fn configured_mesh_authentication_rejects_wrong_and_missing_peer_tokens() {
    let (owner, _commands, address) = mesh_on_a_free_port(|address, resource| {
        super::start_mesh(
            &config(
                address.to_string(),
                Vec::new(),
                Some("mesh-test-owner-token"),
            ),
            format!("auth-owner-{}", std::process::id()).into(),
            STABLE_ID.into(),
            resource,
        )
        .unwrap()
        .unwrap()
    });
    let address = address.to_string();
    let key = format!("trust-hmi-auth-test/{}", std::process::id());
    owner
        .session
        .as_ref()
        .unwrap()
        .declare_queryable(key.clone())
        .callback(|query| {
            let _ = query.reply(query.key_expr().clone(), "permitted").wait();
        })
        .background()
        .wait()
        .unwrap();
    let can_query = |label: &str, token: Option<&str>| {
        let (resource, _commands) = ResourceControl::stub(StdClock::new());
        let peer = super::start_mesh(
            &config("127.0.0.1:0".into(), vec![address.clone()], token),
            format!("auth-{label}-{}", std::process::id()).into(),
            STABLE_ID.into(),
            resource,
        )
        .unwrap()
        .unwrap();
        let Some(session) = peer.session.as_ref() else {
            return false;
        };
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            let replies = session
                .get(key.clone())
                .timeout(Duration::from_millis(250))
                .wait()
                .unwrap();
            if let Ok(Some(reply)) = replies.recv_timeout(Duration::from_millis(350))
                && reply.result().is_ok()
            {
                return true;
            }
        }
        false
    };
    assert!(
        can_query("matching", Some("mesh-test-owner-token")),
        "a matching peer can exchange data"
    );
    assert!(
        !can_query("wrong", Some("different-token")),
        "a wrong token must not reach the protected queryable"
    );
    assert!(
        !can_query("missing", None),
        "an anonymous peer must not reach the protected queryable"
    );
}

#[test]
fn mesh_tls_socket_presents_the_configured_certificate() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    let certificate = std::fs::read(root.join("server-cert.pem")).unwrap();
    let materials = crate::security::TlsMaterials {
        cert_path: root.join("server-cert.pem"),
        key_path: root.join("server-key.pem"),
        ca_path: None,
        certificate_pem: certificate.clone(),
        private_key_pem: std::fs::read(root.join("server-key.pem")).unwrap(),
        ca_pem: certificate,
    };
    let (_owner, _commands, address) = mesh_on_a_free_port(|address, resource| {
        let mut configured = config(address.to_string(), Vec::new(), None);
        configured.tls = true;
        super::start_mesh_with_tls(
            &configured,
            "tls-socket-test".into(),
            STABLE_ID.into(),
            resource,
            Some(&materials),
        )
        .unwrap()
        .unwrap()
    });
    let mut socket = std::net::TcpStream::connect(address).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut client = rustls::ClientConnection::new(
        crate::security::rustls_client_config(&materials).unwrap(),
        "localhost".try_into().unwrap(),
    )
    .unwrap();
    let handshake = client.complete_io(&mut socket);
    assert!(
        handshake.is_ok(),
        "the mesh socket must actually negotiate TLS: {handshake:?}"
    );
    assert!(!client.is_handshaking());
    assert!(client.peer_certificates().is_some());
}
