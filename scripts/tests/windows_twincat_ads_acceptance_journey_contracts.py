from __future__ import annotations

import json
import re
import shutil
import subprocess
import unittest

from .windows_twincat_ads_acceptance_support import (
    ADS_BROWSE_PROOF,
    MODULE,
    PACKAGED_ADS_CUSTOM_PORTS,
    PACKAGED_ADS_CUSTOM_PORT_ACCEPTANCE,
    PACKAGED_ADS_LIVE_VALUES_DAP_PROOF,
    PACKAGED_ADS_LIVE_VALUES_RENDER_PROOF,
    PACKAGED_DAP_STATE,
    function_body,
)


class PackagedAdsJourneyContractsMixin:
    def test_packaged_custom_ads_port_environment_is_strict(self) -> None:
        script = r"""
const { parseExpectedCustomAdsPorts } = require(process.argv[1]);
function rejected(value) {
  try { parseExpectedCustomAdsPorts(value, true); return false; }
  catch (_error) { return true; }
}
console.log(JSON.stringify({
  valid: parseExpectedCustomAdsPorts("9000, 9001", true),
  disabled: parseExpectedCustomAdsPorts("invalid", false),
  empty: rejected(""),
  duplicate: rejected("9000,9000"),
  built_in: rejected("851"),
  too_many: rejected("9000,9001,9002,9003,9004"),
  out_of_range: rejected("65536"),
  malformed: rejected("9000,tcp:9001"),
}));
"""
        completed = subprocess.run(
            ["node", "-e", script, str(PACKAGED_ADS_CUSTOM_PORTS)],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        result = json.loads(completed.stdout)
        self.assertEqual(result["valid"], [9000, 9001])
        self.assertEqual(result["disabled"], [])
        for key in (
            "empty",
            "duplicate",
            "built_in",
            "too_many",
            "out_of_range",
            "malformed",
        ):
            self.assertTrue(result[key], key)

    def test_packaged_custom_ads_port_journey_rechecks_same_card(self) -> None:
        script = r"""
const { runPackagedAdsCustomPortAcceptance } = require(process.argv[1]);
const required = [851, 852, 853, 854, 301, 501];
const target = { ams_net_id: "192.0.2.11.1.1", host: "192.0.2.11" };
let discoveryReads = 0;
let assertion;
const state = {};
function rows(ports) {
  return ports.map((port) => ({
    port,
    status: port === 851 ? "available" : "unavailable",
    visibility: port === 851 ? "responding" : "technical",
  }));
}
function card(ports, stale) {
  return {
    ams_net_id: target.ams_net_id,
    host: target.host,
    results_stale: stale,
    services: rows(ports),
  };
}
async function discoverySnapshot() {
  discoveryReads += 1;
  if (discoveryReads === 1) {
    return {
      advanced_expanded: true,
      custom_ports_value: "9000",
      custom_ports_invalid: false,
      discover_state: "idle",
      discover_text: "Scan ADS again",
      cards: [card(required, true)],
    };
  }
  if (discoveryReads === 2) {
    return {
      discover_state: "probing",
      discover_text: "Checking ADS services…",
      cards: [card([...required, 9000], false)],
    };
  }
  return {
    discover_state: "idle",
    discover_text: "Scan ADS again",
    cards: [card([...required, 9000], false)],
  };
}
async function evaluate(_selector, body) {
  if (body.includes("ads-advanced-toggle")) return { clicked: true };
  if (body.includes("ads-custom-ports")) {
    return {
      ready: true,
      value: "9000",
      input_event_dispatched: true,
      change_event_dispatched: true,
    };
  }
  if (body.includes("ads-discover")) return { clicked: true, text: "Scan ADS again" };
  throw new Error("unexpected evaluate body");
}
function check(id, pass, detail) {
  assertion = { id, pass, detail };
  if (!pass) throw new Error(`failed ${id}`);
}
(async () => {
  await runPackagedAdsCustomPortAcceptance({
    evaluate,
    discoverySnapshot,
    sleep: async () => {},
    check,
    state,
    target,
    expectedCustomPorts: [9000],
  });
  console.log(JSON.stringify({ assertion, state, discoveryReads }));
})().catch((error) => { console.error(error); process.exitCode = 1; });
"""
        completed = subprocess.run(
            ["node", "-e", script, str(PACKAGED_ADS_CUSTOM_PORT_ACCEPTANCE)],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        result = json.loads(completed.stdout)
        self.assertEqual(
            result["assertion"]["id"],
            "packaged-ads-ui-advanced-custom-port-results",
        )
        self.assertTrue(result["assertion"]["pass"])
        recovery = result["state"]["custom_port_recovery"]
        self.assertEqual(recovery["requested_custom_ports"], [9000])
        self.assertTrue(recovery["stale_results_observed"])
        self.assertTrue(recovery["default_results_rechecked"])
        self.assertTrue(recovery["custom_results_present"])
        self.assertTrue(recovery["stale_results_cleared"])
        self.assertTrue(recovery["explicit_result_rows"])
        self.assertEqual(recovery["exact_result_row_count"], 7)
        self.assertIn("probing", recovery["rescan_phases_observed"])
        self.assertEqual(result["discoveryReads"], 3)

    def test_packaged_ads_dap_skips_unsolicited_event_until_snapshot_converges(
        self,
    ) -> None:
        script = r"""
const { requestAdsStateEvent } = require(process.argv[1]);
const listeners = new Set();
let disposed = 0;
const session = {
  id: "ads-session",
  customRequest(command) {
    if (command !== "stAdsState") throw new Error(`unexpected ${command}`);
    emit(snapshot(7, "unsolicited"));
    return new Promise((resolve) => setTimeout(() => {
      const response = snapshot(8, "fresh");
      resolve(response);
      setTimeout(() => emit(response), 5);
    }, 5));
  },
};
const vscode = { debug: { onDidReceiveDebugSessionCustomEvent(listener) {
  listeners.add(listener);
  return { dispose() { listeners.delete(listener); disposed += 1; } };
} } };
function snapshot(scan, value) {
  return { schemaVersion: 1, scan, entries: [{
    connection: "line1", name: "imported_temperature",
    remoteSymbol: "MAIN.temperature", value, valueType: "REAL", access: "read",
    quality: { state: "good", lastUpdateMs: 1234 },
  }] };
}
function emit(body) {
  for (const listener of [...listeners]) {
    listener({ session, event: "stAdsState", body });
  }
}
(async () => {
  const result = await requestAdsStateEvent(
    vscode, session, "MAIN.temperature", 1000
  );
  console.log(JSON.stringify({
    response_event_converged: result.responseEventConverged,
    events_observed: result.eventsObserved,
    response_scan: result.response.scan,
    event_scan: result.eventBody.scan,
    event_value: result.eventBody.entries[0].value,
    disposed,
  }));
})().catch((error) => { console.error(error); process.exitCode = 1; });
"""
        completed = subprocess.run(
            ["node", "-e", script, str(PACKAGED_DAP_STATE)],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        result = json.loads(completed.stdout)
        self.assertTrue(result["response_event_converged"])
        self.assertEqual(result["events_observed"], 2)
        self.assertEqual(result["response_scan"], 8)
        self.assertEqual(result["event_scan"], 8)
        self.assertEqual(result["event_value"], "fresh")
        self.assertEqual(result["disposed"], 1)

    def test_packaged_ads_live_values_requires_fresh_and_later_scan(self) -> None:
        script = r"""
const { readFreshAdsSnapshots } = require(process.argv[1]);
const listeners = new Set();
const now = Date.now();
const restartStartedAtMs = now - 1000;
const queue = [
  snapshot(0, now, "zero-scan"),
  snapshot(9, restartStartedAtMs - 1, "pre-restart"),
  snapshot(10, now, "accepted"),
  snapshot(10, now + 1, "same-scan"),
  snapshot(11, now + 2, "later-value-may-change"),
];
let calls = 0;
const session = {
  id: "ads-session",
  customRequest(command) {
    if (command !== "stAdsState") throw new Error(`unexpected ${command}`);
    const body = queue.shift();
    calls += 1;
    if (!body) throw new Error("proof requested an unexpected extra snapshot");
    setTimeout(() => emit(body), 0);
    return Promise.resolve(body);
  },
};
const vscode = { debug: { onDidReceiveDebugSessionCustomEvent(listener) {
  listeners.add(listener);
  return { dispose() { listeners.delete(listener); } };
} } };
function snapshot(scan, lastUpdateMs, value) {
  return { schemaVersion: 1, scan, entries: [{
    connection: "line1", name: "imported_temperature",
    remoteSymbol: "MAIN.temperature", value, valueType: "REAL", access: "read",
    quality: { state: "good", lastUpdateMs },
  }] };
}
function emit(body) {
  for (const listener of [...listeners]) {
    listener({ session, event: "stAdsState", body });
  }
}
(async () => {
  const proof = await readFreshAdsSnapshots({
    vscode, session, remoteSymbol: "MAIN.temperature", restartStartedAtMs,
    sleep: async () => {},
  });
  console.log(JSON.stringify({
    calls,
    accepted_scan: proof.accepted?.body?.scan,
    accepted_updated_at: proof.accepted?.entry?.quality?.lastUpdateMs,
    later_scan: proof.later?.body?.scan,
    later_good: proof.later?.entry?.quality?.state === "good",
    later_value_changed: proof.later?.entry?.value !== proof.accepted?.entry?.value,
    convergence_failures: proof.convergenceFailures,
  }));
})().catch((error) => { console.error(error); process.exitCode = 1; });
"""
        completed = subprocess.run(
            ["node", "-e", script, str(PACKAGED_ADS_LIVE_VALUES_DAP_PROOF)],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        result = json.loads(completed.stdout)
        self.assertEqual(result["calls"], 5)
        self.assertEqual(result["accepted_scan"], 10)
        self.assertGreater(result["accepted_updated_at"], 0)
        self.assertEqual(result["later_scan"], 11)
        self.assertTrue(result["later_good"])
        self.assertTrue(result["later_value_changed"])
        self.assertEqual(result["convergence_failures"], 0)

    def test_packaged_ads_render_match_retries_and_accepts_empty_string(self) -> None:
        script = r"""
const { readMatchingRenderedAdsValue } = require(process.argv[1]);
const listeners = new Set();
const queue = [snapshot(21, "")];
let renderedReads = 0;
let dapRequests = 0;
const session = {
  id: "ads-session",
  customRequest(command) {
    if (command !== "stAdsState") throw new Error(`unexpected ${command}`);
    const body = queue.shift();
    if (!body) throw new Error("unexpected extra DAP request");
    dapRequests += 1;
    setTimeout(() => emit(body), 0);
    return Promise.resolve(body);
  },
};
const vscode = { debug: { onDidReceiveDebugSessionCustomEvent(listener) {
  listeners.add(listener);
  return { dispose() { listeners.delete(listener); } };
} } };
function snapshot(scan, value) {
  return { schemaVersion: 1, scan, entries: [{
    connection: "line1", name: "imported_text",
    remoteSymbol: "MAIN.text", value, valueType: "STRING(80)", access: "read",
    quality: { state: "good", lastUpdateMs: Date.now() },
  }] };
}
function emit(body) {
  for (const listener of [...listeners]) {
    listener({ session, event: "stAdsState", body });
  }
}
async function readRendered() {
  renderedReads += 1;
  const value = renderedReads === 1 ? "stale-render" : "";
  return { rows: [{
    remote_symbol: "MAIN.text", value, value_type: "STRING(80)", quality: "Good",
  }] };
}
(async () => {
  const proof = await readMatchingRenderedAdsValue({
    vscode, session, remoteSymbol: "MAIN.text", readRendered,
    sleep: async () => {}, timeoutMs: 1000,
  });
  console.log(JSON.stringify({
    comparisons: proof.comparisons,
    dap_requests: dapRequests,
    scan: proof.scan,
    value_is_empty_string: typeof proof.entry.value === "string" && proof.entry.value === "",
    rendered_value_matches: proof.row.value === proof.entry.value,
  }));
})().catch((error) => { console.error(error); process.exitCode = 1; });
"""
        completed = subprocess.run(
            ["node", "-e", script, str(PACKAGED_ADS_LIVE_VALUES_RENDER_PROOF)],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        result = json.loads(completed.stdout)
        self.assertEqual(result["comparisons"], 2)
        self.assertEqual(result["dap_requests"], 1)
        self.assertEqual(result["scan"], 21)
        self.assertTrue(result["value_is_empty_string"])
        self.assertTrue(result["rendered_value_matches"])

    def test_raw_evidence_defaults_to_private_local_app_data_not_the_repo(self) -> None:
        default_block = self.runner.split(
            "$resolvedEvidencePath = if ([string]::IsNullOrWhiteSpace($EvidencePath)) {",
            maxsplit=1,
        )[1].split("} else {", maxsplit=1)[0]
        self.assertIn("LocalApplicationData", default_block)
        self.assertIn("truST\\private-evidence\\windows-twincat-ads", default_block)
        self.assertNotIn("$repositoryRoot", default_block)
        simulator_default = self.simulator_runner.split(
            "$resolvedEvidencePath = if ([string]::IsNullOrWhiteSpace($EvidencePath)) {",
            maxsplit=1,
        )[1].split("} else {", maxsplit=1)[0]
        self.assertIn("LocalApplicationData", simulator_default)
        self.assertIn(
            "truST\\private-evidence\\windows-packaged-simulator",
            simulator_default,
        )
        self.assertNotIn("$repositoryRoot", simulator_default)

    def test_discovery_is_exactly_zero_input_for_identity(self) -> None:
        match = re.search(
            r"\$discoveryArguments\s*=\s*@\((.*?)\)\s*\n"
            r"\s*\$discoveryResult",
            self.runner,
            flags=re.DOTALL,
        )
        self.assertIsNotNone(match, "locate zero-input discovery argument list")
        arguments = match.group(1) if match else ""
        for required in ("'comm'", "'discover'", "'--protocol'", "'ads'", "'--origin'", "'this-host'", "'--json'"):
            self.assertIn(required, arguments)
        for forbidden in ("--host", "--target-net-id", "--ams-port", "--cidr"):
            self.assertNotIn(forbidden, arguments)
        self.assertNotIn("ExpectedTargetNetId", arguments)
        self.assertNotIn("ExpectedRouterSourceNetId", arguments)
        self.assertIn("zero_input_discovery = $true", self.runner)
        self.assertIn("manual_ams_net_id_supplied = $false", self.runner)

    def test_only_observed_native_candidates_can_be_selected(self) -> None:
        candidate_function = function_body(self.runner, "Get-ProvenNativeCandidates")
        self.assertIn("'ads_local_router'", candidate_function)
        self.assertIn("'observed'", candidate_function)
        self.assertIn("Assert-AmsNetId", candidate_function)
        self.assertIn("Zero proven same-computer native ADS candidates", candidate_function)
        self.assertNotIn(".1.1", candidate_function)
        self.assertNotRegex(candidate_function.lower(), r"derive|manual")
        self.assertIn("will not accept another local runtime", candidate_function)
        self.assertIn("duplicate target identity is ambiguous", candidate_function)

    def test_expected_target_identity_is_bound_across_discovery_ui_cli_and_doctor(self) -> None:
        self.assertIn("[string]$ExpectedTargetNetId", self.runner)
        self.assertIn("[string]$ExpectedRouterSourceNetId", self.runner)
        self.assertIn("target_identity_contract", self.runner)
        self.assertIn("discovery_ui_cli_doctor_match", self.runner)
        self.assertIn("source_target_addresses_distinct", self.runner)
        candidate_function = function_body(self.runner, "Get-ProvenNativeCandidates")
        self.assertIn("[string]$ExpectedNetId", candidate_function)
        self.assertIn("$expected.Count -ne 1", candidate_function)
        self.assertNotIn("HashSet[string]", candidate_function)
        doctor = function_body(self.ads_browse_proof, "Assert-NativeDoctorProof")
        self.assertIn("$Candidate.ams_net_id -cne $ExpectedTargetNetId", doctor)
        self.assertIn("$sourceNetId -cne $ExpectedRouterSourceNetId", doctor)
        self.assertIn("$sourceNetId -ceq $targetNetId -and $sourcePort -eq $targetPort", doctor)
        self.assertIn("source_target_comparison", doctor)
        self.assertIn("full_ams_address", doctor)
        self.assertIn("source_target_addresses_distinct", doctor)
        self.assertIn("source_ams_port = $sourcePort", doctor)
        self.assertIn("target_ams_port = $targetPort", doctor)
        self.assertIn("-ExpectedAdsTargetNetId $ExpectedTargetNetId", self.runner)
        self.assertIn(
            "TRUST_PACKAGED_ADS_EXPECTED_TARGET_NET_ID", self.simulator_runner
        )
        self.assertIn("expectedTargetNetId", self.packaged_ads_ui)
        self.assertIn("expectedCards.length === 1", self.packaged_ads_ui)
        self.assertIn("[string]$ExpectedTargetNetId", self.packaged_ads_ui_crosscheck)
        self.assertIn("$uiNetId -ceq $ExpectedTargetNetId", self.packaged_ads_ui_crosscheck)

    def test_router_source_expectation_is_optional_and_equal_net_ids_are_allowed(self) -> None:
        parameter_block = self.runner.split("param(", maxsplit=1)[1].split(")\nSet-StrictMode", maxsplit=1)[0]
        source_parameter = parameter_block.split("[string]$ExpectedRouterSourceNetId", maxsplit=1)[0]
        source_attribute = source_parameter.rsplit("[Parameter", maxsplit=1)[1]
        self.assertNotIn("Mandatory = $true", source_attribute)
        self.assertIn(
            "if (-not [string]::IsNullOrWhiteSpace($ExpectedRouterSourceNetId))",
            self.runner,
        )
        self.assertNotIn(
            "$ExpectedTargetNetId -ceq $ExpectedRouterSourceNetId",
            self.runner,
        )
        doctor = function_body(self.ads_browse_proof, "Assert-NativeDoctorProof")
        self.assertIn(
            "$sourceNetId -ceq $targetNetId -and $sourcePort -eq $targetPort",
            doctor,
        )
        self.assertNotIn("$sourceNetId -ceq $ExpectedTargetNetId", doctor)

    @unittest.skipUnless(shutil.which("pwsh"), "pwsh is not installed on this Linux host")
    def test_powershell_doctor_compares_full_ams_addresses_and_observes_ports(self) -> None:
        command = rf"""
$ErrorActionPreference = 'Stop'
$WarningPreference = 'SilentlyContinue'
Import-Module '{ADS_BROWSE_PROOF}' -Force -DisableNameChecking
$netId = '10.20.30.40.1.1'
$sourcePort = 32768
$targetPort = 851
$steps = New-Object 'System.Collections.Generic.List[object]'
foreach ($id in @('udp_identify', 'local_identity', 'ams_target', 'read_state', 'symbol_upload', 'handle_resolve', 'sumup_read', 'symbol_version')) {{
    [void]$steps.Add([pscustomobject]@{{ id = $id; status = 'pass'; evidence = [pscustomobject]@{{}} }})
}}
$transportEvidence = [pscustomobject]@{{
    probe_transport = 'native_windows_router'; probe_operation = 'read_state'
    source_ams_address_available = $true; source_ams_net_id = $netId
    source_ams_port = $sourcePort; target_ams_net_id = $netId
    target_ams_port = $targetPort; source_target_addresses_distinct = $true
    source_target_comparison = 'full_ams_address'
}}
[void]$steps.Add([pscustomobject]@{{ id = 'tcp_48898'; status = 'pass'; evidence = $transportEvidence }})
$routeEvidence = [pscustomobject]@{{
    route_mode = 'native_local_no_self_route'; source_ams_net_id = $netId
    source_ams_port = $sourcePort; target_ams_net_id = $netId
    target_ams_port = $targetPort; source_target_addresses_distinct = $true
    source_target_comparison = 'full_ams_address'
}}
[void]$steps.Add([pscustomobject]@{{ id = 'route_present'; status = 'pass'; evidence = $routeEvidence }})
[void]$steps.Add([pscustomobject]@{{ id = 'notification'; status = 'pass'; evidence = [pscustomobject]@{{
    read_proven = $true; sample_method = 'subscribed_read_update'; subscription_id = 7
}} }})
[void]$steps.Add([pscustomobject]@{{ id = 'write_guarded'; status = 'skip'; skip_reason = 'writes_disabled'; evidence = [pscustomobject]@{{}} }})
$report = [pscustomobject]@{{
    writes_enabled = $false
    target = [pscustomobject]@{{ ams_net_id = $netId; ip = '127.0.0.1'; ams_port = $targetPort }}
    steps = @($steps.ToArray()); overall = 'pass'
}}
$candidate = [pscustomobject]@{{ ams_net_id = $netId; host = '127.0.0.1' }}
$proof = Assert-NativeDoctorProof -Report $report -Candidate $candidate `
    -ExpectedTargetNetId $netId -CommandEvidence ([pscustomobject]@{{}}) -ReportHash ('a' * 64)
if ($proof.source_ams_port -ne $sourcePort -or $proof.target_ams_port -ne $targetPort) {{ exit 1 }}
$expectedAccepted = Assert-NativeDoctorProof -Report $report -Candidate $candidate `
    -ExpectedTargetNetId $netId -ExpectedRouterSourceNetId $netId `
    -CommandEvidence ([pscustomobject]@{{}}) -ReportHash ('a' * 64)
if (-not $expectedAccepted.source_target_addresses_distinct) {{ exit 2 }}
$transportEvidence.source_ams_port = $targetPort
$transportEvidence.source_target_addresses_distinct = $false
$identicalAccepted = $false
try {{
    $null = Assert-NativeDoctorProof -Report $report -Candidate $candidate `
        -ExpectedTargetNetId $netId -CommandEvidence ([pscustomobject]@{{}}) -ReportHash ('a' * 64)
    $identicalAccepted = $true
}} catch {{}}
if ($identicalAccepted) {{ exit 3 }}
exit 0
"""
        completed = subprocess.run(
            ["pwsh", "-NoProfile", "-Command", command],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

    @unittest.skipUnless(shutil.which("pwsh"), "pwsh is not installed on this Linux host")
    def test_powershell_expected_target_rejects_wrong_or_duplicate_runtime(self) -> None:
        assert_net_id = function_body(self.ads_browse_proof, "Assert-AmsNetId")
        candidates = function_body(self.runner, "Get-ProvenNativeCandidates")
        command = rf"""
$ErrorActionPreference = 'Stop'
$WarningPreference = 'SilentlyContinue'
Import-Module '{MODULE}' -Force -DisableNameChecking
function Assert-AmsNetId {{{assert_net_id}}}
function Get-ProvenNativeCandidates {{{candidates}}}
function New-Candidate([string]$Id, [string]$NetId) {{
    [pscustomobject]@{{
        id = $Id
        label = $Id
        source = 'ads_local_router'
        confidence = 'observed'
        params = [pscustomobject]@{{
            name = 'TwinCAT'
            host = '127.0.0.1'
            ams_net_id = $NetId
            ams_port = 851
        }}
    }}
}}
$expected = '10.20.30.40.1.1' # synthetic; never use private laptop identity in tests
$right = New-Candidate 'right' $expected
$other = New-Candidate 'other' '10.20.30.99.1.1'
$selected = @(Get-ProvenNativeCandidates `
    -Discovery ([pscustomobject]@{{ candidates = @($other, $right) }}) `
    -ExpectedNetId $expected)
if ($selected.Count -ne 1 -or $selected[0].ams_net_id -cne $expected) {{ exit 1 }}

$wrongAccepted = $false
try {{
    $null = Get-ProvenNativeCandidates `
        -Discovery ([pscustomobject]@{{ candidates = @($other) }}) `
        -ExpectedNetId $expected
    $wrongAccepted = $true
}} catch {{}}
if ($wrongAccepted) {{ exit 2 }}

$duplicateAccepted = $false
try {{
    $null = Get-ProvenNativeCandidates `
        -Discovery ([pscustomobject]@{{ candidates = @($right, (New-Candidate 'duplicate' $expected)) }}) `
        -ExpectedNetId $expected
    $duplicateAccepted = $true
}} catch {{}}
if ($duplicateAccepted) {{ exit 3 }}
exit 0
"""
        completed = subprocess.run(
            ["pwsh", "-NoProfile", "-Command", command],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
