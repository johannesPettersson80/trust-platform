from __future__ import annotations

import shutil
import subprocess
import unittest

from .windows_twincat_ads_acceptance_support import (
    ACCEPTANCE_PLAN,
    ACCEPTANCE_REDACTION,
    ADS_BROWSE_PROOF,
    CANDIDATE_MANIFEST_PROOF,
    CANDIDATE_PROVENANCE_PROOF,
    MODULE,
    PACKAGED_ADS_UI,
    PACKAGED_ADS_UI_CROSSCHECK,
    PACKAGED_EXTENSION_INSTALL,
    RUNNER,
    RUNTIME_CONTROL_TOKEN,
    SIMULATOR_CDP,
    SIMULATOR_EXTENSION_TEST,
    SIMULATOR_LAUNCHER,
    SIMULATOR_RUNNER,
    SIMULATOR_VISUAL_PROOF,
    STATIC_ROUTE_PROOF,
    function_body,
)


class PowerShellContractsMixin:
    def test_nested_modules_do_not_force_reload_shared_acceptance_io(self) -> None:
        nested_modules = (
            (PACKAGED_EXTENSION_INSTALL, self.packaged_extension_install),
            (
                PACKAGED_EXTENSION_INSTALL.with_name(
                    "InstalledVsixPayloadProof.psm1"
                ),
                PACKAGED_EXTENSION_INSTALL.with_name(
                    "InstalledVsixPayloadProof.psm1"
                ).read_text(encoding="utf-8"),
            ),
            (SIMULATOR_LAUNCHER, self.simulator_launcher),
            (PACKAGED_ADS_UI_CROSSCHECK, self.packaged_ads_ui_crosscheck),
            (STATIC_ROUTE_PROOF, self.static_route_proof),
            (CANDIDATE_MANIFEST_PROOF, self.candidate_manifest_proof),
            (CANDIDATE_PROVENANCE_PROOF, self.candidate_provenance_proof),
            (ADS_BROWSE_PROOF, self.ads_browse_proof),
        )
        expected = "Import-Module (Join-Path $PSScriptRoot 'AcceptanceIo.psm1')"
        for path, source in nested_modules:
            with self.subTest(module=path.name):
                imports = [
                    line.strip()
                    for line in source.splitlines()
                    if "AcceptanceIo.psm1" in line
                ]
                self.assertEqual(imports, [expected])

    @unittest.skipUnless(
        shutil.which("powershell.exe"),
        "Windows PowerShell 5.1 is unavailable",
    )
    def test_windows_powershell_51_preserves_shared_acceptance_io_exports(
        self,
    ) -> None:
        import_orders = (
            (
                "packaged simulator",
                (MODULE, PACKAGED_EXTENSION_INSTALL, ACCEPTANCE_PLAN),
            ),
            (
                "TwinCAT ADS",
                (
                    MODULE,
                    SIMULATOR_LAUNCHER,
                    PACKAGED_ADS_UI_CROSSCHECK,
                    ACCEPTANCE_PLAN,
                    STATIC_ROUTE_PROOF,
                    CANDIDATE_MANIFEST_PROOF,
                    CANDIDATE_PROVENANCE_PROOF,
                    ADS_BROWSE_PROOF,
                ),
            ),
        )
        exports = (
            "Get-UtcTimestamp",
            "Write-Utf8File",
            "Get-ObjectProperty",
            "Get-StringSha256",
            "Get-FileEvidence",
            "New-FileSnapshot",
            "ConvertTo-NativeArgument",
            "Invoke-CapturedProcess",
            "New-CommandEvidence",
            "Convert-CommandJson",
        )
        expected_exports = ",".join(f"'{name}'" for name in exports)

        for name, paths in import_orders:
            imports = "\n".join(
                "Import-Module '{}' -Force -DisableNameChecking".format(
                    str(path).replace("'", "''")
                )
                for path in paths
            )
            command = f"""
$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSVersion.Major -ne 5 -or $PSVersionTable.PSVersion.Minor -ne 1) {{
    throw "Expected Windows PowerShell 5.1, found $($PSVersionTable.PSVersion)."
}}
{imports}
$expected = @({expected_exports})
foreach ($commandName in $expected) {{
    Get-Command -Name $commandName -CommandType Function -ErrorAction Stop | Out-Null
}}
$timestamp = Get-UtcTimestamp
if ([string]::IsNullOrWhiteSpace($timestamp)) {{
    throw 'Get-UtcTimestamp returned an empty value.'
}}
[DateTimeOffset]::Parse($timestamp, [Globalization.CultureInfo]::InvariantCulture) | Out-Null
"""
            with self.subTest(runner=name):
                completed = subprocess.run(
                    [
                        "powershell.exe",
                        "-NoProfile",
                        "-ExecutionPolicy",
                        "Bypass",
                        "-Command",
                        command,
                    ],
                    check=False,
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(
                    completed.returncode,
                    0,
                    completed.stdout + completed.stderr,
                )

    @unittest.skipUnless(
        shutil.which("powershell.exe"),
        "Windows PowerShell 5.1 is unavailable",
    )
    def test_windows_powershell_51_vscode_cli_uses_child_only_node_mode(
        self,
    ) -> None:
        module = str(PACKAGED_EXTENSION_INSTALL).replace("'", "''")
        command = (
            "$ErrorActionPreference = 'Stop'\n"
            f"Import-Module '{module}' -Force -DisableNameChecking\n"
            r"""
$root = Join-Path ([IO.Path]::GetTempPath()) ('trust vscode cli ' + [Guid]::NewGuid().ToString('N'))
$vscode = Join-Path $root 'Code.exe'
$appRoot = Join-Path $root 'fc3def6774\resources\app'
$cliScript = Join-Path $appRoot 'out\cli.js'
$packageJson = Join-Path $appRoot 'package.json'
$cliLauncher = Join-Path $root 'bin\code.cmd'
$probeEvidence = Join-Path $root 'probe.txt'
$source = @'
using System;
using System.IO;

public static class FakeCode
{
    public static int Main(string[] args)
    {
        string path = Environment.GetEnvironmentVariable("TRUST_FAKE_VSCODE_CLI_EVIDENCE");
        string[] lines = new string[args.Length + 2];
        lines[0] = Environment.GetEnvironmentVariable("ELECTRON_RUN_AS_NODE") ?? "<null>";
        lines[1] = Environment.GetEnvironmentVariable("VSCODE_DEV") ?? "<null>";
        for (int i = 0; i < args.Length; i++)
        {
            lines[i + 2] = args[i];
        }
        File.WriteAllLines(path, lines);
        Console.WriteLine("1.128.0");
        return 0;
    }
}
'@
$previousElectron = [Environment]::GetEnvironmentVariable('ELECTRON_RUN_AS_NODE', 'Process')
$previousVscodeDev = [Environment]::GetEnvironmentVariable('VSCODE_DEV', 'Process')
$previousEvidence = [Environment]::GetEnvironmentVariable('TRUST_FAKE_VSCODE_CLI_EVIDENCE', 'Process')
try {
    [IO.Directory]::CreateDirectory((Split-Path -Parent $cliScript)) | Out-Null
    [IO.File]::WriteAllText($cliScript, '// fake VS Code CLI entrypoint')
    [IO.File]::WriteAllText($packageJson, '{"version":"1.128.0"}')
    [IO.Directory]::CreateDirectory((Split-Path -Parent $cliLauncher)) | Out-Null
    [IO.File]::WriteAllText(
        $cliLauncher,
        "@echo off`r`nset ELECTRON_RUN_AS_NODE=1`r`n" +
            '"%~dp0..\Code.exe" "%~dp0..\fc3def6774\resources\app\out\cli.js" %*' +
            "`r`n"
    )
    Add-Type -TypeDefinition $source -Language CSharp -OutputAssembly $vscode `
        -OutputType ConsoleApplication
    [Environment]::SetEnvironmentVariable('ELECTRON_RUN_AS_NODE', 'parent-electron', 'Process')
    [Environment]::SetEnvironmentVariable('VSCODE_DEV', 'parent-dev', 'Process')
    [Environment]::SetEnvironmentVariable('TRUST_FAKE_VSCODE_CLI_EVIDENCE', $probeEvidence, 'Process')

    $layout = Resolve-VscodeCliLayout -Vscode $vscode
    if (
        $layout.cli_script -cne $cliScript -or
        $layout.package_json -cne $packageJson -or
        $layout.launcher -cne $cliLauncher
    ) {
        throw 'VS Code archive CLI layout was not resolved exactly.'
    }
    $version = Invoke-VscodeCli -Vscode $vscode -Arguments @('--version') -TimeoutSeconds 10
    if ($version.timed_out -or $version.exit_code -ne 0 -or $version.stdout.Trim() -cne '1.128.0') {
        throw 'Fake VS Code version probe failed.'
    }
    $versionEvidence = @(Get-Content -LiteralPath $probeEvidence)
    $expectedVersion = @('1', '<null>', $cliScript, '--version')
    if ($versionEvidence.Count -ne $expectedVersion.Count) {
        throw 'Fake VS Code version probe argument count differed.'
    }
    for ($index = 0; $index -lt $expectedVersion.Count; $index++) {
        if ($versionEvidence[$index] -cne $expectedVersion[$index]) {
            throw "Fake VS Code version probe differed at index $index."
        }
    }

    $installArguments = @(
        '--install-extension', (Join-Path $root 'candidate package.vsix'), '--force',
        "--extensions-dir=$(Join-Path $root 'extensions dir')",
        "--user-data-dir=$(Join-Path $root 'user data')"
    )
    $install = Invoke-VscodeCli -Vscode $vscode -Arguments $installArguments -TimeoutSeconds 10
    if ($install.timed_out -or $install.exit_code -ne 0) {
        throw 'Fake VS Code install probe failed.'
    }
    $installEvidence = @(Get-Content -LiteralPath $probeEvidence)
    $expectedInstall = @('1', '<null>', $cliScript) + $installArguments
    if ($installEvidence.Count -ne $expectedInstall.Count) {
        throw 'Fake VS Code install probe argument count differed.'
    }
    for ($index = 0; $index -lt $expectedInstall.Count; $index++) {
        if ($installEvidence[$index] -cne $expectedInstall[$index]) {
            throw "Fake VS Code install probe differed at index $index."
        }
    }

    $directAppRoot = Join-Path $root 'resources\app'
    $directCli = Join-Path $directAppRoot 'out\cli.js'
    $directPackage = Join-Path $directAppRoot 'package.json'
    [IO.Directory]::CreateDirectory((Split-Path -Parent $directCli)) | Out-Null
    [IO.File]::WriteAllText($directCli, '// installed VS Code CLI entrypoint')
    [IO.File]::WriteAllText($directPackage, '{"version":"1.128.0"}')
    [IO.File]::WriteAllText(
        $cliLauncher,
        '"%~dp0..\Code.exe" "%~dp0..\resources\app\out\cli.js" %*'
    )
    $directLayout = Resolve-VscodeCliLayout -Vscode $vscode
    if (
        $directLayout.cli_script -cne $directCli -or
        $directLayout.package_json -cne $directPackage
    ) {
        throw 'Installed VS Code CLI layout was not resolved exactly.'
    }

    $validArchiveLine = '"%~dp0..\Code.exe" "%~dp0..\fc3def6774\resources\app\out\cli.js" %*'
    [IO.File]::WriteAllText($cliLauncher, $validArchiveLine + "`r`n" + $validArchiveLine)
    try {
        Resolve-VscodeCliLayout -Vscode $vscode | Out-Null
        throw 'Duplicate VS Code CLI targets were accepted.'
    }
    catch {
        if ($_.Exception.Message -eq 'Duplicate VS Code CLI targets were accepted.') { throw }
        if ($_.Exception.Message -notlike 'Expected exactly one Visual Studio Code CLI target*') {
            throw
        }
    }

    [IO.File]::WriteAllText(
        $cliLauncher,
        '"%~dp0..\Code.exe" "%~dp0..\..\resources\app\out\cli.js" %*'
    )
    try {
        Resolve-VscodeCliLayout -Vscode $vscode | Out-Null
        throw 'Escaping VS Code CLI target was accepted.'
    }
    catch {
        if ($_.Exception.Message -eq 'Escaping VS Code CLI target was accepted.') { throw }
        if ($_.Exception.Message -cne 'Visual Studio Code CLI target escapes the desktop installation root.') {
            throw
        }
    }

    [IO.File]::WriteAllText(
        $cliLauncher,
        '"%~dp0..\Code.exe" "%~dp0..\missing\resources\app\out\cli.js" %*'
    )
    try {
        Resolve-VscodeCliLayout -Vscode $vscode | Out-Null
        throw 'Missing VS Code CLI target was accepted.'
    }
    catch {
        if ($_.Exception.Message -eq 'Missing VS Code CLI target was accepted.') { throw }
    }
    if ($env:ELECTRON_RUN_AS_NODE -cne 'parent-electron' -or $env:VSCODE_DEV -cne 'parent-dev') {
        throw 'VS Code CLI invocation changed the parent process environment.'
    }
}
finally {
    [Environment]::SetEnvironmentVariable('ELECTRON_RUN_AS_NODE', $previousElectron, 'Process')
    [Environment]::SetEnvironmentVariable('VSCODE_DEV', $previousVscodeDev, 'Process')
    [Environment]::SetEnvironmentVariable('TRUST_FAKE_VSCODE_CLI_EVIDENCE', $previousEvidence, 'Process')
    if ([IO.Directory]::Exists($root)) {
        [IO.Directory]::Delete($root, $true)
    }
}
"""
        )
        completed = subprocess.run(
            [
                "powershell.exe",
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                command,
            ],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(
            completed.returncode,
            0,
            completed.stdout + completed.stderr,
        )

    @unittest.skipUnless(
        shutil.which("powershell.exe"),
        "Windows PowerShell 5.1 is unavailable",
    )
    def test_windows_powershell_51_installed_vsix_payload_is_exact(self) -> None:
        proof_module = str(
            PACKAGED_EXTENSION_INSTALL.with_name("InstalledVsixPayloadProof.psm1")
        ).replace("'", "''")
        command = rf"""
$ErrorActionPreference = 'Stop'
Import-Module '{proof_module}' -Force -DisableNameChecking
$root = Join-Path ([IO.Path]::GetTempPath()) ('trust-vsix-payload-' + [Guid]::NewGuid().ToString('N'))
$expectedRoot = Join-Path $root 'vsix\extension'
$expectedManifest = Join-Path $root 'vsix\extension.vsixmanifest'
$installedRoot = Join-Path $root 'extensions\trust-platform.trust-lsp-1.0.0-win32-x64'
$expectedScript = Join-Path $expectedRoot 'out\extension.js'
$installedScript = Join-Path $installedRoot 'out\extension.js'
$expectedPackage = Join-Path $expectedRoot 'package.json'
$installedPackage = Join-Path $installedRoot 'package.json'
$installedManifest = Join-Path $installedRoot '.vsixmanifest'

function Assert-Rejected {{
    param(
        [Parameter(Mandatory = $true)][string]$Case,
        [Parameter(Mandatory = $true)][scriptblock]$Action,
        [Parameter(Mandatory = $true)][string[]]$ExpectedMessageParts
    )
    try {{
        & $Action
        throw "Installed payload case '$Case' was accepted."
    }}
    catch {{
        if ($_.Exception.Message -eq "Installed payload case '$Case' was accepted.") {{ throw }}
        foreach ($part in $ExpectedMessageParts) {{
            if ($_.Exception.Message -cnotlike "*$part*") {{
                throw "Installed payload case '$Case' returned the wrong error: $($_.Exception.Message)"
            }}
        }}
    }}
}}

try {{
    [IO.Directory]::CreateDirectory((Split-Path -Parent $expectedScript)) | Out-Null
    [IO.Directory]::CreateDirectory((Split-Path -Parent $installedScript)) | Out-Null
    [IO.File]::WriteAllText($expectedScript, 'exact extension bytes')
    [IO.File]::WriteAllText($installedScript, 'exact extension bytes')
    [IO.File]::WriteAllText($expectedPackage, '{{"name":"trust-lsp","version":"1.0.0"}}')
    [IO.File]::WriteAllText($installedPackage, '{{"name":"trust-lsp","version":"1.0.0","__metadata":{{"installedTimestamp":1}}}}')
    [IO.File]::WriteAllText($expectedManifest, '<Package>alpha</Package>')
    [IO.File]::WriteAllText($installedManifest, '<Package>alpha</Package>')

    Assert-InstalledVsixPayload -ExtractedRoot $expectedRoot `
        -VsixManifestPath $expectedManifest -InstalledRoot $installedRoot

    Remove-Item -LiteralPath $installedManifest -Force
    Assert-Rejected -Case 'missing manifest' -ExpectedMessageParts @('missing:', '.vsixmanifest') -Action {{
        Assert-InstalledVsixPayload -ExtractedRoot $expectedRoot `
            -VsixManifestPath $expectedManifest -InstalledRoot $installedRoot
    }}
    [IO.File]::WriteAllText($installedManifest, '<Package>alpha</Package>')

    [IO.File]::WriteAllText($installedManifest, '<Package>bravo</Package>')
    Assert-Rejected -Case 'same-size manifest mutation' -ExpectedMessageParts @('manifest', 'differs') -Action {{
        Assert-InstalledVsixPayload -ExtractedRoot $expectedRoot `
            -VsixManifestPath $expectedManifest -InstalledRoot $installedRoot
    }}
    [IO.File]::WriteAllText($installedManifest, '<Package>alpha</Package>')

    Remove-Item -LiteralPath $installedScript -Force
    $replacement = Join-Path $installedRoot 'out\replacement.js'
    [IO.File]::WriteAllText($replacement, 'exact extension bytes')
    Assert-Rejected -Case 'same-count replacement' -ExpectedMessageParts @('missing:', 'out/extension.js', 'extra:', 'out/replacement.js') -Action {{
        Assert-InstalledVsixPayload -ExtractedRoot $expectedRoot `
            -VsixManifestPath $expectedManifest -InstalledRoot $installedRoot
    }}
    Remove-Item -LiteralPath $replacement -Force
    [IO.File]::WriteAllText($installedScript, 'exact extension bytes')

    $reservedPayloadManifest = Join-Path $expectedRoot '.vsixmanifest'
    [IO.File]::WriteAllText($reservedPayloadManifest, '<Package>alpha</Package>')
    Assert-Rejected -Case 'reserved payload manifest' -ExpectedMessageParts @('reserved', '.vsixmanifest') -Action {{
        Assert-InstalledVsixPayload -ExtractedRoot $expectedRoot `
            -VsixManifestPath $expectedManifest -InstalledRoot $installedRoot
    }}
    Remove-Item -LiteralPath $reservedPayloadManifest -Force

    $hiddenExtra = Join-Path $installedRoot 'hidden-extra.bin'
    [IO.File]::WriteAllText($hiddenExtra, 'must be rejected')
    [IO.File]::SetAttributes($hiddenExtra, [IO.FileAttributes]::Hidden)
    Assert-Rejected -Case 'hidden extra' -ExpectedMessageParts @('extra:', 'hidden-extra.bin') -Action {{
        Assert-InstalledVsixPayload -ExtractedRoot $expectedRoot `
            -VsixManifestPath $expectedManifest -InstalledRoot $installedRoot
    }}
}}
finally {{
    if ([IO.Directory]::Exists($root)) {{
        [IO.Directory]::Delete($root, $true)
    }}
}}
"""
        completed = subprocess.run(
            [
                "powershell.exe",
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                command,
            ],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(
            completed.returncode,
            0,
            completed.stdout + completed.stderr,
        )

    def test_generic_lists_are_materialized_through_to_array(self) -> None:
        conversions = (
            (self.acceptance_plan, "$ports.ToArray()", "@($ports)"),
            (self.simulator_launcher, "$validated.ToArray()", "@($validated)"),
            (self.runner, "$binaryEvidence.ToArray()", "@($binaryEvidence)"),
            (self.runner, "$found.ToArray()", "@($found)"),
            (self.runner, "$services.ToArray()", "@($services)"),
            (self.runner, "$targetEvidence.ToArray()", "@($targetEvidence)"),
            (
                self.packaged_ads_ui_crosscheck,
                "$mismatches.ToArray()",
                "@($mismatches)",
            ),
        )
        for source, safe, unsafe in conversions:
            self.assertIn(safe, source)
            self.assertNotIn(unsafe, source)

    @unittest.skipUnless(shutil.which("pwsh"), "pwsh is not installed on this Linux host")
    def test_powershell_parser_reports_no_errors_when_pwsh_is_available(self) -> None:
        for path in (
            RUNNER,
            SIMULATOR_RUNNER,
            MODULE,
            SIMULATOR_LAUNCHER,
            PACKAGED_EXTENSION_INSTALL,
            PACKAGED_EXTENSION_INSTALL.with_name("InstalledVsixPayloadProof.psm1"),
            PACKAGED_ADS_UI_CROSSCHECK,
            ACCEPTANCE_PLAN,
            STATIC_ROUTE_PROOF,
            CANDIDATE_MANIFEST_PROOF,
            CANDIDATE_PROVENANCE_PROOF,
            ADS_BROWSE_PROOF,
        ):
            command = (
                "$tokens=$null;$errors=$null;"
                f"[System.Management.Automation.Language.Parser]::ParseFile('{path}',[ref]$tokens,[ref]$errors)|Out-Null;"
                "if($errors.Count){$errors|ForEach-Object{$_.Message};exit 1}"
            )
            completed = subprocess.run(
                ["pwsh", "-NoProfile", "-Command", command],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

    @unittest.skipUnless(shutil.which("pwsh"), "pwsh is not installed on this Linux host")
    def test_powershell_runtime_control_token_parser_is_section_aware(self) -> None:
        helper = function_body(
            self.simulator_runner, "Get-RuntimeControlAuthTokenEvidence"
        )
        helper = helper.rsplit("\n}", 1)[0]
        command = f"""
function Get-RuntimeControlAuthTokenEvidence {{{helper}}}
$tokenless = @'
[runtime.control]
endpoint = "tcp://127.0.0.1:9902"
mode = "production"

[runtime.mesh]
auth_token = ""
'@
$configured = @'
[runtime.control]
endpoint = "tcp://127.0.0.1:9902"
auth_token = "12345678901234567890123456789012"

[runtime.mesh]
auth_token = "mesh"
'@
$dotted = 'runtime.control.auth_token = "123456789012345678901234"'
$meshOnly = "[runtime.mesh]`nauth_token = `"mesh`"`n"
$a = Get-RuntimeControlAuthTokenEvidence -Source $tokenless
$b = Get-RuntimeControlAuthTokenEvidence -Source $configured
$c = Get-RuntimeControlAuthTokenEvidence -Source $dotted
$d = Get-RuntimeControlAuthTokenEvidence -Source $meshOnly
if ($a.present -or $a.value -ne '' -or -not $b.present -or $b.length -ne 32 -or $b.value -ne '12345678901234567890123456789012' -or -not $c.present -or $c.length -ne 24 -or $c.value -ne '123456789012345678901234' -or $d.present -or $d.value -ne '') {{ exit 1 }}
"""
        completed = subprocess.run(
            ["pwsh", "-NoProfile", "-Command", command],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

    @unittest.skipUnless(shutil.which("pwsh"), "pwsh is not installed on this Linux host")
    def test_powershell_file_snapshot_remains_frozen_after_source_mutation(self) -> None:
        command = (
            "$ErrorActionPreference = 'Stop'\n"
            "$WarningPreference = 'SilentlyContinue'\n"
            f"Import-Module '{MODULE}' -Force -DisableNameChecking\n"
            r"""
$root = Join-Path ([IO.Path]::GetTempPath()) ('trust-file-snapshot-' + [Guid]::NewGuid().ToString('N'))
$source = Join-Path $root 'source.vsix'
$destination = Join-Path $root 'frozen.vsix'
try {
    [IO.Directory]::CreateDirectory($root) | Out-Null
    [IO.File]::WriteAllBytes($source, [Text.Encoding]::UTF8.GetBytes('immutable-original'))
    $sourceBefore = Get-FileEvidence -Path $source
    $snapshot = New-FileSnapshot -Source $source -Destination $destination

    [IO.File]::WriteAllBytes($source, [Text.Encoding]::UTF8.GetBytes('mutated-after-snapshot'))
    $sourceAfter = Get-FileEvidence -Path $source
    $snapshotAfter = Get-FileEvidence -Path $destination

    if ($snapshot.path -ceq $sourceAfter.path) {
        throw 'Snapshot reused the mutable source path.'
    }
    if ($snapshot.sha256 -cne $sourceBefore.sha256) {
        throw 'Snapshot hash did not record the original source bytes.'
    }
    if ($snapshotAfter.sha256 -cne $snapshot.sha256) {
        throw 'Frozen snapshot bytes changed after source mutation.'
    }
    if ($sourceAfter.sha256 -ceq $snapshot.sha256) {
        throw 'Source mutation was not distinguishable from the frozen snapshot.'
    }
    if ([IO.File]::ReadAllText($destination) -cne 'immutable-original') {
        throw 'Frozen snapshot content does not contain the original bytes.'
    }

    [pscustomobject][ordered]@{
        original_sha256 = $sourceBefore.sha256
        frozen_sha256 = $snapshotAfter.sha256
        mutated_sha256 = $sourceAfter.sha256
        destination_is_distinct = $snapshot.path -cne $sourceAfter.path
    } | ConvertTo-Json -Compress
}
finally {
    if ([IO.Directory]::Exists($root)) {
        [IO.Directory]::Delete($root, $true)
    }
}
"""
        )
        evidence = self._run_powershell_json(command)
        self.assertEqual(evidence["original_sha256"], evidence["frozen_sha256"])
        self.assertNotEqual(evidence["mutated_sha256"], evidence["frozen_sha256"])
        self.assertTrue(evidence["destination_is_distinct"])

    @unittest.skipUnless(shutil.which("pwsh"), "pwsh is not installed on this Linux host")
    def test_powershell_runtime_version_requires_exact_undecorated_match(self) -> None:
        command = (
            "$ErrorActionPreference = 'Stop'\n"
            "$WarningPreference = 'SilentlyContinue'\n"
            f"Import-Module '{ACCEPTANCE_PLAN}' -Force -DisableNameChecking\n"
            r"""
$expected = '0.24.33'
$exact = Assert-ExactTrustRuntimeVersion -Output 'trust-runtime 0.24.33' -Expected $expected
if ($exact -cne $expected) {
    throw "Exact runtime version returned '$exact' instead of '$expected'."
}

$invalid = [ordered]@{
    prefix = 'trust-runtime 0.24.330'
    suffix = 'trust-runtime 0.24.33-dev'
    malformed = 'trust-runtime version 0.24.33'
}
$rejected = [ordered]@{}
foreach ($case in $invalid.GetEnumerator()) {
    $wasAccepted = $false
    try {
        $null = Assert-ExactTrustRuntimeVersion -Output $case.Value -Expected $expected
        $wasAccepted = $true
    }
    catch {
        $rejected[$case.Key] = $_.Exception.Message
    }
    if ($wasAccepted) {
        throw "Runtime version case '$($case.Key)' was incorrectly accepted: $($case.Value)"
    }
}

[pscustomobject][ordered]@{
    exact = $exact
    rejected = $rejected
} | ConvertTo-Json -Depth 4 -Compress
"""
        )
        evidence = self._run_powershell_json(command)
        self.assertEqual(evidence["exact"], "0.24.33")
        self.assertEqual(set(evidence["rejected"]), {"prefix", "suffix", "malformed"})

    @unittest.skipUnless(shutil.which("pwsh"), "pwsh is not installed on this Linux host")
    def test_powershell_packaged_artifact_identity_rejects_every_mismatch(self) -> None:
        command = (
            "$ErrorActionPreference = 'Stop'\n"
            "$WarningPreference = 'SilentlyContinue'\n"
            f"Import-Module '{SIMULATOR_LAUNCHER}' -Force -DisableNameChecking\n"
            r"""
$outer = [pscustomobject][ordered]@{
    version = '0.24.33'
    vsix_sha256 = ('0' * 64)
    binaries = @(
        [pscustomobject]@{ member = 'extension/bin/trust-runtime.exe'; sha256 = ('a' * 64) },
        [pscustomobject]@{ member = 'extension/bin/trust-debug.exe'; sha256 = ('b' * 64) },
        [pscustomobject]@{ member = 'extension/bin/trust-lsp.exe'; sha256 = ('c' * 64) }
    )
}
$simulator = [pscustomobject][ordered]@{
    package = [pscustomobject][ordered]@{
        version = '0.24.33'
        sha256 = ('0' * 64)
        runtime = [pscustomobject]@{ sha256 = ('a' * 64) }
        debug = [pscustomobject]@{ sha256 = ('b' * 64) }
        lsp = [pscustomobject]@{ sha256 = ('c' * 64) }
    }
}

$baseline = Assert-PackagedSimulatorArtifactIdentity -OuterPackage $outer -SimulatorEvidence $simulator
if ($baseline.exact_version -cne '0.24.33' -or
    $baseline.exact_vsix_sha256 -cne ('0' * 64) -or
    -not $baseline.binary_hashes_match) {
    throw 'Matching packaged artifact identity did not produce valid proof.'
}

$rejected = [ordered]@{}
foreach ($caseName in @('version', 'vsix', 'runtime', 'debug', 'lsp')) {
    $caseOuter = $outer | ConvertTo-Json -Depth 10 | ConvertFrom-Json
    $caseSimulator = $simulator | ConvertTo-Json -Depth 10 | ConvertFrom-Json
    switch ($caseName) {
        'version' { $caseSimulator.package.version = '0.24.34' }
        'vsix' { $caseSimulator.package.sha256 = ('f' * 64) }
        'runtime' { $caseSimulator.package.runtime.sha256 = ('f' * 64) }
        'debug' { $caseSimulator.package.debug.sha256 = ('f' * 64) }
        'lsp' { $caseSimulator.package.lsp.sha256 = ('f' * 64) }
    }

    $wasAccepted = $false
    try {
        $null = Assert-PackagedSimulatorArtifactIdentity `
            -OuterPackage $caseOuter -SimulatorEvidence $caseSimulator
        $wasAccepted = $true
    }
    catch {
        $rejected[$caseName] = $_.Exception.Message
    }
    if ($wasAccepted) {
        throw "Packaged artifact mismatch '$caseName' was incorrectly accepted."
    }
}

[pscustomobject][ordered]@{
    baseline_version = $baseline.exact_version
    rejected = $rejected
} | ConvertTo-Json -Depth 4 -Compress
"""
        )
        evidence = self._run_powershell_json(command)
        self.assertEqual(evidence["baseline_version"], "0.24.33")
        self.assertEqual(
            set(evidence["rejected"]),
            {"version", "vsix", "runtime", "debug", "lsp"},
        )

    @unittest.skipUnless(shutil.which("node"), "node is not installed")
    def test_packaged_evidence_redacts_json_auth_keys_and_exact_token(self) -> None:
        command = r"""
const { safeError, serializeWithoutCredential } = require(process.argv[1]);
const secret = "0123456789abcdef0123456789abcdef0123456789abcdef";
for (const key of ["controlAuthToken", "control_auth_token", "auth_token", "runtime.control.auth_token"]) {
  const redacted = safeError(JSON.stringify({ [key]: secret }));
  if (redacted.includes(secret) || !redacted.includes("<redacted>")) process.exit(1);
}
const leaked = serializeWithoutCredential({ unrelated_detail: secret }, secret);
if (!leaked.credentialFound || leaked.serialized.includes(secret) || !leaked.serialized.includes("<redacted>")) process.exit(2);
const clean = serializeWithoutCredential({ unrelated_detail: "safe" }, secret);
if (clean.credentialFound || clean.serialized.includes(secret)) process.exit(3);
const second = "fedcba9876543210fedcba9876543210fedcba9876543210";
const multiple = serializeWithoutCredential({ first: secret, second }, [secret, second]);
if (!multiple.credentialFound || multiple.serialized.includes(secret) || multiple.serialized.includes(second)) process.exit(4);
"""
        completed = subprocess.run(
            ["node", "-e", command, str(ACCEPTANCE_REDACTION)],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

    @unittest.skipUnless(shutil.which("node"), "node is not installed")
    def test_packaged_simulator_extension_test_is_valid_javascript(self) -> None:
        for path in (
            SIMULATOR_EXTENSION_TEST,
            SIMULATOR_VISUAL_PROOF,
            SIMULATOR_CDP,
            RUNTIME_CONTROL_TOKEN,
            ACCEPTANCE_REDACTION,
            PACKAGED_ADS_UI,
        ):
            completed = subprocess.run(
                ["node", "--check", str(path)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
