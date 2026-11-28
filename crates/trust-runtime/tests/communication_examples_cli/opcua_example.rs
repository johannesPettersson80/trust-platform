//! The OPC UA example as its README writes it (specification 11, "OPC UA server surface, security,
//! and cold start"): an OPC UA client finds its two exposed globals in the folder `truST`, reads
//! them, and has a write refused; a narrower `expose` publishes less, and `validate` refuses the
//! settings its Try-its write.

use std::sync::Arc;

use opcua::client::prelude::{
    AttributeService, Client, ClientBuilder, IdentityToken, Session, ViewService,
};
use opcua::sync::RwLock;
use opcua::types::{
    AttributeId, BrowseDescription, BrowseDescriptionResultMask, BrowseDirection, DataValue,
    MessageSecurityMode, NodeClassMask, NodeId, ReadValueId, ReferenceTypeId, StatusCode,
    TimestampsToReturn, UAString, Variant, WriteValue,
};

use super::*;

/// The example's project under `examples`.
const EXAMPLE: &str = "communication/opcua";

/// The example started with `edits` of its `runtime.toml`. The harness moves the server from port
/// 4840 to a port reserved for the copy, so that the test runs beside the others.
fn start_example(edits: &[(&'static str, &'static str)]) -> runtime_process::Runtime {
    let mut example = ExampleRuntime::new(EXAMPLE);
    for (from, to) in edits {
        example = example.edit_source("runtime.toml", from, to);
    }
    let runtime = example.start();
    runtime.wait_for_snapshot(WAIT, "the first scan", |_| true);
    runtime
}

/// The URL of `runtime`'s OPC UA server, at the address its copy's `runtime.toml` gives it.
fn server_url(runtime: &runtime_process::Runtime) -> String {
    let text = std::fs::read_to_string(runtime.project().join("runtime.toml"))
        .expect("the copy's runtime.toml");
    let document: toml::Value = toml::from_str(&text).expect("the copy's runtime.toml parses");
    let listen = document["runtime"]["opcua"]["listen"]
        .as_str()
        .expect("the OPC UA server's address");
    format!("opc.tcp://{listen}/")
}

/// An anonymous session without security on the server at `url`, as the README's client opens it.
fn connect(url: &str) -> Arc<RwLock<Session>> {
    let pki = unique_temp_dir("opcua-example-pki");
    let mut client = ClientBuilder::new()
        .application_name("truST OPC UA example test")
        .application_uri("urn:trust:test:opcua-example")
        .pki_dir(&pki)
        .create_sample_keypair(true)
        .trust_server_certs(true)
        .session_retry_limit(1)
        .client()
        .expect("an OPC UA client");
    let endpoints = client
        .get_server_endpoints_from_url(url)
        .expect("the server's endpoints");
    let endpoint = Client::find_matching_endpoint(
        endpoints.as_slice(),
        url,
        opcua::crypto::SecurityPolicy::None,
        MessageSecurityMode::None,
    )
    .expect("the endpoint without security");
    client
        .connect_to_endpoint(endpoint, IdentityToken::Anonymous)
        .expect("an anonymous session")
}

/// The values of `nodes`, read in one request, so from one snapshot of the server.
fn read(session: &Arc<RwLock<Session>>, nodes: &[NodeId]) -> Vec<DataValue> {
    let requests: Vec<ReadValueId> = nodes.iter().cloned().map(ReadValueId::from).collect();
    session
        .read()
        .read(&requests, TimestampsToReturn::Neither, 0.0)
        .expect("a read")
}

/// The browse names of the nodes that `folder` organizes.
fn children(session: &Arc<RwLock<Session>>, folder: NodeId) -> Vec<String> {
    let results = session
        .read()
        .browse(&[BrowseDescription {
            node_id: folder,
            browse_direction: BrowseDirection::Forward,
            reference_type_id: ReferenceTypeId::HierarchicalReferences.into(),
            include_subtypes: true,
            node_class_mask: NodeClassMask::all().bits(),
            result_mask: BrowseDescriptionResultMask::all().bits(),
        }])
        .expect("a browse")
        .unwrap_or_default();
    results
        .into_iter()
        .flat_map(|result| result.references.unwrap_or_default())
        .map(|reference| reference.browse_name.name.to_string())
        .collect()
}

