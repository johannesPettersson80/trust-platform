use super::*;

pub(super) fn discovery_health(entry: &DiscoveryEntry) -> String {
    if discovery_is_stale(entry) {
        "runtime_unreachable".to_string()
    } else {
        "connected".to_string()
    }
}

pub(super) fn discovery_is_stale(entry: &DiscoveryEntry) -> bool {
    let seen = discovery_last_seen_ms(entry);
    now_ms().saturating_sub(seen) > DISCOVERY_STALE_AFTER_MS
}

pub(super) fn newest_discovery_seen_ms(entries: &[DiscoveryEntry]) -> Option<u64> {
    entries.iter().map(discovery_last_seen_ms).max()
}

pub(super) fn discovery_last_seen_ms(entry: &DiscoveryEntry) -> u64 {
    entry.last_seen_ns / 1_000_000
}

pub(super) fn same_host_by_discovery(
    runtime_id: &str,
    entry: &DiscoveryEntry,
    entries: &[DiscoveryEntry],
) -> bool {
    if let Some(host_group) = entry.host_group.as_deref() {
        if entries.iter().any(|candidate| {
            candidate.name.as_str() == runtime_id
                && candidate.host_group.as_deref() == Some(host_group)
        }) {
            return true;
        }
    }
    false
}

pub(super) fn mesh_peer_is_live(
    entry: &DiscoveryEntry,
    evidence: Option<&crate::mesh::MeshTopologyEvidence>,
) -> bool {
    let Some(evidence) = evidence else {
        return false;
    };
    evidence.liveliness_snapshot().peers.iter().any(|peer| {
        peer == entry.name.as_str()
            || peer == entry.id.as_str()
            || entry.id.as_str().contains(peer.as_str())
    })
}

pub(super) fn first_address_with_port(entry: &DiscoveryEntry, port: u16) -> Option<String> {
    entry
        .addresses
        .first()
        .map(|address| format!("{address}:{port}"))
}

pub(super) fn discovery_protocols(entry: &DiscoveryEntry) -> Vec<String> {
    let mut protocols = vec!["discovery".to_string()];
    if entry.web_port.is_some() {
        protocols.push("web".to_string());
    }
    if entry.mesh_port.is_some() {
        protocols.push("mesh".to_string());
    }
    if entry.control.is_some() {
        protocols.push("control".to_string());
    }
    protocols
}

pub(super) fn is_self_discovery_entry(runtime_id: &str, entry: &DiscoveryEntry) -> bool {
    entry.name.as_str() == runtime_id || entry.id.as_str() == runtime_id
}

pub(super) fn discovered_runtime_id(entry: &DiscoveryEntry) -> String {
    format!("runtime:{}", sanitize_id(entry.name.as_str()))
}

pub(super) fn discovered_host_id(entry: &DiscoveryEntry) -> String {
    entry.host_group.as_ref().map_or_else(
        || format!("host:discovery:{}", sanitize_id(entry.id.as_str())),
        |host_group| format!("host:{}", sanitize_id(host_group.as_str())),
    )
}

pub(super) fn discovered_external_id(entry: &DiscoveryEntry) -> String {
    format!("external:discovery:{}", sanitize_id(entry.id.as_str()))
}

pub(super) fn shared_mqtt_id(broker: &str) -> String {
    format!("shared:mqtt:{}", sanitize_id(broker))
}

pub(super) fn configured_host_ips(settings: &RuntimeSettings) -> Vec<String> {
    let mut values = [
        settings.web.listen.as_str(),
        settings.opcua.listen.as_str(),
        settings.mesh.listen.as_str(),
    ]
    .iter()
    .filter_map(|value| host_from_listen(value))
    .filter(|host| host != "0.0.0.0" && host != "::" && host != "[::]")
    .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    if values.is_empty() {
        values.push("127.0.0.1".to_string());
    }
    values
}

pub(super) fn host_from_listen(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(stripped) = trimmed.strip_prefix('[') {
        let (host, _) = stripped.split_once(']')?;
        return Some(host.to_string());
    }
    trimmed
        .rsplit_once(':')
        .map(|(host, _)| host.to_string())
        .or_else(|| Some(trimmed.to_string()))
}

pub(super) fn host_name() -> String {
    let env_hostname = std::env::var("HOSTNAME").ok();
    let computer_name = std::env::var("COMPUTERNAME").ok();
    let os_hostname = os_host_name();
    host_name_from_sources(
        env_hostname.as_deref(),
        computer_name.as_deref(),
        os_hostname.as_deref(),
    )
}

