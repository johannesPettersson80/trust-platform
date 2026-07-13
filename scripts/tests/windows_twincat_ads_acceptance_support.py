from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
RUNNER = REPO_ROOT / "scripts" / "accept_windows_twincat_ads.ps1"
SIMULATOR_RUNNER = REPO_ROOT / "scripts" / "accept_windows_packaged_simulator.ps1"
MODULE = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "AcceptanceIo.psm1"
)
SIMULATOR_EXTENSION_TEST = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedSimulatorAcceptance.js"
)
SIMULATOR_VISUAL_PROOF = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedSimulatorVisualProof.js"
)
SIMULATOR_CANVAS_STATE = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedSimulatorCanvasState.js"
)
SIMULATOR_LIFECYCLE = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedSimulatorLifecycle.js"
)
SIMULATOR_LAUNCHER = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedSimulatorLauncher.psm1"
)
PACKAGED_EXTENSION_INSTALL = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedExtensionInstall.psm1"
)
SIMULATOR_CDP = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedSimulatorCdp.js"
)
RUNTIME_CONTROL_TOKEN = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "RuntimeControlToken.js"
)
ACCEPTANCE_REDACTION = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "AcceptanceRedaction.js"
)
PACKAGED_ADS_UI = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedAdsUiAcceptance.js"
)
PACKAGED_ADS_CUSTOM_PORTS = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedAdsCustomPorts.js"
)
PACKAGED_ADS_CUSTOM_PORT_ACCEPTANCE = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedAdsCustomPortAcceptance.js"
)
PACKAGED_ADS_CUSTOM_PORT_DOM = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedAdsCustomPortDom.js"
)
PACKAGED_ADS_DISCOVERY_SNAPSHOT = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedAdsDiscoverySnapshot.js"
)
PACKAGED_ADS_UI_CROSSCHECK = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedAdsUiCrosscheck.psm1"
)
PACKAGED_ADS_LIVE_VALUES = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedAdsLiveValuesAcceptance.js"
)
PACKAGED_ADS_IMPORT_PROOF = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedAdsImportProof.js"
)
PACKAGED_ADS_TOML_PROOF = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedAdsTomlProof.js"
)
PACKAGED_TOML_ASSIGNMENT = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedTomlAssignment.js"
)
PACKAGED_ADS_ROUTE_PROOF = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedAdsRouteProof.js"
)
PACKAGED_ADS_SNAPSHOT_PROOF = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedAdsSnapshotProof.js"
)
PACKAGED_ADS_GENERATED_PROOF = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedAdsGeneratedProof.js"
)
PACKAGED_ADS_LIVE_VALUES_DAP_PROOF = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedAdsLiveValuesDapProof.js"
)
PACKAGED_ADS_LIVE_VALUES_RENDER_PROOF = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedAdsLiveValuesRenderProof.js"
)
PACKAGED_ADS_BROWSE_SELECTION = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedAdsBrowseSelection.js"
)
PACKAGED_DAP_STATE = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedDapState.js"
)
PACKAGED_ADS_DAP_SNAPSHOT = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedAdsDapSnapshot.js"
)
PACKAGED_DAP_IO_STATE = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedDapIoState.js"
)
PACKAGED_BINARY_IDENTITY = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "PackagedBinaryIdentity.js"
)
ACCEPTANCE_PLAN = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "AcceptancePlan.psm1"
)
STATIC_ROUTE_PROOF = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "StaticRouteProof.psm1"
)
CANDIDATE_MANIFEST_PROOF = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "CandidateManifestProof.psm1"
)
CANDIDATE_PROVENANCE_PROOF = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "CandidateProvenanceProof.psm1"
)
ADS_BROWSE_PROOF = (
    REPO_ROOT
    / "scripts"
    / "windows_twincat_ads_acceptance"
    / "AdsBrowseProof.psm1"
)



