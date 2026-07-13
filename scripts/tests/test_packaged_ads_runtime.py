from __future__ import annotations

import json
import sys
import tempfile
import unittest
import zipfile
from contextlib import redirect_stderr
from io import StringIO
from pathlib import Path
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT))

from scripts import test_packaged_ads_runtime as gate  # noqa: E402


class PhaseAssertionTests(unittest.TestCase):
    def test_manual_discovery_requires_manual_source_and_selected_port(self) -> None:
        payload = discovery_payload(source="manual", ams_port=852)

        candidate = gate.assert_discovery_candidate(
            payload,
            expected_source="manual",
            expected_port=852,
        )

        self.assertEqual(candidate["params"]["host"], gate.CI_HOST)

        with self.assertRaisesRegex(gate.GateError, "852"):
            gate.assert_discovery_candidate(
                discovery_payload(source="manual", ams_port=851),
                expected_source="manual",
                expected_port=852,
            )

    def test_host_port_rejection_must_be_actionable(self) -> None:
        result = gate.CommandResult(
            argv=("trust-runtime.exe", "comm", "discover"),
            returncode=1,
            stdout="",
            stderr=(
                "ADS discovery host must be a host or IP only; "
                "use the separate ADS port field."
            ),
        )

        gate.assert_actionable_host_port_rejection(result)

        with self.assertRaisesRegex(gate.GateError, "actionable"):
            gate.assert_actionable_host_port_rejection(
                gate.CommandResult(
                    argv=result.argv,
                    returncode=1,
                    stdout="",
                    stderr="ADS discovery failed",
                )
            )

    def test_native_same_computer_result_requires_a_structured_unavailable_error(self) -> None:
        result = gate.CommandResult(
            argv=("trust-runtime.exe", "comm", "browse-symbols"),
            returncode=0,
            stdout=json.dumps(
                {
                    "schema_version": 1,
                    "protocol": "ads",
                    "kind": "symbols",
                    "tree": [],
                    "error": {
                        "code": "symbol_upload_failed",
                        "message": "TcAdsDll.dll was not found",
                    },
                }
            ),
            stderr="",
        )

        payload = gate.assert_native_same_computer_result(result)
        self.assertEqual(payload["error"]["code"], "symbol_upload_failed")

        with self.assertRaisesRegex(gate.GateError, "unexpected error code"):
            gate.assert_native_same_computer_result(
                gate.CommandResult(
                    argv=result.argv,
                    returncode=0,
                    stdout=json.dumps(
                        {
                            "error": {
                                "code": "missing_route",
                                "message": "wrong same-computer recovery",
                            }
                        }
                    ),
                    stderr="",
                )
            )

        with self.assertRaisesRegex(gate.GateError, "native Windows backend"):
            gate.assert_native_same_computer_result(
                gate.CommandResult(
                    argv=result.argv,
                    returncode=0,
                    stdout=json.dumps(
                        {
                            "error": {
                                "code": "ads_port_unavailable",
                                "message": "raw TCP 48898 connection refused",
                            }
                        }
                    ),
                    stderr="",
                )
            )


class GateCompositionTests(unittest.TestCase):
    def test_packaged_gate_runs_only_the_four_native_windows_phases(self) -> None:
        package = gate.WindowsVsix(
            version="1.2.3",
            target_platform="win32-x64",
            runtime_bytes=b"MZ-ci-runtime",
            debug_bytes=b"MZ-ci-debug",
            lsp_bytes=b"MZ-ci-lsp",
        )
        evidence: dict[str, object] = {"phases": []}
        with tempfile.TemporaryDirectory() as temp_dir:
            vsix_path = Path(temp_dir) / "trust-lsp-win32-x64.vsix"
            vsix_path.write_bytes(b"ci-vsix")
            staged_debug = Path(temp_dir) / "trust-debug.exe"
            staged_debug.write_bytes(package.debug_bytes)
            with (
                patch.object(gate, "read_windows_vsix", return_value=package),
                patch.object(
                    gate,
                    "assert_packaged_debug_matches_staged",
                    return_value={"byte_identical_to_staged_release": True},
                ),
                patch.object(gate, "_runtime_version_phase", return_value={}),
                patch.object(gate, "_manual_discovery_phase", return_value={}),
                patch.object(gate, "_host_port_rejection_phase", return_value={}),
                patch.object(gate, "_native_same_computer_phase", return_value={}),
            ):
                gate.run_packaged_gate(vsix_path, staged_debug, evidence)

        phases = evidence["phases"]
        self.assertIsInstance(phases, list)
        self.assertEqual(
            [phase["name"] for phase in phases],
            [
                "runtime_version",
                "manual_discovery_port_852",
                "host_port_rejection",
                "native_same_computer_no_raw_tcp",
            ],
        )
        self.assertTrue(all(phase["status"] == "pass" for phase in phases))