#[test]
fn opcua_example_serves_its_two_globals_read_only_as_its_readme_writes() {
    // examples/communication/opcua/README.md (specification 11, "OPC UA server surface,
    // security, and cold start"). The folder `truST`, ns=2;i=0, holds the two exposed globals,
    // TankLevel and PumpRunning, as ns=2;s=<name> in the namespace that `namespace_uri` names,
    // after OPC UA's own and the application's urn:trust:runtime:CommOpcUaRes. Read in one
    // request, the level is a Float within the program's range of 20.0 to 80.0 and the pump a
    // Boolean that is TRUE exactly when the level is above 50.0; half a second later the level
    // has moved. A write is refused with BadNotWritable, as the server publishes with read access
    // only. Catches globals published under other identifiers or types, values that are not the
    // program's, a server that takes writes, and a README that names what the server does not.
    let readme = std::fs::read_to_string(
        runtime_process::repo_root().join("examples/communication/opcua/README.md"),
    )
    .expect("the example's README");
    for quoted in [
        "`ns=2;i=0`",
        "`ns=2;s=TankLevel`",
        "`urn:trust:runtime:CommOpcUaRes`",
        "(BadNotWritable)`",
    ] {
        assert!(readme.contains(quoted), "the README names {quoted}");
    }
    let runtime = start_example(&[]);
    let session = connect(&server_url(&runtime));

    let namespaces = read(&session, &[NodeId::new(0, 2255u32)]);
    let Some(Variant::Array(array)) = &namespaces[0].value else {
        panic!("the namespace array: {namespaces:?}");
    };
    let uris: Vec<String> = array
        .values
        .iter()
        .map(|uri| match uri {
            Variant::String(text) => text.to_string(),
            other => panic!("a namespace URI: {other:?}"),
        })
        .collect();
    assert_eq!(
        uris,
        [
            "http://opcfoundation.org/UA/",
            "urn:trust:runtime:CommOpcUaRes",
            "urn:trust:runtime"
        ],
        "OPC UA's, the application's and namespace_uri's namespaces"
    );
    assert_eq!(
        children(&session, NodeId::new(2, 0u32)),
        ["TankLevel", "PumpRunning"],
        "the folder truST holds the exposed globals"
    );

    let level = NodeId::new(2, "TankLevel");
    let pump = NodeId::new(2, "PumpRunning");
    let values = |session: &Arc<RwLock<Session>>| {
        let values = read(session, &[level.clone(), pump.clone()]);
        match (&values[0].value, &values[1].value) {
            (Some(Variant::Float(level)), Some(Variant::Boolean(pump))) => (*level, *pump),
            other => panic!("a Float level and a Boolean pump: {other:?}"),
        }
    };
    let (first, running) = values(&session);
    assert!(
        (20.0..=80.0).contains(&first),
        "the level within its range: {first}"
    );
    assert_eq!(running, first > 50.0, "the pump follows the level {first}");
    std::thread::sleep(std::time::Duration::from_millis(500));
    let (second, _) = values(&session);
    assert_ne!(second, first, "the level moves between reads");

    let written = session
        .read()
        .write(&[WriteValue {
            node_id: level,
            attribute_id: AttributeId::Value as u32,
            index_range: UAString::null(),
            value: DataValue::value_only(Variant::Float(10.0)),
        }])
        .expect("a write's results");
    assert_eq!(
        written,
        [StatusCode::BadNotWritable],
        "the write is refused"
    );
    session.read().disconnect();
    drop(runtime);
}

#[test]
fn opcua_example_try_its_publish_less_and_refuse_open_or_unmoded_settings() {
    // examples/communication/opcua/README.md, "Try it yourself" (specification 11, "OPC UA server
    // surface, security, and cold start"): with `expose = ["TankLevel"]` the folder holds
    // TankLevel alone and PumpRunning reads as BadNodeIdUnknown; `validate` refuses
    // `allow_anonymous = false` without a user, and Basic256Sha256 with the message mode None,
    // each with the error the README quotes. Catches an `expose` that publishes more than it
    // names, a server that admits nobody or anybody without saying so, and a security profile
    // that signs nothing.
    let readme = std::fs::read_to_string(
        runtime_process::repo_root().join("examples/communication/opcua/README.md"),
    )
    .expect("the example's README");
    let narrow = start_example(&[(
        "expose = [\"TankLevel\", \"PumpRunning\"]",
        "expose = [\"TankLevel\"]",
    )]);
    let session = connect(&server_url(&narrow));
    assert_eq!(
        children(&session, NodeId::new(2, 0u32)),
        ["TankLevel"],
        "the folder holds the exposed global alone"
    );
    let pump = read(&session, &[NodeId::new(2, "PumpRunning")]);
    assert_eq!(
        pump[0].status,
        Some(StatusCode::BadNodeIdUnknown),
        "the unexposed global: {pump:?}"
    );
    assert!(readme.contains("(BadNodeIdUnknown)`"));
    session.read().disconnect();

    for (from, to, refusal) in [
        (
            "allow_anonymous = true",
            "allow_anonymous = false",
            "runtime.opcua requires anonymous access or username/password when enabled",
        ),
        (
            "security_policy = \"none\"",
            "security_policy = \"basic256sha256\"",
            "unsupported runtime.opcua security profile Basic256Sha256/None",
        ),
    ] {
        let runtime_toml = narrow.project().join("runtime.toml");
        let shipped = std::fs::read_to_string(&runtime_toml).expect("runtime.toml");
        std::fs::write(&runtime_toml, shipped.replacen(from, to, 1)).expect("edit runtime.toml");
        let output = runtime_process::trust_runtime_command()
            .args(["validate", "--project"])
            .arg(narrow.project())
            .output()
            .expect("run trust-runtime validate");
        std::fs::write(&runtime_toml, shipped).expect("restore runtime.toml");
        assert!(!output.status.success(), "validate refuses `{to}`");
        let stderr = runtime_process::stderr_text(&output);
        assert!(stderr.contains(refusal), "the refusal of `{to}`: {stderr}");
        assert!(readme.contains(refusal), "the README quotes `{refusal}`");
    }
    drop(narrow);
}
