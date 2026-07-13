#requires -Version 5.1
<#
.SYNOPSIS
Runs the packaged truST Simulator Start/Stop journey in a real isolated VS Code
instance on Windows.

.DESCRIPTION
The script extracts the exact win32-x64 VSIX, creates a disposable legacy
tokenless local project, and drives the real truST sidebar and Devices canvas.
It proves that Start migrates runtime.control authentication, launches exactly
one Structured Text debug session, keeps Devices open instead of opening Live
Values, and keeps sidebar, canvas, and status bar in agreement through Stop.
With -RequireAdsUi it also performs one-click ADS discovery, browses ADS 851,
imports one read-only variable, restarts the Simulator, and proves that the
same Good value is rendered under Live Values > Connected variables > ADS.

No extension is installed into the user's profile and the disposable project is
removed after evidence is written.

.EXAMPLE
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\accept_windows_packaged_simulator.ps1 `
  -VsixPath C:\proof\trust-lsp-win32-x64.vsix
#>

[CmdletBinding()]
param(
    [Parameter()]
    [string]$VsixPath,

    [Parameter()]
    [string]$EvidencePath,

    [Parameter()]
    [switch]$RequireAdsUi,

    [Parameter()]
    [ValidatePattern('^[0-9]{1,3}(?:\.[0-9]{1,3}){5}$')]
    [string]$ExpectedAdsTargetNetId,

    [Parameter()]
    [ValidatePattern('^[0-9]{1,5}(?:,[0-9]{1,5}){0,3}$')]
    [string]$ExpectedCustomAdsPorts,

    [Parameter()]
    [ValidateRange(60, 600)]
    [int]$TimeoutSeconds = 420
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'windows_twincat_ads_acceptance\AcceptanceIo.psm1') -Force
Import-Module (Join-Path $PSScriptRoot 'windows_twincat_ads_acceptance\PackagedExtensionInstall.psm1') -Force
Import-Module (Join-Path $PSScriptRoot 'windows_twincat_ads_acceptance\AcceptancePlan.psm1') -Force
Add-Type -AssemblyName System.IO.Compression.FileSystem

$script:SchemaVersion = 1
$script:GateName = 'windows_packaged_simulator_acceptance'
$script:RequiredAdsPorts = @(851, 852, 853, 854, 301, 501)
$script:MaxAdsServiceProbes = 10
$script:RequiredMembers = @(
    'extension/package.json',
    'extension/out/extension.js',
    'extension/bin/trust-runtime.exe',
    'extension/bin/trust-debug.exe',
    'extension/bin/trust-lsp.exe'
)

function Resolve-WindowsVsix {
    param(
        [string]$ExplicitPath,
        [Parameter(Mandatory = $true)][string]$RepositoryRoot
    )
    if (-not [string]::IsNullOrWhiteSpace($ExplicitPath)) {
        return (Resolve-Path -LiteralPath $ExplicitPath -ErrorAction Stop).Path
    }
    $candidates = New-Object 'System.Collections.Generic.List[object]'
    foreach ($directory in @(
        $RepositoryRoot,
        (Join-Path $RepositoryRoot 'editors\vscode'),
        (Join-Path $RepositoryRoot 'gate-artifacts'),
        (Join-Path $RepositoryRoot 'artifacts'),
        (Join-Path $RepositoryRoot 'dist')
    )) {
        if (-not (Test-Path -LiteralPath $directory -PathType Container)) { continue }
        foreach ($item in @(Get-ChildItem -LiteralPath $directory -Filter '*.vsix' -File -Recurse -ErrorAction SilentlyContinue)) {
            [void]$candidates.Add($item)
        }
    }
    foreach ($candidate in @($candidates | Sort-Object LastWriteTimeUtc -Descending)) {
        try {
            $metadata = Get-VsixMetadata -Path $candidate.FullName
            if ($metadata.target_platform -eq 'win32-x64') { return $candidate.FullName }
        }
        catch { }
    }
    throw 'No packaged win32-x64 VSIX was found. Pass -VsixPath or place it under editors\vscode, gate-artifacts, artifacts, or dist.'
}

function Read-ZipText {
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

function Get-VsixMetadata {
    param([Parameter(Mandatory = $true)][string]$Path)
    $archive = [IO.Compression.ZipFile]::OpenRead($Path)
    try {
        foreach ($member in $script:RequiredMembers) {
            if ($null -eq $archive.GetEntry($member)) { throw "VSIX is missing $member" }
        }
        [xml]$manifest = Read-ZipText -Archive $archive -Member 'extension.vsixmanifest'
        $identity = $manifest.SelectSingleNode("//*[local-name()='Identity']")
        if ($null -eq $identity) { throw 'VSIX manifest has no Identity element' }
        $package = (Read-ZipText -Archive $archive -Member 'extension/package.json') | ConvertFrom-Json -ErrorAction Stop
        $version = [string](Get-ObjectProperty -Object $package -Name 'version')
        if ([string]::IsNullOrWhiteSpace($version)) { throw 'VSIX package.json has no version' }
        return [pscustomobject][ordered]@{
            target_platform = $identity.GetAttribute('TargetPlatform')
            version = $version
        }
    }
    finally { $archive.Dispose() }
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

function Get-FreeTcpPort {
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    try {
        $listener.Start()
        return [int]$listener.LocalEndpoint.Port
    }
    finally { $listener.Stop() }
}

function Get-RuntimeControlAuthTokenEvidence {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Source)
    $table = [regex]::Match(
        $Source,
        '(?ms)^\s*\[runtime\.control\]\s*(?:#[^\r\n]*)?\r?\n(?<body>.*?)(?=^\s*\[|\z)'
    )
    $token = if ($table.Success) {
        [regex]::Match($table.Groups['body'].Value, '(?m)^\s*auth_token\s*=\s*["'']([^"'']*)["'']')
    } else {
        [regex]::Match($Source, '(?m)^\s*runtime\.control\.auth_token\s*=\s*["'']([^"'']*)["'']')
    }
    return [pscustomobject][ordered]@{
        present = $token.Success
        length = if ($token.Success) { $token.Groups[1].Value.Length } else { 0 }
        value = if ($token.Success) { $token.Groups[1].Value } else { '' }
    }
}

function New-TokenlessSimulatorProject {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][int]$ControlPort
    )
    [IO.Directory]::CreateDirectory((Join-Path $Root 'src')) | Out-Null
    [IO.Directory]::CreateDirectory((Join-Path $Root '.vscode')) | Out-Null
    Write-Utf8File -Path (Join-Path $Root 'trust-lsp.toml') -Content @'
include_paths = ["src"]
'@
    Write-Utf8File -Path (Join-Path $Root 'io.toml') -Content @'
[io]
driver = "simulated"
params = {}
'@
    Write-Utf8File -Path (Join-Path $Root '.vscode\settings.json') -Content @'
{
  "debug.showInStatusBar": "never"
}
'@
    # A non-trivial but valid program keeps the real Compile/preflight phase
    # observable long enough to prove the Start operation lock without a test
    # hook in production code.
    $program = New-Object Text.StringBuilder
    [void]$program.AppendLine('PROGRAM Main')
    [void]$program.AppendLine('VAR')
    [void]$program.AppendLine('    counter : DINT := 0;')
    [void]$program.AppendLine('END_VAR')
    foreach ($index in 1..3000) {
        [void]$program.AppendLine('counter := counter + 1;')
    }
    [void]$program.AppendLine('END_PROGRAM')
    Write-Utf8File -Path (Join-Path $Root 'src\Main.st') -Content $program.ToString()
    Write-Utf8File -Path (Join-Path $Root 'src\config.st') -Content @'
CONFIGURATION Config
RESOURCE MainRes ON PLC
    TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
'@
    $runtime = @"
[bundle]
version = 1

[resource]
name = "Simulator"
cycle_interval_ms = 10

[runtime.control]
endpoint = "tcp://127.0.0.1:$ControlPort"
mode = "production"
debug_enabled = false

[runtime.web]
enabled = false
listen = "127.0.0.1:8080"
auth = "local"
tls = false

[runtime.tls]
mode = "disabled"
require_remote = false

[runtime.discovery]
enabled = false
service_name = "truST"
advertise = false
interfaces = []

[runtime.mesh]
enabled = false
listen = "0.0.0.0:5200"
tls = false
auth_token = ""
publish = []

[runtime.observability]
enabled = false
sample_interval_ms = 1000
mode = "all"
include = []
history_path = "history/historian.jsonl"
max_entries = 20000
prometheus_enabled = true
prometheus_path = "/metrics"

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 1000
action = "halt"

[runtime.fault]
policy = "halt"
"@
    Write-Utf8File -Path (Join-Path $Root 'runtime.toml') -Content $runtime
}

function Resolve-VscodeExecutable {
    param([Parameter(Mandatory = $true)][string]$RepositoryRoot)
    $candidates = New-Object 'System.Collections.Generic.List[string]'
    if (-not [string]::IsNullOrWhiteSpace($env:TRUST_VSCODE_EXECUTABLE)) {
        [void]$candidates.Add($env:TRUST_VSCODE_EXECUTABLE)
    }
    $testRoot = Join-Path $RepositoryRoot 'editors\vscode\.vscode-test'
    if (Test-Path -LiteralPath $testRoot -PathType Container) {
        foreach ($item in @(Get-ChildItem -LiteralPath $testRoot -Filter 'Code.exe' -File -Recurse -ErrorAction SilentlyContinue | Sort-Object LastWriteTimeUtc -Descending)) {
            [void]$candidates.Add($item.FullName)
        }
    }
    foreach ($registryPath in @(
        'HKCU:\Software\Microsoft\Windows\CurrentVersion\App Paths\Code.exe',
        'HKLM:\Software\Microsoft\Windows\CurrentVersion\App Paths\Code.exe'
    )) {
        try {
            $value = (Get-Item -LiteralPath $registryPath -ErrorAction Stop).GetValue('')
            if (-not [string]::IsNullOrWhiteSpace([string]$value)) { [void]$candidates.Add([string]$value) }
        }
        catch { }
    }
    foreach ($candidate in @(
        (Join-Path $env:LOCALAPPDATA 'Programs\Microsoft VS Code\Code.exe'),
        (Join-Path $env:ProgramFiles 'Microsoft VS Code\Code.exe'),
        (Join-Path ${env:ProgramFiles(x86)} 'Microsoft VS Code\Code.exe')
    )) {
        if (-not [string]::IsNullOrWhiteSpace($candidate)) { [void]$candidates.Add($candidate) }
    }
    foreach ($name in @('code.cmd', 'code')) {
        try {
            $command = Get-Command $name -ErrorAction Stop | Select-Object -First 1
            $source = [string]$command.Source
            if ($source -match '(?i)[\\/]bin[\\/]code(?:\.cmd)?$') {
                [void]$candidates.Add((Join-Path (Split-Path -Parent (Split-Path -Parent $source)) 'Code.exe'))
            }
        }
        catch { }
    }
    $seen = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
    foreach ($candidate in $candidates) {
        if ([string]::IsNullOrWhiteSpace($candidate)) { continue }
        try { $resolved = (Resolve-Path -LiteralPath $candidate -ErrorAction Stop).Path }
        catch { continue }
        if ($seen.Add($resolved)) { return $resolved }
    }
    throw 'Visual Studio Code was not found. Install VS Code or set TRUST_VSCODE_EXECUTABLE to Code.exe.'
}

function Invoke-VscodeAcceptance {
    param(
        [Parameter(Mandatory = $true)][string]$Executable,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][int]$Timeout
    )
    $startInfo = New-Object Diagnostics.ProcessStartInfo
    $startInfo.FileName = $Executable
    $startInfo.Arguments = (@($Arguments | ForEach-Object { ConvertTo-NativeArgument $_ }) -join ' ')
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $false
    $process = New-Object Diagnostics.Process
    $process.StartInfo = $startInfo
    $timer = [Diagnostics.Stopwatch]::StartNew()
    if (-not $process.Start()) { throw "Failed to start $Executable" }
    $processId = $process.Id
    $timedOut = -not $process.WaitForExit($Timeout * 1000)
    if ($timedOut) {
        try { & (Join-Path $env:SystemRoot 'System32\taskkill.exe') /PID $processId /T /F | Out-Null } catch { }
        try { $process.WaitForExit(10000) | Out-Null } catch { }
    }
    $timer.Stop()
    $exitCode = if ($timedOut -or -not $process.HasExited) { -1 } else { $process.ExitCode }
    $process.Dispose()
    return [pscustomobject][ordered]@{
        executable = $Executable
        arguments = @($Arguments)
        exit_code = $exitCode
        timed_out = $timedOut
        duration_ms = [Int64]$timer.ElapsedMilliseconds
    }
}

function Get-SafeLauncherEvidence {
    param([Parameter(Mandatory = $true)]$Result)
    return [pscustomobject][ordered]@{
        executable = $Result.executable
        exit_code = $Result.exit_code
        timed_out = $Result.timed_out
        duration_ms = $Result.duration_ms
        argument_names = @($Result.arguments | ForEach-Object {
            if ($_ -match '^--([^=]+)=') { return "--$($Matches[1])=<path-or-value>" }
            if ($_ -match '^--') { return $_ }
            return '<workspace-path>'
        })
    }
}

if ($env:OS -ne 'Windows_NT') {
    throw 'This packaged simulator acceptance must run on Windows.'
}

$repositoryRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$resolvedEvidencePath = if ([string]::IsNullOrWhiteSpace($EvidencePath)) {
    $stamp = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssZ')
    $localAppData = [Environment]::GetFolderPath([Environment+SpecialFolder]::LocalApplicationData)
    if ([string]::IsNullOrWhiteSpace($localAppData)) {
        throw 'LocalApplicationData is unavailable; pass -EvidencePath to a private location outside the repository.'
    }
    Join-Path $localAppData "truST\private-evidence\windows-packaged-simulator\$stamp.json"
} else {
    [IO.Path]::GetFullPath($EvidencePath)
}
$resolvedEvidenceDirectory = Split-Path -Parent $resolvedEvidencePath
$evidenceFileStem = [IO.Path]::GetFileNameWithoutExtension($resolvedEvidencePath)
$screenshotOutputDirectory = Join-Path $resolvedEvidenceDirectory ($evidenceFileStem + '-screenshots')
$temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) ("trust-packaged-simulator-" + [Guid]::NewGuid().ToString('N'))
$expandedRoot = Join-Path $temporaryRoot 'vsix'
$projectRoot = Join-Path $temporaryRoot 'project'
$userDataRoot = Join-Path $temporaryRoot 'user-data'
$extensionsRoot = Join-Path $temporaryRoot 'extensions'
$driverRoot = Join-Path $temporaryRoot 'acceptance-driver'
$innerEvidencePath = Join-Path $temporaryRoot 'extension-host-evidence.json'
$exitCode = 0
$failure = $null
$migratedControlAuthToken = ''
$finalEvidence = [ordered]@{
    schema_version = $script:SchemaVersion
    gate = $script:GateName
    generated_at_utc = Get-UtcTimestamp
    status = 'running'
    host = [ordered]@{
        computer_name = [Environment]::MachineName
        os_version = [Environment]::OSVersion.VersionString
        is_64_bit_os = [Environment]::Is64BitOperatingSystem
        is_64_bit_process = [Environment]::Is64BitProcess
        powershell_version = $PSVersionTable.PSVersion.ToString()
    }
    package = $null
    vscode = $null
    launcher = $null
    extension_host = $null
    screenshots = [ordered]@{
        path = $screenshotOutputDirectory
        count = 0
    }
    safety_contract = [ordered]@{
        isolated_user_data = $true
        extension_installed_to_user_profile = $false
        disposable_tokenless_project = $true
        credential_value_recorded = $false
        exact_credential_scan_performed = $false
        isolated_vsix_installed = $false
        production_extension_mode_required = $true
        binary_settings_pinned = $false
        path_fallback_blocked = $false
        test_binary_overrides_cleared = $false
        temporary_root_cleanup_verified = $false
        cleanup_failure_credential_removed = $null
    }
    error = $null
}

$environmentNames = @(
    'TRUST_PACKAGED_EXTENSION_ROOT',
    'TRUST_PACKAGED_SIMULATOR_PROJECT',
    'TRUST_PACKAGED_SIMULATOR_EVIDENCE',
    'TRUST_PACKAGED_SIMULATOR_VERSION',
    'TRUST_PACKAGED_SIMULATOR_CDP_PORT',
    'TRUST_PACKAGED_SIMULATOR_SCREENSHOT_DIR',
    'TRUST_PACKAGED_ADS_UI_REQUIRED',
    'TRUST_PACKAGED_ADS_EXPECTED_TARGET_NET_ID',
    'TRUST_PACKAGED_ADS_EXPECTED_CUSTOM_PORTS',
    'TRUST_PACKAGED_PATH_FALLBACK_BLOCKED',
    'PATH',
    'CARGO_TARGET_DIR',
    'ST_LSP_TEST_SERVER',
    'ST_RUNTIME_TEST_BIN',
    'ST_DEBUG_TEST_BIN'
)
$previousEnvironment = @{}
foreach ($name in $environmentNames) {
    $previousEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}

try {
    if (Test-Path -LiteralPath $screenshotOutputDirectory) {
        Remove-Item -LiteralPath $screenshotOutputDirectory -Recurse -Force -ErrorAction Stop
    }
    [IO.Directory]::CreateDirectory($screenshotOutputDirectory) | Out-Null
    if ($RequireAdsUi.IsPresent -and [string]::IsNullOrWhiteSpace($ExpectedAdsTargetNetId)) {
        throw 'RequireAdsUi also requires ExpectedAdsTargetNetId so an arbitrary local runtime cannot satisfy acceptance.'
    }
    $validatedExpectedCustomAdsPorts = @()
    if ($RequireAdsUi.IsPresent) {
        if ([string]::IsNullOrWhiteSpace($ExpectedCustomAdsPorts)) {
            throw 'RequireAdsUi also requires ExpectedCustomAdsPorts so custom-port discovery is proven.'
        }
        $requestedCustomPorts = @($ExpectedCustomAdsPorts.Split(',') | ForEach-Object { [int]$_ })
        $plannedPorts = @(Resolve-AdsAcceptanceProbePorts -Required $script:RequiredAdsPorts `
            -Custom $requestedCustomPorts -Maximum $script:MaxAdsServiceProbes)
        $validatedExpectedCustomAdsPorts = @(
            $plannedPorts | Where-Object { $script:RequiredAdsPorts -notcontains $_ }
        )
        if ($validatedExpectedCustomAdsPorts.Count -ne $requestedCustomPorts.Count) {
            throw 'ExpectedCustomAdsPorts must contain only unique, non-default ADS service ports.'
        }
    } elseif (-not [string]::IsNullOrWhiteSpace($ExpectedCustomAdsPorts)) {
        throw 'ExpectedCustomAdsPorts may only be supplied with RequireAdsUi.'
    }
    [IO.Directory]::CreateDirectory($expandedRoot) | Out-Null
    [IO.Directory]::CreateDirectory($extensionsRoot) | Out-Null
    $resolvedVsix = Resolve-WindowsVsix -ExplicitPath $VsixPath -RepositoryRoot $repositoryRoot
    $metadata = Get-VsixMetadata -Path $resolvedVsix
    if ($metadata.target_platform -ne 'win32-x64') {
        throw "VSIX TargetPlatform is '$($metadata.target_platform)'; expected win32-x64"
    }
    [IO.Compression.ZipFile]::ExtractToDirectory($resolvedVsix, $expandedRoot)
    $extractedExtensionRoot = Join-Path $expandedRoot 'extension'
    foreach ($binary in @('trust-runtime.exe', 'trust-debug.exe', 'trust-lsp.exe')) {
        Assert-WindowsPe -Path (Join-Path $extractedExtensionRoot "bin\$binary")
    }
    $vsixEvidence = Get-FileEvidence -Path $resolvedVsix
    $finalEvidence.package = [ordered]@{
        path = $resolvedVsix
        version = $metadata.version
        target_platform = $metadata.target_platform
        sha256 = $vsixEvidence.sha256
        size_bytes = $vsixEvidence.size_bytes
        extension_js = Get-FileEvidence -Path (Join-Path $extractedExtensionRoot 'out\extension.js')
        runtime = Get-FileEvidence -Path (Join-Path $extractedExtensionRoot 'bin\trust-runtime.exe')
        debug = Get-FileEvidence -Path (Join-Path $extractedExtensionRoot 'bin\trust-debug.exe')
        lsp = Get-FileEvidence -Path (Join-Path $extractedExtensionRoot 'bin\trust-lsp.exe')
    }

    $vscode = Resolve-VscodeExecutable -RepositoryRoot $repositoryRoot
    $vscodeFile = Get-FileEvidence -Path $vscode
    $vscodeCliLayout = Resolve-VscodeCliLayout -Vscode $vscode
    $versionResult = Invoke-VscodeCli -Vscode $vscode -Arguments @('--version') -TimeoutSeconds 30
    $versionLines = @($versionResult.stdout -split '\r?\n' | Where-Object {
        -not [string]::IsNullOrWhiteSpace($_)
    })
    $vscodeVersion = if ($versionLines.Count -gt 0) { [string]$versionLines[0] } else { $null }
    $finalEvidence.vscode = [ordered]@{
        path = $vscode
        version = $vscodeVersion
        sha256 = $vscodeFile.sha256
        size_bytes = $vscodeFile.size_bytes
        cli_launcher = Get-FileEvidence -Path $vscodeCliLayout.launcher
        cli_script = Get-FileEvidence -Path ([string]$versionResult.arguments[0])
        cli_package = Get-FileEvidence -Path $vscodeCliLayout.package_json
        version_probe = New-CommandEvidence $versionResult
        install = $null
    }
    if ($versionResult.timed_out -or $versionResult.exit_code -ne 0) {
        throw (
            "Visual Studio Code CLI version probe failed " +
            "(timed_out=$($versionResult.timed_out), exit_code=$($versionResult.exit_code))."
        )
    }
    if ([string]::IsNullOrWhiteSpace($vscodeVersion)) {
        throw 'Visual Studio Code CLI returned no version.'
    }

    [IO.Directory]::CreateDirectory($userDataRoot) | Out-Null
    $installation = Install-IsolatedPackagedExtension -Vscode $vscode -Vsix $resolvedVsix `
        -ExtensionsRoot $extensionsRoot -UserDataRoot $userDataRoot `
        -ExpectedVersion $metadata.version -ExtractedRoot $extractedExtensionRoot `
        -VsixManifestPath (Join-Path $expandedRoot 'extension.vsixmanifest')
    $extensionRoot = [string]$installation.extension_root
    $finalEvidence.vscode.install = $installation.command
    $finalEvidence.safety_contract.isolated_vsix_installed = $true
    $finalEvidence.package.installed_payload_matches_vsix = $true
    $finalEvidence.package.installed_executed_files_byte_identical = $true
    New-AcceptanceDriverExtension -Root $driverRoot

    $controlPort = Get-FreeTcpPort
    $cdpPort = Get-FreeTcpPort
    while ($cdpPort -eq $controlPort) { $cdpPort = Get-FreeTcpPort }
    New-TokenlessSimulatorProject -Root $projectRoot -ControlPort $controlPort
    New-IsolatedUserData -Root $userDataRoot -ExtensionRoot $extensionRoot
    $finalEvidence.safety_contract.binary_settings_pinned = $true
    Disable-PackagedBinaryPathFallback
    $finalEvidence.safety_contract.path_fallback_blocked = $true
    $finalEvidence.safety_contract.test_binary_overrides_cleared = $true
    $beforeRuntime = Get-Content -LiteralPath (Join-Path $projectRoot 'runtime.toml') -Raw
    $beforeControlAuth = Get-RuntimeControlAuthTokenEvidence -Source $beforeRuntime
    if ($beforeControlAuth.present) {
        throw 'The packaged Simulator acceptance fixture did not begin tokenless.'
    }

    [Environment]::SetEnvironmentVariable('TRUST_PACKAGED_EXTENSION_ROOT', $extensionRoot, 'Process')
    [Environment]::SetEnvironmentVariable('TRUST_PACKAGED_SIMULATOR_PROJECT', $projectRoot, 'Process')
    [Environment]::SetEnvironmentVariable('TRUST_PACKAGED_SIMULATOR_EVIDENCE', $innerEvidencePath, 'Process')
    [Environment]::SetEnvironmentVariable('TRUST_PACKAGED_SIMULATOR_VERSION', $metadata.version, 'Process')
    [Environment]::SetEnvironmentVariable('TRUST_PACKAGED_SIMULATOR_CDP_PORT', [string]$cdpPort, 'Process')
    [Environment]::SetEnvironmentVariable('TRUST_PACKAGED_SIMULATOR_SCREENSHOT_DIR', $screenshotOutputDirectory, 'Process')
    [Environment]::SetEnvironmentVariable('TRUST_PACKAGED_PATH_FALLBACK_BLOCKED', '1', 'Process')
    [Environment]::SetEnvironmentVariable(
        'TRUST_PACKAGED_ADS_UI_REQUIRED',
        $(if ($RequireAdsUi.IsPresent) { '1' } else { '0' }),
        'Process'
    )
    [Environment]::SetEnvironmentVariable(
        'TRUST_PACKAGED_ADS_EXPECTED_TARGET_NET_ID',
        $(if ($RequireAdsUi.IsPresent) { $ExpectedAdsTargetNetId } else { '' }),
        'Process'
    )
    [Environment]::SetEnvironmentVariable(
        'TRUST_PACKAGED_ADS_EXPECTED_CUSTOM_PORTS',
        $(if ($RequireAdsUi.IsPresent) { $validatedExpectedCustomAdsPorts -join ',' } else { '' }),
        'Process'
    )

    $testPath = Join-Path $PSScriptRoot 'windows_twincat_ads_acceptance\PackagedSimulatorAcceptance.js'
    $arguments = @(
        $projectRoot,
        "--user-data-dir=$userDataRoot",
        "--extensions-dir=$extensionsRoot",
        '--no-sandbox',
        '--disable-gpu-sandbox',
        '--disable-updates',
        '--skip-welcome',
        '--skip-release-notes',
        '--disable-workspace-trust',
        "--remote-debugging-port=$cdpPort",
        "--extensionTestsPath=$testPath",
        "--extensionDevelopmentPath=$driverRoot"
    )
    $run = Invoke-VscodeAcceptance -Executable $vscode -Arguments $arguments -Timeout $TimeoutSeconds
    $finalEvidence.launcher = Get-SafeLauncherEvidence -Result $run
    if (-not (Test-Path -LiteralPath $innerEvidencePath -PathType Leaf)) {
        throw 'The packaged VS Code Extension Host did not write acceptance evidence.'
    }
    $innerRaw = Get-Content -LiteralPath $innerEvidencePath -Raw
    $runtimeTomlPath = Join-Path $projectRoot 'runtime.toml'
    if (Test-Path -LiteralPath $runtimeTomlPath -PathType Leaf) {
        $afterRuntime = Get-Content -LiteralPath $runtimeTomlPath -Raw
        $afterControlAuth = Get-RuntimeControlAuthTokenEvidence -Source $afterRuntime
        if ($afterControlAuth.present) {
            $migratedControlAuthToken = [string]$afterControlAuth.value
        }
    }
    if (-not [string]::IsNullOrEmpty($migratedControlAuthToken)) {
        $finalEvidence.safety_contract.exact_credential_scan_performed = $true
        if ($innerRaw.Contains($migratedControlAuthToken)) {
            $finalEvidence.safety_contract.credential_value_recorded = $true
            throw 'The packaged VS Code Simulator evidence contained the disposable runtime control credential.'
        }
    }
    $inner = $innerRaw | ConvertFrom-Json -ErrorAction Stop
    $finalEvidence.extension_host = $inner
    if (-not [string]::IsNullOrEmpty($migratedControlAuthToken)) {
        $prospectiveFinalJson = $finalEvidence | ConvertTo-Json -Depth 100
        if ($prospectiveFinalJson.Contains($migratedControlAuthToken)) {
            $finalEvidence.safety_contract.credential_value_recorded = $true
            throw 'The packaged Simulator evidence contained the disposable runtime control credential.'
        }
    }
    if ($run.timed_out) { throw 'The packaged VS Code Simulator journey timed out.' }
    if ($run.exit_code -ne 0) { throw "The packaged VS Code Simulator journey exited with $($run.exit_code)." }
    if ((Get-ObjectProperty -Object $inner -Name 'status') -ne 'pass') {
        $innerError = [string](Get-ObjectProperty -Object $inner -Name 'error')
        throw "The packaged VS Code Simulator journey failed: $innerError"
    }
    if ([string]::IsNullOrEmpty($migratedControlAuthToken) -or $migratedControlAuthToken.Length -lt 24) {
        throw 'The packaged Start journey did not persist a strong runtime.control auth token.'
    }
}
catch {
    $exitCode = 1
    $failure = "$($_.Exception.GetType().Name): $($_.Exception.Message)"
}
finally {
    foreach ($name in $environmentNames) {
        [Environment]::SetEnvironmentVariable($name, $previousEnvironment[$name], 'Process')
    }
    $finalEvidence.screenshots.count = if (Test-Path -LiteralPath $screenshotOutputDirectory -PathType Container) {
        @(Get-ChildItem -LiteralPath $screenshotOutputDirectory -File -Recurse -ErrorAction Stop).Count
    } else {
        0
    }
    try {
        if (Test-Path -LiteralPath $temporaryRoot) {
            Remove-Item -LiteralPath $temporaryRoot -Recurse -Force -ErrorAction Stop
        }
        if (Test-Path -LiteralPath $temporaryRoot) {
            throw 'The disposable packaged Simulator directory still exists after cleanup.'
        }
        $finalEvidence.safety_contract.temporary_root_cleanup_verified = $true
    }
    catch {
        $exitCode = 1
        $cleanupFailure = 'Disposable packaged Simulator cleanup failed.'
        $failure = if ([string]::IsNullOrWhiteSpace($failure)) { $cleanupFailure } else { "$failure; $cleanupFailure" }
        $runtimeToml = Join-Path $projectRoot 'runtime.toml'
        $credentialRemoved = -not (Test-Path -LiteralPath $runtimeToml -PathType Leaf)
        if (-not $credentialRemoved) {
            try {
                Remove-Item -LiteralPath $runtimeToml -Force -ErrorAction Stop
                $credentialRemoved = -not (Test-Path -LiteralPath $runtimeToml)
            }
            catch {
                try {
                    Write-Utf8File -Path $runtimeToml -Content "# Acceptance cleanup removed the disposable control credential.`n"
                    $scrubbed = Get-Content -LiteralPath $runtimeToml -Raw -ErrorAction Stop
                    $credentialRemoved = $scrubbed -notmatch '(?m)^\s*(?:runtime\.control\.)?auth_token\s*='
                }
                catch { $credentialRemoved = $false }
            }
        }
        $finalEvidence.safety_contract.cleanup_failure_credential_removed = $credentialRemoved
        if (-not $credentialRemoved) {
            $failure = "$failure; Disposable runtime.control credential cleanup could not be verified."
        }
    }
    $finalEvidence.status = if ($exitCode -eq 0) { 'pass' } else { 'fail' }
    $finalEvidence.error = $failure
    $finalEvidence.completed_at_utc = Get-UtcTimestamp
    $parent = Split-Path -Parent $resolvedEvidencePath
    [IO.Directory]::CreateDirectory($parent) | Out-Null
    $json = $finalEvidence | ConvertTo-Json -Depth 100
    if (-not [string]::IsNullOrEmpty($migratedControlAuthToken) -and $json.Contains($migratedControlAuthToken)) {
        $exitCode = 1
        $finalEvidence.status = 'fail'
        $finalEvidence.safety_contract.credential_value_recorded = $true
        $credentialFailure = 'Packaged Simulator evidence contained the disposable runtime control credential and was redacted.'
        $failure = if ([string]::IsNullOrWhiteSpace($failure)) { $credentialFailure } else { "$failure; $credentialFailure" }
        $finalEvidence.error = $failure
        $json = $finalEvidence | ConvertTo-Json -Depth 100
        $json = $json.Replace($migratedControlAuthToken, '<redacted>')
    }
    $encoding = New-Object Text.UTF8Encoding($false)
    [IO.File]::WriteAllText($resolvedEvidencePath, ($json + [Environment]::NewLine), $encoding)
}

[ordered]@{
    status = $finalEvidence.status
    evidence = $resolvedEvidencePath
    package_version = if ($null -eq $finalEvidence.package) { $null } else { $finalEvidence.package.version }
    vscode_version = if ($null -eq $finalEvidence.vscode) { $null } else { $finalEvidence.vscode.version }
    error = $finalEvidence.error
} | ConvertTo-Json -Compress
exit $exitCode
