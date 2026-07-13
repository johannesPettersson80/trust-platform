use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_REGISTRY_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConfiguredRuntime {
    pub(super) name: String,
    pub(super) ams_net_id: String,
}

#[derive(Debug, Default)]
pub(super) struct RuntimeRegistryScan {
    pub(super) runtimes: Vec<ConfiguredRuntime>,
    pub(super) warnings: Vec<String>,
}

#[cfg(windows)]
pub(super) fn scan_installed_runtimes() -> RuntimeRegistryScan {
    let mut warnings = Vec::new();
    let mut roots = match trust_ads_windows::trusted_program_data_root() {
        Ok(root) => vec![root],
        Err(error) => {
            warnings.push(format!(
                "resolve Windows ProgramData known folder for local ADS runtimes: {error}"
            ));
            vec![PathBuf::from(r"C:\ProgramData")]
        }
    };
    roots.sort();
    roots.dedup();
    let mut scan = scan_program_data_roots(&roots);
    warnings.append(&mut scan.warnings);
    scan.warnings = warnings;
    scan
}

fn scan_program_data_roots(roots: &[PathBuf]) -> RuntimeRegistryScan {
    let mut scan = RuntimeRegistryScan::default();
    let mut seen_registry_paths = BTreeSet::new();
    for root in roots {
        for (name, path) in registry_candidates(root, &mut scan.warnings) {
            if !seen_registry_paths.insert(path.clone()) {
                continue;
            }
            match read_runtime_net_id(&path) {
                Ok(ams_net_id) => scan.runtimes.push(ConfiguredRuntime { name, ams_net_id }),
                Err(error) => scan.warnings.push(format!("{}: {error}", path.display())),
            }
        }
    }
    scan
}

fn registry_candidates(program_data: &Path, warnings: &mut Vec<String>) -> Vec<(String, PathBuf)> {
    let twincat = program_data.join("Beckhoff").join("TwinCAT").join("3.1");
    let mut candidates = Vec::new();

    let runtimes = twincat.join("Runtimes");
    let entries = match fs::read_dir(&runtimes) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return candidates,
        Err(error) => {
            warnings.push(format!("{}: {error}", runtimes.display()));
            return candidates;
        }
    };
    let mut instance_paths = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|kind| kind.is_dir())
                .map(|_| entry.path())
        })
        .collect::<Vec<_>>();
    instance_paths.sort();
    for instance in instance_paths {
        let registry = instance.join("3.1").join("TcRegistry.xml");
        if !registry.is_file() {
            continue;
        }
        let name = instance
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("TwinCAT Usermode Runtime")
            .to_string();
        candidates.push((name, registry));
    }
    candidates
}

