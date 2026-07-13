Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'AcceptanceIo.psm1')

function Get-ExpectedStaticRoutePaths {
    $commonData = [Environment]::GetFolderPath('CommonApplicationData')
    if ([string]::IsNullOrWhiteSpace($commonData)) {
        throw 'Windows CommonApplicationData is unavailable; exact TwinCAT route paths cannot be proven.'
    }
    return @(
        (Join-Path $commonData 'Beckhoff\TwinCAT\3.1\Runtimes\UmRT_Default\3.1\StaticRoutes.xml'),
        (Join-Path $commonData 'Beckhoff\TwinCAT\3.1\Runtimes\UmRT_Default\3.1\Target\StaticRoutes.xml')
    )
}

function Get-StaticRoutesSnapshot {
    param([string[]]$ExpectedPaths = @())
    if ($ExpectedPaths.Count -eq 0) {
        $ExpectedPaths = @(Get-ExpectedStaticRoutePaths)
    }
    if ($ExpectedPaths.Count -ne 2) {
        throw "Expected exactly two TwinCAT Usermode StaticRoutes.xml paths, got $($ExpectedPaths.Count)."
    }
    $seen = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
    $items = New-Object 'System.Collections.Generic.List[object]'
    foreach ($path in $ExpectedPaths) {
        if ([IO.Path]::GetFileName($path) -cne 'StaticRoutes.xml') {
            throw "Expected TwinCAT route path does not name StaticRoutes.xml: $path"
        }
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Required TwinCAT Usermode route file is missing: $path"
        }
        $file = Get-FileEvidence -Path $path
        if (-not $seen.Add($file.path)) {
            throw "The two required TwinCAT route paths resolve to the same file: $($file.path)"
        }
        [void]$items.Add([pscustomobject][ordered]@{
            role = if ((Split-Path -Parent $file.path) -match '[\\/]Target$') { 'target' } else { 'runtime' }
            path = $file.path
            size_bytes = $file.size_bytes
            sha256 = $file.sha256
        })
    }
    return [pscustomobject][ordered]@{
        expected_paths = @($ExpectedPaths)
        files = @($items | Sort-Object role)
    }
}

function Compare-StaticRoutesSnapshots {
    param(
        [Parameter(Mandatory = $true)]$Before,
        [Parameter(Mandatory = $true)]$After
    )
    $beforeMap = New-Object 'System.Collections.Generic.Dictionary[string,object]' ([StringComparer]::OrdinalIgnoreCase)
    $afterMap = New-Object 'System.Collections.Generic.Dictionary[string,object]' ([StringComparer]::OrdinalIgnoreCase)
    foreach ($item in @($Before.files)) { $beforeMap[$item.path] = $item }
    foreach ($item in @($After.files)) { $afterMap[$item.path] = $item }

    $added = New-Object 'System.Collections.Generic.List[string]'
    $removed = New-Object 'System.Collections.Generic.List[string]'
    $changed = New-Object 'System.Collections.Generic.List[object]'
    foreach ($path in $afterMap.Keys) {
        if (-not $beforeMap.ContainsKey($path)) { [void]$added.Add($path) }
    }
    foreach ($path in $beforeMap.Keys) {
        if (-not $afterMap.ContainsKey($path)) {
            [void]$removed.Add($path)
            continue
        }
        $left = $beforeMap[$path]
        $right = $afterMap[$path]
        if ($left.size_bytes -ne $right.size_bytes -or $left.sha256 -ne $right.sha256) {
            [void]$changed.Add([pscustomobject][ordered]@{
                path = $path
                before_size_bytes = $left.size_bytes
                after_size_bytes = $right.size_bytes
                before_sha256 = $left.sha256
                after_sha256 = $right.sha256
            })
        }
    }
    $identical = $beforeMap.Count -eq 2 -and $afterMap.Count -eq 2 -and
        $added.Count -eq 0 -and $removed.Count -eq 0 -and $changed.Count -eq 0
    return [pscustomobject][ordered]@{
        byte_identical = $identical
        before_count = $beforeMap.Count
        after_count = $afterMap.Count
        added_paths = @($added | Sort-Object)
        removed_paths = @($removed | Sort-Object)
        changed_files = @($changed | Sort-Object path)
    }
}

Export-ModuleMember -Function @(
    'Get-ExpectedStaticRoutePaths',
    'Get-StaticRoutesSnapshot',
    'Compare-StaticRoutesSnapshots'
)
