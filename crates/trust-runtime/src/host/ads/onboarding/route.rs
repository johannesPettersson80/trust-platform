use serde::{Deserialize, Serialize};

use crate::ads::diagnostics::{
    CredentialChannelClassification, LocalIdentity, LocalNetworkClassification,
    RouteActionAvailability, RouteArtifact, RouteArtifactKind, RoutePlan, TargetIdentity,
};

use super::errors::{OnboardingWireError, OnboardingWireErrorKind};
use super::wire::AdsOnboardingWire;

/// Credentials used for one AddRoute action. This type is never serialized.
#[derive(Clone, PartialEq, Eq)]
pub struct RouteCredentials {
    /// TwinCAT route user name.
    pub username: String,
    /// TwinCAT route password. Must never be serialized or logged.
    pub password: String,
}

/// Request to create an ADS route on a TwinCAT target.
#[derive(Clone, PartialEq, Eq)]
pub struct RouteAddRequest {
    /// Route name to create on the TwinCAT target.
    pub route_name: String,
    /// TwinCAT target that receives the route.
    pub target: TargetIdentity,
    /// Runtime-host identity to register in the route.
    pub local: LocalIdentity,
    /// One-shot route credentials.
    pub credentials: RouteCredentials,
}

/// Request to remove a named ADS route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteRemoveRequest {
    /// Route name to remove.
    pub route_name: String,
    /// TwinCAT target containing the route.
    pub target: TargetIdentity,
}

/// Request to generate route artifacts and action availability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutePlanRequest {
    /// Route direction/product role.
    #[serde(default)]
    pub role: RoutePlanRole,
    /// Route name for generated artifacts.
    pub route_name: String,
    /// TwinCAT target that receives route setup.
    pub target: TargetIdentity,
    /// Runtime-host identity used by route setup.
    pub local: LocalIdentity,
    /// Credential channel classification for automatic route actions.
    pub channel: CredentialChannelClassification,
}

/// ADS route plan direction.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutePlanRole {
    /// truST connects out to a TwinCAT ADS target.
    #[default]
    Client,
    /// External ADS clients connect into the truST ADS server.
    Server,
}

/// Build a route plan using runtime-host identity values.
#[must_use]
pub fn build_route_plan(request: RoutePlanRequest) -> RoutePlan {
    let availability = automatic_route_availability(request.channel, &request.local);
    RoutePlan {
        route_name: request.route_name.clone(),
        target: request.target.clone(),
        local: request.local.clone(),
        channel: request.channel,
        automatic_route: availability,
        artifacts: vec![
            powershell_route_artifact(&request),
            static_routes_xml_artifact(&request),
            manual_steps_artifact(&request),
            removal_powershell_artifact(&request),
        ],
    }
}

/// Builds the route removal artifact for a named static ADS route.
#[must_use]
pub fn build_route_remove_artifact(route_name: &str) -> RouteArtifact {
    RouteArtifact {
        kind: RouteArtifactKind::RemovalPowershell,
        label: "Download route removal PowerShell".to_string(),
        filename: Some(format!("remove-ads-route-{}.ps1", slug(route_name))),
        content_type: "text/x-powershell".to_string(),
        content: powershell_remove_route_script(route_name),
    }
}

/// Execute automatic AddRoute only when credentials are legal for the channel.
pub fn add_route_with_channel_policy<W: AdsOnboardingWire>(
    wire: &mut W,
    request: &RouteAddRequest,
    channel: CredentialChannelClassification,
) -> Result<(), OnboardingWireError> {
    if !channel.permits_credentials() {
        return Err(OnboardingWireError::new(
            OnboardingWireErrorKind::UnsupportedOperation,
            "automatic ADS route-add is disabled for this credential channel",
        ));
    }
    if matches!(
        request.local.classification,
        LocalNetworkClassification::Public | LocalNetworkClassification::NatSuspect
    ) {
        return Err(OnboardingWireError::new(
            OnboardingWireErrorKind::NatOrPublic,
            "automatic ADS route-add is disabled for public or NAT-suspect runtime identities",
        ));
    }
    wire.add_route(request)
}

fn automatic_route_availability(
    channel: CredentialChannelClassification,
    local: &LocalIdentity,
) -> RouteActionAvailability {
    if matches!(
        local.classification,
        LocalNetworkClassification::Public | LocalNetworkClassification::NatSuspect
    ) {
        return RouteActionAvailability::DisabledNatOrPublic;
    }
    if !channel.permits_credentials() {
        return RouteActionAvailability::DisabledUntrustedChannel;
    }
    RouteActionAvailability::Available
}

