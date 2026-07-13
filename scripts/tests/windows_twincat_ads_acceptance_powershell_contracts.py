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