fn read_runtime_net_id(path: &Path) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|error| format!("open TcRegistry.xml: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("inspect TcRegistry.xml: {error}"))?;
    if metadata.len() > MAX_REGISTRY_BYTES as u64 {
        return Err(format!(
            "TcRegistry.xml is {} bytes; limit is {MAX_REGISTRY_BYTES}",
            metadata.len()
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take((MAX_REGISTRY_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read TcRegistry.xml: {error}"))?;
    if bytes.len() > MAX_REGISTRY_BYTES {
        return Err(format!(
            "TcRegistry.xml exceeded the {MAX_REGISTRY_BYTES}-byte limit while reading"
        ));
    }
    let xml = decode_xml(&bytes)?;
    parse_runtime_net_id(&xml)
}

fn decode_xml(bytes: &[u8]) -> Result<String, String> {
    if let Some(bytes) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(bytes.to_vec())
            .map_err(|error| format!("decode UTF-8 TcRegistry.xml: {error}"));
    }
    if let Some(bytes) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16(bytes, true);
    }
    if let Some(bytes) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16(bytes, false);
    }
    String::from_utf8(bytes.to_vec())
        .map_err(|error| format!("decode UTF-8 TcRegistry.xml: {error}"))
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<String, String> {
    if !bytes.len().is_multiple_of(2) {
        return Err("UTF-16 TcRegistry.xml has an odd byte length".to_string());
    }
    let words = bytes
        .chunks_exact(2)
        .map(|pair| {
            if little_endian {
                u16::from_le_bytes([pair[0], pair[1]])
            } else {
                u16::from_be_bytes([pair[0], pair[1]])
            }
        })
        .collect::<Vec<_>>();
    String::from_utf16(&words).map_err(|error| format!("decode UTF-16 TcRegistry.xml: {error}"))
}

fn parse_runtime_net_id(xml: &str) -> Result<String, String> {
    let document = roxmltree::Document::parse(xml)
        .map_err(|error| format!("parse TcRegistry.xml: {error}"))?;
    let value = document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "Value")
        .find(|node| {
            node.attribute("Name")
                .is_some_and(|name| name.eq_ignore_ascii_case("AmsNetId"))
                && node
                    .attribute("Type")
                    .is_none_or(|kind| kind.eq_ignore_ascii_case("BIN"))
                && node.ancestors().any(|ancestor| {
                    ancestor.is_element()
                        && ancestor.tag_name().name() == "Key"
                        && ancestor
                            .attribute("Name")
                            .is_some_and(|name| name.eq_ignore_ascii_case("System"))
                })
        })
        .and_then(|node| node.text())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "System/AmsNetId BIN value is missing".to_string())?;
    net_id_from_registry_bin(value)
}

fn net_id_from_registry_bin(value: &str) -> Result<String, String> {
    let compact = value
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect::<String>();
    if compact.len() != 12 || !compact.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "AmsNetId BIN must contain exactly 12 hexadecimal digits, got '{value}'"
        ));
    }
    (0..6)
        .map(|index| {
            u8::from_str_radix(&compact[index * 2..index * 2 + 2], 16)
                .map(|octet| octet.to_string())
                .map_err(|error| format!("decode AmsNetId BIN: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|octets| octets.join("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPRESENTATIVE_REGISTRY: &str = r#"<?xml version="1.0"?>
<TcRegistry>
  <Key Name="HKLM"><Key Name="Software"><Key Name="Beckhoff"><Key Name="TwinCAT3">
    <Value Name="CurrentVersion" Type="SZ">3.1</Value>
    <Key Name="System">
      <Value Name="RunAsDevice" Type="DW">1</Value>
      <Value Name="AmsNetId" Type="BIN">0A141E280101</Value>
    </Key>
  </Key></Key></Key></Key>
</TcRegistry>"#;

    #[test]
    fn parses_usermode_runtime_target_net_id_from_representative_registry() {
        assert_eq!(
            parse_runtime_net_id(REPRESENTATIVE_REGISTRY).as_deref(),
            Ok("10.20.30.40.1.1")
        );
        assert_eq!(
            net_id_from_registry_bin("C0 A8 04 01 01 01").as_deref(),
            Ok("192.168.4.1.1.1")
        );
    }

    #[test]
    fn rejects_missing_or_malformed_runtime_target_identity() {
        assert!(parse_runtime_net_id("<TcRegistry />")
            .expect_err("missing identity")
            .contains("missing"));
        assert!(net_id_from_registry_bin("10.20.30.40.1.1").is_err());
        assert!(net_id_from_registry_bin("0A141E28010Z").is_err());
    }

    #[test]
    fn enumerates_each_program_data_runtime_and_preserves_duplicate_configured_ids() {
        let root = std::env::temp_dir().join(format!(
            "trust-ads-runtime-registry-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        let base = root.join("Beckhoff").join("TwinCAT").join("3.1");
        let default_dir = base.join("Runtimes").join("UmRT_Default").join("3.1");
        let second_dir = base.join("Runtimes").join("UmRT_Test").join("3.1");
        fs::create_dir_all(&default_dir).expect("create default runtime fixture");
        fs::create_dir_all(&second_dir).expect("create second runtime fixture");
        fs::write(default_dir.join("TcRegistry.xml"), REPRESENTATIVE_REGISTRY)
            .expect("write default registry");
        fs::write(second_dir.join("TcRegistry.xml"), REPRESENTATIVE_REGISTRY)
            .expect("write duplicate registry");

        let scan = scan_program_data_roots(std::slice::from_ref(&root));

        assert!(scan.warnings.is_empty(), "warnings: {:?}", scan.warnings);
        assert_eq!(
            scan.runtimes,
            vec![
                ConfiguredRuntime {
                    name: "UmRT_Default".to_string(),
                    ams_net_id: "10.20.30.40.1.1".to_string(),
                },
                ConfiguredRuntime {
                    name: "UmRT_Test".to_string(),
                    ams_net_id: "10.20.30.40.1.1".to_string(),
                },
            ]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn decodes_utf16_little_endian_registry_files() {
        let mut encoded = vec![0xFF, 0xFE];
        for word in REPRESENTATIVE_REGISTRY.encode_utf16() {
            encoded.extend_from_slice(&word.to_le_bytes());
        }
        let xml = decode_xml(&encoded).expect("decode UTF-16 registry");
        assert_eq!(parse_runtime_net_id(&xml).as_deref(), Ok("10.20.30.40.1.1"));
    }

    #[test]
    fn rejects_oversized_registry_before_unbounded_read() {
        let root = std::env::temp_dir().join(format!(
            "trust-ads-oversized-registry-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        fs::write(&root, vec![b'x'; MAX_REGISTRY_BYTES + 1]).expect("write oversized fixture");

        let error = read_runtime_net_id(&root).expect_err("oversized registry must be rejected");

        assert!(error.contains("limit"));
        let _ = fs::remove_file(root);
    }
}
