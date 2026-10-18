//! The announcement contracts of runtime discovery (specification 11, runtime discovery; design
//! faults E6, E7, E8, E9 and E10). Every expected value is a literal; nothing here starts mDNS.

use std::path::PathBuf;

use super::*;
use crate::control::fleet_handlers::host::host_name_from_sources;
use crate::runtime_id;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create temp dir");
    root
}

/// R1 (E6): `discovery.interfaces` limits both announce and browse to the named interfaces, and an
/// empty list uses every non-loopback interface (specification 11: "`interfaces` limits announcing
/// and browsing to the named network interfaces; an empty list uses every interface").
#[test]
fn announce_and_browse_only_on_named_interfaces() {
    assert_eq!(
        interface_selection(&[SmolStr::new("eth0"), SmolStr::new("wlan0")]),
        InterfaceSelection::Named(vec!["eth0".to_string(), "wlan0".to_string()]),
        "the named interfaces are used, in the order the configuration names them"
    );
    assert_eq!(
        interface_selection(&[]),
        InterfaceSelection::AllNonLoopback,
        "an empty list uses every non-loopback interface"
    );
}

/// R2 (E7): the TXT record carries `version` (the truST version), `platform` and `auth`, which is
/// `token` while the control endpoint needs a token (specification 11: "The TXT record also
/// carries `version` ... `platform` ... and `auth` (`token` when control needs a token, otherwise
/// `none`)").
#[test]
fn txt_record_carries_version_platform_auth() {
    let tcp = ControlEndpoint::parse("tcp://127.0.0.1:9000").expect("tcp endpoint");
    let props = announcement_properties(&Announcement {
        runtime_name: "runtime-a",
        runtime_id: "id-1234",
        version: "1.2.3",
        platform: "stm32f407",
        control_endpoint: Some(&tcp),
        web_listen: Some("0.0.0.0:8080"),
        web_tls: false,
        mesh_listen: None,
        host_group: None,
    });
    assert_eq!(props.get("version").map(String::as_str), Some("1.2.3"));
    assert_eq!(props.get("platform").map(String::as_str), Some("stm32f407"));
    assert_eq!(props.get("auth").map(String::as_str), Some("token"));
    assert_eq!(props.get("id").map(String::as_str), Some("id-1234"));
}

/// R3 (E8): the mDNS host name comes from `host_name()` and so from the operating system's host
/// name, never only from `$HOSTNAME` (specification 11: "The host name is the operating system's
/// host name"). The fault returned `trust` when `$HOSTNAME` was unset.
#[test]
fn mdns_host_name_is_the_os_host_name() {
    assert_eq!(
        host_name_from_sources(None, None, Some("plc-a-a1")),
        "plc-a-a1"
    );
    assert_eq!(
        host_name_from_sources(Some(""), Some(" "), Some("plc-a-a1")),
        "plc-a-a1",
        "an empty environment host name falls through to the operating system's"
    );
}

/// R4 (E9, O36): the stable runtime ID is generated once and read back on every start, so two
/// starts on one project directory give one ID (specification 11: "The `id` stays the same across
/// restarts").
#[test]
fn discovery_id_is_stable_across_restarts() {
    let project = unique_temp_dir("trust-runtime-id-stable");
    let first = runtime_id::stable_runtime_id(project.as_path()).expect("first start");
    let second = runtime_id::stable_runtime_id(project.as_path()).expect("second start");
    assert_eq!(first, second);
    // 16 random bytes in lower-case hex, never a process ID.
    assert_eq!(
        first.len(),
        32,
        "the ID is the random part, not a process ID"
    );
    std::fs::remove_dir_all(project).ok();
}

/// R4 (O36): two runtimes on one host keep their own ID, because each project keeps its own file
/// (specification 11: "differs for two runtimes on one host").
#[test]
fn two_projects_on_one_host_have_different_runtime_ids() {
    let first_project = unique_temp_dir("trust-runtime-id-a");
    let second_project = unique_temp_dir("trust-runtime-id-b");
    let first = runtime_id::stable_runtime_id(first_project.as_path()).expect("first project");
    let second = runtime_id::stable_runtime_id(second_project.as_path()).expect("second project");
    assert_ne!(first, second);
    std::fs::remove_dir_all(first_project).ok();
    std::fs::remove_dir_all(second_project).ok();
}

/// R4 (O36): a file that holds an invalid ID is an error and is never silently replaced with a
/// fresh ID.
#[test]
fn invalid_runtime_id_file_is_an_error_not_replaced() {
    let project = unique_temp_dir("trust-runtime-id-invalid");
    let path = runtime_id::runtime_id_path(project.as_path());
    std::fs::create_dir_all(path.parent().expect("state folder")).expect("state folder");
    std::fs::write(&path, "not a valid id!").expect("write invalid id");
    let error =
        runtime_id::stable_runtime_id(project.as_path()).expect_err("an invalid ID must fail");
    assert!(
        error
            .to_string()
            .contains("does not hold a valid runtime ID"),
        "{error}"
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("read back"),
        "not a valid id!",
        "the invalid file is left as it is"
    );
    std::fs::remove_dir_all(project).ok();
}

/// R5 (E10): an announcement never offers a `unix://` control endpoint, which only works on that
/// computer, and it omits `web_port` and `web_tls` while the web server is disabled
/// (specification 11: "`control` names only an endpoint that another computer can use ...; Web
/// port and TLS flag are announced only while the web server is enabled").
#[test]
fn announcement_omits_local_socket_control_and_disabled_web() {
    #[cfg(unix)]
    {
        let unix = ControlEndpoint::parse("unix:///tmp/trust.sock").expect("unix endpoint");
        let props = announcement_properties(&Announcement {
            runtime_name: "runtime-a",
            runtime_id: "id-1234",
            version: "1.2.3",
            platform: "linux-x86_64",
            control_endpoint: Some(&unix),
            web_listen: None,
            web_tls: false,
            mesh_listen: Some("0.0.0.0:5200"),
            host_group: None,
        });
        assert_eq!(
            props.get("control"),
            None,
            "a local socket is not announced"
        );
        assert_eq!(
            props.get("web_port"),
            None,
            "a disabled web server has no port"
        );
        assert_eq!(props.get("web_tls"), None);
        assert_eq!(props.get("auth").map(String::as_str), Some("none"));
        assert_eq!(props.get("mesh_port").map(String::as_str), Some("5200"));
    }

    let tcp = ControlEndpoint::parse("tcp://127.0.0.1:9000").expect("tcp endpoint");
    let props = announcement_properties(&Announcement {
        runtime_name: "runtime-a",
        runtime_id: "id-1234",
        version: "1.2.3",
        platform: "linux-x86_64",
        control_endpoint: Some(&tcp),
        web_listen: Some("0.0.0.0:8080"),
        web_tls: true,
        mesh_listen: None,
        host_group: None,
    });
    assert_eq!(
        props.get("control").map(String::as_str),
        Some("tcp://127.0.0.1:9000")
    );
    assert_eq!(props.get("web_port").map(String::as_str), Some("8080"));
    assert_eq!(props.get("web_tls").map(String::as_str), Some("true"));
}
