// The mesh's runtimes in the runtime's log (specification 11, "A mesh says which peers come, go or
// stay away"): every runtime that joins or leaves, and once, 5 s after the mesh started, every
// runtime that a subscription names and that is not in the mesh then.

/// How often the log looks at the mesh's runtimes.
const MESH_PRESENCE_LOOK: std::time::Duration = std::time::Duration::from_millis(500);

/// Logs the changes of the runtimes of the mesh whose runtimes `peers` are, from a thread that ends
/// with the mesh; `subscribe` is the mesh's `[runtime.mesh.subscribe]`.
fn log_mesh_presence(
    peers: trust_runtime::mesh::MeshPeers,
    subscribe: &indexmap::IndexMap<SmolStr, SmolStr>,
    logger: RuntimeLogger,
) {
    let mut watch =
        trust_runtime::mesh::MeshPresenceWatch::new(subscribe, std::time::Instant::now());
    std::thread::spawn(move || {
        while let Some(current) = peers.current() {
            for change in watch.changes(&current, std::time::Instant::now()) {
                log_mesh_peer_change(&logger, change);
            }
            std::thread::sleep(MESH_PRESENCE_LOOK);
        }
    });
}

/// The log line of `change`: `mesh_peer_joined` at level `info`, `mesh_peer_left` and
/// `mesh_peer_missing` at level `warn`.
fn log_mesh_peer_change(logger: &RuntimeLogger, change: trust_runtime::mesh::MeshPeerChange) {
    use trust_runtime::mesh::{MeshPeerChange, MESH_PEER_GRACE};
    match change {
        MeshPeerChange::Joined(peer) => {
            logger.log(LogLevel::Info, "mesh_peer_joined", json!({ "peer": peer }));
        }
        MeshPeerChange::Left(peer) => {
            logger.log(LogLevel::Warn, "mesh_peer_left", json!({ "peer": peer }));
        }
        MeshPeerChange::Missing {
            peer,
            subscriptions,
        } => logger.log(
            LogLevel::Warn,
            "mesh_peer_missing",
            json!({
                "peer": peer,
                "subscriptions": subscriptions,
                "waited_ms": MESH_PEER_GRACE.as_millis(),
                "hint": "no runtime of this name has joined: check that it runs, that a `connect` \
                         entry reaches it, that the subscription names it by its resource name, \
                         `<site>/<resource>` in another site, and that both meshes have the same \
                         `auth_token`",
            }),
        ),
    }
}
