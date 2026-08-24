fn parse_retain_mode(retain: &RetainSection) -> Result<RetainMode, RuntimeError> {
    let mode = RetainMode::parse(&retain.mode)?;
    if matches!(mode, RetainMode::File)
        && retain
            .path
            .as_deref()
            .is_none_or(|path| path.trim().is_empty())
    {
        return Err(RuntimeError::InvalidConfig(
            "runtime.retain.path required when mode=file".into(),
        ));
    }
    Ok(mode)
}

fn parse_tls_section(section: Option<TlsSection>) -> Result<ParsedTls, RuntimeError> {
    let tls_section = section.unwrap_or(TlsSection {
        mode: Some("disabled".into()),
        cert_path: None,
        key_path: None,
        ca_path: None,
        require_remote: Some(false),
    });
    let mode = TlsMode::parse(tls_section.mode.as_deref().unwrap_or("disabled"))?;
    let cert_path = parse_optional_path("runtime.tls.cert_path", tls_section.cert_path)?;
    let key_path = parse_optional_path("runtime.tls.key_path", tls_section.key_path)?;
    let ca_path = parse_optional_path("runtime.tls.ca_path", tls_section.ca_path)?;
    let require_remote = tls_section.require_remote.unwrap_or(false);

    if mode.enabled() {
        if cert_path.is_none() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.tls.cert_path required when TLS is enabled".into(),
            ));
        }
        if key_path.is_none() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.tls.key_path required when TLS is enabled".into(),
            ));
        }
        if matches!(mode, TlsMode::Provisioned) && ca_path.is_none() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.tls.ca_path required when runtime.tls.mode='provisioned'".into(),
            ));
        }
    }

    Ok(ParsedTls {
        config: TlsConfig {
            mode,
            cert_path,
            key_path,
            ca_path,
            require_remote,
        },
        mode,
        require_remote,
    })
}

fn parse_web_section(
    section: Option<WebSection>,
    control_auth_token: Option<&SmolStr>,
    tls_mode: TlsMode,
    tls_require_remote: bool,
) -> Result<ParsedWeb, RuntimeError> {
    let web_section = section.unwrap_or(WebSection {
        enabled: Some(true),
        listen: Some("0.0.0.0:8080".into()),
        auth: Some("local".into()),
        tls: Some(false),
    });

    if web_section
        .listen
        .as_deref()
        .is_some_and(|listen| listen.trim().is_empty())
    {
        return Err(RuntimeError::InvalidConfig(
            "runtime.web.listen must not be empty".into(),
        ));
    }

    let web_auth = WebAuthMode::parse(web_section.auth.as_deref().unwrap_or("local"))?;
    if matches!(web_auth, WebAuthMode::Token) && control_auth_token.is_none() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.web.auth=token requires runtime.control.auth_token".into(),
        ));
    }

    let web_enabled = web_section.enabled.unwrap_or(true);
    let web_listen = web_section.listen.unwrap_or_else(|| "0.0.0.0:8080".into());
    let web_tls = web_section.tls.unwrap_or(false);

    if web_tls && !tls_mode.enabled() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.web.tls=true requires runtime.tls.mode != 'disabled'".into(),
        ));
    }
    if tls_require_remote && web_enabled && listen_is_remote(&web_listen) && !web_tls {
        return Err(RuntimeError::InvalidConfig(
            "runtime.web.tls must be true when runtime.tls.require_remote=true and runtime.web.listen is remote".into(),
        ));
    }

    Ok(ParsedWeb {
        config: WebConfig {
            enabled: web_enabled,
            listen: SmolStr::new(web_listen),
            auth: web_auth,
            tls: web_tls,
        },
    })
}

