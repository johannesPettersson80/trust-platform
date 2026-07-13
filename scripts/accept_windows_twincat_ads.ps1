#requires -Version 5.1
<#
.SYNOPSIS
Runs the fail-closed packaged truST ADS acceptance journey on a Windows computer
that has TwinCAT installed and running.
.DESCRIPTION
The script extracts trust-runtime.exe from a packaged win32-x64 VSIX, discovers
same-computer ADS targets without a host or AMS Net ID input, probes the bounded
logical ADS service set, and validates the native-router evidence emitted by the
ADS doctor. It also drives the exact packaged extension through Simulator Start
and Stop in an isolated VS Code profile. It never adds an ADS route and never
enables the doctor write probe.

ExpectedTargetNetId is a fail-closed proof expectation, not a discovery input.
ExpectedRouterSourceNetId is an optional assertion for environments where the
router identity is known in advance. Zero-input discovery, packaged UI, CLI
service probes, and ADS Doctor must all resolve the target exactly once. Doctor
must report distinct full source and target AMS addresses (Net ID plus port); a
native client may legitimately share the target Net ID when its AMS port differs.

The two TwinCAT Usermode Runtime route files observed on the acceptance laptop
are required and hashed before and after the journey. A missing, unreadable, or
changed file fails the run. Structured JSON evidence is written even on failure.

#>

