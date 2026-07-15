#!/usr/bin/env python3
"""Prove ADS discovery, local-router identity, and source behavior from a Windows VSIX.

The gate intentionally owns only deterministic loopback protocol responders. Real
TwinCAT symbol browsing remains a separate device-in-the-loop acceptance proof.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import zipfile
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, TypeVar
from xml.etree import ElementTree


ADS_UDP_MAGIC = 0x7114_6603
ADS_IDENTIFY_SERVICE = 1
ADS_IDENTIFY_REPLY_SERVICE = ADS_IDENTIFY_SERVICE | 0x8000_0000
ADS_UDP_HEADER_SIZE = 24
ADS_UDP_PORT = 48_899
ADS_TCP_PORT = 48_898

TAG_STATUS = 1
TAG_TWINCAT_VERSION = 3
TAG_COMPUTER_NAME = 5
TAG_NET_ID = 7

CI_HOST = "127.0.0.1"
CI_DIRECT_HOST = "127.0.0.2"
CI_AMS_NET_ID = "127.0.0.1.1.1"
CI_AMS_NET_ID_BYTES = bytes((127, 0, 0, 1, 1, 1))
CI_ADS_PORT = 851
CI_MANUAL_ADS_PORT = 852

AMS_ROUTER_OPEN_REQUEST = bytes((0x00, 0x10, 0x02, 0, 0, 0, 0, 0))
AMS_ROUTER_OPEN_REPLY = bytes(
    (0x00, 0x10, 0x08, 0, 0, 0, 127, 0, 0, 1, 1, 1, 0x21, 0xE6)
)
AMS_ROUTER_CLOSE_REQUEST = bytes((0x01, 0x00, 0x02, 0, 0, 0, 0x21, 0xE6))

COMMAND_TIMEOUT_SECONDS = 20.0
RESPONDER_TIMEOUT_SECONDS = 10.0
DIRECT_PROBE_HOLD_SECONDS = 2.0
DIRECT_FALLBACK_MAX_SECONDS = 1.5
MAX_AMS_FRAME_BYTES = 1024 * 1024
WINDOWS_RUNTIME_MEMBER = "extension/bin/trust-runtime.exe"
WINDOWS_DEBUG_MEMBER = "extension/bin/trust-debug.exe"
PACKAGE_JSON_MEMBER = "extension/package.json"
VSIX_MANIFEST_MEMBER = "extension.vsixmanifest"


class GateError(RuntimeError):
    """A deterministic packaged-runtime proof failed."""


@dataclass(frozen=True)
class UdpMessage:
    magic: int
    reserved: int
    service: int
    net_id: bytes
    source_port: int
    item_count: int
    tags: dict[int, bytes]


@dataclass(frozen=True)
class WindowsVsix:
    version: str
    target_platform: str
    runtime_bytes: bytes
    debug_bytes: bytes


@dataclass(frozen=True)
class CommandResult:
    argv: tuple[str, ...]
    returncode: int
    stdout: str
    stderr: str
    timed_out: bool = False

    def evidence(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True)
class UdpTranscript:
    request_hex: str
    peer: str


@dataclass(frozen=True)
class RouterTranscript:
    probe_open_request_hex: str
    probe_close_request_hex: str
    open_request_hex: str
    first_ams_frame_hex: str
    target_ads_port: int
    router_open_count: int


@dataclass(frozen=True)
class LocalRouterIdentityTranscript:
    open_request_hex: str
    open_reply_hex: str
    close_request_hex: str
    ams_net_id: str
    assigned_source_port: int


@dataclass(frozen=True)
class DirectFallbackTranscript:
    probe_open_request_hex: str
    first_ams_frame_hex: str
    target_ads_port: int
    probe_hold_seconds: float
    fallback_frame_elapsed_seconds: float


T = TypeVar("T")


def build_identify_request() -> bytes:
    return struct.pack(
        "<III6sHI",
        ADS_UDP_MAGIC,
        0,
        ADS_IDENTIFY_SERVICE,
        bytes(6),
        0,
        0,
    )


def build_identify_response(
    *,
    net_id: bytes = CI_AMS_NET_ID_BYTES,
    hostname: str = "CI-TWINCAT",
    twincat_version: tuple[int, int, int] = (3, 1, 4026),
) -> bytes:
    if len(net_id) != 6:
        raise GateError("ADS AMS Net ID must contain exactly six bytes")
    major, minor, build = twincat_version
    if not (0 <= major <= 255 and 0 <= minor <= 255 and 0 <= build <= 65_535):
        raise GateError("TwinCAT version components are outside their wire ranges")
    hostname_bytes = hostname.encode("utf-8") + b"\0"
    version_bytes = bytes((major, minor)) + struct.pack("<H", build)
    tags = b"".join(
        (
            _encode_tag(TAG_STATUS, struct.pack("<I", 0)),
            _encode_tag(TAG_NET_ID, net_id),
            _encode_tag(TAG_COMPUTER_NAME, hostname_bytes),
            _encode_tag(TAG_TWINCAT_VERSION, version_bytes),
        )
    )
    header = struct.pack(
        "<III6sHI",
        ADS_UDP_MAGIC,
        0,
        ADS_IDENTIFY_REPLY_SERVICE,
        net_id,
        0,
        4,
    )
    return header + tags


def parse_udp_message(packet: bytes) -> UdpMessage:
    if len(packet) < ADS_UDP_HEADER_SIZE:
        raise GateError(
            f"ADS UDP message is {len(packet)} bytes, expected at least {ADS_UDP_HEADER_SIZE}"
        )
    magic, reserved, service, net_id, source_port, item_count = struct.unpack_from(
        "<III6sHI", packet, 0
    )
    offset = ADS_UDP_HEADER_SIZE
    tags: dict[int, bytes] = {}
    for item_index in range(item_count):
        if offset + 4 > len(packet):
            raise GateError(f"ADS UDP tag {item_index} is missing its header")
        tag, length = struct.unpack_from("<HH", packet, offset)
        offset += 4
        end = offset + length
        if end > len(packet):
            raise GateError(
                f"ADS UDP tag {tag} declares {length} bytes beyond the packet"
            )
        tags[tag] = packet[offset:end]
        offset = end
    if offset != len(packet):
        raise GateError(f"ADS UDP message has {len(packet) - offset} trailing bytes")
    return UdpMessage(
        magic=magic,
        reserved=reserved,
        service=service,
        net_id=net_id,
        source_port=source_port,
        item_count=item_count,
        tags=tags,
    )


def assert_identify_request(packet: bytes) -> None:
    message = parse_udp_message(packet)
    if message.magic != ADS_UDP_MAGIC:
        raise GateError(
            f"ADS Identify request magic was 0x{message.magic:08x}, "
            f"expected 0x{ADS_UDP_MAGIC:08x}"
        )
    if message.service != ADS_IDENTIFY_SERVICE:
        raise GateError(
            f"ADS UDP service was 0x{message.service:08x}, expected Identify"
        )
    if message.item_count != 0:
        raise GateError("ADS Identify request unexpectedly carried tagged items")


def assert_discovery_candidate(
    payload: dict[str, Any],
    *,
    expected_source: str,
    expected_port: int,
) -> dict[str, Any]:
    candidates = payload.get("candidates")
    if not isinstance(candidates, list):
        raise GateError("ADS discovery JSON did not contain a candidates array")
    for candidate in candidates:
        if not isinstance(candidate, dict):
            continue
        params = candidate.get("params")
        if not isinstance(params, dict):
            continue
        if (
            params.get("host") == CI_HOST
            and params.get("ams_net_id") == CI_AMS_NET_ID
            and params.get("ams_port") == expected_port
            and candidate.get("source") == expected_source
        ):
            return candidate
    raise GateError(
        "ADS discovery did not return "
        f"{expected_source} candidate {CI_AMS_NET_ID}@{CI_HOST}:{expected_port}"
    )


def assert_same_computer_router_candidate(
    payload: dict[str, Any],
    *,
    expected_net_id: str,
    expected_port: int,
) -> dict[str, Any]:
    candidates = payload.get("candidates")
    if not isinstance(candidates, list) or len(candidates) != 1:
        count = len(candidates) if isinstance(candidates, list) else "missing"
        raise GateError(
            "same-computer router identity must return exactly one candidate; "
            f"got {count}"
        )
    candidate = candidates[0]
    if not isinstance(candidate, dict):
        raise GateError("same-computer router identity candidate was not an object")
    params = candidate.get("params")
    if not isinstance(params, dict):
        raise GateError("same-computer router identity candidate had no params object")
    if (
        params.get("host") != CI_HOST
        or params.get("ams_net_id") != expected_net_id
        or params.get("ams_port") != expected_port
    ):
        raise GateError(
            "same-computer router identity did not preserve "
            f"{expected_net_id}@{CI_HOST}:{expected_port}"
        )
    if candidate.get("confidence") != "observed":
        raise GateError(
            "same-computer router identity must be marked observed, not manually declared"
        )
    if candidate.get("source") != "ads_local_router":
        raise GateError(
            "same-computer router identity must report source ads_local_router; "
            f"got {candidate.get('source')!r}"
        )
    return candidate


def assert_actionable_host_port_rejection(result: CommandResult) -> None:
    if result.returncode == 0:
        raise GateError("ADS host:port input unexpectedly succeeded")
    detail = f"{result.stdout}\n{result.stderr}".lower()
    if "host or ip only" not in detail or "ads port" not in detail:
        raise GateError(
            "ADS host:port rejection was not actionable; expected both "
            "'host or IP only' and separate 'ADS port' guidance"
        )


def assert_source_request_transcript(
    open_request: bytes,
    first_ams_frame: bytes,
    expected_target_port: int,
) -> int:
    if open_request != AMS_ROUTER_OPEN_REQUEST:
        raise GateError(
            "loopback ADS client did not send the AMS router open-port request; "
            f"got {open_request.hex()}"
        )
    return _assert_ams_tcp_target(
        first_ams_frame,
        expected_target_port,
        context="router registration was not followed by",
    )


def assert_direct_fallback_transcript(
    probe_open_request: bytes,
    first_ams_frame: bytes,
    expected_target_port: int,
    *,
    probe_hold_seconds: float,
    fallback_frame_elapsed_seconds: float,
) -> int:
    if probe_open_request != AMS_ROUTER_OPEN_REQUEST:
        raise GateError(
            "direct loopback proof did not receive the bounded AMS router probe; "
            f"got {probe_open_request.hex()}"
        )
    if probe_hold_seconds <= 0.5:
        raise GateError(
            "direct loopback server released the router-probe connection too soon; "
            f"held it for {probe_hold_seconds:.3f}s"
        )
    if fallback_frame_elapsed_seconds >= DIRECT_FALLBACK_MAX_SECONDS:
        raise GateError(
            "packaged runtime did not open the direct AMS/TCP fallback within "
            f"{DIRECT_FALLBACK_MAX_SECONDS:.1f}s; first frame arrived after "
            f"{fallback_frame_elapsed_seconds:.3f}s"
        )
    return _assert_ams_tcp_target(
        first_ams_frame,
        expected_target_port,
        context="direct loopback fallback did not begin with",
    )


def _assert_ams_tcp_target(
    first_ams_frame: bytes,
    expected_target_port: int,
    *,
    context: str,
) -> int:
    if len(first_ams_frame) < 14 or first_ams_frame[:2] != b"\0\0":
        raise GateError(f"{context} an AMS/TCP frame")
    frame_length = struct.unpack_from("<I", first_ams_frame, 2)[0]
    if frame_length < 32 or frame_length > MAX_AMS_FRAME_BYTES:
        raise GateError(
            f"AMS/TCP frame declared invalid length {frame_length} after router registration"
        )
    if len(first_ams_frame) != 6 + frame_length:
        raise GateError(
            "AMS/TCP frame byte count did not match its declared payload length"
        )
    target_ads_port = struct.unpack_from("<H", first_ams_frame, 12)[0]
    if target_ads_port != expected_target_port:
        raise GateError(
            "ADS browse targeted logical service "
            f"{target_ads_port}, expected {expected_target_port}"
        )
    return target_ads_port


def assert_router_probe_close(close_request: bytes) -> None:
    if close_request != AMS_ROUTER_CLOSE_REQUEST:
        raise GateError(
            "bounded AMS router detection did not close its temporary source port; "
            f"got {close_request.hex()}"
        )


def assert_local_router_identity_transcript(
    open_request: bytes,
    open_reply: bytes,
    close_request: bytes,
) -> tuple[str, int]:
    if open_request != AMS_ROUTER_OPEN_REQUEST:
        raise GateError(
            "same-computer discovery did not request an identity from the local AMS router; "
            f"got {open_request.hex()}"
        )
    if (
        len(open_reply) != 14
        or open_reply[:6] != bytes((0x00, 0x10, 0x08, 0, 0, 0))
        or open_reply[6:12] == bytes(6)
    ):
        raise GateError("local AMS router identity reply was malformed")
    expected_close = bytes((0x01, 0x00, 0x02, 0, 0, 0, open_reply[12], open_reply[13]))
    if close_request != expected_close:
        raise GateError(
            "same-computer discovery did not close the router-assigned identity port; "
            f"got {close_request.hex()}, expected {expected_close.hex()}"
        )
    ams_net_id = ".".join(str(byte) for byte in open_reply[6:12])
    assigned_source_port = struct.unpack_from("<H", open_reply, 12)[0]
    return ams_net_id, assigned_source_port


def decode_output(output: bytes | str | None) -> str:
    if output is None:
        return ""
    if isinstance(output, str):
        return output
    return output.decode("utf-8", errors="replace")


def read_windows_vsix(path: Path) -> WindowsVsix:
    try:
        with zipfile.ZipFile(path) as archive:
            names = set(archive.namelist())
            required = {
                VSIX_MANIFEST_MEMBER,
                PACKAGE_JSON_MEMBER,
                WINDOWS_RUNTIME_MEMBER,
                WINDOWS_DEBUG_MEMBER,
            }
            missing = sorted(required - names)
            if missing:
                raise GateError(
                    "VSIX is missing required packaged member(s): " + ", ".join(missing)
                )
            manifest = archive.read(VSIX_MANIFEST_MEMBER)
            package_json = archive.read(PACKAGE_JSON_MEMBER)
            runtime_bytes = archive.read(WINDOWS_RUNTIME_MEMBER)
            debug_bytes = archive.read(WINDOWS_DEBUG_MEMBER)
    except (OSError, zipfile.BadZipFile, KeyError) as error:
        raise GateError(f"failed to read Windows VSIX '{path}': {error}") from error

    try:
        root = ElementTree.fromstring(manifest)
    except ElementTree.ParseError as error:
        raise GateError(f"VSIX manifest is invalid XML: {error}") from error
    identity = next(
        (
            element
            for element in root.iter()
            if element.tag.rsplit("}", 1)[-1] == "Identity"
        ),
        None,
    )
    if identity is None:
        raise GateError("VSIX manifest does not contain an Identity element")
    target_platform = identity.attrib.get("TargetPlatform", "")
    if target_platform != "win32-x64":
        raise GateError(
            f"VSIX target platform was '{target_platform}', expected 'win32-x64'"
        )

    try:
        package = json.loads(decode_output(package_json))
    except json.JSONDecodeError as error:
        raise GateError(
            f"VSIX extension/package.json is invalid JSON: {error}"
        ) from error
    version = package.get("version") if isinstance(package, dict) else None
    if not isinstance(version, str) or not version.strip():
        raise GateError("VSIX extension/package.json is missing a non-empty version")
    if not runtime_bytes.startswith(b"MZ"):
        raise GateError(
            "packaged extension/bin/trust-runtime.exe is not a Windows PE file"
        )
    if not debug_bytes.startswith(b"MZ"):
        raise GateError(
            "packaged extension/bin/trust-debug.exe is not a Windows PE file"
        )
    return WindowsVsix(
        version=version.strip(),
        target_platform=target_platform,
        runtime_bytes=runtime_bytes,
        debug_bytes=debug_bytes,
    )


def assert_packaged_debug_matches_staged(
    package: WindowsVsix, staged_debug_path: Path
) -> dict[str, Any]:
    try:
        staged_bytes = staged_debug_path.read_bytes()
    except OSError as error:
        raise GateError(
            f"failed to read staged Windows debug adapter '{staged_debug_path}': {error}"
        ) from error
    if not staged_bytes.startswith(b"MZ"):
        raise GateError("staged trust-debug.exe is not a Windows PE file")
    if package.debug_bytes != staged_bytes:
        raise GateError(
            "packaged extension/bin/trust-debug.exe is not byte-identical to the "
            "staged release debug adapter"
        )
    digest = hashlib.sha256(staged_bytes).hexdigest()
    return {
        "member": WINDOWS_DEBUG_MEMBER,
        "packaged_sha256": digest,
        "staged_path": str(staged_debug_path),
        "staged_sha256": digest,
        "size_bytes": len(staged_bytes),
        "byte_identical_to_staged_release": True,
    }


def write_evidence(path: Path, evidence: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(evidence, indent=2, sort_keys=True, ensure_ascii=True) + "\n",
        encoding="utf-8",
    )


def run_command(
    runtime: Path,
    args: list[str],
    *,
    timeout_seconds: float = COMMAND_TIMEOUT_SECONDS,
) -> CommandResult:
    display_argv = (runtime.name, *args)
    try:
        completed = subprocess.run(
            [str(runtime), *args],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout_seconds,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        return CommandResult(
            argv=display_argv,
            returncode=-1,
            stdout=decode_output(error.stdout),
            stderr=(
                decode_output(error.stderr)
                + f"\ncommand exceeded {timeout_seconds:.1f}s timeout"
            ).strip(),
            timed_out=True,
        )
    return CommandResult(
        argv=display_argv,
        returncode=completed.returncode,
        stdout=decode_output(completed.stdout),
        stderr=decode_output(completed.stderr),
    )


class UdpIdentifyResponder:
    def __init__(self, *, timeout_seconds: float = RESPONDER_TIMEOUT_SECONDS) -> None:
        self._timeout_seconds = timeout_seconds
        self._socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        _set_exclusive_address_use(self._socket)
        try:
            self._socket.bind((CI_HOST, ADS_UDP_PORT))
        except OSError as error:
            self._socket.close()
            raise GateError(
                f"cannot bind ADS Identify UDP {CI_HOST}:{ADS_UDP_PORT}; "
                f"the fixed protocol port is already in use ({error})"
            ) from error
        self._socket.settimeout(timeout_seconds)
        self._thread = threading.Thread(
            target=self._serve,
            name="ci-ads-identify-responder",
            daemon=True,
        )
        self._error: BaseException | None = None
        self._request: bytes | None = None
        self._peer: tuple[str, int] | None = None

    def start(self) -> None:
        self._thread.start()

    def finish(self) -> UdpTranscript:
        self._thread.join(self._timeout_seconds + 1.0)
        if self._thread.is_alive():
            self.close()
            raise GateError("ADS Identify responder did not finish within its timeout")
        if self._error is not None:
            raise GateError(
                f"ADS Identify responder failed: {self._error}"
            ) from self._error
        if self._request is None or self._peer is None:
            raise GateError("packaged runtime did not send an ADS Identify request")
        return UdpTranscript(
            request_hex=self._request.hex(),
            peer=f"{self._peer[0]}:{self._peer[1]}",
        )

    def close(self) -> None:
        try:
            self._socket.close()
        except OSError:
            pass

    def _serve(self) -> None:
        try:
            request, peer = self._socket.recvfrom(576)
            self._request = request
            self._peer = (str(peer[0]), int(peer[1]))
            assert_identify_request(request)
            self._socket.sendto(build_identify_response(), peer)
        except BaseException as error:  # thread boundary must preserve every failure
            self._error = error
        finally:
            self.close()


class AmsRouterProbe:
    def __init__(self, *, timeout_seconds: float = RESPONDER_TIMEOUT_SECONDS) -> None:
        self._timeout_seconds = timeout_seconds
        self._listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        _set_exclusive_address_use(self._listener)
        try:
            self._listener.bind((CI_HOST, ADS_TCP_PORT))
            self._listener.listen(3)
        except OSError as error:
            self._listener.close()
            raise GateError(
                f"cannot bind AMS router TCP {CI_HOST}:{ADS_TCP_PORT}; "
                f"the fixed protocol port is already in use ({error})"
            ) from error
        self._listener.settimeout(timeout_seconds)
        self._thread = threading.Thread(
            target=self._serve,
            name="ci-ams-router-probe",
            daemon=True,
        )
        self._error: BaseException | None = None
        self._identity_finished = threading.Event()
        self._identity_open_request: bytes | None = None
        self._identity_close_request: bytes | None = None
        self._probe_open_request: bytes | None = None
        self._probe_close_request: bytes | None = None
        self._open_request: bytes | None = None
        self._first_ams_frame: bytes | None = None

    def start(self) -> None:
        self._thread.start()

    def finish_identity(self) -> LocalRouterIdentityTranscript:
        if not self._identity_finished.wait(self._timeout_seconds + 1.0):
            self.close()
            raise GateError(
                "local AMS router identity proof did not finish within its timeout"
            )
        if self._error is not None:
            raise GateError(
                f"local AMS router identity proof failed: {self._error}"
            ) from self._error
        if (
            self._identity_open_request is None
            or self._identity_close_request is None
        ):
            raise GateError(
                "packaged runtime did not complete local AMS router identity discovery"
            )
        ams_net_id, assigned_source_port = assert_local_router_identity_transcript(
            self._identity_open_request,
            AMS_ROUTER_OPEN_REPLY,
            self._identity_close_request,
        )
        return LocalRouterIdentityTranscript(
            open_request_hex=self._identity_open_request.hex(),
            open_reply_hex=AMS_ROUTER_OPEN_REPLY.hex(),
            close_request_hex=self._identity_close_request.hex(),
            ams_net_id=ams_net_id,
            assigned_source_port=assigned_source_port,
        )

    def finish(self) -> RouterTranscript:
        self._thread.join(self._timeout_seconds + 1.0)
        if self._thread.is_alive():
            self.close()
            raise GateError("AMS router probe did not finish within its timeout")
        if self._error is not None:
            raise GateError(f"AMS router probe failed: {self._error}") from self._error
        if (
            self._probe_open_request is None
            or self._probe_close_request is None
            or self._open_request is None
            or self._first_ams_frame is None
        ):
            raise GateError("packaged runtime did not complete the AMS router probe")
        if self._probe_open_request != AMS_ROUTER_OPEN_REQUEST:
            raise GateError("packaged runtime did not begin with bounded AMS router detection")
        assert_router_probe_close(self._probe_close_request)
        target_ads_port = assert_source_request_transcript(
            self._open_request,
            self._first_ams_frame,
            CI_MANUAL_ADS_PORT,
        )
        return RouterTranscript(
            probe_open_request_hex=self._probe_open_request.hex(),
            probe_close_request_hex=self._probe_close_request.hex(),
            open_request_hex=self._open_request.hex(),
            first_ams_frame_hex=self._first_ams_frame.hex(),
            target_ads_port=target_ads_port,
            router_open_count=2,
        )

    def close(self) -> None:
        try:
            self._listener.close()
        except OSError:
            pass

    def _serve(self) -> None:
        try:
            identity, _ = self._listener.accept()
            with identity:
                identity.settimeout(self._timeout_seconds)
                self._identity_open_request = _recv_exact(identity, 8)
                if self._identity_open_request != AMS_ROUTER_OPEN_REQUEST:
                    return
                identity.sendall(AMS_ROUTER_OPEN_REPLY)
                self._identity_close_request = _recv_exact(identity, 8)
            self._identity_finished.set()

            probe, _ = self._listener.accept()
            with probe:
                probe.settimeout(self._timeout_seconds)
                self._probe_open_request = _recv_exact(probe, 8)
                if self._probe_open_request != AMS_ROUTER_OPEN_REQUEST:
                    return
                probe.sendall(AMS_ROUTER_OPEN_REPLY)
                self._probe_close_request = _recv_exact(probe, 8)

            connection, _ = self._listener.accept()
            with connection:
                connection.settimeout(self._timeout_seconds)
                self._open_request = _recv_exact(connection, 8)
                if self._open_request != AMS_ROUTER_OPEN_REQUEST:
                    return
                connection.sendall(AMS_ROUTER_OPEN_REPLY)
                ams_header = _recv_exact(connection, 6)
                frame_length = struct.unpack_from("<I", ams_header, 2)[0]
                if frame_length > MAX_AMS_FRAME_BYTES:
                    raise GateError(
                        f"AMS/TCP frame declared excessive length {frame_length}"
                    )
                frame_body = _recv_exact(connection, frame_length)
                self._first_ams_frame = ams_header + frame_body
        except BaseException as error:  # thread boundary must preserve every failure
            self._error = error
        finally:
            self._identity_finished.set()
            self.close()


class DirectLoopbackFallbackProbe:
    """A direct ADS server that never answers the temporary router probe."""

    def __init__(self, *, timeout_seconds: float = RESPONDER_TIMEOUT_SECONDS) -> None:
        self._timeout_seconds = timeout_seconds
        self._listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        _set_exclusive_address_use(self._listener)
        try:
            self._listener.bind((CI_DIRECT_HOST, ADS_TCP_PORT))
            self._listener.listen(2)
        except OSError as error:
            self._listener.close()
            raise GateError(
                f"cannot bind direct ADS TCP {CI_DIRECT_HOST}:{ADS_TCP_PORT}; "
                f"the fixed protocol port is already in use ({error})"
            ) from error
        self._listener.settimeout(timeout_seconds)
        self._thread = threading.Thread(
            target=self._serve,
            name="ci-direct-ads-fallback-probe",
            daemon=True,
        )
        self._error: BaseException | None = None
        self._probe_open_request: bytes | None = None
        self._first_ams_frame: bytes | None = None
        self._probe_hold_seconds: float | None = None
        self._fallback_frame_elapsed_seconds: float | None = None

    def start(self) -> None:
        self._thread.start()

    def finish(self) -> DirectFallbackTranscript:
        self._thread.join(self._timeout_seconds + DIRECT_PROBE_HOLD_SECONDS + 1.0)
        if self._thread.is_alive():
            self.close()
            raise GateError("direct ADS fallback probe did not finish within its timeout")
        if self._error is not None:
            raise GateError(
                f"direct ADS fallback probe failed: {self._error}"
            ) from self._error
        if (
            self._probe_open_request is None
            or self._first_ams_frame is None
            or self._probe_hold_seconds is None
            or self._fallback_frame_elapsed_seconds is None
        ):
            raise GateError("packaged runtime did not complete the direct ADS fallback proof")
        target_ads_port = assert_direct_fallback_transcript(
            self._probe_open_request,
            self._first_ams_frame,
            CI_MANUAL_ADS_PORT,
            probe_hold_seconds=self._probe_hold_seconds,
            fallback_frame_elapsed_seconds=self._fallback_frame_elapsed_seconds,
        )
        return DirectFallbackTranscript(
            probe_open_request_hex=self._probe_open_request.hex(),
            first_ams_frame_hex=self._first_ams_frame.hex(),
            target_ads_port=target_ads_port,
            probe_hold_seconds=self._probe_hold_seconds,
            fallback_frame_elapsed_seconds=self._fallback_frame_elapsed_seconds,
        )

    def close(self) -> None:
        try:
            self._listener.close()
        except OSError:
            pass

    def _serve(self) -> None:
        try:
            probe, _ = self._listener.accept()
            with probe:
                probe.settimeout(self._timeout_seconds)
                probe_started = time.monotonic()
                self._probe_open_request = _recv_exact(probe, 8)
                if self._probe_open_request != AMS_ROUTER_OPEN_REQUEST:
                    return

                # A pyads-style direct server keeps this incomplete router request
                # open. The packaged runtime must time out its detection and create
                # a second, direct AMS/TCP connection before we release the first.
                self._listener.settimeout(DIRECT_FALLBACK_MAX_SECONDS)
                connection, _ = self._listener.accept()
                with connection:
                    connection.settimeout(self._timeout_seconds)
                    ams_header = _recv_exact(connection, 6)
                    frame_length = struct.unpack_from("<I", ams_header, 2)[0]
                    if frame_length > MAX_AMS_FRAME_BYTES:
                        raise GateError(
                            f"AMS/TCP frame declared excessive length {frame_length}"
                        )
                    frame_body = _recv_exact(connection, frame_length)
                    self._first_ams_frame = ams_header + frame_body
                    self._fallback_frame_elapsed_seconds = (
                        time.monotonic() - probe_started
                    )

                remaining = DIRECT_PROBE_HOLD_SECONDS - (
                    time.monotonic() - probe_started
                )
                if remaining > 0:
                    time.sleep(remaining)
                self._probe_hold_seconds = time.monotonic() - probe_started
        except BaseException as error:  # thread boundary must preserve every failure
            self._error = error
        finally:
            self.close()


def run_packaged_gate(
    vsix_path: Path, staged_debug_path: Path, evidence: dict[str, Any]
) -> None:
    package = read_windows_vsix(vsix_path)
    vsix_bytes = vsix_path.read_bytes()
    evidence["package"] = {
        "path": vsix_path.name,
        "target_platform": package.target_platform,
        "version": package.version,
        "sha256": hashlib.sha256(vsix_bytes).hexdigest(),
        "size_bytes": len(vsix_bytes),
    }
    evidence["debug_adapter"] = assert_packaged_debug_matches_staged(
        package, staged_debug_path
    )
    with tempfile.TemporaryDirectory(prefix="trust-vsix-ads-") as temp_dir:
        runtime = Path(temp_dir) / "extension" / "bin" / "trust-runtime.exe"
        runtime.parent.mkdir(parents=True)
        runtime.write_bytes(package.runtime_bytes)
        runtime.chmod(0o755)
        evidence["runtime"] = {
            "member": WINDOWS_RUNTIME_MEMBER,
            "sha256": hashlib.sha256(package.runtime_bytes).hexdigest(),
            "size_bytes": len(package.runtime_bytes),
        }

        _record_phase(
            evidence,
            "runtime_version",
            lambda: _runtime_version_phase(runtime, package.version),
        )
        _record_phase(
            evidence,
            "manual_discovery_port_852",
            lambda: _manual_discovery_phase(runtime),
        )
        _record_phase(
            evidence,
            "host_port_rejection",
            lambda: _host_port_rejection_phase(runtime),
        )
        _record_phase(
            evidence,
            "udp_identify",
            lambda: _udp_identify_phase(runtime),
        )
        router_probe = AmsRouterProbe()
        router_probe.start()
        try:
            _record_phase(
                evidence,
                "same_computer_local_router_identity",
                lambda: _same_computer_router_identity_phase(runtime, router_probe),
            )
            _record_phase(
                evidence,
                "loopback_source_request_port_852",
                lambda: _source_request_phase(runtime, router_probe),
            )
        finally:
            router_probe.close()
        _record_phase(
            evidence,
            "direct_loopback_fallback_port_852",
            lambda: _direct_fallback_phase(runtime),
        )


def _runtime_version_phase(runtime: Path, expected_version: str) -> dict[str, Any]:
    result = run_command(runtime, ["--version"])
    _require_success(result, "packaged runtime --version")
    output = f"{result.stdout}\n{result.stderr}"
    if expected_version not in output:
        raise GateError(
            f"packaged runtime version did not contain VSIX version {expected_version}"
        )
    return {"command": result.evidence(), "version": output.strip()}


def _manual_discovery_phase(runtime: Path) -> dict[str, Any]:
    result = run_command(
        runtime,
        [
            "comm",
            "discover",
            "--protocol",
            "ads",
            "--origin",
            "this-host",
            "--host",
            CI_HOST,
            "--target-net-id",
            CI_AMS_NET_ID,
            "--ams-port",
            str(CI_MANUAL_ADS_PORT),
            "--json",
        ],
    )
    payload = _command_json(result, "manual ADS discovery")
    candidate = assert_discovery_candidate(
        payload,
        expected_source="manual",
        expected_port=CI_MANUAL_ADS_PORT,
    )
    return {"command": result.evidence(), "candidate": candidate}


def _host_port_rejection_phase(runtime: Path) -> dict[str, Any]:
    result = run_command(
        runtime,
        [
            "comm",
            "discover",
            "--protocol",
            "ads",
            "--origin",
            "this-host",
            "--host",
            f"{CI_HOST}:{CI_ADS_PORT}",
            "--ams-port",
            str(CI_ADS_PORT),
            "--json",
        ],
    )
    assert_actionable_host_port_rejection(result)
    return {"command": result.evidence()}


def _udp_identify_phase(runtime: Path) -> dict[str, Any]:
    responder = UdpIdentifyResponder()
    responder.start()
    try:
        result = run_command(
            runtime,
            [
                "comm",
                "discover",
                "--protocol",
                "ads",
                "--origin",
                "this-host",
                "--host",
                CI_HOST,
                "--ams-port",
                str(CI_ADS_PORT),
                "--json",
            ],
        )
        transcript = responder.finish()
    finally:
        responder.close()
    payload = _command_json(result, "directed ADS Identify discovery")
    candidate = assert_discovery_candidate(
        payload,
        expected_source="ads_identify",
        expected_port=CI_ADS_PORT,
    )
    return {
        "command": result.evidence(),
        "candidate": candidate,
        "responder": asdict(transcript),
    }


def _same_computer_router_identity_phase(
    runtime: Path,
    probe: AmsRouterProbe,
) -> dict[str, Any]:
    result = run_command(
        runtime,
        [
            "comm",
            "discover",
            "--protocol",
            "ads",
            "--origin",
            "this-host",
            "--host",
            CI_HOST,
            "--ams-port",
            str(CI_ADS_PORT),
            "--json",
        ],
    )
    transcript = probe.finish_identity()
    payload = _command_json(result, "same-computer local AMS router discovery")
    candidate = assert_same_computer_router_candidate(
        payload,
        expected_net_id=transcript.ams_net_id,
        expected_port=CI_ADS_PORT,
    )
    return {
        "command": result.evidence(),
        "candidate": candidate,
        "local_router_identity": asdict(transcript),
        "udp_responder_started": False,
    }


def _source_request_phase(runtime: Path, probe: AmsRouterProbe) -> dict[str, Any]:
    target = json.dumps(
        {
            "host": CI_HOST,
            "ams_net_id": CI_AMS_NET_ID,
            "ams_port": CI_MANUAL_ADS_PORT,
            "name": "CI-TWINCAT",
        },
        separators=(",", ":"),
        sort_keys=True,
    )
    result = run_command(
        runtime,
        [
            "comm",
            "browse-symbols",
            "--protocol",
            "ads",
            "--target",
            target,
            "--kind",
            "symbols",
            "--json",
        ],
    )
    transcript = probe.finish()
    if result.timed_out:
        raise GateError(
            "loopback ADS browse did not terminate after router probe closed"
        )
    return {"command": result.evidence(), "router": asdict(transcript)}


def _direct_fallback_phase(runtime: Path) -> dict[str, Any]:
    probe = DirectLoopbackFallbackProbe()
    probe.start()
    target = json.dumps(
        {
            "host": CI_DIRECT_HOST,
            "ams_net_id": CI_AMS_NET_ID,
            "ams_port": CI_MANUAL_ADS_PORT,
            "name": "CI-DIRECT-ADS",
        },
        separators=(",", ":"),
        sort_keys=True,
    )
    try:
        result = run_command(
            runtime,
            [
                "comm",
                "browse-symbols",
                "--protocol",
                "ads",
                "--target",
                target,
                "--kind",
                "symbols",
                "--json",
            ],
        )
        transcript = probe.finish()
    finally:
        probe.close()
    if result.timed_out:
        raise GateError("direct loopback ADS browse did not terminate")
    return {"command": result.evidence(), "direct_fallback": asdict(transcript)}


def _record_phase(
    evidence: dict[str, Any],
    name: str,
    action: Callable[[], dict[str, Any]],
) -> None:
    phase: dict[str, Any] = {"name": name, "status": "running"}
    evidence.setdefault("phases", []).append(phase)
    try:
        phase["detail"] = action()
    except BaseException as error:
        phase["status"] = "fail"
        phase["error"] = f"{type(error).__name__}: {error}"
        raise
    phase["status"] = "pass"


def _command_json(result: CommandResult, context: str) -> dict[str, Any]:
    _require_success(result, context)
    try:
        payload = json.loads(result.stdout.strip())
    except json.JSONDecodeError as error:
        raise GateError(f"{context} did not return valid JSON: {error}") from error
    if not isinstance(payload, dict):
        raise GateError(f"{context} returned JSON that was not an object")
    return payload


def _require_success(result: CommandResult, context: str) -> None:
    if result.returncode != 0:
        raise GateError(
            f"{context} failed with exit {result.returncode}: "
            f"{(result.stderr or result.stdout).strip()}"
        )


def _encode_tag(tag: int, payload: bytes) -> bytes:
    if not (0 <= tag <= 65_535):
        raise GateError(f"ADS UDP tag {tag} is outside the u16 range")
    if len(payload) > 65_535:
        raise GateError(f"ADS UDP tag {tag} payload exceeds the u16 length range")
    return struct.pack("<HH", tag, len(payload)) + payload


def _set_exclusive_address_use(sock: socket.socket) -> None:
    option = getattr(socket, "SO_EXCLUSIVEADDRUSE", None)
    if option is not None:
        sock.setsockopt(socket.SOL_SOCKET, option, 1)


def _recv_exact(sock: socket.socket, size: int) -> bytes:
    chunks: list[bytes] = []
    remaining = size
    while remaining:
        chunk = sock.recv(remaining)
        if not chunk:
            raise GateError(f"socket closed with {remaining} expected bytes unread")
        chunks.append(chunk)
        remaining -= len(chunk)
    return b"".join(chunks)


def _base_evidence(vsix_path: Path) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "gate": "windows_packaged_ads_runtime",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "status": "running",
        "package": {"path": vsix_path.name},
        "phases": [],
    }


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Prove the exact trust-runtime.exe and staged trust-debug.exe packaged "
            "in a win32-x64 VSIX"
        )
    )
    parser.add_argument("--vsix", type=Path, required=True)
    parser.add_argument("--staged-debug", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    evidence = _base_evidence(args.vsix)
    exit_code = 0
    try:
        run_packaged_gate(args.vsix, args.staged_debug, evidence)
        evidence["status"] = "pass"
    except BaseException as error:
        evidence["status"] = "fail"
        evidence["error"] = f"{type(error).__name__}: {error}"
        exit_code = 1
    try:
        write_evidence(args.evidence, evidence)
    except BaseException as error:
        print(
            json.dumps(
                {
                    "status": "fail",
                    "error": f"failed to write evidence: {type(error).__name__}: {error}",
                },
                ensure_ascii=True,
            ),
            file=sys.stderr,
        )
        return 1

    summary = {
        "status": evidence["status"],
        "evidence": str(args.evidence),
    }
    stream = sys.stdout if exit_code == 0 else sys.stderr
    print(json.dumps(summary, ensure_ascii=True), file=stream)
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