fn parse_deploy_section(section: Option<DeploySection>) -> Result<ParsedDeploy, RuntimeError> {
    let deploy_section = section.unwrap_or(DeploySection {
        require_signed: Some(false),
        keyring_path: None,
    });
    if deploy_section
        .keyring_path
        .as_deref()
        .is_some_and(|path| path.trim().is_empty())
    {
        return Err(RuntimeError::InvalidConfig(
            "runtime.deploy.keyring_path must not be empty".into(),
        ));
    }
    if deploy_section.require_signed.unwrap_or(false)
        && deploy_section
            .keyring_path
            .as_deref()
            .is_none_or(|path| path.trim().is_empty())
    {
        return Err(RuntimeError::InvalidConfig(
            "runtime.deploy.keyring_path required when runtime.deploy.require_signed=true".into(),
        ));
    }

    Ok(ParsedDeploy {
        config: DeployConfig {
            require_signed: deploy_section.require_signed.unwrap_or(false),
            keyring_path: deploy_section.keyring_path.and_then(|path| {
                let path = path.trim();
                if path.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(path))
                }
            }),
        },
    })
}

fn parse_discovery_section(
    section: Option<DiscoverySection>,
) -> Result<ParsedDiscovery, RuntimeError> {
    let discovery_section = section.unwrap_or(DiscoverySection {
        enabled: Some(true),
        service_name: Some("truST".into()),
        advertise: Some(true),
        interfaces: None,
        host_group: None,
    });
    if discovery_section
        .service_name
        .as_deref()
        .is_some_and(|name| name.trim().is_empty())
    {
        return Err(RuntimeError::InvalidConfig(
            "runtime.discovery.service_name must not be empty".into(),
        ));
    }

    Ok(ParsedDiscovery {
        config: DiscoveryConfig {
            enabled: discovery_section.enabled.unwrap_or(true),
            service_name: SmolStr::new(
                discovery_section
                    .service_name
                    .unwrap_or_else(|| "truST".into()),
            ),
            advertise: discovery_section.advertise.unwrap_or(true),
            interfaces: discovery_section
                .interfaces
                .unwrap_or_default()
                .into_iter()
                .map(|value| {
                    parse_nonempty_entry(value, "runtime.discovery.interfaces")
                        .map(SmolStr::new)
                })
                .collect::<Result<Vec<_>, RuntimeError>>()?,
            host_group: discovery_section
                .host_group
                .map(|value| parse_nonempty_entry(value, "runtime.discovery.host_group"))
                .transpose()?
                .map(SmolStr::new),
        },
    })
}