fn powershell_route_artifact(request: &RoutePlanRequest) -> RouteArtifact {
    let label = match request.role {
        RoutePlanRole::Client => "Download PowerShell",
        RoutePlanRole::Server => "Download PowerShell for TwinCAT station",
    };
    RouteArtifact {
        kind: RouteArtifactKind::Powershell,
        label: label.to_string(),
        filename: Some(format!("add-ads-route-{}.ps1", slug(&request.route_name))),
        content_type: "text/x-powershell".to_string(),
        content: powershell_route_script(request),
    }
}

fn static_routes_xml_artifact(request: &RoutePlanRequest) -> RouteArtifact {
    RouteArtifact {
        kind: RouteArtifactKind::StaticRoutesXml,
        label: "Copy StaticRoutes.xml snippet".to_string(),
        filename: Some(format!("ads-route-{}.xml", slug(&request.route_name))),
        content_type: "application/xml".to_string(),
        content: static_routes_xml_snippet(
            request.route_name.as_str(),
            request.local.chosen_ip.as_str(),
            request.local.ams_net_id.as_str(),
        ),
    }
}

fn manual_steps_artifact(request: &RoutePlanRequest) -> RouteArtifact {
    let content = match request.role {
        RoutePlanRole::Client => client_manual_steps(request),
        RoutePlanRole::Server => server_manual_steps(request),
    };
    RouteArtifact {
        kind: RouteArtifactKind::ManualSteps,
        label: "Manual TwinCAT route steps".to_string(),
        filename: Some(format!(
            "ads-route-{}-manual.txt",
            slug(&request.route_name)
        )),
        content_type: "text/plain".to_string(),
        content,
    }
}

fn client_manual_steps(request: &RoutePlanRequest) -> String {
    format!(
        "\
Do not use Broadcast Search for the truST runtime host.
truST is an ADS client, not a TwinCAT target, so the fingerprint step can fail with ADS Error 1861.
Use a manual/static route with these exact values.

Route name: {route_name}
Address: {address}
AMS Net ID: {net_id}
Transport: TCP/IP
Target PLC: {target_name} ({target_ip}, {target_net_id})
",
        route_name = request.route_name,
        address = request.local.chosen_ip,
        net_id = request.local.ams_net_id,
        target_name = request
            .target
            .name
            .as_deref()
            .unwrap_or("unnamed TwinCAT target"),
        target_ip = request.target.ip,
        target_net_id = request.target.ams_net_id,
    )
}

fn server_manual_steps(request: &RoutePlanRequest) -> String {
    format!(
        "\
Add this route on the external ADS client / TwinCAT engineering station so it can reach the truST ADS server.
Use the truST runtime host identity exactly as shown below.

Route name: {route_name}
Address: {address}
AMS Net ID: {net_id}
Transport: TCP/IP
truST ADS server: {target_name} ({target_ip}, {target_net_id})
",
        route_name = request.route_name,
        address = request.local.chosen_ip,
        net_id = request.local.ams_net_id,
        target_name = request
            .target
            .name
            .as_deref()
            .unwrap_or("truST runtime"),
        target_ip = request.target.ip,
        target_net_id = request.target.ams_net_id,
    )
}

fn removal_powershell_artifact(request: &RoutePlanRequest) -> RouteArtifact {
    build_route_remove_artifact(request.route_name.as_str())
}

fn static_routes_xml_snippet(route_name: &str, address: &str, net_id: &str) -> String {
    format!(
        "\
<Route>
  <Name>{}</Name>
  <Address>{}</Address>
  <NetId>{}</NetId>
  <Type>TCP_IP</Type>
  <Flags>0</Flags>
</Route>
",
        xml_escape(route_name),
        xml_escape(address),
        xml_escape(net_id)
    )
}

