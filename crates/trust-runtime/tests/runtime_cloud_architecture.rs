use std::fs;
use std::path::PathBuf;

fn read_source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).expect("read source")
}

fn marker_index(source_path: &str, source_text: &str, marker: &str) -> usize {
    source_text
        .find(marker)
        .unwrap_or_else(|| panic!("{source_path} should contain marker '{marker}'"))
}

fn assert_marker_before(source_path: &str, source_text: &str, earlier: &str, later: &str) {
    let earlier_idx = marker_index(source_path, source_text, earlier);
    let later_idx = marker_index(source_path, source_text, later);
    assert!(
        earlier_idx < later_idx,
        "{source_path} should keep '{earlier}' before '{later}'"
    );
}

#[test]
fn runtime_cloud_core_modules_do_not_import_transport_layers() {
    let sources = [
        "src/runtime_cloud/config_policy.rs",
        "src/runtime_cloud/control_proxy_policy.rs",
        "src/runtime_cloud/contracts.rs",
        "src/runtime_cloud/ha.rs",
        "src/runtime_cloud/io_proxy_policy.rs",
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
fn runtime_cloud_proxy_routes_are_policy_first_adapters() {
    let source_path = "src/web/runtime_cloud_routes/actions.rs";
    let source_text = read_source(source_path);
    let dispatch_idx = marker_index(source_path, &source_text, "fn handle_post_dispatch");
    let dispatch_section = &source_text[dispatch_idx..];
    assert_marker_before(
        source_path,
        dispatch_section,
        "runtime_cloud_preflight_for_action(",
        "map_action_to_control_request(&action)",
    );
    assert_marker_before(
        source_path,
        dispatch_section,
        "map_action_to_control_request(&action)",
        "dispatch_control_request(",
    );

    let source_path = "src/web/runtime_cloud_routes/control_proxy.rs";
    let source_text = read_source(source_path);
    let control_proxy_idx = marker_index(source_path, &source_text, "fn handle_post_control_proxy");
    let control_proxy_section = &source_text[control_proxy_idx..];
    assert_marker_before(
        source_path,
        control_proxy_section,
        "runtime_cloud_control_proxy_plan(",
        "runtime_cloud_preflight_for_action(",
    );
    assert_marker_before(
        source_path,
        control_proxy_section,
        "runtime_cloud_preflight_for_action(",
        "dispatch_control_request(",
    );

    let source_path = "src/web/runtime_cloud_routes/io_proxy.rs";
    let source_text = read_source(source_path);
    let get_idx = marker_index(source_path, &source_text, "fn handle_get_io_config");
    let get_section = &source_text[get_idx..];
    assert_marker_before(
        source_path,
        get_section,
        "runtime_cloud_io_proxy_plan(",
        "runtime_cloud_io_preflight(",
    );
    assert_marker_before(
        source_path,
        get_section,
        "runtime_cloud_io_preflight(",
        "load_io_config(",
    );

    let post_idx = marker_index(source_path, &source_text, "fn handle_post_io_config");
    let post_section = &source_text[post_idx..];
    assert_marker_before(
        source_path,
        post_section,
        "runtime_cloud_io_proxy_plan(",
        "runtime_cloud_io_preflight(",
    );
    assert_marker_before(
        source_path,
        post_section,
        "runtime_cloud_io_preflight(",
        "save_io_config(",
    );
}

#[test]
fn runtime_cloud_state_adapters_delegate_domain_state_to_policy_modules() {
    let cases = [
        (
            "src/web/runtime_cloud_state/config.rs",
            &[
                "config_policy::runtime_cloud_config_initial_state(",
                "config_policy::runtime_cloud_config_snapshot(",
                "config_policy::runtime_cloud_config_write_desired(",
                "config_policy::runtime_cloud_config_prepare_reconcile(",
                "config_policy::runtime_cloud_config_apply_success(",
                "config_policy::runtime_cloud_config_apply_failure(",
            ][..],
        ),
        (
            "src/web/runtime_cloud_state/links.rs",
            &[
                "link_policy::runtime_cloud_link_transport_for(",
                "link_policy::runtime_cloud_set_link_transport(",
                "link_policy::runtime_cloud_apply_link_transport_preferences(",
                "link_policy::runtime_cloud_addresses_share_host(",
                "link_policy::runtime_cloud_compute_host_groups(",
                "link_policy::runtime_cloud_topology_feature_flags(",
            ][..],
        ),
        (
            "src/web/runtime_cloud_state/rollouts.rs",
            &[
                "rollout_policy::runtime_cloud_rollouts_snapshot(",
                "rollout_policy::runtime_cloud_rollout_create(",
                "rollout_policy::runtime_cloud_rollout_apply_action(",
                "rollout_policy::runtime_cloud_rollouts_reconcile_once(",
            ][..],
        ),
    ];

    for (source_path, required_markers) in cases {
        let source_text = read_source(source_path);
        for marker in required_markers {
            assert!(
                source_text.contains(marker),
                "{source_path} should delegate domain state behavior through '{marker}'"
            );
        }
    }
}

#[test]
fn realtime_t0_hot_path_keeps_mesh_apis_and_key_parsing_out_of_band() {
    let source_path = "src/host/realtime/transport.rs";
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
