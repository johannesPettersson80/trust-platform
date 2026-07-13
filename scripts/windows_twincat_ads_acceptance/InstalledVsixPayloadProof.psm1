Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'AcceptanceIo.psm1')

function Get-PayloadRootPrefix {
    param([Parameter(Mandatory = $true)][string]$Root)

    $prefix = (Resolve-Path -LiteralPath $Root -ErrorAction Stop).Path
    if (-not $prefix.EndsWith([string][IO.Path]::DirectorySeparatorChar)) {
        $prefix += [IO.Path]::DirectorySeparatorChar
    }
    return $prefix
}

function Get-PayloadRelativePath {
    param(
        [Parameter(Mandatory = $true)][string]$RootPrefix,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $fullPath = [IO.Path]::GetFullPath($Path)
    if (-not $fullPath.StartsWith($RootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Installed payload path escapes its expected root: $fullPath"
    }
    return $fullPath.Substring($RootPrefix.Length).Replace('\', '/')
}

function Assert-ExactManifestBytes {
    param(
        [Parameter(Mandatory = $true)][string]$Expected,
        [Parameter(Mandatory = $true)][string]$Installed
    )

    $expectedEvidence = Get-FileEvidence -Path $Expected
    $installedEvidence = Get-FileEvidence -Path $Installed
    [byte[]]$expectedBytes = [IO.File]::ReadAllBytes($Expected)
    [byte[]]$installedBytes = [IO.File]::ReadAllBytes($Installed)
    $bytesMatch = $expectedBytes.Length -eq $installedBytes.Length
    if ($bytesMatch) {
        for ($index = 0; $index -lt $expectedBytes.Length; $index++) {
            if ($expectedBytes[$index] -ne $installedBytes[$index]) {
                $bytesMatch = $false
                break
            }
        }
    }
    if (
        $expectedEvidence.size_bytes -ne $installedEvidence.size_bytes -or
        $expectedEvidence.sha256 -cne $installedEvidence.sha256 -or
        -not $bytesMatch
    ) {
        throw 'Installed VSIX manifest differs from the archive root extension.vsixmanifest.'
    }
}

function Assert-InstalledVsixPayload {
    param(
        [Parameter(Mandatory = $true)][string]$ExtractedRoot,
        [Parameter(Mandatory = $true)][string]$VsixManifestPath,
        [Parameter(Mandatory = $true)][string]$InstalledRoot
    )

    $expectedPrefix = Get-PayloadRootPrefix -Root $ExtractedRoot
    $installedPrefix = Get-PayloadRootPrefix -Root $InstalledRoot
    $expectedFiles = @(Get-ChildItem -LiteralPath $ExtractedRoot -File -Recurse -Force)
    $installedFiles = @(Get-ChildItem -LiteralPath $InstalledRoot -File -Recurse -Force)
    [string[]]$expectedPayloadPaths = @(
        foreach ($file in $expectedFiles) {
            Get-PayloadRelativePath -RootPrefix $expectedPrefix -Path $file.FullName
        }
    )
    $expectedPathSet = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
    foreach ($relative in $expectedPayloadPaths) {
        if ([StringComparer]::OrdinalIgnoreCase.Equals($relative, '.vsixmanifest')) {
            throw 'VSIX extension payload contains the reserved installed manifest path .vsixmanifest.'
        }
        if (-not $expectedPathSet.Add($relative)) {
            throw 'VSIX extension payload contains duplicate installed paths.'
        }
    }
    [void]$expectedPathSet.Add('.vsixmanifest')
    [string[]]$expectedRelativePaths = @($expectedPayloadPaths) + @('.vsixmanifest')
    [string[]]$installedRelativePaths = @(
        foreach ($file in $installedFiles) {
            Get-PayloadRelativePath -RootPrefix $installedPrefix -Path $file.FullName
        }
    )
    $installedPathSet = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
    foreach ($relative in $installedRelativePaths) {
        if (-not $installedPathSet.Add($relative)) {
            throw 'Installed extension contains duplicate file paths.'
        }
    }
    $missing = @($expectedRelativePaths | Where-Object { -not $installedPathSet.Contains($_) })
    $extra = @($installedRelativePaths | Where-Object { -not $expectedPathSet.Contains($_) })
    if ($missing.Count -gt 0 -or $extra.Count -gt 0) {
        $missingText = if ($missing.Count -gt 0) { [string]::Join(', ', @($missing)) } else { '<none>' }
        $extraText = if ($extra.Count -gt 0) { [string]::Join(', ', @($extra)) } else { '<none>' }
        throw "Installed packaged file set differs from VSIX payload (missing: $missingText; extra: $extraText)."
    }

    foreach ($expectedFile in $expectedFiles) {
        $relative = Get-PayloadRelativePath -RootPrefix $expectedPrefix -Path $expectedFile.FullName
        $installedPath = Join-Path $InstalledRoot $relative
        if ($relative -ceq 'package.json') {
            $expectedPackage = Get-Content -LiteralPath $expectedFile.FullName -Raw | ConvertFrom-Json
            $installedPackage = Get-Content -LiteralPath $installedPath -Raw | ConvertFrom-Json
            $installedPackage.PSObject.Properties.Remove('__metadata')
            $expectedJson = $expectedPackage | ConvertTo-Json -Depth 100 -Compress
            $installedJson = $installedPackage | ConvertTo-Json -Depth 100 -Compress
            if ($installedJson -cne $expectedJson) {
                throw 'Installed package.json differs from the VSIX beyond VS Code-owned __metadata.'
            }
            continue
        }
        $installed = Get-FileEvidence -Path $installedPath
        $extracted = Get-FileEvidence -Path $expectedFile.FullName
        if ($installed.sha256 -cne $extracted.sha256 -or $installed.size_bytes -ne $extracted.size_bytes) {
            throw "Installed packaged file differs from the exact VSIX member: $relative"
        }
    }
    Assert-ExactManifestBytes -Expected $VsixManifestPath `
        -Installed (Join-Path $InstalledRoot '.vsixmanifest')
}

Export-ModuleMember -Function 'Assert-InstalledVsixPayload'
