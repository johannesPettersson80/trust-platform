use std::collections::BTreeSet;

use serde::Deserialize;
use trust_ads_core::{
    AdsDataTypeDescriptor, AdsRoute, AdsSecurityPolicy, AmsNetId, ArrayDimension, IecDataType,
    PointAccess, TransportSecurity, UpdateMode,
};

use super::diagnostics::LocalIdentity;
use super::identity::is_canonical_ams_net_id;
use super::transport::{AdsNotificationMode, AdsPointAddress};
use super::RuntimeError;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsClientConfig {
    pub connections: Vec<AdsConnectionConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsConnectionConfig {
    pub route: AdsRoute,
    pub points: Vec<AdsPointConfig>,
}

impl AdsConnectionConfig {
    pub fn security_warnings(&self) -> Vec<String> {
        ads_security_warnings(&self.route)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsPointConfig {
    pub point_name: String,
    pub address: AdsPointAddress,
    pub data_type: AdsDataTypeDescriptor,
    pub access: PointAccess,
    pub mode: UpdateMode,
    pub notification_mode: AdsNotificationMode,
    pub allow_retain_read: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdsToml {
    connections: Vec<ConnectionSection>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectionSection {
    name: String,
    target_net_id: String,
    host: String,
    ams_port: Option<u16>,
    local_net_id: Option<String>,
    transport: Option<String>,
    insecure_transport: Option<bool>,
    auto_add_route: Option<bool>,
    #[serde(default)]
    points: Vec<PointSection>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PointSection {
    var: String,
    symbol: Option<String>,
    index_group: Option<u32>,
    index_offset: Option<u32>,
    size: Option<u32>,
    #[serde(rename = "type")]
    type_name: String,
    string_len: Option<u16>,
    dimensions: Option<Vec<DimensionSection>>,
    access: Option<String>,
    mode: Option<String>,
    notification_mode: Option<String>,
    allow_retain_read: Option<bool>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct DimensionSection {
    lower: i64,
    upper: i64,
}

pub fn parse_ads_toml(text: &str) -> Result<AdsClientConfig, RuntimeError> {
    let raw: AdsToml = toml::from_str(text)
        .map_err(|err| RuntimeError::InvalidConfig(format!("ads.toml: {err}").into()))?;
    raw.into_config()
}

pub fn validate_ads_config_local_identity(
    config: &AdsClientConfig,
    local: &LocalIdentity,
) -> Result<(), RuntimeError> {
    for connection in &config.connections {
        match connection.route.local_net_id.as_ref() {
            Some(local_net_id) if local_net_id.0 == local.ams_net_id => {}
            Some(local_net_id) => {
                return invalid(format!(
                    "ADS connection '{}' local_net_id '{}' does not match runtime-host identity '{}'",
                    connection.route.name, local_net_id.0, local.ams_net_id
                ));
            }
            None => {
                return invalid(format!(
                    "ADS connection '{}' is missing local_net_id; production-ready evidence requires the runtime-host identity to be pinned",
                    connection.route.name
                ));
            }
        }
    }
    Ok(())
}

impl AdsToml {
    fn into_config(self) -> Result<AdsClientConfig, RuntimeError> {
        if self.connections.is_empty() {
            return invalid("ads.toml requires at least one [[connections]] entry");
        }
        let mut connection_names = BTreeSet::new();
        let mut point_names = BTreeSet::new();
        let connections = self
            .connections
            .into_iter()
            .map(|connection| connection.into_config(&mut connection_names, &mut point_names))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AdsClientConfig { connections })
    }
}

impl ConnectionSection {
    fn into_config(
        self,
        connection_names: &mut BTreeSet<String>,
        point_names: &mut BTreeSet<String>,
    ) -> Result<AdsConnectionConfig, RuntimeError> {
        let name = non_empty("connections.name", self.name)?;
        if !connection_names.insert(name.clone()) {
            return invalid(format!("ADS connection name '{name}' must be unique"));
        }
        let target_net_id = self.target_net_id;
        if !is_canonical_ams_net_id(&target_net_id) {
            return invalid(format!(
                "invalid target AMS Net ID '{target_net_id}'; expected six decimal octets in canonical form"
            ));
        }
        if let Some(local_net_id) = self.local_net_id.as_deref() {
            if !is_canonical_ams_net_id(local_net_id) {
                return invalid(format!(
                    "invalid local AMS Net ID '{local_net_id}'; expected six decimal octets in canonical form"
                ));
            }
        }
        let host = non_empty("connections.host", self.host)?;
        let transport = parse_transport(self.transport.as_deref().unwrap_or("secure"))?;
        let insecure_transport = self.insecure_transport.unwrap_or(false);
        if matches!(transport, TransportSecurity::Plain) && !insecure_transport {
            return invalid(format!(
                "connections '{name}' uses transport='plain' but insecure_transport=true is missing"
            ));
        }
        if matches!(transport, TransportSecurity::Secure) && insecure_transport {
            return invalid(format!(
                "connections '{name}' sets insecure_transport=true without transport='plain'"
            ));
        }

        let route = AdsRoute {
            name: name.clone(),
            target_net_id: AmsNetId::new(target_net_id),
            host,
            ams_port: self.ams_port.unwrap_or(851),
            local_net_id: self.local_net_id.map(AmsNetId::new),
            security: AdsSecurityPolicy {
                transport,
                auto_add_route: self.auto_add_route.unwrap_or(false),
            },
        };
        if route.ams_port == 0 {
            return invalid(format!("connections '{name}' ams_port must be >= 1"));
        }
        for warning in ads_security_warnings(&route) {
            tracing::warn!(target: "trust_runtime::ads", connection = %route.name, "{warning}");
        }

        let points = self
            .points
            .into_iter()
            .map(|point| point.into_config(&name, point_names))
            .collect::<Result<Vec<_>, _>>()?;
        if points.is_empty() {
            return invalid(format!(
                "ADS connection '{name}' requires at least one point"
            ));
        }
        Ok(AdsConnectionConfig { route, points })
    }
}

impl PointSection {
    fn into_config(
        self,
        connection_name: &str,
        point_names: &mut BTreeSet<String>,
    ) -> Result<AdsPointConfig, RuntimeError> {
        let point_name = non_empty("connections.points.var", self.var.clone())?;
        if !point_names.insert(point_name.clone()) {
            return invalid(format!(
                "ADS point '{point_name}' is bound more than once; one binding per declared variable is required"
            ));
        }
        let address = parse_address(connection_name, &point_name, &self)?;
        let data_type = parse_data_type(
            self.type_name.as_str(),
            self.string_len,
            self.dimensions.unwrap_or_default(),
        )?;
        validate_index_address_size(&address, &data_type)?;
        let access = parse_access(self.access.as_deref().unwrap_or("read"))?;
        let mode = parse_mode(self.mode.as_deref().unwrap_or("poll"))?;
        let notification_mode =
            parse_notification_mode(self.notification_mode.as_deref().unwrap_or("on_change"))?;
        if mode != UpdateMode::Notify && self.notification_mode.is_some() {
            return invalid(format!(
                "point '{}' sets notification_mode but mode is not 'notify'",
                self.var
            ));
        }
        Ok(AdsPointConfig {
            point_name,
            address,
            data_type,
            access,
            mode,
            notification_mode,
            allow_retain_read: self.allow_retain_read.unwrap_or(false),
        })
    }
}

fn parse_address(
    connection_name: &str,
    point_name: &str,
    point: &PointSection,
) -> Result<AdsPointAddress, RuntimeError> {
    let has_symbol = point
        .symbol
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_index =
        point.index_group.is_some() || point.index_offset.is_some() || point.size.is_some();
    match (has_symbol, has_index) {
        (true, false) => Ok(AdsPointAddress::Symbol(
            point.symbol
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .to_string(),
        )),
        (false, true) => {
            let index_group = point.index_group.ok_or_else(|| {
                RuntimeError::InvalidConfig(
                    format!(
                        "connections '{connection_name}' point '{point_name}' index_group is required for index addressing"
                    )
                    .into(),
                )
            })?;
            let index_offset = point.index_offset.ok_or_else(|| {
                RuntimeError::InvalidConfig(
                    format!(
                        "connections '{connection_name}' point '{point_name}' index_offset is required for index addressing"
                    )
                    .into(),
                )
            })?;
            let size = point.size.ok_or_else(|| {
                RuntimeError::InvalidConfig(
                    format!(
                        "connections '{connection_name}' point '{point_name}' size is required for index addressing"
                    )
                    .into(),
                )
            })?;
            if size == 0 {
                return invalid(format!(
                    "connections '{connection_name}' point '{point_name}' size must be >= 1"
                ));
            }
            Ok(AdsPointAddress::Index {
                index_group,
                index_offset,
                size,
            })
        }
        (true, true) => invalid(format!(
            "connections '{connection_name}' point '{point_name}' must use either symbol or index_group/index_offset/size, not both"
        )),
        (false, false) => invalid(format!(
            "connections '{connection_name}' point '{point_name}' requires symbol or index_group/index_offset/size"
        )),
    }
}

fn parse_data_type(
    raw: &str,
    string_len: Option<u16>,
    dimensions: Vec<DimensionSection>,
) -> Result<AdsDataTypeDescriptor, RuntimeError> {
    let trimmed = raw.trim();
    let (iec_type, parsed_string_len) = parse_iec_type(trimmed)?;
    if !matches!(iec_type, IecDataType::String) && string_len.is_some() {
        return invalid("ADS string_len is valid only for STRING");
    }
    if matches!(parsed_string_len, Some(0)) || matches!(string_len, Some(0)) {
        return invalid("ADS STRING capacity must be at least 1");
    }
    let string_len = parsed_string_len.or(string_len);
    if matches!(iec_type, IecDataType::String) && string_len.is_none() {
        return invalid("ADS STRING type requires a length, for example type='STRING(80)'");
    }
    let dimensions = dimensions
        .into_iter()
        .map(|dimension| {
            if dimension.upper < dimension.lower {
                return invalid(format!(
                    "ADS array dimension upper {} is below lower {}",
                    dimension.upper, dimension.lower
                ));
            }
            Ok(ArrayDimension {
                lower: dimension.lower,
                upper: dimension.upper,
            })
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    let descriptor = AdsDataTypeDescriptor {
        source_name: trimmed.to_string(),
        iec_type,
        dimensions,
        string_len,
    };
    declared_byte_len(&descriptor)?;
    Ok(descriptor)
}

fn declared_byte_len(descriptor: &AdsDataTypeDescriptor) -> Result<usize, RuntimeError> {
    descriptor.byte_len().map_err(|error| {
        RuntimeError::InvalidConfig(
            format!("ADS type metadata byte length overflows: {error}").into(),
        )
    })
}

fn validate_index_address_size(
    address: &AdsPointAddress,
    descriptor: &AdsDataTypeDescriptor,
) -> Result<(), RuntimeError> {
    let AdsPointAddress::Index { size, .. } = address else {
        return Ok(());
    };
    let declared_size = declared_byte_len(descriptor)?;
    let configured_size = usize::try_from(*size).map_err(|_| {
        RuntimeError::InvalidConfig(
            format!("ADS index address size {size} exceeds the platform byte range").into(),
        )
    })?;
    if configured_size != declared_size {
        return invalid(format!(
            "ADS index address size {size} does not match declared type size {declared_size}"
        ));
    }
    Ok(())
}

fn parse_iec_type(raw: &str) -> Result<(IecDataType, Option<u16>), RuntimeError> {
    let upper = raw.to_ascii_uppercase();
    if let Some(inner) = upper
        .strip_prefix("STRING(")
        .and_then(|tail| tail.strip_suffix(')'))
    {
        let len = inner.parse::<u16>().map_err(|err| {
            RuntimeError::InvalidConfig(
                format!("invalid ADS STRING length '{inner}': {err}").into(),
            )
        })?;
        return Ok((IecDataType::String, Some(len)));
    }
    let data_type = match upper.as_str() {
        "BOOL" => IecDataType::Bool,
        "SINT" => IecDataType::Sint,
        "INT" => IecDataType::Int,
        "DINT" => IecDataType::Dint,
        "LINT" => IecDataType::Lint,
        "USINT" => IecDataType::Usint,
        "UINT" => IecDataType::Uint,
        "UDINT" => IecDataType::Udint,
        "ULINT" => IecDataType::Ulint,
        "REAL" => IecDataType::Real,
        "LREAL" => IecDataType::Lreal,
        "BYTE" => IecDataType::Byte,
        "WORD" => IecDataType::Word,
        "DWORD" => IecDataType::Dword,
        "LWORD" => IecDataType::Lword,
        "STRING" => IecDataType::String,
        _ => {
            return invalid(format!(
                "unsupported ADS IEC type '{raw}'; bind STRUCT leaf members as scalar points"
            ))
        }
    };
    Ok((data_type, None))
}

fn parse_transport(raw: &str) -> Result<TransportSecurity, RuntimeError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "plain" => Ok(TransportSecurity::Plain),
        "secure" => Ok(TransportSecurity::Secure),
        _ => invalid(format!("invalid ADS transport '{raw}'")),
    }
}

fn parse_access(raw: &str) -> Result<PointAccess, RuntimeError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "read" => Ok(PointAccess::Read),
        "write" => Ok(PointAccess::Write),
        "read_write" | "read-write" | "readwrite" => Ok(PointAccess::ReadWrite),
        _ => invalid(format!("invalid ADS point access '{raw}'")),
    }
}

fn parse_mode(raw: &str) -> Result<UpdateMode, RuntimeError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "poll" => Ok(UpdateMode::Poll),
        "notify" => Ok(UpdateMode::Notify),
        _ => invalid(format!("invalid ADS point mode '{raw}'")),
    }
}

fn parse_notification_mode(raw: &str) -> Result<AdsNotificationMode, RuntimeError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "on_change" | "on-change" | "onchange" => Ok(AdsNotificationMode::OnChange),
        "cyclic" | "cycle" => Ok(AdsNotificationMode::Cyclic),
        _ => invalid(format!("invalid ADS notification_mode '{raw}'")),
    }
}

fn ads_security_warnings(route: &AdsRoute) -> Vec<String> {
    if matches!(route.security.transport, TransportSecurity::Plain) {
        vec![format!(
            "ADS connection '{}' uses plain ADS transport; traffic is cleartext and authenticated only by AMS route trust. Use an isolated OT network segment and keep writes explicit.",
            route.name
        )]
    } else {
        Vec::new()
    }
}

fn non_empty(label: &str, value: String) -> Result<String, RuntimeError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return invalid(format!("{label} must not be empty"));
    }
    Ok(trimmed)
}

fn invalid<T>(message: impl Into<String>) -> Result<T, RuntimeError> {
    Err(RuntimeError::InvalidConfig(message.into().into()))
}