fn parse_mesh_section(
    section: Option<MeshSection>,
    tls_mode: TlsMode,
    tls_require_remote: bool,
) -> Result<ParsedMesh, RuntimeError> {
    let mesh_section = section.unwrap_or(MeshSection {
        enabled: Some(false),
        role: Some("peer".into()),
        listen: Some("0.0.0.0:5200".into()),
        connect: None,
        tls: Some(false),
        auth_token: None,
        publish: None,
        subscribe: None,
        zenohd_version: Some("1.7.2".into()),
        plugin_versions: None,
    });
    if mesh_section
        .listen
        .as_deref()
        .is_some_and(|listen| listen.trim().is_empty())
    {
        return Err(RuntimeError::InvalidConfig(
            "runtime.mesh.listen must not be empty".into(),
        ));
    }

    let enabled = mesh_section.enabled.unwrap_or(false);
    let role = MeshRole::parse(mesh_section.role.as_deref().unwrap_or("peer"))?;
    let listen = mesh_section.listen.unwrap_or_else(|| "0.0.0.0:5200".into());
    let connect = mesh_section
        .connect
        .unwrap_or_default()
        .into_iter()
        .map(|endpoint| {
            parse_nonempty_entry(endpoint, "runtime.mesh.connect").map(SmolStr::new)
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    let tls = mesh_section.tls.unwrap_or(false);
    if tls && !tls_mode.enabled() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.mesh.tls=true requires runtime.tls.mode != 'disabled'".into(),
        ));
    }
    if tls_require_remote && enabled && listen_is_remote(&listen) && !tls {
        return Err(RuntimeError::InvalidConfig(
            "runtime.mesh.tls must be true when runtime.tls.require_remote=true and runtime.mesh.listen is remote".into(),
        ));
    }

    let zenohd_version = mesh_section
        .zenohd_version
        .as_deref()
        .unwrap_or("1.7.2")
        .trim()
        .to_string();
    if zenohd_version.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.mesh.zenohd_version must not be empty".into(),
        ));
    }

    Ok(ParsedMesh {
        config: MeshConfig {
            enabled,
            role,
            listen: SmolStr::new(listen),
            connect,
            tls,
            auth_token: mesh_section
                .auth_token
                .map(|value| parse_nonempty_entry(value, "runtime.mesh.auth_token"))
                .transpose()?
                .map(SmolStr::new),
            publish: mesh_section
                .publish
                .unwrap_or_default()
                .into_iter()
                .map(|value| {
                    parse_nonempty_entry(value, "runtime.mesh.publish").map(SmolStr::new)
                })
                .collect::<Result<Vec<_>, RuntimeError>>()?,
            subscribe: mesh_section
                .subscribe
                .unwrap_or_default()
                .into_iter()
                .map(|(source, target)| {
                    Ok((
                        SmolStr::new(parse_nonempty_entry(
                            source,
                            "runtime.mesh.subscribe",
                        )?),
                        SmolStr::new(parse_nonempty_entry(
                            target,
                            "runtime.mesh.subscribe",
                        )?),
                    ))
                })
                .collect::<Result<IndexMap<_, _>, RuntimeError>>()?,
            zenohd_version: SmolStr::new(zenohd_version),
            plugin_versions: mesh_section
                .plugin_versions
                .unwrap_or_default()
                .into_iter()
                .map(|(name, version)| {
                    Ok((
                        SmolStr::new(parse_nonempty_entry(
                            name,
                            "runtime.mesh.plugin_versions",
                        )?),
                        SmolStr::new(parse_nonempty_entry(
                            version,
                            "runtime.mesh.plugin_versions",
                        )?),
                    ))
                })
                .collect::<Result<IndexMap<_, _>, RuntimeError>>()?,
        },
    })
}

fn parse_runtime_cloud_section(
    section: Option<RuntimeCloudSection>,
) -> Result<ParsedRuntimeCloud, RuntimeError> {
    let cloud_section = section.unwrap_or(RuntimeCloudSection {
        profile: Some("dev".into()),
        wan: None,
        links: None,
    });
    let profile = RuntimeCloudProfile::parse(cloud_section.profile.as_deref().unwrap_or("dev"))?;
    let wan_allow_write = cloud_section
        .wan
        .and_then(|wan| wan.allow_write)
        .unwrap_or_default()
        .into_iter()
        .map(|rule| {
            let action = rule.action.trim();
            if action.is_empty() {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.cloud.wan.allow_write[].action must not be empty".into(),
                ));
            }
            let target = rule.target.trim();
            if target.is_empty() {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.cloud.wan.allow_write[].target must not be empty".into(),
                ));
            }
            Ok(RuntimeCloudWanAllowRule {
                action: SmolStr::new(action),
                target: SmolStr::new(target),
            })
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    let link_preferences = cloud_section
        .links
        .and_then(|links| links.transports)
        .unwrap_or_default()
        .into_iter()
        .map(|rule| {
            let source = rule.source.trim();
            if source.is_empty() {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.cloud.links.transports[].source must not be empty".into(),
                ));
            }
            let target = rule.target.trim();
            if target.is_empty() {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.cloud.links.transports[].target must not be empty".into(),
                ));
            }
            Ok(RuntimeCloudLinkPreferenceRule {
                source: SmolStr::new(source),
                target: SmolStr::new(target),
                transport: RuntimeCloudPreferredTransport::parse(rule.transport.as_str())?,
            })
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;

    Ok(ParsedRuntimeCloud {
        profile,
        wan_allow_write,
        link_preferences,
    })
}