fn powershell_route_script(request: &RoutePlanRequest) -> String {
    let snippet = static_routes_xml_snippet(
        request.route_name.as_str(),
        request.local.chosen_ip.as_str(),
        request.local.ams_net_id.as_str(),
    );
    let role_comment = match request.role {
        RoutePlanRole::Client => {
            "# Run this on the TwinCAT PLC/runtime that the truST ADS client connects to."
        }
        RoutePlanRole::Server => {
            "# Run this on the external ADS client / TwinCAT engineering station, not on the truST runtime host."
        }
    };
    format!(
        r#"# Generated by truST ADS onboarding.
# Adds or replaces one TwinCAT ADS static route. No TwinCAT credentials are stored in this file.
{role_comment}
$ErrorActionPreference = 'Stop'

function Get-TrustRouteEncodingLabel([string]$Path) {{
  if (-not (Test-Path $Path)) {{ return 'absent' }}
  $bytes = [System.IO.File]::ReadAllBytes($Path)
  if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) {{ return 'UTF-8 BOM' }}
  if ($bytes.Length -ge 2 -and $bytes[0] -eq 0xFF -and $bytes[1] -eq 0xFE) {{ return 'UTF-16 LE BOM' }}
  if ($bytes.Length -ge 2 -and $bytes[0] -eq 0xFE -and $bytes[1] -eq 0xFF) {{ return 'UTF-16 BE BOM' }}
  return 'no BOM / existing default'
}}

$RouteName = '{route_name}'
$RouteAddress = '{address}'
$RouteNetId = '{net_id}'
$RouteXml = @'
{snippet}'@

$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {{
  Write-Error 'Run this script from an elevated PowerShell prompt before it changes StaticRoutes.xml.'
  exit 1
}}

$runtimeRoots = @(Get-ChildItem -Path "$env:ProgramData\Beckhoff\TwinCAT\3.1\Runtimes" -Directory -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName)
$candidateFiles = @()
$candidateFiles += $runtimeRoots | ForEach-Object {{ Join-Path $_ '3.1\StaticRoutes.xml' }}
$candidateFiles += $runtimeRoots | ForEach-Object {{ Join-Path $_ '3.1\Target\StaticRoutes.xml' }}
$candidateFiles += @(
  "$env:ProgramData\Beckhoff\TwinCAT\3.1\Target\StaticRoutes.xml",
  'C:\TwinCAT\3.1\Target\StaticRoutes.xml'
)
$candidateFiles += Get-ChildItem -Path "$env:ProgramData\Beckhoff\TwinCAT\3.1\Runtimes" -Filter StaticRoutes.xml -Recurse -ErrorAction SilentlyContinue | ForEach-Object {{ $_.FullName }}

$routeFile = $candidateFiles | Where-Object {{ $_ -and (Test-Path $_) }} | Select-Object -First 1
if (-not $routeFile) {{
  if ($runtimeRoots.Count -eq 1) {{
    $routeFile = Join-Path $runtimeRoots[0] '3.1\StaticRoutes.xml'
  }} else {{
    $routeFile = "$env:ProgramData\Beckhoff\TwinCAT\3.1\Target\StaticRoutes.xml"
  }}
  New-Item -ItemType Directory -Force -Path (Split-Path $routeFile) | Out-Null
  '<TcConfig><RemoteConnections></RemoteConnections></TcConfig>' | Set-Content -Path $routeFile -Encoding UTF8
}}

$originalEncoding = Get-TrustRouteEncodingLabel $routeFile
$backup = "$routeFile.trust-backup-$(Get-Date -Format yyyyMMddHHmmss)"
Copy-Item -Path $routeFile -Destination $backup -Force

[xml]$xml = Get-Content -Path $routeFile -Raw
$container = $xml.SelectSingleNode('//RemoteConnections')
if (-not $container) {{
  $container = $xml.CreateElement('RemoteConnections')
  $xml.DocumentElement.AppendChild($container) | Out-Null
}}

$existing = @($container.ChildNodes | Where-Object {{ $_.Name -eq 'Route' -and $_.SelectSingleNode('Name') -and $_.SelectSingleNode('Name').InnerText -eq $RouteName }})
foreach ($node in $existing) {{ $container.RemoveChild($node) | Out-Null }}

$fragment = $xml.CreateDocumentFragment()
$fragment.InnerXml = $RouteXml
$container.AppendChild($fragment) | Out-Null
$xml.Save($routeFile)
$newEncoding = Get-TrustRouteEncodingLabel $routeFile

Write-Host "Applied ADS route '$RouteName'"
Write-Host "  Address: $RouteAddress"
Write-Host "  AMS Net ID: $RouteNetId"
Write-Host "Backup: $backup"
Write-Host 'Unrelated ADS routes were preserved; only the named route was replaced.'
if ($originalEncoding -ne $newEncoding) {{
  Write-Warning "StaticRoutes.xml encoding/BOM changed: $originalEncoding -> $newEncoding"
}} else {{
  Write-Host "StaticRoutes.xml encoding/BOM: $newEncoding"
}}
Write-Host 'Restart the TwinCAT router or Usermode Runtime if the route is not picked up immediately.'
"#,
        route_name = powershell_single_quote(request.route_name.as_str()),
        address = powershell_single_quote(request.local.chosen_ip.as_str()),
        net_id = powershell_single_quote(request.local.ams_net_id.as_str()),
        snippet = snippet,
        role_comment = role_comment,
    )
}

