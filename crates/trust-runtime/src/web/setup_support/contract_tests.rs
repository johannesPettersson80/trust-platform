use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-web-setup-{}-{label}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).expect("create project");
        Self { root }
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, text).expect("write fixture");
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn request(
    driver: Option<&str>,
    params: Option<serde_json::Value>,
    drivers: Option<Vec<IoDriverConfigRequest>>,
) -> IoConfigRequest {
    IoConfigRequest {
        driver: driver.map(ToOwned::to_owned),
        params,
        drivers,
        safe_state: None,
        use_system_io: None,
    }
}

fn driver(name: &str, params: Option<serde_json::Value>) -> IoDriverConfigRequest {
    IoDriverConfigRequest {
        name: name.to_string(),
        params,
    }
}

#[test]
fn explicit_bundle_root_is_preserved() {
    let root = PathBuf::from("/tmp/trust-explicit-project");
    assert_eq!(default_bundle_root(&Some(root.clone())), root);
}

#[test]
fn default_resource_name_uses_final_component_and_sanitizes_punctuation() {
    assert_eq!(
        default_resource_name(Path::new("/tmp/Plant A-17")).as_str(),
        "Plant_A_17"
    );
    assert_eq!(default_resource_name(Path::new("/")).as_str(), "trust-plc");
}

#[test]
fn json_to_toml_preserves_supported_scalar_shapes() {
    assert_eq!(
        json_to_toml(&Value::Null),
        toml::Value::String(String::new())
    );
    assert_eq!(json_to_toml(&json!(true)), toml::Value::Boolean(true));
    assert_eq!(json_to_toml(&json!(-17)), toml::Value::Integer(-17));
    assert_eq!(json_to_toml(&json!(17)), toml::Value::Integer(17));
    assert_eq!(json_to_toml(&json!(1.5)), toml::Value::Float(1.5));
    assert_eq!(
        json_to_toml(&json!("value")),
        toml::Value::String("value".to_string())
    );
}

#[test]
fn json_to_toml_recurses_through_arrays_and_objects() {
    let converted = json_to_toml(&json!({
        "enabled": true,
        "pins": [1, 2],
        "nested": {"label": "A"}
    }));
    assert_eq!(converted["enabled"], toml::Value::Boolean(true));
    assert_eq!(
        converted["pins"],
        toml::Value::Array(vec![toml::Value::Integer(1), toml::Value::Integer(2)])
    );
    assert_eq!(
        converted["nested"]["label"],
        toml::Value::String("A".to_string())
    );
}

