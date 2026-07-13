Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Resolve-AdsAcceptanceProbePorts {
    param(
        [Parameter(Mandatory = $true)][int[]]$Required,
        [Parameter()][int[]]$Custom = @(),
        [Parameter(Mandatory = $true)][int]$Maximum
    )
    $requiredPorts = New-Object 'System.Collections.Generic.List[int]'
    $requiredSet = New-Object 'System.Collections.Generic.HashSet[int]'
    foreach ($port in @($Required)) {
        if ($port -lt 1 -or $port -gt 65535) {
            throw "ADS service port $port is outside 1-65535"
        }
        if ($requiredSet.Add($port)) { [void]$requiredPorts.Add($port) }
    }
    if ($requiredPorts.Count -ge $Maximum) {
        throw "The required ADS service ports must leave room for at least one custom port within the $Maximum-port limit"
    }

    $customPorts = New-Object 'System.Collections.Generic.List[int]'
    $customSet = New-Object 'System.Collections.Generic.HashSet[int]'
    foreach ($port in @($Custom)) {
        if ($port -lt 1 -or $port -gt 65535) {
            throw "ADS service port $port is outside 1-65535"
        }
        if (-not $requiredSet.Contains($port) -and $customSet.Add($port)) {
            [void]$customPorts.Add($port)
        }
    }
    if ($customPorts.Count -eq 0) {
        throw 'At least one truly custom ADS service port is required in addition to the built-in ports'
    }
    $maximumCustom = [Math]::Min(4, $Maximum - $requiredPorts.Count)
    if ($customPorts.Count -gt $maximumCustom) {
        throw "At most $maximumCustom custom ADS service ports may be probed within the $Maximum-port limit"
    }

    $ports = New-Object 'System.Collections.Generic.List[int]'
    foreach ($port in $requiredPorts) { [void]$ports.Add($port) }
    foreach ($port in $customPorts) { [void]$ports.Add($port) }
    return @($ports)
}

function Assert-ExactTrustRuntimeVersion {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Output,
        [Parameter(Mandatory = $true)][string]$Expected
    )
    $match = [regex]::Match(
        $Output.Trim(),
        '^trust-runtime\s+([0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?)$'
    )
    if (-not $match.Success -or $match.Groups[1].Value -cne $Expected) {
        throw "Packaged trust-runtime reported '$($Output.Trim())'; expected exact version '$Expected'."
    }
    return $match.Groups[1].Value
}

Export-ModuleMember -Function @(
    'Resolve-AdsAcceptanceProbePorts',
    'Assert-ExactTrustRuntimeVersion'
)
