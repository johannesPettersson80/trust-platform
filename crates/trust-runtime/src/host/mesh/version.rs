use crate::config::{MeshConfig, MeshRole};
use crate::error::RuntimeError;

pub const ZENOH_BASELINE_VERSION: &str = "1.7.2";
pub const ZENOHD_BASELINE_VERSION: &str = "1.7.2";

pub fn validate_zenoh_version_policy(config: &MeshConfig) -> Result<(), RuntimeError> {
    let baseline = parse_version_family(ZENOH_BASELINE_VERSION)?;
    let zenohd = parse_version_family(config.zenohd_version.as_str())?;
    if baseline != zenohd {
        return Err(RuntimeError::InvalidConfig(
            format!(
                "runtime.mesh.zenohd_version '{}' is not compatible with baseline '{}'",
                config.zenohd_version, ZENOH_BASELINE_VERSION
            )
            .into(),
        ));
    }

    if config.role == MeshRole::Router && config.plugin_versions.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.mesh.plugin_versions must enumerate deployed plugins for router role".into(),
        ));
    }

    for (plugin, version) in &config.plugin_versions {
        let family = parse_version_family(version.as_str())?;
        if family != baseline {
            return Err(RuntimeError::InvalidConfig(
                format!(
                    "runtime.mesh.plugin_versions['{}'] '{}' is not compatible with baseline '{}'",
                    plugin, version, ZENOH_BASELINE_VERSION
                )
                .into(),
            ));
        }
    }

    Ok(())
}

fn parse_version_family(version: &str) -> Result<(u16, u16), RuntimeError> {
    let trimmed = version.trim();
    let mut parts = trimmed.split('.');
    let major = parts
        .next()
        .ok_or_else(|| RuntimeError::InvalidConfig(format!("invalid version '{trimmed}'").into()))?
        .parse::<u16>()
        .map_err(|_| RuntimeError::InvalidConfig(format!("invalid version '{trimmed}'").into()))?;
    let minor = parts
        .next()
        .ok_or_else(|| RuntimeError::InvalidConfig(format!("invalid version '{trimmed}'").into()))?
        .parse::<u16>()
        .map_err(|_| RuntimeError::InvalidConfig(format!("invalid version '{trimmed}'").into()))?;
    Ok((major, minor))
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;
    use smol_str::SmolStr;

    use super::*;

    fn mesh_config(role: MeshRole) -> MeshConfig {
        MeshConfig {
            enabled: true,
            role,
            listen: SmolStr::new("0.0.0.0:5200"),
            connect: Vec::new(),
            tls: false,
            auth_token: None,
            publish: Vec::new(),
            subscribe: IndexMap::new(),
            zenohd_version: SmolStr::new(ZENOHD_BASELINE_VERSION),
            plugin_versions: IndexMap::new(),
        }
    }

    #[test]
    fn version_policy_rejects_mixed_major_minor_for_router_plugins() {
        let mut config = mesh_config(MeshRole::Router);
        config
            .plugin_versions
            .insert(SmolStr::new("storage-manager"), SmolStr::new("1.8.0"));
        let error = validate_zenoh_version_policy(&config).expect_err("mixed minor must fail");
        assert!(error.to_string().contains("not compatible"));
    }

    #[test]
    fn version_policy_accepts_matching_release_family() {
        let mut config = mesh_config(MeshRole::Router);
        config.plugin_versions.insert(
            SmolStr::new("storage-manager"),
            SmolStr::new(ZENOHD_BASELINE_VERSION),
        );
        validate_zenoh_version_policy(&config).expect("matching family should pass");
    }
}