[CmdletBinding()]
param(
    [Parameter()]
    [string]$VsixPath,
    [Parameter(Mandatory = $true)]
    [string]$CandidateManifestPath,
    [Parameter()]
    [ValidatePattern('^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?$')]
    [string]$ExpectedVersion,
    [Parameter()]
    [ValidatePattern('^[0-9A-Fa-f]{64}$')]
    [string]$ExpectedVsixSha256,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9]{1,3}(?:\.[0-9]{1,3}){5}$')]
    [string]$ExpectedTargetNetId,
    [Parameter()]
    [ValidatePattern('^[0-9]{1,3}(?:\.[0-9]{1,3}){5}$')]
    [string]$ExpectedRouterSourceNetId,
    [Parameter()]
    [string]$EvidencePath,
    [Parameter()]
    [int[]]$CustomAdsPorts = @(9000),
    [Parameter()]
    [ValidateRange(5, 300)]
    [int]$CommandTimeoutSeconds = 30,
    [Parameter()]
    [ValidateRange(60, 600)]
    [int]$SimulatorTimeoutSeconds = 420
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'windows_twincat_ads_acceptance\AcceptanceIo.psm1') -Force
Import-Module (Join-Path $PSScriptRoot 'windows_twincat_ads_acceptance\PackagedSimulatorLauncher.psm1') -Force
Import-Module (Join-Path $PSScriptRoot 'windows_twincat_ads_acceptance\PackagedAdsUiCrosscheck.psm1') -Force
Import-Module (Join-Path $PSScriptRoot 'windows_twincat_ads_acceptance\AcceptancePlan.psm1') -Force
Import-Module (Join-Path $PSScriptRoot 'windows_twincat_ads_acceptance\StaticRouteProof.psm1') -Force
Import-Module (Join-Path $PSScriptRoot 'windows_twincat_ads_acceptance\CandidateManifestProof.psm1') -Force
Import-Module (Join-Path $PSScriptRoot 'windows_twincat_ads_acceptance\CandidateProvenanceProof.psm1') -Force
Import-Module (Join-Path $PSScriptRoot 'windows_twincat_ads_acceptance\AdsBrowseProof.psm1') -Force
$script:SchemaVersion = 1
$script:GateName = 'windows_twincat_ads_acceptance'
$script:RequiredAdsPorts = @(851, 852, 853, 854, 301, 501)
$script:MaxAdsServiceProbes = 10
$script:RuntimeMember = 'extension/bin/trust-runtime.exe'
$script:PackagedBinaryMembers = @(
    'extension/bin/trust-runtime.exe',
    'extension/bin/trust-debug.exe',
    'extension/bin/trust-lsp.exe'
)
function Get-VsixTargetPlatform {
    param([Parameter(Mandatory = $true)][string]$Path)
    $archive = [IO.Compression.ZipFile]::OpenRead($Path)
    try {
        $entry = $archive.GetEntry('extension.vsixmanifest')
        if ($null -eq $entry) { return $null }
        $stream = $entry.Open()
        $reader = New-Object IO.StreamReader($stream, [Text.Encoding]::UTF8, $true)
        try { [xml]$manifest = $reader.ReadToEnd() }
        finally { $reader.Dispose(); $stream.Dispose() }
        $identity = $manifest.SelectSingleNode("//*[local-name()='Identity']")
        if ($null -eq $identity) { return $null }
        return $identity.GetAttribute('TargetPlatform')
    }
    finally {
        $archive.Dispose()
    }
}
function Resolve-PackagedVsix {
    param(
        [string]$ExplicitPath,
        [Parameter(Mandatory = $true)][string]$RepositoryRoot
    )
    if (-not [string]::IsNullOrWhiteSpace($ExplicitPath)) {
        return (Resolve-Path -LiteralPath $ExplicitPath -ErrorAction Stop).Path
    }
    $candidateFiles = New-Object 'System.Collections.Generic.List[object]'
    foreach ($directory in @(
        $RepositoryRoot,
        (Join-Path $RepositoryRoot 'editors\vscode'),
        (Join-Path $RepositoryRoot 'gate-artifacts'),
        (Join-Path $RepositoryRoot 'artifacts'),
        (Join-Path $RepositoryRoot 'dist')
    )) {
        if (-not (Test-Path -LiteralPath $directory -PathType Container)) { continue }
        $recurse = $directory -notin @($RepositoryRoot, (Join-Path $RepositoryRoot 'editors\vscode'))
        $items = if ($recurse) {
            @(Get-ChildItem -LiteralPath $directory -Filter '*.vsix' -File -Recurse -ErrorAction SilentlyContinue)
        } else {
            @(Get-ChildItem -LiteralPath $directory -Filter '*.vsix' -File -ErrorAction SilentlyContinue)
        }
        foreach ($item in $items) { [void]$candidateFiles.Add($item) }
    }
    $seen = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
    $valid = New-Object 'System.Collections.Generic.List[object]'
    foreach ($candidate in @($candidateFiles | Sort-Object LastWriteTimeUtc -Descending)) {
        if (-not $seen.Add($candidate.FullName)) { continue }
        try {
            if ((Get-VsixTargetPlatform -Path $candidate.FullName) -eq 'win32-x64') {
                [void]$valid.Add($candidate)
            }
        }
        catch { }
    }
    if ($valid.Count -eq 0) {
        throw 'No packaged win32-x64 VSIX was found. Build or copy one into editors\vscode, gate-artifacts, artifacts, or pass -VsixPath.'
    }
    return $valid[0].FullName
}
function Read-ZipEntryText {
    param(
        [Parameter(Mandatory = $true)]$Archive,
        [Parameter(Mandatory = $true)][string]$Member
    )
    $entry = $Archive.GetEntry($Member)
    if ($null -eq $entry) { throw "VSIX is missing $Member" }
    $stream = $entry.Open()
    $reader = New-Object IO.StreamReader($stream, [Text.Encoding]::UTF8, $true)
    try { return $reader.ReadToEnd() }
    finally { $reader.Dispose(); $stream.Dispose() }
}

