from __future__ import annotations

import re
import shutil
import subprocess
import unittest

from .windows_twincat_ads_acceptance_support import (
    ADS_BROWSE_PROOF,
    MODULE,
    function_body,
)


class AdsDiscoveryContractsMixin:
    def test_service_probe_set_is_bounded_and_includes_all_required_ports(self) -> None:
        self.assertIn("$script:RequiredAdsPorts = @(851, 852, 853, 854, 301, 501)", self.runner)
        self.assertIn("$script:MaxAdsServiceProbes = 10", self.runner)
        port_function = function_body(
            self.acceptance_plan, "Resolve-AdsAcceptanceProbePorts"
        )
        self.assertIn("1", port_function)
        self.assertIn("65535", port_function)
        self.assertIn("At most", port_function)
        self.assertIn("truly custom ADS service port", port_function)

    def test_cli_ui_parity_includes_advanced_custom_ads_ports(self) -> None:
        crosscheck = function_body(
            self.packaged_ads_ui_crosscheck,
            "Test-PackagedAdsUiAgainstCliServices",
        )
        self.assertIn("[int[]]$ExpectedAdsPorts", crosscheck)
        self.assertIn("$expectedPorts = @($ExpectedAdsPorts)", crosscheck)
        self.assertIn("foreach ($port in $expectedPorts)", crosscheck)
        self.assertIn("compared_ads_ports = @($expectedPorts)", crosscheck)
        self.assertIn("custom_port_recovery", crosscheck)
        self.assertIn("result_rows", crosscheck)
        self.assertIn("$builtInPorts -contains $port", crosscheck)
        self.assertIn("advanced_custom_ports_match = $customPortsMatch", crosscheck)
        self.assertIn(
            "-ExpectedAdsPorts $evidence.probe_ports",
            self.runner,
        )

    def test_service_results_distinguish_response_from_unavailable_or_failed(self) -> None:
        probe = function_body(self.runner, "Invoke-AdsServiceProbe")
        proof = self.ads_browse_proof
        for status in (
            "available",
            "unsupported",
            "empty",
            "unavailable",
            "check_failed",
        ):
            self.assertIn(f"'{status}'", probe + proof)
        self.assertIn("symbol_upload_unsupported", proof)
        self.assertIn("ads_port_unavailable", proof)
        self.assertIn("route.status=not_required", probe)
        self.assertIn("requested self-route recovery", probe)
        for marker in (
            "schema_version",
            "wrong protocol or result kind",
            "symbol tree is not an array",
            "invalid structured error evidence",
            "unexplained_empty_browse_response",
        ):
            self.assertIn(marker, proof)
        self.assertIn("Get-AdsBrowseResponseContractError", probe)
        self.assertIn("New-AdsProbeContractFailure", probe)

    @unittest.skipUnless(shutil.which("pwsh"), "pwsh is not installed on this Linux host")
    def test_powershell_browse_contract_rejects_parseable_malformed_results(self) -> None:
        command = rf"""
$ErrorActionPreference = 'Stop'
$WarningPreference = 'SilentlyContinue'
Import-Module '{ADS_BROWSE_PROOF}' -Force -DisableNameChecking
$invalid = @(
    '{{}}',
    '{{"schema_version":1,"protocol":"modbus_tcp","kind":"symbols","tree":[]}}',
    '{{"schema_version":1,"protocol":"ads","kind":"devices","tree":[]}}',
    '{{"schema_version":1,"protocol":"ads","kind":"symbols","tree":{{}}}}'
)
foreach ($json in $invalid) {{
    $response = $json | ConvertFrom-Json
    if ([string]::IsNullOrWhiteSpace((Get-AdsBrowseResponseContractError $response))) {{ exit 1 }}
}}
$empty = '{{"schema_version":1,"protocol":"ads","kind":"symbols","tree":[]}}' | ConvertFrom-Json
if ($null -ne (Get-AdsBrowseResponseContractError $empty)) {{ exit 2 }}
$emptyClass = Get-AdsBrowseProbeClassification -ErrorObject $null -SymbolCount 0
if ($emptyClass.status -ne 'check_failed' -or $emptyClass.responded) {{ exit 3 }}
$explicit = [pscustomobject]@{{ code = 'empty_symbol_table'; message = 'empty' }}
$explicitClass = Get-AdsBrowseProbeClassification -ErrorObject $explicit -SymbolCount 0
if ($explicitClass.status -ne 'empty' -or -not $explicitClass.responded) {{ exit 4 }}
"""
        completed = subprocess.run(
            ["pwsh", "-NoProfile", "-Command", command],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

    def test_doctor_requires_native_round_trip_no_self_route_and_no_write(self) -> None:
        doctor = function_body(self.ads_browse_proof, "Assert-NativeDoctorProof")
        for marker in (
            "native_windows_router",
            "read_state",
            "native_local_no_self_route",
            "writes_disabled",
            "add_route",
        ):
            self.assertIn(marker, doctor)
        self.assertIn("writes_enabled", doctor)
        self.assertIn("$false", doctor)
        self.assertIn("probe_transport", doctor)
        self.assertIn("probe_operation", doctor)
        self.assertIn("source_ams_net_id", doctor)
        self.assertIn("source_ams_port", doctor)
        self.assertIn("target_ams_port", doctor)
        self.assertIn("read_proven", doctor)
        self.assertIn("subscribed_read_update", doctor)
        self.assertIn("notification_subscription_id", doctor)
        self.assertIn("drained subscribed read update", doctor)
        self.assertIn("no proven candidate exposed a browsable symbol table on ADS port 851", self.runner)

    def test_runtime_command_allowlist_has_no_mutating_ads_action(self) -> None:
        normalized = re.sub(
            r"\s+",
            " ",
            (self.runner + self.ads_browse_proof).lower(),
        )
        self.assertIn("@('comm', 'browse-symbols'", normalized)
        self.assertIn("@('ads', 'doctor'", normalized)
        self.assertIn("@('comm', 'discover'", normalized)
        for forbidden in (
            "@('ads', 'route'",
            "@('comm', 'apply'",
            "--write-symbol",
            "--write-value",
            "--writes-enabled",
            "add-route",
            "remove-route",
        ):
            self.assertNotIn(forbidden, normalized)
        self.assertIn("native_local_no_self_route_required = $true", self.runner)
        self.assertIn("doctor_write_probe_disabled = $true", self.runner)
        self.assertIn("imported_binding_read_only_required = $true", self.runner)

    def test_all_relevant_static_routes_are_hashed_before_and_after(self) -> None:
        self.assertIn("CommonApplicationData", self.static_route_proof)
        self.assertIn(
            "Beckhoff\\TwinCAT\\3.1\\Runtimes\\UmRT_Default\\3.1\\StaticRoutes.xml",
            self.static_route_proof,
        )
        self.assertIn(
            "Beckhoff\\TwinCAT\\3.1\\Runtimes\\UmRT_Default\\3.1\\Target\\StaticRoutes.xml",
            self.static_route_proof,
        )
        self.assertIn("Expected exactly two TwinCAT Usermode", self.static_route_proof)
        self.assertIn("Required TwinCAT Usermode route file is missing", self.static_route_proof)
        self.assertIn("$beforeMap.Count -eq 2", self.static_route_proof)
        self.assertIn("$afterMap.Count -eq 2", self.static_route_proof)
        self.assertIn("Get-StaticRoutesSnapshot", self.runner)
        self.assertIn("Compare-StaticRoutesSnapshots", self.runner)
        self.assertIn("byte_identical", self.runner)
        self.assertIn("before_sha256", self.static_route_proof)
        self.assertIn("after_sha256", self.static_route_proof)
        self.assertIn("[IO.FileAccess]::Read", self.module)
        for mutator in ("Set-Content", "Add-Content", "Out-File", "Copy-Item", "Move-Item"):
            self.assertNotIn(mutator, self.runner)
            self.assertNotIn(mutator, self.module)
            self.assertNotIn(mutator, self.static_route_proof)

    @unittest.skipUnless(shutil.which("pwsh"), "pwsh is not installed on this Linux host")
    def test_powershell_route_snapshot_rejects_zero_one_or_duplicate_files(self) -> None:
        expected = function_body(self.static_route_proof, "Get-ExpectedStaticRoutePaths")
        snapshot = function_body(self.static_route_proof, "Get-StaticRoutesSnapshot")
        compare = function_body(self.static_route_proof, "Compare-StaticRoutesSnapshots")
        command = rf"""
$ErrorActionPreference = 'Stop'
$WarningPreference = 'SilentlyContinue'
Import-Module '{MODULE}' -Force -DisableNameChecking
function Get-ExpectedStaticRoutePaths {{{expected}}}
function Get-StaticRoutesSnapshot {{{snapshot}}}
function Compare-StaticRoutesSnapshots {{{compare}}}
$root = Join-Path ([IO.Path]::GetTempPath()) ('trust-route-proof-' + [Guid]::NewGuid().ToString('N'))
try {{
    [IO.Directory]::CreateDirectory((Join-Path $root 'Target')) | Out-Null
    $runtime = Join-Path $root 'StaticRoutes.xml'
    $target = Join-Path $root 'Target\StaticRoutes.xml'
    [IO.File]::WriteAllText($runtime, '<runtime/>')
    [IO.File]::WriteAllText($target, '<target/>')
    $before = Get-StaticRoutesSnapshot -ExpectedPaths @($runtime, $target)
    $after = Get-StaticRoutesSnapshot -ExpectedPaths @($runtime, $target)
    $comparison = Compare-StaticRoutesSnapshots -Before $before -After $after
    if (-not $comparison.byte_identical -or $comparison.before_count -ne 2 -or $comparison.after_count -ne 2) {{ exit 1 }}
    foreach ($invalid in @(@(), @($runtime), @($runtime, $runtime))) {{
        $accepted = $false
        try {{ $null = Get-StaticRoutesSnapshot -ExpectedPaths $invalid; $accepted = $true }} catch {{ }}
        if ($accepted) {{ exit 2 }}
    }}
}}
finally {{
    if ([IO.Directory]::Exists($root)) {{ [IO.Directory]::Delete($root, $true) }}
}}
"""
        completed = subprocess.run(
            ["pwsh", "-NoProfile", "-Command", command],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

    def test_vsix_and_every_packaged_windows_binary_are_hashed(self) -> None:
        for member in (
            "extension/bin/trust-runtime.exe",
            "extension/bin/trust-debug.exe",
            "extension/bin/trust-lsp.exe",
        ):
            self.assertIn(member, self.runner)
        self.assertIn("TargetPlatform", self.runner)
        self.assertIn("win32-x64", self.runner)
        self.assertIn("vsix_sha256", self.runner)
        self.assertIn("binaries = @($expanded.package.binaries)", self.runner)
        self.assertIn("Assert-WindowsPe", self.runner)

    def test_native_success_requires_tcadsdll_and_real_reply_evidence(self) -> None:
        self.assertIn("Get-TcAdsDllEvidence", self.runner)
        self.assertIn("native ADS success cannot be claimed", self.runner)
        self.assertIn("native_reply_required = $true", self.runner)
        self.assertIn("native_reply_proven = $true", self.ads_browse_proof)
        self.assertIn("candidates_with_full_851_proof", self.runner)

    def test_evidence_is_written_in_finally_even_when_a_probe_fails(self) -> None:
        top_level = self.runner.split("$beforeRoutes = $null", maxsplit=1)[1]
        finally_block = top_level.split("finally {", maxsplit=1)[1]
        self.assertIn("Compare-StaticRoutesSnapshots", finally_block)
        self.assertIn("[IO.File]::WriteAllText", finally_block)
        self.assertIn("$evidence.status", finally_block)
        self.assertIn("$evidence.error", finally_block)
        self.assertIn("ConvertTo-Json -Depth 100", finally_block)

    def test_process_capture_is_deadlock_safe_utf8_and_redacts_full_stdout(self) -> None:
        process = function_body(self.module, "Invoke-CapturedProcess")
        self.assertIn("ReadToEndAsync", process)
        self.assertIn("WaitForExit", process)
        self.assertIn("UTF8Encoding($false, $false)", process)
        self.assertNotRegex(process, r"WaitForExit\(\s*\)")
        self.assertIn("$process.WaitForExit(2000)", process)
        self.assertIn("taskkill.exe", process)
        self.assertIn("/PID $process.Id /T /F", process)
        self.assertIn("$process.WaitForExit(10000)", process)
        self.assertLess(process.index("$process.Kill()"), process.index("taskkill.exe"))
        self.assertIn("termination_completed = $terminated", process)
        self.assertIn("taskkill_fallback_attempted = $taskkillAttempted", process)
        evidence = function_body(self.module, "New-CommandEvidence")
        self.assertIn("stdout_sha256", evidence)
        self.assertIn("stdout_size_bytes", evidence)
        self.assertNotRegex(evidence, r"(?m)^\s*stdout\s*=")

    def test_packaged_screenshots_persist_beside_final_evidence_and_env_is_restored(self) -> None:
        self.assertIn(
            "$screenshotOutputDirectory = Join-Path $resolvedEvidenceDirectory",
            self.simulator_runner,
        )
        self.assertIn("TRUST_PACKAGED_SIMULATOR_SCREENSHOT_DIR", self.simulator_runner)
        self.assertIn(
            "SetEnvironmentVariable('TRUST_PACKAGED_SIMULATOR_SCREENSHOT_DIR', $screenshotOutputDirectory, 'Process')",
            self.simulator_runner,
        )
        self.assertIn("screenshots = [ordered]@{", self.simulator_runner)
        self.assertIn("$finalEvidence.screenshots.count", self.simulator_runner)
        self.assertIn("foreach ($name in $environmentNames)", self.simulator_runner)
        self.assertNotIn(
            "$screenshotOutputDirectory = Join-Path $temporaryRoot",
            self.simulator_runner,
        )
        for marker in (
            'requiredPath("TRUST_PACKAGED_SIMULATOR_SCREENSHOT_DIR")',
            'await screenshots.capture("01-initial-stopped")',
            'await screenshots.capture("02-running-editor-preserved")',
            'await screenshots.capture("03-devices-running-consistent")',
            'await screenshots.capture("04-devices-stopped-consistent")',
            'await screenshots.capture("05-devices-restarted-consistent")',
            'await screenshots.capture("06-ads-discovered-and-imported")',
            'await screenshots.capture("07-live-values-ads-good")',
        ):
            self.assertIn(marker, self.simulator_extension_test)
        for marker in (
            '"Page.captureScreenshot"',
            '"packaged-journey-screenshots-captured"',
            'crypto.createHash("sha256")',
            'bytes.readUInt32BE(16)',
            'bytes.readUInt32BE(20)',
        ):
            self.assertIn(marker, self.simulator_visual_proof)
        self.assertIn(
            "$simulatorEvidencePath = Join-Path $resolvedEvidenceDirectory",
            self.runner,
        )
        self.assertNotIn(
            "$simulatorEvidencePath = Join-Path $temporaryRoot",
            self.runner,
        )
        self.assertIn("-EvidencePath $simulatorEvidencePath", self.runner)
        self.assertIn("[Parameter()][string]$EvidencePath", self.simulator_launcher)
        self.assertIn("[IO.Path]::GetFullPath($EvidencePath)", self.simulator_launcher)
        self.assertIn("'-EvidencePath', $resolvedEvidencePath", self.simulator_launcher)
        self.assertIn(
            "screenshots.assertComplete(check, adsUiRequired ? 7 : 5)",
            self.simulator_extension_test,
        )
