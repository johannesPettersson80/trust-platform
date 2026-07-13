Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'AcceptanceIo.psm1') -Force

function Get-ValidatedExpectedCustomAdsPorts {
    param([Parameter(Mandatory = $true)][int[]]$Ports)
    $builtIn = @(851, 852, 853, 854, 301, 501)
    $seen = New-Object 'System.Collections.Generic.HashSet[int]'
    $validated = New-Object 'System.Collections.Generic.List[int]'
    foreach ($port in @($Ports)) {
        if ($port -lt 1 -or $port -gt 65535) {
            throw "Expected custom ADS service port $port is outside 1-65535."
        }
        if ($builtIn -contains $port) {
            throw "Expected custom ADS service port $port is a built-in ADS acceptance port."
        }
        if (-not $seen.Add($port)) {
            throw "Expected custom ADS service port $port was supplied more than once."
        }
        [void]$validated.Add($port)
    }
    if ($validated.Count -lt 1 -or $validated.Count -gt 4) {
        throw 'Packaged ADS UI acceptance requires between one and four unique custom ADS service ports.'
    }
    return @($validated.ToArray())
}

function Invoke-PackagedSimulatorAcceptance {
    param(
        [Parameter(Mandatory = $true)][string]$ScriptRoot,
        [Parameter(Mandatory = $true)][string]$VsixPath,
        [Parameter(Mandatory = $true)][string]$TemporaryRoot,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds,
        [Parameter()][string]$EvidencePath,
        [Parameter()][switch]$RequireAdsUi,
        [Parameter()][string]$ExpectedAdsTargetNetId,
        [Parameter()][int[]]$ExpectedCustomAdsPorts = @()
    )
    $resolvedEvidencePath = if ([string]::IsNullOrWhiteSpace($EvidencePath)) {
        Join-Path $TemporaryRoot 'packaged-simulator-acceptance.json'
    } else {
        [IO.Path]::GetFullPath($EvidencePath)
    }
    $runner = Join-Path $ScriptRoot 'accept_windows_packaged_simulator.ps1'
    $powerShellHost = if (Test-Path -LiteralPath (Join-Path $PSHOME 'powershell.exe') -PathType Leaf) {
        Join-Path $PSHOME 'powershell.exe'
    } elseif (Test-Path -LiteralPath (Join-Path $PSHOME 'pwsh.exe') -PathType Leaf) {
        Join-Path $PSHOME 'pwsh.exe'
    } else {
        throw 'Could not locate the current PowerShell executable for packaged Simulator acceptance.'
    }
    $arguments = @(
        '-NoProfile',
        '-ExecutionPolicy', 'Bypass',
        '-File', $runner,
        '-VsixPath', $VsixPath,
        '-EvidencePath', $resolvedEvidencePath,
        '-TimeoutSeconds', [string]$TimeoutSeconds
    )
    if ($RequireAdsUi.IsPresent) {
        if ([string]::IsNullOrWhiteSpace($ExpectedAdsTargetNetId)) {
            throw 'Packaged ADS UI acceptance requires the expected target AMS Net ID.'
        }
        $validatedCustomPorts = @(Get-ValidatedExpectedCustomAdsPorts -Ports $ExpectedCustomAdsPorts)
        $arguments += '-RequireAdsUi'
        $arguments += @('-ExpectedAdsTargetNetId', $ExpectedAdsTargetNetId)
        $arguments += @('-ExpectedCustomAdsPorts', ($validatedCustomPorts -join ','))
    } elseif (@($ExpectedCustomAdsPorts).Count -gt 0) {
        throw 'ExpectedCustomAdsPorts may only be supplied with RequireAdsUi.'
    }
    $result = Invoke-CapturedProcess -FilePath $powerShellHost -Arguments $arguments `
        -TimeoutSeconds ($TimeoutSeconds + 30)
    $evidence = if (Test-Path -LiteralPath $resolvedEvidencePath -PathType Leaf) {
        Get-Content -LiteralPath $resolvedEvidencePath -Raw | ConvertFrom-Json -ErrorAction Stop
    } else {
        [pscustomobject][ordered]@{
            status = 'fail'
            error = 'Packaged Simulator acceptance did not write evidence.'
        }
    }
    $failed = $result.timed_out -or $result.exit_code -ne 0 -or
        (Get-ObjectProperty -Object $evidence -Name 'status') -ne 'pass'
    return [pscustomobject][ordered]@{
        evidence = $evidence
        failed = $failed
    }
}

function Assert-PackagedSimulatorArtifactIdentity {
    param(
        [Parameter(Mandatory = $true)]$OuterPackage,
        [Parameter(Mandatory = $true)]$SimulatorEvidence
    )
    $inner = Get-ObjectProperty -Object $SimulatorEvidence -Name 'package'
    if ($null -eq $inner) { throw 'Packaged Simulator evidence has no package identity.' }
    $outerVersion = [string](Get-ObjectProperty -Object $OuterPackage -Name 'version')
    $innerVersion = [string](Get-ObjectProperty -Object $inner -Name 'version')
    $outerVsix = [string](Get-ObjectProperty -Object $OuterPackage -Name 'vsix_sha256')
    $innerVsix = [string](Get-ObjectProperty -Object $inner -Name 'sha256')
    if ($outerVersion -cne $innerVersion -or $outerVsix -cne $innerVsix) {
        throw 'Packaged Simulator did not load the exact VSIX version and SHA-256 used by ADS CLI proof.'
    }
    $innerKeys = [ordered]@{
        'extension/bin/trust-runtime.exe' = 'runtime'
        'extension/bin/trust-debug.exe' = 'debug'
        'extension/bin/trust-lsp.exe' = 'lsp'
    }
    $outerBinaries = @(Get-ObjectProperty -Object $OuterPackage -Name 'binaries')
    foreach ($member in $innerKeys.Keys) {
        $outerRows = @($outerBinaries | Where-Object {
            [string](Get-ObjectProperty -Object $_ -Name 'member') -ceq $member
        })
        $innerFile = Get-ObjectProperty -Object $inner -Name $innerKeys[$member]
        if ($outerRows.Count -ne 1 -or $null -eq $innerFile -or
            [string](Get-ObjectProperty -Object $outerRows[0] -Name 'sha256') -cne
                [string](Get-ObjectProperty -Object $innerFile -Name 'sha256')) {
            throw "Packaged Simulator binary identity differs for $member."
        }
    }
    return [pscustomobject][ordered]@{
        exact_vsix_sha256 = $outerVsix
        exact_version = $outerVersion
        binary_hashes_match = $true
    }
}

Export-ModuleMember -Function @(
    'Invoke-PackagedSimulatorAcceptance',
    'Assert-PackagedSimulatorArtifactIdentity'
)
