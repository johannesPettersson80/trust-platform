use std::fs;
use std::path::PathBuf;

fn read_source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).expect("read source")
}

#[test]
fn runtime_cloud_core_modules_do_not_import_transport_layers() {
    let sources = [
        "src/runtime_cloud/config_policy.rs",
        "src/runtime_cloud/control_proxy_policy.rs",
        "src/runtime_cloud/contracts.rs",
        "src/runtime_cloud/ha.rs",
        "src/runtime_cloud/link_policy.rs",
        "src/runtime_cloud/profile_policy.rs",
        "src/runtime_cloud/projection.rs",
        "src/runtime_cloud/rollout_policy.rs",
        "src/runtime_cloud/routing.rs",
    ];
    let forbidden = [
        "crate::web",
        "crate::discovery",
        "crate::mesh",
        "crate::runtime::mesh",
    ];

    for source in sources {
        let text = read_source(source);
        for pattern in forbidden {
            assert!(
                !text.contains(pattern),
                "{source} must not import transport/runtime module '{pattern}'"
            );
        }
    }
}

#[test]
fn runtime_cloud_dispatch_route_uses_contract_preflight_before_dispatch_mapping() {
    let source_path = "src/web/runtime_cloud_routes/actions.rs";
    let source_text = read_source(source_path);
    let dispatch_marker = "fn handle_post_dispatch";
    let dispatch_idx = source_text
        .find(dispatch_marker)
        .expect("dispatch route should exist");
    let dispatch_section = &source_text[dispatch_idx..];
    let preflight_idx = dispatch_section
        .find("runtime_cloud_preflight_for_action(")
        .unwrap_or_else(|| panic!("{source_path} dispatch route should run preflight helper"));
    let mapper_idx = dispatch_section
        .find("map_action_to_control_request(&action)")
        .unwrap_or_else(|| {
            panic!("{source_path} dispatch route should map actions through contract mapper")
        });
    assert!(
        preflight_idx < mapper_idx,
        "dispatch route must run preflight before control request mapping"
    );
}

#[test]
fn realtime_t0_hot_path_keeps_mesh_apis_and_key_parsing_out_of_band() {
    let source_path = "src/realtime/transport.rs";
    let source_text = read_source(source_path);
    let forbidden = [
        "crate::mesh",
        "crate::discovery",
        "zenoh",
        "mdns",
        "keyexpr",
        "split('/')",
        "split(\"/\")",
    ];
    for pattern in forbidden {
        assert!(
            !source_text.contains(pattern),
            "{source_path} must not depend on mesh/discovery or key parsing pattern '{pattern}'"
        );
    }

    assert!(
        source_text.contains("generic IP mesh is non-HardRT"),
        "{source_path} should expose deterministic diagnostics for non-HardRT mesh routes"
    );
}
