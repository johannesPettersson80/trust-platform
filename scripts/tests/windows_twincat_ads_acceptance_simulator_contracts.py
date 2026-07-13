from __future__ import annotations

import json
import re
import subprocess
import tempfile
import tomllib
from pathlib import Path

from .windows_twincat_ads_acceptance_support import (
    PACKAGED_ADS_IMPORT_PROOF,
    PACKAGED_EXTENSION_INSTALL,
    function_body,
)


class SimulatorContractsMixin:
    def test_runner_is_windows_powershell_51_and_split_below_god_file_size(self) -> None:
        self.assertTrue(self.runner.startswith("#requires -Version 5.1\n"))
        self.assertIn("AcceptanceIo.psm1", self.runner)
        self.assertLess(len(self.runner.splitlines()), 800)
        self.assertLess(len(self.module.splitlines()), 300)
        self.assertLess(len(self.static_route_proof.splitlines()), 200)
        self.assertLess(len(self.candidate_manifest_proof.splitlines()), 200)
        self.assertLess(len(self.candidate_provenance_proof.splitlines()), 200)
        self.assertLess(len(self.ads_browse_proof.splitlines()), 350)
        self.assertIn("StaticRouteProof.psm1", self.runner)
        self.assertIn("CandidateManifestProof.psm1", self.runner)
        self.assertIn("CandidateProvenanceProof.psm1", self.runner)
        self.assertIn("AdsBrowseProof.psm1", self.runner)
        for unsupported in ("ForEach-Object -Parallel", "??", "?."):
            self.assertNotIn(unsupported, self.runner)
            self.assertNotIn(unsupported, self.module)

    def test_packaged_simulator_runner_is_powershell_51_and_bounded(self) -> None:
        self.assertTrue(self.simulator_runner.startswith("#requires -Version 5.1\n"))
        self.assertLess(len(self.simulator_runner.splitlines()), 800)
        self.assertLess(len(self.simulator_extension_test.splitlines()), 800)
        self.assertLess(len(self.simulator_visual_proof.splitlines()), 100)
        self.assertLess(len(self.simulator_cdp.splitlines()), 300)
        self.assertLess(len(self.runtime_control_token.splitlines()), 100)
        self.assertLess(len(self.acceptance_redaction.splitlines()), 100)
        self.assertLess(len(self.simulator_launcher.splitlines()), 300)
        self.assertLess(len(self.packaged_extension_install.splitlines()), 200)
        self.assertLess(len(self.packaged_ads_ui.splitlines()), 300)
        self.assertLess(len(self.packaged_ads_custom_ports.splitlines()), 100)
        self.assertLess(
            len(self.packaged_ads_custom_port_acceptance.splitlines()), 200
        )
        self.assertLess(len(self.packaged_ads_custom_port_dom.splitlines()), 100)
        self.assertLess(len(self.packaged_ads_discovery_snapshot.splitlines()), 100)
        self.assertLess(len(self.packaged_ads_ui_crosscheck.splitlines()), 200)
        self.assertLess(len(self.packaged_ads_live_values.splitlines()), 200)
        self.assertLess(len(self.packaged_ads_import_proof.splitlines()), 100)
        self.assertLess(len(self.packaged_ads_toml_proof.splitlines()), 100)
        self.assertLess(len(self.packaged_toml_assignment.splitlines()), 100)
        self.assertLess(len(self.packaged_ads_route_proof.splitlines()), 100)
        self.assertLess(len(self.packaged_ads_snapshot_proof.splitlines()), 100)
        self.assertLess(len(self.packaged_ads_generated_proof.splitlines()), 100)
        self.assertLess(len(self.packaged_ads_live_values_dap_proof.splitlines()), 150)
        self.assertLess(
            len(self.packaged_ads_live_values_render_proof.splitlines()), 100
        )
        self.assertLess(len(self.packaged_ads_browse_selection.splitlines()), 100)
        self.assertLess(len(self.packaged_dap_state.splitlines()), 100)
        self.assertLess(len(self.packaged_ads_dap_snapshot.splitlines()), 100)
        self.assertLess(len(self.packaged_dap_io_state.splitlines()), 100)
        self.assertLess(len(self.packaged_binary_identity.splitlines()), 100)
        self.assertLess(len(self.acceptance_plan.splitlines()), 200)
        self.assertIn("AcceptanceIo.psm1", self.simulator_runner)
        self.assertIn("--extensionTestsPath=", self.simulator_runner)
        self.assertIn("--extensionDevelopmentPath=", self.simulator_runner)
        self.assertIn("--user-data-dir=", self.simulator_runner)
        self.assertIn("--extensions-dir=", self.simulator_runner)
        self.assertNotIn("--install-extension", self.simulator_runner)
        self.assertIn("Install-IsolatedPackagedExtension", self.simulator_runner)
        self.assertIn("New-AcceptanceDriverExtension", self.simulator_runner)
        self.assertIn(
            "Invoke-VscodeCli -Vscode $vscode -Arguments @('--version')",
            self.simulator_runner,
        )
        self.assertIn("version_probe = New-CommandEvidence", self.simulator_runner)
        self.assertIn("cli_script = Get-FileEvidence", self.simulator_runner)
        self.assertIn("cli_launcher = Get-FileEvidence", self.simulator_runner)
        self.assertIn("cli_package = Get-FileEvidence", self.simulator_runner)
        self.assertIn("function Resolve-VscodeCliLayout", self.packaged_extension_install)
        self.assertIn("function Invoke-VscodeCli", self.packaged_extension_install)
        self.assertIn(
            "resources\\\\app\\\\out\\\\cli", self.packaged_extension_install
        )
        layout = function_body(
            self.packaged_extension_install, "Resolve-VscodeCliLayout"
        )
        self.assertIn("bin\\code.cmd", layout)
        self.assertIn("[Regex]::Match", layout)
        self.assertIn("[IO.Path]::GetFullPath", layout)
        self.assertIn(".StartsWith(", layout)
        self.assertIn("package.json", layout)
        self.assertIn("ELECTRON_RUN_AS_NODE = '1'", self.packaged_extension_install)
        self.assertIn("VSCODE_DEV = $null", self.packaged_extension_install)
        install = function_body(
            self.packaged_extension_install, "Install-IsolatedPackagedExtension"
        )
        self.assertIn("Invoke-VscodeCli -Vscode $Vscode", install)
        self.assertNotIn("Invoke-CapturedProcess -FilePath $Vscode", install)
        desktop_launch = function_body(
            self.simulator_runner, "Invoke-VscodeAcceptance"
        )
        self.assertNotIn("Invoke-VscodeCli", desktop_launch)
        self.assertNotIn("ELECTRON_RUN_AS_NODE", desktop_launch)
        self.assertIn(
            "$run = Invoke-VscodeAcceptance -Executable $vscode",
            self.simulator_runner,
        )

    def test_packaged_simulator_uses_exact_vsix_and_bundled_windows_binaries(self) -> None:
        for member in (
            "extension/package.json",
            "extension/out/extension.js",
            "extension/bin/trust-runtime.exe",
            "extension/bin/trust-debug.exe",
            "extension/bin/trust-lsp.exe",
        ):
            self.assertIn(member, self.simulator_runner)
        self.assertIn("ExtractToDirectory", self.simulator_runner)
        self.assertIn("win32-x64", self.simulator_runner)
        self.assertIn("Assert-WindowsPe", self.simulator_runner)
        self.assertIn("exact-installed-extension-loaded", self.simulator_extension_test)
        self.assertIn("packaged-extension-version", self.simulator_extension_test)
        self.assertIn("provePackagedProductIdentity", self.simulator_extension_test)
        self.assertIn("packaged-extension-production-mode", self.packaged_binary_identity)
        self.assertIn("vscode.ExtensionMode.Production", self.packaged_binary_identity)
        self.assertIn("Install-IsolatedPackagedExtension", self.simulator_runner)
        self.assertIn("installed_payload_matches_vsix", self.simulator_runner)
        self.assertIn("installed_executed_files_byte_identical", self.simulator_runner)
        payload_proof = PACKAGED_EXTENSION_INSTALL.with_name(
            "InstalledVsixPayloadProof.psm1"
        ).read_text(encoding="utf-8")
        self.assertIn("__metadata", payload_proof)
        self.assertIn("Disable-PackagedBinaryPathFallback", self.simulator_runner)
        self.assertIn("TRUST_PACKAGED_PATH_FALLBACK_BLOCKED", self.simulator_runner)
        self.assertIn(
            "trust.languageServer.executablePath", self.packaged_extension_install
        )
        self.assertIn(
            "trust.debugAdapter.executablePath", self.packaged_extension_install
        )
        self.assertIn("trust.runtime.executablePath", self.packaged_extension_install)
        self.assertIn("provePackagedBinaryIdentity", self.packaged_binary_identity)
        self.assertIn("exact-packaged-binaries-selected", self.packaged_binary_identity)
        self.assertIn("explicit-isolated-installed-settings", self.packaged_binary_identity)
        self.assertIn("path_fallback_blocked", self.packaged_binary_identity)
        for binary in ("trust-runtime.exe", "trust-debug.exe", "trust-lsp.exe"):
            self.assertIn(binary, self.packaged_binary_identity)

    def test_installed_vsix_payload_requires_exact_manifest_and_file_set(self) -> None:
        proof_path = PACKAGED_EXTENSION_INSTALL.with_name(
            "InstalledVsixPayloadProof.psm1"
        )
        self.assertTrue(
            proof_path.is_file(),
            "installed VSIX payload verification must have one bounded owner",
        )
        proof = proof_path.read_text(encoding="utf-8")
        self.assertLess(len(proof.splitlines()), 150)
        self.assertIn(
            "InstalledVsixPayloadProof.psm1", self.packaged_extension_install
        )
        install = function_body(
            self.packaged_extension_install, "Install-IsolatedPackagedExtension"
        )
        self.assertIn("Assert-InstalledVsixPayload", install)
        self.assertIn("-VsixManifestPath $VsixManifestPath", install)
        self.assertIn(
            "-VsixManifestPath (Join-Path $expandedRoot 'extension.vsixmanifest')",
            self.simulator_runner,
        )
        verification = function_body(proof, "Assert-InstalledVsixPayload")
        self.assertIn("'.vsixmanifest'", verification)
        self.assertIn("Get-ChildItem", verification)
        self.assertIn("-Force", verification)
        self.assertIn("$missing", verification)
        self.assertIn("$extra", verification)
        self.assertIn("reserved installed manifest path", verification)
        self.assertIn("HashSet[string]", verification)
        self.assertIn("$expectedPathSet.Add", verification)
        self.assertIn("$installedPathSet.Contains", verification)
        self.assertIn("Assert-ExactManifestBytes", verification)
        exact_manifest = function_body(proof, "Assert-ExactManifestBytes")
        self.assertIn("Get-FileEvidence", exact_manifest)
        self.assertIn("size_bytes", exact_manifest)
        self.assertIn("sha256", exact_manifest)
        self.assertIn("[IO.File]::ReadAllBytes", exact_manifest)

    def test_packaged_simulator_drives_actual_sidebar_and_cross_surface_states(self) -> None:
        source = self.simulator_extension_test
        self.assertIn('"#action"', source)
        self.assertIn('".react-flow"', source)
        self.assertIn("button.click()", source)
        self.assertIn("starting-is-one-disabled-attempt", source)
        self.assertIn("-debug-session-count", source)
        self.assertIn("running-surfaces-agree", source)
        self.assertIn("stopped-surfaces-agree-after-stop", source)
        self.assertIn("final-stopped-surfaces-agree", source)
        self.assertIn("second-start-after-stop-has-no-stale-session", source)
        self.assertIn("fresh-reload-start-before-devices", source)
        self.assertIn("truST:\\s*Simulator running", source)
        self.assertIn("truST:\\s*Simulator stopped", source)
        self.assertIn("truST:\\s*Simulator starting", source)
        self.assertNotIn('executeCommand("trust-lsp.debug.start"', source)
        self.assertNotIn("startDebugging(", source)
        first_start = source.index('beginStartAttempt("first-start", false)')
        open_devices = source.index('executeCommand("trust-lsp.networkCanvas.open")')
        first_stop = source.index('stopAndWait("first-stop", 1)')
        second_start = source.index('beginStartAttempt(\n      "second-start"')
        pre_ads_stop = source.index('stopAndWait("pre-ads-stop", 2)')
        ads_ui = source.index(
            "const importedVariable = await runPackagedAdsUiAcceptance"
        )
        final_stop = source.index('stopAndWait("final-stop", 3)')
        self.assertLess(first_start, open_devices)
        self.assertLess(open_devices, first_stop)
        self.assertLess(first_stop, second_start)
        self.assertLess(second_start, pre_ads_stop)
        self.assertLess(pre_ads_stop, ads_ui)
        self.assertLess(ads_ui, final_stop)

    def test_packaged_simulator_reproduces_auth_and_live_values_regressions(self) -> None:
        self.assertIn("disposable_tokenless_project = $true", self.simulator_runner)
        self.assertIn("fixture did not begin tokenless", self.simulator_runner)
        self.assertIn("tokenless-project-migrated-before-launch", self.simulator_extension_test)
        self.assertIn("session_control_auth_present", self.simulator_extension_test)
        self.assertIn("session_control_endpoint_loopback", self.simulator_extension_test)
        self.assertIn("start-keeps-devices-open", self.simulator_extension_test)
        self.assertIn("live_values_ever_opened", self.simulator_extension_test)
        self.assertIn("live_values_ever_focused", self.simulator_extension_test)
        self.assertIn("onDidChangeTabs", self.simulator_extension_test)
        self.assertIn("setInterval(() => observeTabs", self.simulator_extension_test)
        self.assertIn("devices-first-paint-bounded", self.simulator_extension_test)
        self.assertIn("No auth token provided", self.simulator_visual_proof)
        self.assertIn("no-visible-auth-error-after-second-start", self.simulator_extension_test)
        self.assertIn("session_control_auth_stable", self.simulator_extension_test)
        self.assertIn("crypto.timingSafeEqual", self.simulator_extension_test)
        self.assertIn("first_and_second_session_control_auth_equal", self.simulator_extension_test)
        self.assertIn("credential_value_recorded = $false", self.simulator_runner)
        self.assertIn("exact_credential_scan_performed = $false", self.simulator_runner)
        self.assertIn("$innerRaw.Contains($migratedControlAuthToken)", self.simulator_runner)
        self.assertIn("$json.Contains($migratedControlAuthToken)", self.simulator_runner)
        self.assertIn("$json.Replace($migratedControlAuthToken, '<redacted>')", self.simulator_runner)
        self.assertLess(
            self.simulator_runner.index("$innerRaw.Contains($migratedControlAuthToken)"),
            self.simulator_runner.index("journey exited with"),
        )
        self.assertIn("serializeWithoutCredential", self.simulator_extension_test)
        self.assertIn("temporary_root_cleanup_verified", self.simulator_runner)
        self.assertIn("cleanup_failure_credential_removed", self.simulator_runner)
        self.assertNotIn(
            "ErrorAction SilentlyContinue",
            self.simulator_runner.rsplit("\nfinally {", 1)[1],
        )
        self.assertNotRegex(
            self.simulator_extension_test,
            r"proof\.[A-Za-z0-9_.]+\s*=\s*token\s*[;\n]",
        )

    def test_disposable_fixture_is_semantically_tokenless_only_for_runtime_control(self) -> None:
        match = re.search(
            r'\$runtime\s*=\s*@"\n(?P<toml>.*?)\n"@',
            self.simulator_runner,
            flags=re.DOTALL,
        )
        self.assertIsNotNone(match, "locate the actual disposable runtime.toml fixture")
        parsed = tomllib.loads(match.group("toml") if match else "")
        self.assertNotIn("auth_token", parsed["runtime"]["control"])
        self.assertEqual(parsed["runtime"]["mesh"]["auth_token"], "")
        helper = function_body(
            self.simulator_runner, "Get-RuntimeControlAuthTokenEvidence"
        )
        helper = helper.rsplit("\n}", 1)[0]
        self.assertIn("[runtime\\.control\\]", helper)
        self.assertIn("runtime\\.control\\.auth_token", helper)
        self.assertNotIn("(?:runtime\\.control\\.)?auth_token", helper)

    def test_twin_cat_one_command_runs_packaged_simulator_by_default(self) -> None:
        self.assertIn("PackagedSimulatorLauncher.psm1", self.runner)
        self.assertIn("Invoke-PackagedSimulatorAcceptance", self.runner)
        self.assertIn("accept_windows_packaged_simulator.ps1", self.simulator_launcher)
        self.assertIn("packaged_simulator_acceptance_required", self.runner)
        self.assertIn("packaged_ads_ui_acceptance_required = $true", self.runner)
        self.assertNotIn("SkipSimulatorAcceptance", self.runner)
        self.assertIn("evidence.simulator", self.runner)
        self.assertIn(
            "Packaged Simulator/ADS/Live Values acceptance failed", self.runner
        )
        self.assertIn("packaged_simulator", self.runner)
        self.assertIn("-RequireAdsUi", self.runner)
        self.assertIn("TRUST_PACKAGED_ADS_UI_REQUIRED", self.simulator_runner)
        self.assertIn("Test-PackagedAdsUiAgainstCliServices", self.runner)
        self.assertIn("service_statuses_match", self.packaged_ads_ui_crosscheck)
        self.assertIn("symbol_count", self.packaged_ads_ui_crosscheck)

    def test_package_provenance_is_explicit_frozen_and_cross_checked(self) -> None:
        for marker in (
            "ExpectedVersion",
            "ExpectedVsixSha256",
            "CandidateManifestPath",
            "Read-WindowsAdsCandidateProvenance",
            "New-FileSnapshot",
            "Assert-ExactTrustRuntimeVersion",
            "Assert-PackagedSimulatorArtifactIdentity",
            "simulator_artifact_identity",
        ):
            self.assertIn(marker, self.runner)
        self.assertIn("exact VSIX version and SHA-256", self.simulator_launcher)
        self.assertIn("binary identity differs", self.simulator_launcher)
        self.assertNotIn('-notlike "*$', self.runner)
        for marker in (
            "Discover ADS devices",
            "packaged-ads-ui-one-zero-input-action",
            "packaged-ads-ui-finds-expected-native-target-and-851",
            "Object.freeze([851, 852, 853, 854, 301, 501])",
            "selectable_variable_count > 0",
            "inner_discover_click_count: 0",
            "selectOneVariable",
            "addSelectedVariable",
            "runPackagedAdsCustomPortAcceptance",
        ):
            self.assertIn(
                marker,
                self.packaged_ads_ui + self.packaged_ads_custom_ports,
            )
        for marker in (
            "TRUST_PACKAGED_ADS_EXPECTED_CUSTOM_PORTS",
            "parseExpectedCustomAdsPorts",
            "expectedCustomAdsPorts",
            "custom_port_recovery: null",
        ):
            self.assertIn(marker, self.simulator_extension_test)
        for marker in (
            "packaged-ads-ui-advanced-custom-port-results",
            "stale_results_observed",
            "default_results_rechecked",
            "custom_results_present",
            "explicit_result_rows",
            'rescan_phases_observed.includes("probing")',
        ):
            self.assertIn(marker, self.packaged_ads_custom_port_acceptance)
        for marker in (
            "HTMLInputElement.prototype",
            "dispatchEvent(new Event('input',{bubbles:true}))",
            "dispatchEvent(new Event('change',{bubbles:true}))",
            "Scan ADS again",
        ):
            self.assertIn(marker, self.packaged_ads_custom_port_dom)
        for marker in (
            "data-result-visibility",
            "ads-results-stale",
            "ads-probe-progress",
            "custom_ports_value",
        ):
            self.assertIn(marker, self.packaged_ads_discovery_snapshot)
        self.assertIn("configured_access:'read'", self.packaged_ads_browse_selection)
        self.assertIn(
            "(snapshot) => Object.values(snapshot).every(Boolean)",
            self.packaged_ads_ui,
        )
        self.assertIn("nonStringScalar", self.packaged_ads_browse_selection)
        self.assertIn("read-only-non-string-scalar", self.packaged_ads_browse_selection)
        self.assertIn("selected_type", self.packaged_ads_browse_selection)
        for marker in (
            "response_event_converged",
            "accepted_quality_recent_after_restart",
            "later_scan_strictly_higher",
            "later_imported_entry_still_good",
            'executeCommand("trust-lsp.debug.openIoPanel")',
            'row?.button_count === 0',
            '"live-values-renders-imported-ads-variable-read-only"',
        ):
            self.assertIn(marker, self.packaged_ads_live_values)
        self.assertNotIn("response_event_identical", self.packaged_ads_live_values)
        self.assertNotIn("accepted.entry.value.length", self.packaged_ads_live_values)
        self.assertIn(
            'typeof accepted.entry.value === "string"',
            self.packaged_ads_live_values,
        )
        self.assertIn("readMatchingRenderedAdsValue", self.packaged_ads_live_values)
        self.assertIn("row.value === entry.value", self.packaged_ads_live_values_render_proof)
        self.assertIn(
            "row.value_type === entry.valueType",
            self.packaged_ads_live_values_render_proof,
        )
        self.assertIn('session.customRequest("stAdsState")', self.packaged_dap_state)
        self.assertIn('session.customRequest("stIoState")', self.packaged_dap_io_state)
        self.assertIn("selectedAdsSnapshot", self.packaged_dap_state)
        for marker in (
            "Number.isSafeInteger(candidate.body.scan)",
            "updatedAt >= restartStartedAtMs",
            "candidate.body.scan > accepted.body.scan",
            "sameImportedEntry(candidate.entry, accepted.entry)",
        ):
            self.assertIn(marker, self.packaged_ads_live_values_dap_proof)
        self.assertIn('selected_remote_symbol_present', self.packaged_ads_import_proof)
        self.assertIn('selected_point_mapping_exact', self.packaged_ads_import_proof)
        self.assertIn('selected_connection_route_exact', self.packaged_ads_import_proof)
        for marker in (
            "route.target_net_id === expected.targetNetId",
            "normalizeHost(route.host) === normalizeHost(expected.host)",
            "route.ams_port === expected.amsPort",
            'route.transport === "plain"',
            "route.insecure_transport === true",
        ):
            self.assertIn(marker, self.packaged_ads_route_proof)
        self.assertIn('generated_typed_local_declaration', self.packaged_ads_import_proof)
        self.assertIn('generated_quality_mapping', self.packaged_ads_import_proof)
        self.assertIn('runtime_ads_enabled', self.packaged_ads_import_proof)
        self.assertIn('imported_read_only_variable', self.packaged_ads_ui_crosscheck)
        self.assertIn(
            'imported_variable_rendered_in_live_values',
            self.packaged_ads_ui_crosscheck,
        )
        for marker in (
            "candidate_commit_sha",
            "windows-ads-msvc-candidate-",
            "vsix_sha256",
            "Assert-WindowsAdsCandidateVsix",
        ):
            self.assertIn(marker, self.candidate_manifest_proof)
        self.assertIn("candidate_manifest = $candidateProof", self.runner)
        self.assertIn("candidate_provenance = $candidateProvenance", self.runner)
        self.assertIn("candidate_commit_bound = $true", self.runner)
        self.assertIn("candidate_api_provenance_bound = $true", self.runner)
        for marker in (
            "github_actions_artifact_api_v1",
            "github_api_job_success",
            "artifact_archive_digest_verified",
            "artifact_archive_sha256",
        ):
            self.assertIn(marker, self.candidate_provenance_proof)

    def test_packaged_ads_import_artifact_reader_requires_all_restart_inputs(self) -> None:
        with tempfile.TemporaryDirectory(prefix="trust-ads-import-proof-") as temp:
            root = Path(temp)
            (root / "src" / "generated").mkdir(parents=True)
            (root / "ads" / "snapshots").mkdir(parents=True)
            ads_toml = (
                '[[connections]]\nname = "line1"\n'
                'target_net_id = "192.0.2.11.1.1"\n'
                'host = "192.0.2.11"\nams_port = 851\n'
                'transport = "plain"\ninsecure_transport = true\n\n'
                '[[connections.points]]\nsymbol = "MAIN.temperature"\n'
                'var = "imported_temperature"\ntype = "REAL"\n'
                'access = "read"\nmode = "poll"\n'
            )
            runtime_toml = (
                '[runtime.log]\r\nlevel = "info"\r\n\r\n'
                '[runtime.ads] # imported\r\nenabled = true\r\n'
                'config_path = "ads.toml"\r\n'
                'worker_tick_interval_ms = 20\r\n\r\n'
                '[runtime.web]\r\nenabled = false\r\n'
            )
            generated_st = (
                "TYPE\n"
                "  ADS_QUALITY : (Stale := 0, Good := 1, Error := 2);\n"
                "END_TYPE\nVAR_GLOBAL\n"
                "  imported_temperature : REAL;\n"
                "  imported_temperature_quality : ADS_QUALITY := Stale;\n"
                "END_VAR\n"
            )
            snapshot = {
                "schema_version": 1,
                "route_name": "line1",
                "symbols": [
                    {
                        "name": "MAIN.temperature",
                        "data_type": {"source_name": "REAL", "iec_type": "REAL"},
                        "index_group": 16416,
                        "index_offset": 0,
                        "byte_size": 4,
                        "flags": ["read"],
                    }
                ],
            }
            script = (
                "const p=require(process.argv[1]);"
                "console.log(JSON.stringify(p.readProjectAdsImport(process.argv[2],process.argv[3],"
                "{targetNetId:process.argv[4],host:process.argv[5],amsPort:Number(process.argv[6])})));"
            )

            def write_valid() -> None:
                (root / "ads.toml").write_text(ads_toml, encoding="utf-8")
                (root / "runtime.toml").write_text(runtime_toml, encoding="utf-8")
                (root / "src" / "generated" / "ads_generated.st").write_text(
                    generated_st, encoding="utf-8"
                )
                (root / "ads" / "snapshots" / "line1.symbols.json").write_text(
                    json.dumps(snapshot), encoding="utf-8"
                )

            def read_proof() -> dict[str, object]:
                completed = subprocess.run(
                    [
                        "node",
                        "-e",
                        script,
                        str(PACKAGED_ADS_IMPORT_PROOF),
                        str(root),
                        "MAIN.temperature",
                        "192.0.2.11.1.1",
                        "192.0.2.11",
                        "851",
                    ],
                    check=False,
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(completed.returncode, 0, completed.stderr)
                return json.loads(completed.stdout)

            write_valid()
            result = read_proof()
            self.assertTrue(all(result.values()), result)

            (root / "ads.toml").write_text(
                '# symbol = "MAIN.temperature"\n'
                '[[connections]]\nname = "line1"\n'
                'evidence = "MAIN.temperature"\n'
                '[[connections.points]]\nsymbol = "MAIN.other"\n'
                'var = "other"\ntype = "REAL"\naccess = "read"\n'
                '# [[connections.points]]\n# symbol = "MAIN.temperature"\n',
                encoding="utf-8",
            )
            self.assertFalse(read_proof()["selected_point_mapping_exact"])

            (root / "ads.toml").write_text(
                ads_toml.replace('access = "read"', 'access = "read_write"'),
                encoding="utf-8",
            )
            self.assertFalse(read_proof()["selected_remote_symbol_present"])

            (root / "ads.toml").write_text(
                ads_toml
                + '\n[[connections.points]]\nsymbol = "MAIN.temperature"\n'
                + 'var = "shadow_temperature"\ntype = "REAL"\naccess = "read"\n',
                encoding="utf-8",
            )
            self.assertFalse(read_proof()["selected_point_mapping_exact"])

            for original, replacement in (
                (
                    'target_net_id = "192.0.2.11.1.1"',
                    'target_net_id = "192.0.2.99.1.1"',
                ),
                ('host = "192.0.2.11"', 'host = "192.0.2.99"'),
                ("ams_port = 851", "ams_port = 852"),
                ('transport = "plain"', 'transport = "secure"'),
                ("insecure_transport = true", "insecure_transport = false"),
            ):
                (root / "ads.toml").write_text(
                    ads_toml.replace(original, replacement), encoding="utf-8"
                )
                wrong_route = read_proof()
                self.assertTrue(wrong_route["selected_point_mapping_exact"])
                self.assertFalse(wrong_route["selected_connection_route_exact"])

            write_valid()
            unrelated_snapshot = {
                "schema_version": 1,
                "route_name": "line1",
                "selected_in_comment": "MAIN.temperature",
                "symbols": [
                    {
                        **snapshot["symbols"][0],
                        "name": "MAIN.other",
                    }
                ],
            }
            (root / "ads" / "snapshots" / "line1.symbols.json").write_text(
                json.dumps(unrelated_snapshot), encoding="utf-8"
            )
            self.assertFalse(read_proof()["selected_snapshot_structural"])

            wrong_type_snapshot = json.loads(json.dumps(snapshot))
            wrong_type_snapshot["symbols"][0]["data_type"]["iec_type"] = "LREAL"
            (root / "ads" / "snapshots" / "line1.symbols.json").write_text(
                json.dumps(wrong_type_snapshot), encoding="utf-8"
            )
            self.assertFalse(read_proof()["selected_symbol_snapshot_present"])

            write_valid()
            (root / "src" / "generated" / "ads_generated.st").write_text(
                "TYPE\n  ADS_QUALITY : (Stale := 0, Good := 1, Error := 2);\n"
                "END_TYPE\nVAR_GLOBAL\n(*\n  imported_temperature : REAL;\n"
                "  imported_temperature_quality : ADS_QUALITY := Stale;\n*)\n"
                "  unrelated : REAL;\n"
                "  unrelated_quality : ADS_QUALITY := Stale;\nEND_VAR\n",
                encoding="utf-8",
            )
            comment_only = read_proof()
            self.assertFalse(comment_only["generated_typed_local_declaration"])
            self.assertFalse(comment_only["generated_quality_mapping"])

            (root / "src" / "generated" / "ads_generated.st").write_text(
                generated_st.replace("imported_temperature : REAL;", "imported_temperature : LREAL;"),
                encoding="utf-8",
            )
            wrong_type = read_proof()
            self.assertFalse(wrong_type["generated_typed_local_declaration"])
            self.assertTrue(wrong_type["generated_quality_mapping"])

            (root / "src" / "generated" / "ads_generated.st").write_text(
                generated_st.replace(
                    "imported_temperature_quality : ADS_QUALITY := Stale;",
                    "unrelated_quality : ADS_QUALITY := Stale;",
                ),
                encoding="utf-8",
            )
            wrong_quality = read_proof()
            self.assertTrue(wrong_quality["generated_typed_local_declaration"])
            self.assertFalse(wrong_quality["generated_quality_mapping"])

            write_valid()
            (root / "runtime.toml").write_text(
                '[runtime.ads]\nenabled = true\nenabled = false\nconfig_path = "ads.toml"\nworker_tick_interval_ms = 20\n',
                encoding="utf-8",
            )
            self.assertFalse(read_proof()["runtime_ads_enabled"])