#[test]
fn explicit_driver_list_takes_precedence_over_legacy_driver() {
    let payload = request(
        Some("legacy"),
        Some(json!({"ignored": true})),
        Some(vec![driver("first", None), driver("second", None)]),
    );
    let configs = driver_configs_from_payload(&payload).expect("drivers");
    assert_eq!(
        configs
            .iter()
            .map(|config| config.name.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
}

#[test]
fn empty_driver_list_is_rejected_even_when_legacy_driver_exists() {
    let payload = request(Some("legacy"), None, Some(Vec::new()));
    assert!(driver_configs_from_payload(&payload)
        .unwrap_err()
        .to_string()
        .contains("io.drivers must contain at least one driver"));
}

#[test]
fn driver_names_are_trimmed_and_all_entries_are_enabled() {
    let payload = request(
        None,
        None,
        Some(vec![
            driver("  loopback  ", None),
            driver(" simulated ", None),
        ]),
    );
    let configs = driver_configs_from_payload(&payload).expect("drivers");
    assert_eq!(configs[0].name.as_str(), "loopback");
    assert_eq!(configs[1].name.as_str(), "simulated");
    assert!(configs.iter().all(|config| config.enabled));
}

#[test]
fn blank_driver_name_is_rejected_with_its_index() {
    let payload = request(
        None,
        None,
        Some(vec![driver("ok", None), driver(" \t ", None)]),
    );
    let error = driver_configs_from_payload(&payload).expect_err("blank name");
    assert!(error.to_string().contains("io.drivers[1].name"));
}

#[test]
fn omitted_driver_params_default_to_empty_table() {
    let payload = request(None, None, Some(vec![driver("loopback", None)]));
    let configs = driver_configs_from_payload(&payload).expect("drivers");
    assert!(configs[0].params.as_table().expect("table").is_empty());
}

#[test]
fn driver_params_preserve_nested_object() {
    let payload = request(
        None,
        None,
        Some(vec![driver(
            "simulated",
            Some(json!({"mode": "sine", "nested": {"rate": 2}})),
        )]),
    );
    let configs = driver_configs_from_payload(&payload).expect("drivers");
    assert_eq!(configs[0].params["mode"].as_str(), Some("sine"));
    assert_eq!(configs[0].params["nested"]["rate"].as_integer(), Some(2));
}

#[test]
fn non_object_driver_params_are_rejected() {
    for params in [json!(null), json!(17), json!("text"), json!([1, 2])] {
        let payload = request(None, None, Some(vec![driver("loopback", Some(params))]));
        assert!(driver_configs_from_payload(&payload).is_err());
    }
}

#[test]
fn legacy_driver_requires_trimmed_nonempty_name() {
    assert!(driver_configs_from_payload(&request(None, None, None)).is_err());
    assert!(driver_configs_from_payload(&request(Some(" \t "), None, None)).is_err());
    let configs =
        driver_configs_from_payload(&request(Some(" loopback "), None, None)).expect("driver");
    assert_eq!(configs[0].name.as_str(), "loopback");
}

#[test]
fn legacy_driver_params_must_be_object() {
    assert!(
        driver_configs_from_payload(&request(Some("loopback"), Some(json!([1, 2])), None)).is_err()
    );
}

#[test]
fn io_toml_renderer_preserves_driver_and_safe_state_order() {
    let configs = driver_configs_from_payload(&request(
        None,
        None,
        Some(vec![driver("loopback", None), driver("simulated", None)]),
    ))
    .expect("drivers");
    let text = render_io_toml(
        configs,
        vec![
            IoSafeStateEntry {
                address: "%QX0.0".to_string(),
                value: "FALSE".to_string(),
            },
            IoSafeStateEntry {
                address: "%QW2".to_string(),
                value: "17".to_string(),
            },
        ],
    );
    assert!(text.find("loopback").expect("loopback") < text.find("simulated").expect("simulated"));
    assert!(text.find("%QX0.0").expect("first safe") < text.find("%QW2").expect("second safe"));
    crate::config::validate_io_toml_text(&text).expect("valid rendered I/O");
}

#[test]
fn io_address_projection_covers_areas_sizes_bits_and_wildcards() {
    for (raw, expected) in [
        ("%IX0.7", "%IX0.7"),
        ("%QB1", "%QB1"),
        ("%MW2", "%MW2"),
        ("%ID3", "%ID3"),
        ("%QL4", "%QL4"),
        ("%Q*", "%Q*"),
    ] {
        let address = IoAddress::parse(raw).expect(raw);
        assert_eq!(format_io_address(&address), expected, "{raw}");
    }
}

#[test]
fn safe_state_value_projection_covers_boolean_integer_and_bit_string_widths() {
    let cases = [
        (crate::value::Value::Bool(false), "FALSE"),
        (crate::value::Value::Bool(true), "TRUE"),
        (crate::value::Value::SInt(-1), "-1"),
        (crate::value::Value::Int(-2), "-2"),
        (crate::value::Value::DInt(-3), "-3"),
        (crate::value::Value::LInt(-4), "-4"),
        (crate::value::Value::USInt(1), "1"),
        (crate::value::Value::UInt(2), "2"),
        (crate::value::Value::UDInt(3), "3"),
        (crate::value::Value::ULInt(4), "4"),
        (crate::value::Value::Byte(255), "255"),
        (crate::value::Value::Word(65_535), "65535"),
        (crate::value::Value::DWord(u32::MAX), "4294967295"),
        (crate::value::Value::LWord(u64::MAX), "18446744073709551615"),
    ];
    for (value, expected) in cases {
        assert_eq!(format_io_safe_state_value(&value), expected);
    }
}

#[test]
fn io_response_uses_first_driver_as_legacy_projection() {
    let text = render_io_toml(
        driver_configs_from_payload(&request(
            None,
            None,
            Some(vec![
                driver("loopback", Some(json!({"first": true}))),
                driver("simulated", Some(json!({"second": true}))),
            ]),
        ))
        .expect("drivers"),
        Vec::new(),
    );
    let fixture = TempProject::new("response");
    fixture.write("io.toml", &text);
    let config = IoConfig::load(fixture.root.join("io.toml")).expect("I/O config");
    let response = io_config_to_response(config, "project", false);
    assert_eq!(response.driver, "loopback");
    assert_eq!(response.params["first"], true);
    assert_eq!(response.drivers.len(), 2);
    assert_eq!(response.source, "project");
    assert!(!response.use_system_io);
}

#[test]
fn project_io_file_takes_precedence_in_load_response() {
    let fixture = TempProject::new("load-project");
    let text = render_io_toml(
        driver_configs_from_payload(&request(Some("loopback"), None, None)).expect("driver"),
        Vec::new(),
    );
    fixture.write("io.toml", &text);
    let response = load_io_config(&Some(fixture.root.clone())).expect("load project I/O");
    assert_eq!(response.source, "project");
    assert_eq!(response.driver, "loopback");
    assert!(!response.use_system_io);
}

#[test]
fn saving_project_io_writes_validated_configuration() {
    let fixture = TempProject::new("save-project");
    let mut payload = request(None, None, Some(vec![driver("loopback", Some(json!({})))]));
    payload.safe_state = Some(vec![IoSafeStateEntry {
        address: "%QX0.0".to_string(),
        value: "FALSE".to_string(),
    }]);
    let message = save_io_config(&Some(fixture.root.clone()), &payload).expect("save");
    assert!(message.contains("I/O config saved"));
    let text = std::fs::read_to_string(fixture.root.join("io.toml")).expect("read I/O");
    crate::config::validate_io_toml_text(&text).expect("valid I/O");
    assert!(text.contains("%QX0.0"));
}

#[test]
fn selecting_system_io_removes_only_project_io_file() {
    let fixture = TempProject::new("use-system");
    fixture.write("io.toml", "project");
    fixture.write("keep.txt", "keep");
    let mut payload = request(Some("loopback"), None, None);
    payload.use_system_io = Some(true);
    let message = save_io_config(&Some(fixture.root.clone()), &payload).expect("use system");
    assert!(message.contains("Using system I/O config"));
    assert!(!fixture.root.join("io.toml").exists());
    assert_eq!(
        std::fs::read_to_string(fixture.root.join("keep.txt")).expect("keep"),
        "keep"
    );
}

#[test]
fn source_listing_is_sorted_case_insensitive_and_regular_file_only() {
    let fixture = TempProject::new("source-list");
    fixture.write("src/zeta.st", "");
    fixture.write("src/Alpha.ST", "");
    fixture.write("src/ignored.txt", "");
    std::fs::create_dir_all(fixture.root.join("src/fake.st")).expect("fake directory");
    assert_eq!(
        list_sources(&fixture.root),
        vec!["Alpha.ST".to_string(), "zeta.st".to_string()]
    );
}

#[test]
fn missing_source_directory_lists_empty() {
    let fixture = TempProject::new("source-list-missing");
    assert!(list_sources(&fixture.root).is_empty());
}

#[test]
fn source_read_accepts_contained_nested_file() {
    let fixture = TempProject::new("source-read");
    fixture.write("src/nested/main.st", "PROGRAM Main\nEND_PROGRAM\n");
    assert_eq!(
        read_source_file(&fixture.root, "nested/main.st").expect("read source"),
        "PROGRAM Main\nEND_PROGRAM\n"
    );
}

#[test]
fn source_read_rejects_parent_traversal() {
    let fixture = TempProject::new("source-traversal");
    fixture.write("src/main.st", "");
    fixture.write("outside.st", "secret");
    assert!(read_source_file(&fixture.root, "../outside.st").is_err());
}

#[cfg(unix)]
#[test]
fn source_read_rejects_symbolic_escape() {
    let fixture = TempProject::new("source-symlink");
    fixture.write("src/main.st", "");
    fixture.write("outside.st", "secret");
    std::os::unix::fs::symlink(
        fixture.root.join("outside.st"),
        fixture.root.join("src/linked.st"),
    )
    .expect("symlink");
    assert!(read_source_file(&fixture.root, "linked.st").is_err());
}

#[test]
fn hmi_asset_read_accepts_only_contained_lowercase_svg_file() {
    let fixture = TempProject::new("hmi-asset");
    fixture.write("hmi/pump.svg", "<svg></svg>");
    fixture.write("hmi/pump.SVG", "<svg></svg>");
    fixture.write("hmi/script.js", "alert(1)");
    assert_eq!(
        read_hmi_asset_file(&fixture.root, "pump.svg").expect("SVG"),
        "<svg></svg>"
    );
    assert!(read_hmi_asset_file(&fixture.root, "pump.SVG").is_err());
    assert!(read_hmi_asset_file(&fixture.root, "script.js").is_err());
}

#[test]
fn hmi_asset_read_rejects_parent_traversal() {
    let fixture = TempProject::new("hmi-traversal");
    fixture.write("hmi/pump.svg", "");
    fixture.write("outside.svg", "<svg>secret</svg>");
    assert!(read_hmi_asset_file(&fixture.root, "../outside.svg").is_err());
}

#[cfg(unix)]
#[test]
fn hmi_asset_read_rejects_symbolic_escape() {
    let fixture = TempProject::new("hmi-symlink");
    fixture.write("hmi/pump.svg", "");
    fixture.write("outside.svg", "<svg>secret</svg>");
    std::os::unix::fs::symlink(
        fixture.root.join("outside.svg"),
        fixture.root.join("hmi/linked.svg"),
    )
    .expect("symlink");
    assert!(read_hmi_asset_file(&fixture.root, "linked.svg").is_err());
}

#[test]
fn setup_apply_writes_valid_runtime_and_project_io_without_system_mutation() {
    let fixture = TempProject::new("apply");
    let payload = SetupApplyRequest {
        project_path: Some(fixture.root.display().to_string()),
        resource_name: Some("Line_A".to_string()),
        cycle_ms: Some(20),
        driver: Some("loopback".to_string()),
        write_system_io: Some(false),
        overwrite_system_io: Some(false),
        use_system_io: Some(false),
    };
    let message = apply_setup(&Some(fixture.root.clone()), payload).expect("apply setup");
    assert!(message.contains("Setup applied"));
    crate::config::RuntimeConfig::load(fixture.root.join("runtime.toml")).expect("runtime");
    crate::config::IoConfig::load(fixture.root.join("io.toml")).expect("I/O");
}

#[test]
fn setup_apply_system_selection_removes_existing_project_io() {
    let fixture = TempProject::new("apply-system");
    fixture.write("io.toml", "old project I/O");
    let payload = SetupApplyRequest {
        project_path: Some(fixture.root.display().to_string()),
        resource_name: Some("Line_A".to_string()),
        cycle_ms: Some(20),
        driver: Some("loopback".to_string()),
        write_system_io: Some(false),
        overwrite_system_io: Some(false),
        use_system_io: Some(true),
    };
    apply_setup(&Some(fixture.root.clone()), payload).expect("apply setup");
    assert!(!fixture.root.join("io.toml").exists());
    assert!(fixture.root.join("runtime.toml").is_file());
}
