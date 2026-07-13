Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'AcceptanceIo.psm1')

function New-IsolatedUserData {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$ExtensionRoot
    )
    $settingsPath = Join-Path $Root 'User\settings.json'
    $settings = [ordered]@{
        'window.commandCenter' = $false
        'chat.commandCenter.enabled' = $false
        'workbench.layoutControl.enabled' = $false
        'workbench.startupEditor' = 'none'
        'workbench.tips.enabled' = $false
        'telemetry.telemetryLevel' = 'off'
        'update.mode' = 'none'
        'extensions.ignoreRecommendations' = $true
        'git.enabled' = $false
        'git.openRepositoryInParentFolders' = 'never'
        'trust.languageServer.executablePath' = (Join-Path $ExtensionRoot 'bin\trust-lsp.exe')
        'trust.debugAdapter.executablePath' = (Join-Path $ExtensionRoot 'bin\trust-debug.exe')
        'trust.runtime.executablePath' = (Join-Path $ExtensionRoot 'bin\trust-runtime.exe')
    }
    Write-Utf8File -Path $settingsPath -Content ($settings | ConvertTo-Json -Depth 4)
}

function Disable-PackagedBinaryPathFallback {
    $systemEntries = @(
        (Join-Path $env:SystemRoot 'System32'),
        $env:SystemRoot,
        (Join-Path $env:SystemRoot 'System32\Wbem'),
        (Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0')
    ) | Where-Object { Test-Path -LiteralPath $_ -PathType Container } | Select-Object -Unique
    [Environment]::SetEnvironmentVariable('PATH', [string]::Join(';', $systemEntries), 'Process')
    [Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', $null, 'Process')
    foreach ($name in @('ST_LSP_TEST_SERVER', 'ST_RUNTIME_TEST_BIN', 'ST_DEBUG_TEST_BIN')) {
        [Environment]::SetEnvironmentVariable($name, $null, 'Process')
    }
    foreach ($binary in @('trust-runtime.exe', 'trust-debug.exe', 'trust-lsp.exe')) {
        if ($null -ne (Get-Command $binary -ErrorAction SilentlyContinue)) {
            throw "Packaged acceptance could still resolve $binary through PATH."
        }
    }
}

function Invoke-VscodeCli {
    param(
        [Parameter(Mandatory = $true)][string]$Vscode,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds
    )
    $resolvedVscode = (Resolve-Path -LiteralPath $Vscode -ErrorAction Stop).Path
    $cliScript = Join-Path (Split-Path -Parent $resolvedVscode) 'resources\app\out\cli.js'
    if (-not (Test-Path -LiteralPath $cliScript -PathType Leaf)) {
        throw "Visual Studio Code CLI script was not found beside $resolvedVscode."
    }
    [string[]]$cliArguments = @($cliScript) + @($Arguments)
    return Invoke-CapturedProcess -FilePath $resolvedVscode -Arguments $cliArguments `
        -TimeoutSeconds $TimeoutSeconds -EnvironmentOverrides @{
            ELECTRON_RUN_AS_NODE = '1'
            VSCODE_DEV = $null
        }
}

function Install-IsolatedPackagedExtension {
    param(
        [Parameter(Mandatory = $true)][string]$Vscode,
        [Parameter(Mandatory = $true)][string]$Vsix,
        [Parameter(Mandatory = $true)][string]$ExtensionsRoot,
        [Parameter(Mandatory = $true)][string]$UserDataRoot,
        [Parameter(Mandatory = $true)][string]$ExpectedVersion,
        [Parameter(Mandatory = $true)][string]$ExtractedRoot
    )
    $install = Invoke-VscodeCli -Vscode $Vscode -Arguments @(
        '--install-extension', $Vsix, '--force',
        "--extensions-dir=$ExtensionsRoot", "--user-data-dir=$UserDataRoot"
    ) -TimeoutSeconds 90
    if ($install.timed_out -or $install.exit_code -ne 0) {
        $command = New-CommandEvidence $install
        throw (
            "Visual Studio Code CLI did not install the isolated packaged VSIX " +
            "(timed_out=$($command.timed_out), exit_code=$($command.exit_code), " +
            "stderr_size_bytes=$([Text.Encoding]::UTF8.GetByteCount($command.stderr)))."
        )
    }
    $matches = @(Get-ChildItem -LiteralPath $ExtensionsRoot -Directory | Where-Object {
        $packagePath = Join-Path $_.FullName 'package.json'
        if (-not (Test-Path -LiteralPath $packagePath -PathType Leaf)) { return $false }
        $package = Get-Content -LiteralPath $packagePath -Raw | ConvertFrom-Json
        return $package.publisher -ceq 'trust-platform' -and
            $package.name -ceq 'trust-lsp' -and $package.version -ceq $ExpectedVersion
    })
    if ($matches.Count -ne 1) {
        throw "Expected exactly one isolated installed truST $ExpectedVersion extension, found $($matches.Count)."
    }
    $installedRoot = $matches[0].FullName
    $expectedFiles = @(Get-ChildItem -LiteralPath $ExtractedRoot -File -Recurse)
    $installedFiles = @(Get-ChildItem -LiteralPath $installedRoot -File -Recurse)
    if ($installedFiles.Count -ne $expectedFiles.Count) {
        throw "Installed packaged file count $($installedFiles.Count) differs from VSIX payload $($expectedFiles.Count)."
    }
    foreach ($expectedFile in $expectedFiles) {
        $relative = $expectedFile.FullName.Substring($ExtractedRoot.Length).TrimStart('\', '/')
        if ($relative -ceq 'package.json') {
            $expectedPackage = Get-Content -LiteralPath $expectedFile.FullName -Raw | ConvertFrom-Json
            $installedPackagePath = Join-Path $installedRoot $relative
            $installedPackage = Get-Content -LiteralPath $installedPackagePath -Raw | ConvertFrom-Json
            $installedPackage.PSObject.Properties.Remove('__metadata')
            $expectedJson = $expectedPackage | ConvertTo-Json -Depth 100 -Compress
            $installedJson = $installedPackage | ConvertTo-Json -Depth 100 -Compress
            if ($installedJson -cne $expectedJson) {
                throw 'Installed package.json differs from the VSIX beyond VS Code-owned __metadata.'
            }
            continue
        }
        $installed = Get-FileEvidence -Path (Join-Path $installedRoot $relative)
        $extracted = Get-FileEvidence -Path $expectedFile.FullName
        if ($installed.sha256 -cne $extracted.sha256 -or $installed.size_bytes -ne $extracted.size_bytes) {
            throw "Installed packaged file differs from the exact VSIX member: $relative"
        }
    }
    return [pscustomobject][ordered]@{
        extension_root = $installedRoot
        command = New-CommandEvidence $install
    }
}

function New-AcceptanceDriverExtension {
    param([Parameter(Mandatory = $true)][string]$Root)
    $package = [ordered]@{
        name = 'trust-packaged-acceptance-driver'
        publisher = 'trust-local-acceptance'
        version = '0.0.0'
        engines = [ordered]@{ vscode = '^1.90.0' }
        main = './extension.js'
        activationEvents = @()
    }
    Write-Utf8File -Path (Join-Path $Root 'package.json') -Content ($package | ConvertTo-Json -Depth 4)
    Write-Utf8File -Path (Join-Path $Root 'extension.js') -Content "'use strict'; exports.activate = function () {}; exports.deactivate = function () {};`n"
}

Export-ModuleMember -Function @(
    'New-IsolatedUserData',
    'Disable-PackagedBinaryPathFallback',
    'Invoke-VscodeCli',
    'Install-IsolatedPackagedExtension',
    'New-AcceptanceDriverExtension'
)