fn powershell_remove_route_script(route_name: &str) -> String {
    format!(
        r#"# Generated by truST ADS onboarding.
# Removes one TwinCAT ADS static route by name.
$ErrorActionPreference = 'Stop'
function Get-TrustRouteEncodingLabel([string]$Path) {{
  if (-not (Test-Path $Path)) {{ return 'absent' }}
  $bytes = [System.IO.File]::ReadAllBytes($Path)
  if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) {{ return 'UTF-8 BOM' }}
  if ($bytes.Length -ge 2 -and $bytes[0] -eq 0xFF -and $bytes[1] -eq 0xFE) {{ return 'UTF-16 LE BOM' }}
  if ($bytes.Length -ge 2 -and $bytes[0] -eq 0xFE -and $bytes[1] -eq 0xFF) {{ return 'UTF-16 BE BOM' }}
  return 'no BOM / existing default'
}}

$RouteName = '{route_name}'

$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {{
  Write-Error 'Run this script from an elevated PowerShell prompt before it changes StaticRoutes.xml.'
  exit 1
}}

$runtimeRoots = @(Get-ChildItem -Path "$env:ProgramData\Beckhoff\TwinCAT\3.1\Runtimes" -Directory -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName)
$candidateFiles = @()
$candidateFiles += $runtimeRoots | ForEach-Object {{ Join-Path $_ '3.1\StaticRoutes.xml' }}
$candidateFiles += $runtimeRoots | ForEach-Object {{ Join-Path $_ '3.1\Target\StaticRoutes.xml' }}
$candidateFiles += @(
  "$env:ProgramData\Beckhoff\TwinCAT\3.1\Target\StaticRoutes.xml",
  'C:\TwinCAT\3.1\Target\StaticRoutes.xml'
)
$candidateFiles += Get-ChildItem -Path "$env:ProgramData\Beckhoff\TwinCAT\3.1\Runtimes" -Filter StaticRoutes.xml -Recurse -ErrorAction SilentlyContinue | ForEach-Object {{ $_.FullName }}

foreach ($routeFile in $candidateFiles | Where-Object {{ $_ -and (Test-Path $_) }}) {{
  $originalEncoding = Get-TrustRouteEncodingLabel $routeFile
  $backup = "$routeFile.trust-backup-$(Get-Date -Format yyyyMMddHHmmss)"
  Copy-Item -Path $routeFile -Destination $backup -Force
  [xml]$xml = Get-Content -Path $routeFile -Raw
  $container = $xml.SelectSingleNode('//RemoteConnections')
  if (-not $container) {{ continue }}
  $existing = @($container.ChildNodes | Where-Object {{ $_.Name -eq 'Route' -and $_.SelectSingleNode('Name') -and $_.SelectSingleNode('Name').InnerText -eq $RouteName }})
  foreach ($node in $existing) {{ $container.RemoveChild($node) | Out-Null }}
  $xml.Save($routeFile)
  $newEncoding = Get-TrustRouteEncodingLabel $routeFile
  Write-Host "Removed ADS route '$RouteName' from $routeFile"
  Write-Host "Backup: $backup"
  Write-Host 'Unrelated ADS routes were preserved.'
  if ($originalEncoding -ne $newEncoding) {{
    Write-Warning "StaticRoutes.xml encoding/BOM changed: $originalEncoding -> $newEncoding"
  }} else {{
    Write-Host "StaticRoutes.xml encoding/BOM: $newEncoding"
  }}
}}
"#,
        route_name = powershell_single_quote(route_name),
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn powershell_single_quote(value: &str) -> String {
    value.replace('\'', "''")
}

fn slug(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            slug.push(ch.to_ascii_lowercase());
        } else {
            slug.push('-');
        }
    }
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "route".to_string()
    } else {
        trimmed.to_string()
    }
}
