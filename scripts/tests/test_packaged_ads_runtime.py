from __future__ import annotations

import json
import struct
import sys
import tempfile
import unittest
import zipfile
from contextlib import redirect_stderr
from io import StringIO
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT))

from scripts import test_packaged_ads_runtime as gate  # noqa: E402


class AdsUdpProtocolTests(unittest.TestCase):
    def test_identify_encoder_round_trips_expected_ads_fields(self) -> None:
        packet = gate.build_identify_response(
            net_id=gate.CI_AMS_NET_ID_BYTES,
            hostname="CI-TWINCAT",
            twincat_version=(3, 1, 4026),
        )

        message = gate.parse_udp_message(packet)

        self.assertEqual(message.magic, gate.ADS_UDP_MAGIC)
        self.assertEqual(message.service, gate.ADS_IDENTIFY_REPLY_SERVICE)
        self.assertEqual(message.net_id, gate.CI_AMS_NET_ID_BYTES)
        self.assertEqual(message.item_count, 4)
        self.assertEqual(message.tags[gate.TAG_STATUS], struct.pack("<I", 0))
        self.assertEqual(message.tags[gate.TAG_NET_ID], gate.CI_AMS_NET_ID_BYTES)
        self.assertEqual(message.tags[gate.TAG_COMPUTER_NAME], b"CI-TWINCAT\0")
        self.assertEqual(
            message.tags[gate.TAG_TWINCAT_VERSION],
            bytes((3, 1, 0xBA, 0x0F)),
        )

    def test_identify_request_parser_rejects_wrong_magic(self) -> None:
        request = bytearray(gate.build_identify_request())
        request[0:4] = struct.pack("<I", 0xDEADBEEF)

        with self.assertRaisesRegex(gate.GateError, "magic"):
            gate.assert_identify_request(bytes(request))


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

    def test_directed_discovery_requires_ads_identify_source(self) -> None:
        candidate = gate.assert_discovery_candidate(
            discovery_payload(source="ads_identify", ams_port=851),
            expected_source="ads_identify",
            expected_port=851,
        )

        self.assertEqual(candidate["params"]["ams_net_id"], gate.CI_AMS_NET_ID)

    def test_same_computer_router_identity_requires_one_observed_candidate_and_close(self) -> None:
        payload = discovery_payload(source="ads_local_router", ams_port=gate.CI_ADS_PORT)
        payload["candidates"][0]["confidence"] = "observed"

        ams_net_id, assigned_source_port = gate.assert_local_router_identity_transcript(
            gate.AMS_ROUTER_OPEN_REQUEST,
            gate.AMS_ROUTER_OPEN_REPLY,
            gate.AMS_ROUTER_CLOSE_REQUEST,
        )
        candidate = gate.assert_same_computer_router_candidate(
            payload,
            expected_net_id=ams_net_id,
            expected_port=gate.CI_ADS_PORT,
        )

        self.assertEqual(ams_net_id, gate.CI_AMS_NET_ID)
        self.assertEqual(assigned_source_port, 58_913)
        self.assertEqual(candidate["confidence"], "observed")

        duplicate = discovery_payload(source="ads_local_router", ams_port=gate.CI_ADS_PORT)
        duplicate["candidates"][0]["confidence"] = "observed"
        duplicate["candidates"].append(dict(duplicate["candidates"][0]))
        with self.assertRaisesRegex(gate.GateError, "exactly one"):
            gate.assert_same_computer_router_candidate(
                duplicate,
                expected_net_id=gate.CI_AMS_NET_ID,
                expected_port=gate.CI_ADS_PORT,
            )

        wrong_source = discovery_payload(
            source="ads_identify", ams_port=gate.CI_ADS_PORT
        )
        wrong_source["candidates"][0]["confidence"] = "observed"
        with self.assertRaisesRegex(gate.GateError, "ads_local_router"):
            gate.assert_same_computer_router_candidate(
                wrong_source,
                expected_net_id=gate.CI_AMS_NET_ID,
                expected_port=gate.CI_ADS_PORT,
            )

        with self.assertRaisesRegex(gate.GateError, "close"):
            gate.assert_local_router_identity_transcript(
                gate.AMS_ROUTER_OPEN_REQUEST,
                gate.AMS_ROUTER_OPEN_REPLY,
                bytes(8),
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

    def test_source_request_transcript_requires_router_open_and_ams_frame(self) -> None:
        first_ams_frame = (
            b"\x00\x00"
            + struct.pack("<I", 32)
            + gate.CI_AMS_NET_ID_BYTES
            + struct.pack("<H", gate.CI_MANUAL_ADS_PORT)
            + bytes(24)
        )

        self.assertEqual(
            gate.assert_source_request_transcript(
                gate.AMS_ROUTER_OPEN_REQUEST,
                first_ams_frame,
                gate.CI_MANUAL_ADS_PORT,
            ),
            gate.CI_MANUAL_ADS_PORT,
        )

        with self.assertRaisesRegex(gate.GateError, "open-port"):
            gate.assert_source_request_transcript(
                b"\x00" * 8,
                first_ams_frame,
                gate.CI_MANUAL_ADS_PORT,
            )
        with self.assertRaisesRegex(gate.GateError, "AMS/TCP"):
            gate.assert_source_request_transcript(
                gate.AMS_ROUTER_OPEN_REQUEST,
                b"\x01\x00" + struct.pack("<I", 32),
                gate.CI_MANUAL_ADS_PORT,
            )
        wrong_port_frame = bytearray(first_ams_frame)
        struct.pack_into("<H", wrong_port_frame, 12, gate.CI_ADS_PORT)
        with self.assertRaisesRegex(gate.GateError, "expected 852"):
            gate.assert_source_request_transcript(
                gate.AMS_ROUTER_OPEN_REQUEST,
                bytes(wrong_port_frame),
                gate.CI_MANUAL_ADS_PORT,
            )

    def test_router_probe_close_requires_the_assigned_source_port(self) -> None:
        gate.assert_router_probe_close(gate.AMS_ROUTER_CLOSE_REQUEST)

        with self.assertRaisesRegex(gate.GateError, "temporary source port"):
            gate.assert_router_probe_close(bytes(8))

    def test_direct_fallback_requires_a_held_probe_and_bounded_ams_frame(self) -> None:
        first_ams_frame = (
            b"\x00\x00"
            + struct.pack("<I", 32)
            + gate.CI_AMS_NET_ID_BYTES
            + struct.pack("<H", gate.CI_MANUAL_ADS_PORT)
            + bytes(24)
        )

        self.assertEqual(
            gate.assert_direct_fallback_transcript(
                gate.AMS_ROUTER_OPEN_REQUEST,
                first_ams_frame,
                gate.CI_MANUAL_ADS_PORT,
                probe_hold_seconds=2.0,
                fallback_frame_elapsed_seconds=0.51,
            ),
            gate.CI_MANUAL_ADS_PORT,
        )
        with self.assertRaisesRegex(gate.GateError, "too soon"):
            gate.assert_direct_fallback_transcript(
                gate.AMS_ROUTER_OPEN_REQUEST,
                first_ams_frame,
                gate.CI_MANUAL_ADS_PORT,
                probe_hold_seconds=0.5,
                fallback_frame_elapsed_seconds=0.51,
            )
        with self.assertRaisesRegex(gate.GateError, "within 1.5s"):
            gate.assert_direct_fallback_transcript(
                gate.AMS_ROUTER_OPEN_REQUEST,
                first_ams_frame,
                gate.CI_MANUAL_ADS_PORT,
                probe_hold_seconds=2.0,
                fallback_frame_elapsed_seconds=1.5,
            )
        with self.assertRaisesRegex(gate.GateError, "direct loopback fallback"):
            gate.assert_direct_fallback_transcript(
                gate.AMS_ROUTER_OPEN_REQUEST,
                gate.AMS_ROUTER_OPEN_REQUEST,
                gate.CI_MANUAL_ADS_PORT,
                probe_hold_seconds=2.0,
                fallback_frame_elapsed_seconds=0.51,
            )


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

            package = gate.read_windows_vsix(vsix_path)

        self.assertEqual(package.version, "1.2.3")
        self.assertEqual(package.runtime_bytes, b"MZ-ci-runtime")
        self.assertEqual(package.debug_bytes, b"MZ-ci-debug")
        self.assertEqual(package.target_platform, "win32-x64")

    def test_packaged_debug_adapter_must_match_the_executed_release_binary(self) -> None:
        package = gate.WindowsVsix(
            version="1.2.3",
            target_platform="win32-x64",
            runtime_bytes=b"MZ-ci-runtime",
            debug_bytes=b"MZ-ci-debug",
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
