struct ParsedControl {
    auth_token: Option<SmolStr>,
    mode: ControlMode,
    debug_enabled: bool,
}

struct ParsedTls {
    config: TlsConfig,
    mode: TlsMode,
    require_remote: bool,
}

struct ParsedWeb {
    config: WebConfig,
}

struct ParsedDeploy {
    config: DeployConfig,
}

struct ParsedDiscovery {
    config: DiscoveryConfig,
}

struct ParsedMesh {
    config: MeshConfig,
}

struct ParsedRuntimeCloud {
    profile: RuntimeCloudProfile,
    wan_allow_write: Vec<RuntimeCloudWanAllowRule>,
    link_preferences: Vec<RuntimeCloudLinkPreferenceRule>,
}

struct ParsedRealtime {
    config: LinuxRtConfig,
}