function Extract-ZipEntry {
    param(
        [Parameter(Mandatory = $true)]$Archive,
        [Parameter(Mandatory = $true)][string]$Member,
        [Parameter(Mandatory = $true)][string]$Destination
    )
    $entry = $Archive.GetEntry($Member)
    if ($null -eq $entry) { throw "VSIX is missing $Member" }
    [IO.Directory]::CreateDirectory((Split-Path -Parent $Destination)) | Out-Null
    $inputStream = $entry.Open()
    $outputStream = [IO.File]::Open($Destination, [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try { $inputStream.CopyTo($outputStream) }
    finally { $outputStream.Dispose(); $inputStream.Dispose() }
}

function Assert-WindowsPe {
    param([Parameter(Mandatory = $true)][string]$Path)
    $stream = [IO.File]::OpenRead($Path)
    try {
        if ($stream.Length -lt 2 -or $stream.ReadByte() -ne 0x4D -or $stream.ReadByte() -ne 0x5A) {
            throw "$Path is not a Windows PE binary"
        }
    }
    finally { $stream.Dispose() }
}

function Expand-PackagedRuntime {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$DestinationRoot
    )
    $archive = [IO.Compression.ZipFile]::OpenRead($Path)
    try {
        [xml]$manifest = Read-ZipEntryText -Archive $archive -Member 'extension.vsixmanifest'
        $identity = $manifest.SelectSingleNode("//*[local-name()='Identity']")
        if ($null -eq $identity -or $identity.GetAttribute('TargetPlatform') -ne 'win32-x64') {
            $actual = if ($null -eq $identity) { '<missing>' } else { $identity.GetAttribute('TargetPlatform') }
            throw "VSIX TargetPlatform is '$actual'; expected win32-x64"
        }
        $package = (Read-ZipEntryText -Archive $archive -Member 'extension/package.json') | ConvertFrom-Json -ErrorAction Stop
        $version = [string](Get-ObjectProperty -Object $package -Name 'version')
        if ([string]::IsNullOrWhiteSpace($version)) { throw 'VSIX package.json has no version' }

        $binaryEvidence = New-Object 'System.Collections.Generic.List[object]'
        $runtimePath = $null
        foreach ($member in $script:PackagedBinaryMembers) {
            $destination = Join-Path $DestinationRoot ($member.Replace('/', '\'))
            Extract-ZipEntry -Archive $archive -Member $member -Destination $destination
            Assert-WindowsPe -Path $destination
            $file = Get-FileEvidence -Path $destination
            [void]$binaryEvidence.Add([pscustomobject][ordered]@{
                member = $member
                sha256 = $file.sha256
                size_bytes = $file.size_bytes
            })
            if ($member -eq $script:RuntimeMember) { $runtimePath = $destination }
        }
        return [pscustomobject][ordered]@{
            runtime_path = $runtimePath
            package = [pscustomobject][ordered]@{
                path = $Path
                target_platform = 'win32-x64'
                version = $version
                binaries = @($binaryEvidence.ToArray())
            }
        }
    }
    finally { $archive.Dispose() }
}

function Get-TcAdsDllEvidence {
    $programFiles = [Environment]::GetFolderPath('ProgramFiles')
    $programFilesX86 = [Environment]::GetFolderPath('ProgramFilesX86')
    $roots = @(
        (Join-Path $programFiles 'Beckhoff\TwinCAT'),
        (Join-Path $programFilesX86 'Beckhoff\TwinCAT'),
        'C:\TwinCAT',
        'C:\Program Files\Beckhoff\TwinCAT',
        'C:\Program Files (x86)\Beckhoff\TwinCAT'
    )
    $candidates = New-Object 'System.Collections.Generic.List[string]'
    [void]$candidates.Add((Join-Path ([Environment]::SystemDirectory) 'TcAdsDll.dll'))
    foreach ($root in $roots) {
        if ([string]::IsNullOrWhiteSpace($root)) { continue }
        foreach ($relative in @(
            'AdsApi\TcAdsDll\x64\TcAdsDll.dll',
            'AdsApi\TcAdsDll\TcAdsDll.dll',
            '3.1\System\TcAdsDll.dll',
            'Common64\TcAdsDll.dll'
        )) {
            [void]$candidates.Add((Join-Path $root $relative))
        }
    }
    $seen = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
    $found = New-Object 'System.Collections.Generic.List[object]'
    foreach ($candidate in $candidates) {
        if (-not $seen.Add($candidate) -or -not (Test-Path -LiteralPath $candidate -PathType Leaf)) { continue }
        [void]$found.Add((Get-FileEvidence -Path $candidate))
    }
    if ($found.Count -eq 0) {
        throw 'TcAdsDll.dll was not found in the trusted Windows system or TwinCAT installation paths; native ADS success cannot be claimed.'
    }
    return @($found.ToArray())
}

function Get-ProvenNativeCandidates {
    param(
        [Parameter(Mandatory = $true)]$Discovery,
        [Parameter(Mandatory = $true)][string]$ExpectedNetId
    )
    Assert-AmsNetId -Value $ExpectedNetId
    $rawCandidates = @(Get-ObjectProperty -Object $Discovery -Name 'candidates')
    $proven = New-Object 'System.Collections.Generic.List[object]'
    foreach ($candidate in $rawCandidates) {
        if ((Get-ObjectProperty $candidate 'source') -ne 'ads_local_router') { continue }
        if ((Get-ObjectProperty $candidate 'confidence') -ne 'observed') { continue }
        $params = Get-ObjectProperty $candidate 'params'
        if ($null -eq $params) { continue }
        $netId = [string](Get-ObjectProperty $params 'ams_net_id')
        $candidateHost = [string](Get-ObjectProperty $params 'host')
        Assert-AmsNetId -Value $netId
        if ([string]::IsNullOrWhiteSpace($candidateHost)) { throw "Native candidate $netId had no host" }
        [void]$proven.Add([pscustomobject][ordered]@{
            id = Get-ObjectProperty $candidate 'id'
            label = Get-ObjectProperty $candidate 'label'
            source = 'ads_local_router'
            confidence = 'observed'
            name = Get-ObjectProperty $params 'name'
            host = $candidateHost
            ams_net_id = $netId
            discovered_ams_port = Get-ObjectProperty $params 'ams_port'
        })
    }
    $expected = @($proven | Where-Object { $_.ams_net_id -ceq $ExpectedNetId })
    if ($expected.Count -eq 0) {
        throw "Zero proven same-computer native ADS candidates matched expected target $ExpectedNetId. Start the intended TwinCAT runtime; this harness will not accept another local runtime, guess an AMS Net ID, or fall back to raw ADS/TCP."
    }
    if ($expected.Count -ne 1) {
        throw "Native discovery returned $($expected.Count) observed candidates for expected target $ExpectedNetId; duplicate target identity is ambiguous and cannot be accepted."
    }
    return @($expected)
}

function Get-SymbolLeafCount {
    param($Nodes)
    $count = 0
    foreach ($node in @($Nodes)) {
        $children = @(Get-ObjectProperty $node 'children')
        if ($children.Count -gt 0 -and $null -ne $children[0]) {
            $count += Get-SymbolLeafCount $children
        } else {
            $count += 1
        }
    }
    return $count
}

function Invoke-AdsServiceProbe {
    param(
        [Parameter(Mandatory = $true)][string]$Runtime,
        [Parameter(Mandatory = $true)]$Candidate,
        [Parameter(Mandatory = $true)][int]$Port,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds
    )
    $target = [ordered]@{
        host = $Candidate.host
        ams_net_id = $Candidate.ams_net_id
        ams_port = $Port
        name = $Candidate.name
    } | ConvertTo-Json -Compress -Depth 10
    $arguments = @('comm', 'browse-symbols', '--protocol', 'ads', '--target', $target, '--kind', 'symbols', '--json')
    $result = Invoke-CapturedProcess -FilePath $Runtime -Arguments $arguments -TimeoutSeconds $TimeoutSeconds
    $command = New-CommandEvidence $result
    if ($result.timed_out -or $result.exit_code -ne 0) {
        return [pscustomobject][ordered]@{
            port = $Port
            status = 'check_failed'
            responded = $false
            browsable = $false
            symbol_count = 0
            error = if ($result.timed_out) { 'command_timeout' } else { $result.stderr.Trim() }
            command = $command
        }
    }
    try { $response = $result.stdout | ConvertFrom-Json -ErrorAction Stop }
    catch {
        return [pscustomobject][ordered]@{
            port = $Port
            status = 'check_failed'
            responded = $false
            browsable = $false
            symbol_count = 0
            error = "invalid_json: $($_.Exception.Message)"
            command = $command
        }
    }
    $contractError = Get-AdsBrowseResponseContractError -Response $response
    if (-not [string]::IsNullOrWhiteSpace($contractError)) {
        return New-AdsProbeContractFailure -Port $Port -Message $contractError `
            -CommandEvidence $command
    }
    $route = Get-ObjectProperty $response 'route'
    if ($null -ne $route -and (Get-ObjectProperty $route 'status') -eq 'missing') {
        throw "Native same-computer ADS port $Port requested self-route recovery; refusing the result"
    }
    $errorObject = Get-ObjectProperty $response 'error'
    $tree = @(Get-ObjectProperty $response 'tree')
    $symbolCount = Get-SymbolLeafCount $tree
    $classification = Get-AdsBrowseProbeClassification `
        -ErrorObject $errorObject -SymbolCount $symbolCount
    $status = $classification.status
    $responded = $classification.responded
    $browsable = $classification.browsable
    $errorObject = $classification.error
    if ($browsable) {
        if ($null -eq $route -or (Get-ObjectProperty $route 'status') -ne 'not_required') {
            throw "Native same-computer ADS port $Port did not report route.status=not_required"
        }
    }
    return [pscustomobject][ordered]@{
        port = $Port
        status = $status
        responded = $responded
        browsable = $browsable
        symbol_count = $symbolCount
        error = if ($null -eq $errorObject) { $null } else { [pscustomobject][ordered]@{
            code = Get-ObjectProperty $errorObject 'code'
            message = Get-ObjectProperty $errorObject 'message'
        }}
        root_symbols = @($tree | Select-Object -First 20 | ForEach-Object { Get-ObjectProperty $_ 'name' })
        command = $command
    }
}

if ($env:OS -ne 'Windows_NT') {
    throw 'This acceptance harness must run on the Windows computer that hosts TwinCAT.'
}
Assert-AmsNetId -Value $ExpectedTargetNetId
if (-not [string]::IsNullOrWhiteSpace($ExpectedRouterSourceNetId)) {
    Assert-AmsNetId -Value $ExpectedRouterSourceNetId
}
$candidateManifest = Read-WindowsAdsCandidateManifest -Path $CandidateManifestPath
$candidateProvenancePath = Join-Path (Split-Path -Parent $candidateManifest.path) `
    'windows-ads-msvc-candidate-provenance.json'
$candidateProvenance = Read-WindowsAdsCandidateProvenance `
    -Path $candidateProvenancePath -CandidateManifest $candidateManifest
if (-not [string]::IsNullOrWhiteSpace($ExpectedVersion) -and
    $ExpectedVersion -cne $candidateManifest.version) {
    throw "Explicit expected version '$ExpectedVersion' differs from candidate manifest '$($candidateManifest.version)'."
}
if (-not [string]::IsNullOrWhiteSpace($ExpectedVsixSha256) -and
    $ExpectedVsixSha256.ToLowerInvariant() -cne $candidateManifest.vsix_sha256) {
    throw 'Explicit expected VSIX SHA-256 differs from the candidate manifest.'
}
$ExpectedVersion = $candidateManifest.version
$ExpectedVsixSha256 = $candidateManifest.vsix_sha256
Add-Type -AssemblyName System.IO.Compression.FileSystem
$repositoryRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$resolvedEvidencePath = if ([string]::IsNullOrWhiteSpace($EvidencePath)) {
    $stamp = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssZ')
    $localAppData = [Environment]::GetFolderPath([Environment+SpecialFolder]::LocalApplicationData)
    if ([string]::IsNullOrWhiteSpace($localAppData)) {
        throw 'LocalApplicationData is unavailable; pass -EvidencePath to a private location outside the repository.'
    }
    Join-Path $localAppData "truST\private-evidence\windows-twincat-ads\$stamp.json"
} else {
    [IO.Path]::GetFullPath($EvidencePath)
}
$resolvedEvidenceDirectory = Split-Path -Parent $resolvedEvidencePath
$evidenceFileStem = [IO.Path]::GetFileNameWithoutExtension($resolvedEvidencePath)
$simulatorEvidencePath = Join-Path $resolvedEvidenceDirectory `
    ($evidenceFileStem + '-packaged-simulator.json')
$temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) ("trust-windows-ads-acceptance-" + [Guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($temporaryRoot) | Out-Null
$plannedAdsPorts = @(Resolve-AdsAcceptanceProbePorts -Required $script:RequiredAdsPorts `
    -Custom $CustomAdsPorts -Maximum $script:MaxAdsServiceProbes)
$plannedCustomAdsPorts = @($plannedAdsPorts | Where-Object { $script:RequiredAdsPorts -notcontains $_ })
$evidence = [ordered]@{
    schema_version = $script:SchemaVersion
    gate = $script:GateName
    # Private entropy prevents the public whole-file SHA-256 commitment from
    # becoming a dictionary oracle for the redacted AMS identities. The
    # attestation validator requires this value but never publishes it.
    private_evidence_nonce = ([Guid]::NewGuid().ToString('N') + [Guid]::NewGuid().ToString('N'))
    generated_at_utc = Get-UtcTimestamp
    status = 'running'
    host = [ordered]@{
        computer_name = [Environment]::MachineName
        os_version = [Environment]::OSVersion.VersionString
        is_64_bit_os = [Environment]::Is64BitOperatingSystem
        is_64_bit_process = [Environment]::Is64BitProcess
        powershell_version = $PSVersionTable.PSVersion.ToString()
    }
    safety_contract = [ordered]@{
        zero_input_discovery = $true
        manual_ams_net_id_supplied = $false
        native_local_no_self_route_required = $true
        doctor_write_probe_disabled = $true
        imported_binding_read_only_required = $true
        static_routes_byte_identical = $false
        native_reply_required = $true
        packaged_simulator_acceptance_required = $true
        packaged_ads_ui_acceptance_required = $true
        candidate_commit_bound = $true
        candidate_api_provenance_bound = $true
    }
    target_identity_contract = [ordered]@{
        expected_ams_net_id = $ExpectedTargetNetId
        expected_router_source_ams_net_id = if ([string]::IsNullOrWhiteSpace($ExpectedRouterSourceNetId)) { $null } else { $ExpectedRouterSourceNetId }
        discovery_input_supplied = $false
        expected_target_match_count = 0
        source_target_addresses_distinct = $false
        discovery_ui_cli_doctor_match = $false
    }
    probe_ports = @($plannedAdsPorts)
    custom_probe_ports = @($plannedCustomAdsPorts)
    static_routes = $null
    package = $null
    native_client_dlls = @()
    runtime_version = $null
    simulator = $null
    discovery = $null
    targets = @()
    summary = $null
    error = $null
}

$exitCode = 0
$failure = $null
$beforeRoutes = $null
$componentFailures = New-Object 'System.Collections.Generic.List[string]'
try {
    Write-Host '[1/7] Hashing both required TwinCAT Usermode StaticRoutes.xml files...'
    $beforeRoutes = Get-StaticRoutesSnapshot

    Write-Host '[2/7] Selecting and extracting the packaged win32-x64 VSIX...'
    $sourceVsix = Resolve-PackagedVsix -ExplicitPath $VsixPath -RepositoryRoot $repositoryRoot
    $resolvedVsix = Join-Path $temporaryRoot 'accepted-win32-x64.vsix'
    $vsixFile = New-FileSnapshot -Source $sourceVsix -Destination $resolvedVsix
    $expanded = Expand-PackagedRuntime -Path $resolvedVsix -DestinationRoot $temporaryRoot
    $candidateProof = Assert-WindowsAdsCandidateVsix -Manifest $candidateManifest `
        -SourcePath $sourceVsix -Snapshot $vsixFile -Provenance $candidateProvenance `
        -ActualVersion $expanded.package.version `
        -ActualTargetPlatform $expanded.package.target_platform
    $evidence.package = [ordered]@{
        source_path = $sourceVsix
        target_platform = $expanded.package.target_platform
        version = $expanded.package.version
        vsix_sha256 = $vsixFile.sha256
        vsix_size_bytes = $vsixFile.size_bytes
        candidate_manifest = $candidateProof
        candidate_provenance = $candidateProvenance
        binaries = @($expanded.package.binaries)
        simulator_artifact_identity = $null
    }
    $runtime = $expanded.runtime_path

    Write-Host '[3/7] Proving the packaged runtime version and installed native ADS client...'
    $evidence.native_client_dlls = @(Get-TcAdsDllEvidence)
    $versionResult = Invoke-CapturedProcess -FilePath $runtime -Arguments @('--version') -TimeoutSeconds $CommandTimeoutSeconds
    if ($versionResult.exit_code -ne 0 -or $versionResult.timed_out) {
        throw 'Packaged trust-runtime.exe did not report its version successfully.'
    }
    $exactRuntimeVersion = Assert-ExactTrustRuntimeVersion `
        -Output $versionResult.stdout -Expected $expanded.package.version
    $evidence.runtime_version = [ordered]@{
        value = $exactRuntimeVersion
        command = New-CommandEvidence $versionResult
    }

    Write-Host '[4/7] Running packaged Simulator, ADS import, and Live Values acceptance in isolated VS Code...'
    try {
        $simulatorAcceptance = Invoke-PackagedSimulatorAcceptance `
            -ScriptRoot $PSScriptRoot `
            -VsixPath $resolvedVsix `
            -TemporaryRoot $temporaryRoot `
            -TimeoutSeconds $SimulatorTimeoutSeconds `
            -EvidencePath $simulatorEvidencePath `
            -RequireAdsUi `
            -ExpectedAdsTargetNetId $ExpectedTargetNetId `
            -ExpectedCustomAdsPorts $plannedCustomAdsPorts
        $evidence.simulator = $simulatorAcceptance.evidence
        $evidence.package.simulator_artifact_identity = Assert-PackagedSimulatorArtifactIdentity `
            -OuterPackage $evidence.package -SimulatorEvidence $evidence.simulator
        if ($simulatorAcceptance.failed) {
            [void]$componentFailures.Add('Packaged Simulator/ADS/Live Values acceptance failed; inspect evidence.simulator.')
        }
    }
    catch {
        $evidence.simulator = [ordered]@{
            status = 'fail'
            error = "$($_.Exception.GetType().Name): $($_.Exception.Message)"
        }
        [void]$componentFailures.Add('Packaged Simulator/ADS/Live Values acceptance failed; inspect evidence.simulator.')
    }

    Write-Host '[5/7] Running zero-input same-computer plus LAN ADS discovery...'
    $discoveryArguments = @('comm', 'discover', '--protocol', 'ads', '--origin', 'this-host', '--json')
    $discoveryResult = Invoke-CapturedProcess -FilePath $runtime -Arguments $discoveryArguments -TimeoutSeconds $CommandTimeoutSeconds
    $discovery = Convert-CommandJson -Result $discoveryResult -Context 'zero-input ADS discovery'
    $nativeCandidates = @(Get-ProvenNativeCandidates -Discovery $discovery `
        -ExpectedNetId $ExpectedTargetNetId)
    $evidence.target_identity_contract.expected_target_match_count = $nativeCandidates.Count
    $evidence.discovery = [ordered]@{
        command = New-CommandEvidence $discoveryResult
        warnings = @(Get-ObjectProperty $discovery 'warnings')
        total_candidate_count = @((Get-ObjectProperty $discovery 'candidates')).Count
        proven_native_candidate_count = $nativeCandidates.Count
        proven_native_candidates = @($nativeCandidates)
        packaged_ui_crosscheck = $null
    }
    Write-Host "[6/7] Probing ADS services $($evidence.probe_ports -join ', ') and proving port 851..."
    $targetEvidence = New-Object 'System.Collections.Generic.List[object]'
    $fullProofCount = 0
    foreach ($candidate in $nativeCandidates) {
        $services = New-Object 'System.Collections.Generic.List[object]'
        foreach ($port in $evidence.probe_ports) {
            [void]$services.Add((Invoke-AdsServiceProbe -Runtime $runtime -Candidate $candidate -Port $port -TimeoutSeconds $CommandTimeoutSeconds))
        }
        $respondingPorts = @($services | Where-Object { $_.responded } | ForEach-Object { $_.port })
        $port851 = @($services | Where-Object { $_.port -eq 851 })[0]
        $doctor = $null
        if ($port851.browsable) {
            $doctor = Invoke-Doctor851 -Runtime $runtime -Candidate $candidate `
                -ExpectedTargetNetId $ExpectedTargetNetId `
                -ExpectedRouterSourceNetId $ExpectedRouterSourceNetId `
                -TimeoutSeconds ([Math]::Max($CommandTimeoutSeconds, 60))
            $fullProofCount += 1
        }
        [void]$targetEvidence.Add([pscustomobject][ordered]@{
            candidate = $candidate
            responding_ads_ports = @($respondingPorts)
            services = @($services.ToArray())
            doctor_851 = $doctor
            accepted_851 = $null -ne $doctor
        })
    }
    $evidence.targets = @($targetEvidence.ToArray())
    $uiCrosscheck = Test-PackagedAdsUiAgainstCliServices `
        -SimulatorEvidence $evidence.simulator -CliTargets $evidence.targets `
        -ExpectedTargetNetId $ExpectedTargetNetId -ExpectedAdsPorts $evidence.probe_ports
    $evidence.discovery.packaged_ui_crosscheck = $uiCrosscheck.evidence
    if (-not $uiCrosscheck.passed) {
        [void]$componentFailures.Add(
            'Packaged VS Code ADS service statuses or ADS 851 variables did not match the same target proven by the packaged CLI.'
        )
    }
    $evidence.target_identity_contract.discovery_ui_cli_doctor_match = `
        $uiCrosscheck.passed -and $fullProofCount -eq 1
    $provenDoctorRows = @($targetEvidence | Where-Object { $null -ne $_.doctor_851 })
    $evidence.target_identity_contract.source_target_addresses_distinct = `
        $provenDoctorRows.Count -eq 1 -and `
        $provenDoctorRows[0].doctor_851.source_target_addresses_distinct -eq $true
    if ($fullProofCount -eq 0) {
        throw 'Native ADS discovery succeeded, but no proven candidate exposed a browsable symbol table on ADS port 851. Real TwinCAT port-851 acceptance cannot be claimed.'
    }
    $evidence.summary = [ordered]@{
        packaged_simulator = if ($null -eq $evidence.simulator) { 'not_run' } else { Get-ObjectProperty -Object $evidence.simulator -Name 'status' }
        proven_native_candidates = $nativeCandidates.Count
        candidates_with_full_851_proof = $fullProofCount
        responding_ports_by_target = @($targetEvidence | ForEach-Object { [ordered]@{
            ams_net_id = $_.candidate.ams_net_id
            ports = @($_.responding_ads_ports)
        }})
    }
    if ($componentFailures.Count -gt 0) {
        throw ($componentFailures -join '; ')
    }
}
catch {
    $exitCode = 1
    $failure = "$($_.Exception.GetType().Name): $($_.Exception.Message)"
}
finally {
    Write-Host '[7/7] Re-hashing TwinCAT StaticRoutes.xml and writing evidence...'
    try {
        if ($null -eq $beforeRoutes) {
            throw 'The before snapshot was not completed; route byte identity cannot be proven.'
        }
        $afterRoutes = Get-StaticRoutesSnapshot
        $comparison = Compare-StaticRoutesSnapshots -Before $beforeRoutes -After $afterRoutes
        $evidence.static_routes = [ordered]@{
            expected_paths = @($beforeRoutes.expected_paths)
            before = @($beforeRoutes.files)
            after = @($afterRoutes.files)
            comparison = $comparison
        }
        $evidence.safety_contract.static_routes_byte_identical = $comparison.byte_identical
        if (-not $comparison.byte_identical) {
            throw 'One or more TwinCAT StaticRoutes.xml files changed, appeared, or disappeared during acceptance.'
        }
    }
    catch {
        $exitCode = 1
        $routeFailure = "$($_.Exception.GetType().Name): $($_.Exception.Message)"
        $failure = if ([string]::IsNullOrWhiteSpace($failure)) { $routeFailure } else { "$failure; $routeFailure" }
    }

    $evidence.status = if ($exitCode -eq 0) { 'pass' } else { 'fail' }
    $evidence.error = $failure
    $evidence.completed_at_utc = Get-UtcTimestamp
    [IO.Directory]::CreateDirectory($resolvedEvidenceDirectory) | Out-Null
    $json = $evidence | ConvertTo-Json -Depth 100
    $encoding = New-Object Text.UTF8Encoding($false)
    [IO.File]::WriteAllText($resolvedEvidencePath, ($json + [Environment]::NewLine), $encoding)
    if (Test-Path -LiteralPath $temporaryRoot) {
        Remove-Item -LiteralPath $temporaryRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$resultSummary = [ordered]@{
    status = $evidence.status
    evidence = $resolvedEvidencePath
    proven_native_candidates = if ($null -eq $evidence.discovery) { 0 } else { $evidence.discovery.proven_native_candidate_count }
    packaged_simulator = if ($null -eq $evidence.simulator) { 'not_run' } else { Get-ObjectProperty -Object $evidence.simulator -Name 'status' }
    route_byte_identical = $evidence.safety_contract.static_routes_byte_identical
    error = $evidence.error
}
$resultSummary | ConvertTo-Json -Compress
exit $exitCode