class ArtifactAndEncodingTests(unittest.TestCase):
    def test_decode_output_replaces_invalid_utf8(self) -> None:
        self.assertEqual(gate.decode_output(b"ADS \xff failure"), "ADS \ufffd failure")

    def test_evidence_writer_uses_ascii_safe_json(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "evidence.json"
            gate.write_evidence(path, {"detail": "r\u00e4ksm\u00f6rg\u00e5s"})

            text = path.read_text(encoding="utf-8")

        self.assertIn(r"r\u00e4ksm\u00f6rg\u00e5s", text)
        self.assertNotIn("r\u00e4ksm\u00f6rg\u00e5s", text)

    def test_vsix_reader_requires_win32_x64_and_exact_runtime_member(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            vsix_path = Path(temp_dir) / "trust-lsp-win32-x64.vsix"
            with zipfile.ZipFile(vsix_path, "w") as archive:
                archive.writestr(
                    "extension.vsixmanifest",
                    manifest_xml(target_platform="win32-x64"),
                )
                archive.writestr(
                    "extension/package.json",
                    json.dumps({"name": "trust-lsp", "version": "1.2.3"}),
                )
                archive.writestr("extension/bin/trust-runtime.exe", b"MZ-ci-runtime")
                archive.writestr("extension/bin/trust-debug.exe", b"MZ-ci-debug")
                archive.writestr("extension/bin/trust-lsp.exe", b"MZ-ci-lsp")

            package = gate.read_windows_vsix(vsix_path)

        self.assertEqual(package.version, "1.2.3")
        self.assertEqual(package.runtime_bytes, b"MZ-ci-runtime")
        self.assertEqual(package.debug_bytes, b"MZ-ci-debug")
        self.assertEqual(package.lsp_bytes, b"MZ-ci-lsp")
        self.assertEqual(package.target_platform, "win32-x64")

    def test_packaged_debug_adapter_must_match_the_executed_release_binary(self) -> None:
        package = gate.WindowsVsix(
            version="1.2.3",
            target_platform="win32-x64",
            runtime_bytes=b"MZ-ci-runtime",
            debug_bytes=b"MZ-ci-debug",
            lsp_bytes=b"MZ-ci-lsp",
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            staged = Path(temp_dir) / "trust-debug.exe"
            staged.write_bytes(b"MZ-ci-debug")
            evidence = gate.assert_packaged_debug_matches_staged(package, staged)

            self.assertTrue(evidence["byte_identical_to_staged_release"])
            self.assertEqual(evidence["packaged_sha256"], evidence["staged_sha256"])

            staged.write_bytes(b"MZ-different-debug")
            with self.assertRaisesRegex(gate.GateError, "not byte-identical"):
                gate.assert_packaged_debug_matches_staged(package, staged)

    def test_main_always_writes_failure_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            invalid_vsix = Path(temp_dir) / "invalid.vsix"
            invalid_vsix.write_bytes(b"not a zip file")
            staged_debug = Path(temp_dir) / "trust-debug.exe"
            staged_debug.write_bytes(b"MZ-ci-debug")
            evidence_path = Path(temp_dir) / "evidence" / "result.json"

            with redirect_stderr(StringIO()):
                exit_code = gate.main(
                    [
                        "--vsix",
                        str(invalid_vsix),
                        "--staged-debug",
                        str(staged_debug),
                        "--evidence",
                        str(evidence_path),
                    ]
                )
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))

        self.assertEqual(exit_code, 1)
        self.assertEqual(evidence["status"], "fail")
        self.assertIn("failed to read Windows VSIX", evidence["error"])


def discovery_payload(*, source: str, ams_port: int) -> dict[str, object]:
    return {
        "schema_version": 1,
        "protocol": "ads",
        "candidates": [
            {
                "id": f"ads:{gate.CI_AMS_NET_ID}",
                "source": source,
                "params": {
                    "host": gate.CI_HOST,
                    "ams_net_id": gate.CI_AMS_NET_ID,
                    "ams_port": ams_port,
                },
            }
        ],
    }


def manifest_xml(*, target_platform: str) -> str:
    return f"""<?xml version="1.0" encoding="utf-8"?>
<PackageManifest xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011">
  <Metadata>
    <Identity Id="trust-lsp" Version="1.2.3" Publisher="trust-platform"
              TargetPlatform="{target_platform}" />
  </Metadata>
</PackageManifest>
"""


if __name__ == "__main__":
    unittest.main()
