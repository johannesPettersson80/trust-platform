use super::fields::{ads_fields, ads_server_fields};
use super::{runtime_protocol_schema, CommProtocolSchema};

pub(super) fn ads_server_protocol_schema() -> CommProtocolSchema {
    runtime_protocol_schema(
        "ads_server",
        "Share over ADS",
        "Let ADS clients read selected truST values.",
        "supervisory_service",
        ads_server_fields(),
    )
}

pub(super) fn ads_protocol_schema() -> CommProtocolSchema {
    CommProtocolSchema {
        id: "ads",
        driver: "",
        title: "Read from ADS",
        purpose: "Read selected variables from another ADS device.",
        availability: "default",
        category: "peer_link",
        categories: vec!["peer_link"],
        config_home: "ads.toml",
        apply_mode: "file",
        lifecycle_effect: "restart_required",
        supports_test: true,
        supports_multi_instance: true,
        actions: vec![
            "add",
            "edit",
            "upsert",
            "remove",
            "disable",
            "discover",
            "browse_symbols",
            "doctor",
            "route_script",
        ],
        fields: ads_fields(),
        instances: Vec::new(),
    }
}