class WindowsTwinCatAdsAcceptanceContractSupport:
    @classmethod
    def setUpClass(cls) -> None:
        cls.runner = RUNNER.read_text(encoding="utf-8")
        cls.simulator_runner = SIMULATOR_RUNNER.read_text(encoding="utf-8")
        cls.module = MODULE.read_text(encoding="utf-8")
        cls.simulator_extension_test = SIMULATOR_EXTENSION_TEST.read_text(
            encoding="utf-8"
        )
        cls.simulator_visual_proof = SIMULATOR_VISUAL_PROOF.read_text(
            encoding="utf-8"
        )
        cls.simulator_canvas_state = SIMULATOR_CANVAS_STATE.read_text(
            encoding="utf-8"
        )
        cls.simulator_lifecycle = SIMULATOR_LIFECYCLE.read_text(encoding="utf-8")
        cls.simulator_launcher = SIMULATOR_LAUNCHER.read_text(encoding="utf-8")
        cls.packaged_extension_install = PACKAGED_EXTENSION_INSTALL.read_text(
            encoding="utf-8"
        )
        cls.simulator_cdp = SIMULATOR_CDP.read_text(encoding="utf-8")
        cls.runtime_control_token = RUNTIME_CONTROL_TOKEN.read_text(encoding="utf-8")
        cls.acceptance_redaction = ACCEPTANCE_REDACTION.read_text(encoding="utf-8")
        cls.packaged_ads_ui = PACKAGED_ADS_UI.read_text(encoding="utf-8")
        cls.packaged_ads_custom_ports = PACKAGED_ADS_CUSTOM_PORTS.read_text(
            encoding="utf-8"
        )
        cls.packaged_ads_custom_port_acceptance = (
            PACKAGED_ADS_CUSTOM_PORT_ACCEPTANCE.read_text(encoding="utf-8")
        )
        cls.packaged_ads_custom_port_dom = PACKAGED_ADS_CUSTOM_PORT_DOM.read_text(
            encoding="utf-8"
        )
        cls.packaged_ads_discovery_snapshot = (
            PACKAGED_ADS_DISCOVERY_SNAPSHOT.read_text(encoding="utf-8")
        )
        cls.packaged_ads_ui_crosscheck = PACKAGED_ADS_UI_CROSSCHECK.read_text(
            encoding="utf-8"
        )
        cls.packaged_ads_live_values = PACKAGED_ADS_LIVE_VALUES.read_text(
            encoding="utf-8"
        )
        cls.packaged_ads_import_proof = PACKAGED_ADS_IMPORT_PROOF.read_text(
            encoding="utf-8"
        )
        cls.packaged_ads_toml_proof = PACKAGED_ADS_TOML_PROOF.read_text(
            encoding="utf-8"
        )
        cls.packaged_toml_assignment = PACKAGED_TOML_ASSIGNMENT.read_text(
            encoding="utf-8"
        )
        cls.packaged_ads_route_proof = PACKAGED_ADS_ROUTE_PROOF.read_text(
            encoding="utf-8"
        )
        cls.packaged_ads_snapshot_proof = PACKAGED_ADS_SNAPSHOT_PROOF.read_text(
            encoding="utf-8"
        )
        cls.packaged_ads_generated_proof = PACKAGED_ADS_GENERATED_PROOF.read_text(
            encoding="utf-8"
        )
        cls.packaged_ads_live_values_dap_proof = (
            PACKAGED_ADS_LIVE_VALUES_DAP_PROOF.read_text(encoding="utf-8")
        )
        cls.packaged_ads_live_values_render_proof = (
            PACKAGED_ADS_LIVE_VALUES_RENDER_PROOF.read_text(encoding="utf-8")
        )
        cls.packaged_ads_browse_selection = PACKAGED_ADS_BROWSE_SELECTION.read_text(
            encoding="utf-8"
        )
        cls.packaged_dap_state = PACKAGED_DAP_STATE.read_text(encoding="utf-8")
        cls.packaged_ads_dap_snapshot = PACKAGED_ADS_DAP_SNAPSHOT.read_text(
            encoding="utf-8"
        )
        cls.packaged_dap_io_state = PACKAGED_DAP_IO_STATE.read_text(
            encoding="utf-8"
        )
        cls.packaged_binary_identity = PACKAGED_BINARY_IDENTITY.read_text(
            encoding="utf-8"
        )
        cls.acceptance_plan = ACCEPTANCE_PLAN.read_text(encoding="utf-8")
        cls.static_route_proof = STATIC_ROUTE_PROOF.read_text(encoding="utf-8")
        cls.candidate_manifest_proof = CANDIDATE_MANIFEST_PROOF.read_text(
            encoding="utf-8"
        )
        cls.candidate_provenance_proof = CANDIDATE_PROVENANCE_PROOF.read_text(
            encoding="utf-8"
        )
        cls.ads_browse_proof = ADS_BROWSE_PROOF.read_text(encoding="utf-8")

    def _run_powershell_json(self, command: str) -> object:
        completed = subprocess.run(
            ["pwsh", "-NoProfile", "-Command", command],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        output = completed.stdout.strip().splitlines()
        self.assertTrue(output, "PowerShell regression probe emitted no JSON")
        try:
            return json.loads(output[-1])
        except json.JSONDecodeError as error:
            self.fail(
                f"PowerShell regression probe emitted invalid JSON: {error}\n"
                f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
            )



def function_body(source: str, name: str) -> str:
    match = re.search(
        rf"(?ms)^function\s+{re.escape(name)}\s*\{{(.*?)(?=^function\s+|^Export-ModuleMember|^if \(\$env:OS|\Z)",
        source,
    )
    if not match:
        raise AssertionError(f"PowerShell function {name} was not found")
    body_with_closing_brace = match.group(1).rstrip()
    if not body_with_closing_brace.endswith("}"):
        raise AssertionError(f"PowerShell function {name} had no closing brace")
    return body_with_closing_brace[:-1]