pub(super) fn host_name_from_sources(
    env_hostname: Option<&str>,
    computer_name: Option<&str>,
    os_hostname: Option<&str>,
) -> String {
    env_hostname
        .and_then(normalized_host_name)
        .or_else(|| computer_name.and_then(normalized_host_name))
        .or_else(|| os_hostname.and_then(normalized_host_name))
        .unwrap_or_else(|| "local-host".to_string())
}

pub(super) fn os_host_name() -> Option<String> {
    os_host_name_from_files()
        .or_else(os_host_name_from_command)
        .and_then(|value| normalized_host_name(value.as_str()))
}

#[cfg(unix)]
pub(super) fn os_host_name_from_files() -> Option<String> {
    ["/proc/sys/kernel/hostname", "/etc/hostname"]
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
}

#[cfg(not(unix))]
pub(super) fn os_host_name_from_files() -> Option<String> {
    None
}

pub(super) fn os_host_name_from_command() -> Option<String> {
    let output = std::process::Command::new("hostname").output().ok()?;
    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        None
    }
}

pub(super) fn normalized_host_name(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(super) fn stable_host_id(hostname: &str) -> String {
    format!("host:{}", sanitize_id(hostname))
}

pub(super) fn endpoint_id(runtime_id: &str, protocol: &str) -> String {
    format!("endpoint:{runtime_id}:{protocol}")
}

pub(super) fn endpoint_instance_id(runtime_id: &str, protocol: &str, index: usize) -> String {
    if index == 0 {
        endpoint_id(runtime_id, protocol)
    } else {
        format!("endpoint:{runtime_id}:{protocol}:{index}")
    }
}

pub(super) fn protocol_from_driver_name(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "modbus-tcp" | "modbus_tcp" => "modbus_tcp".to_string(),
        other => other.replace('-', "_"),
    }
}

pub(super) fn driver_role(protocol: &str) -> &'static str {
    match protocol {
        "modbus_tcp" => "client",
        "ethercat" => "master",
        "mqtt" => "client",
        _ => "owned_driver",
    }
}

pub(super) fn runtime_cloud_configured(settings: &RuntimeSettings) -> bool {
    !matches!(
        settings.runtime_cloud.profile,
        crate::config::RuntimeCloudProfile::Dev
    ) || !settings.runtime_cloud.link_preferences.is_empty()
        || !settings.runtime_cloud.wan_allow_write.is_empty()
}

pub(super) fn host_board() -> Option<String> {
    [
        "/sys/firmware/devicetree/base/model",
        "/sys/class/dmi/id/product_name",
    ]
    .iter()
    .find_map(read_trimmed)
}

pub(super) fn host_temp_c() -> Option<f64> {
    let entries = fs::read_dir("/sys/class/thermal").ok()?;
    for entry in entries.flatten() {
        let path = entry.path().join("temp");
        let Some(raw) = read_trimmed(path.as_path()) else {
            continue;
        };
        let Ok(value) = raw.parse::<f64>() else {
            continue;
        };
        if value > 1_000.0 {
            return Some((value / 1_000.0 * 10.0).round() / 10.0);
        }
        return Some(value);
    }
    None
}

pub(super) fn host_uptime_s() -> Option<u64> {
    let text = read_trimmed("/proc/uptime")?;
    text.split_whitespace()
        .next()
        .and_then(|value| value.parse::<f64>().ok())
        .map(|value| value as u64)
}

pub(super) fn host_load() -> Option<f64> {
    let text = read_trimmed("/proc/loadavg")?;
    text.split_whitespace()
        .next()
        .and_then(|value| value.parse::<f64>().ok())
}

pub(super) fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim_matches(char::from(0)).trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn looks_containerized(cgroup: &str) -> bool {
    cgroup.contains("docker")
        || cgroup.contains("kubepods")
        || cgroup.contains("containerd")
        || cgroup.contains("libpod")
}

pub(super) fn parse_container_id(cgroup: &str) -> Option<String> {
    cgroup.split(['/', ':', '\n']).rev().find_map(|part| {
        let trimmed = part.trim();
        if trimmed.len() >= 12 && trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
            Some(trimmed.chars().take(12).collect())
        } else {
            trimmed
                .strip_prefix("docker-")
                .and_then(|value| value.split('.').next())
                .filter(|value| value.len() >= 12)
                .map(|value| value.chars().take(12).collect())
        }
    })
}

pub(super) fn sanitize_id(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches('-');
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized.to_string()
    }
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
