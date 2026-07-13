#!/usr/bin/env python3
"""Prove the packaged Windows VSIX uses native same-computer ADS safely.

The stock-Windows lane proves local ADS never falls back to raw loopback TCP.
Real TwinCAT symbol browsing remains a separate device-in-the-loop acceptance.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import socket
import subprocess
import sys
import tempfile
import zipfile
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable
from xml.etree import ElementTree


ADS_TCP_PORT = 48_898
CI_HOST = "127.0.0.1"
CI_AMS_NET_ID = "127.0.0.1.1.1"
CI_ADS_PORT = 851
CI_MANUAL_ADS_PORT = 852

COMMAND_TIMEOUT_SECONDS = 20.0
WINDOWS_RUNTIME_MEMBER = "extension/bin/trust-runtime.exe"
WINDOWS_DEBUG_MEMBER = "extension/bin/trust-debug.exe"
WINDOWS_LSP_MEMBER = "extension/bin/trust-lsp.exe"
PACKAGE_JSON_MEMBER = "extension/package.json"
VSIX_MANIFEST_MEMBER = "extension.vsixmanifest"


class GateError(RuntimeError):
    """A deterministic packaged-runtime proof failed."""


@dataclass(frozen=True)
class WindowsVsix:
    version: str
    target_platform: str
    runtime_bytes: bytes
    debug_bytes: bytes
    lsp_bytes: bytes


@dataclass(frozen=True)
class CommandResult:
    argv: tuple[str, ...]
    returncode: int
    stdout: str
    stderr: str
    timed_out: bool = False

    def evidence(self) -> dict[str, Any]:
        return asdict(self)


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


def assert_actionable_host_port_rejection(result: CommandResult) -> None:
    if result.returncode == 0:
        raise GateError("ADS host:port input unexpectedly succeeded")
    detail = f"{result.stdout}\n{result.stderr}".lower()
    if "host or ip only" not in detail or "ads port" not in detail:
        raise GateError(
            "ADS host:port rejection was not actionable; expected both "
            "'host or IP only' and separate 'ADS port' guidance"
        )


def assert_native_same_computer_result(result: CommandResult) -> dict[str, Any]:
    if result.timed_out:
        raise GateError("native same-computer ADS browse timed out")
    payload = _command_json(result, "native same-computer ADS browse")
    error = payload.get("error")
    if not isinstance(error, dict):
        raise GateError(
            "stock Windows native ADS proof expected a structured unavailable error"
        )
    code = error.get("code")
    if code not in {"symbol_upload_failed", "ads_port_unavailable"}:
        raise GateError(
            "native same-computer ADS browse returned an unexpected error code: "
            f"{code!r}"
        )
    message = str(error.get("message", ""))
    if not message.strip():
        raise GateError("native same-computer ADS error did not include a message")
    if "tcadsdll" not in message.lower() and "native windows ads" not in message.lower():
        raise GateError(
            "same-computer ADS error did not prove the native Windows backend; "
            f"got {message!r}"
        )
    return payload


class RawAdsTcpTrap:
    """Detect any forbidden raw ADS/TCP connection to the same Windows host."""

    def __init__(self) -> None:
        self._listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        _set_exclusive_address_use(self._listener)
        try:
            self._listener.bind((CI_HOST, ADS_TCP_PORT))
            self._listener.listen(1)
        except OSError as error:
            self._listener.close()
            raise GateError(
                f"cannot reserve raw ADS TCP trap {CI_HOST}:{ADS_TCP_PORT}: {error}"
            ) from error

    def assert_unused(self) -> None:
        self._listener.settimeout(0.25)
        try:
            connection, peer = self._listener.accept()
        except TimeoutError:
            return
        else:
            connection.close()
            raise GateError(
                "same-computer ADS opened forbidden raw TCP 48898 connection "
                f"from {peer}; Windows must use TcAdsDll"
            )
        finally:
            self.close()

    def close(self) -> None:
        try:
            self._listener.close()
        except OSError:
            pass


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
                WINDOWS_LSP_MEMBER,
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
            lsp_bytes = archive.read(WINDOWS_LSP_MEMBER)
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
    if not lsp_bytes.startswith(b"MZ"):
        raise GateError(
            "packaged extension/bin/trust-lsp.exe is not a Windows PE file"
        )
    return WindowsVsix(
        version=version.strip(),
        target_platform=target_platform,
        runtime_bytes=runtime_bytes,
        debug_bytes=debug_bytes,
        lsp_bytes=lsp_bytes,
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
            "native_same_computer_no_raw_tcp",
            lambda: _native_same_computer_phase(runtime),
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


def _native_same_computer_phase(runtime: Path) -> dict[str, Any]:
    trap = RawAdsTcpTrap()
    target = json.dumps(
        {
            "host": CI_HOST,
            "ams_net_id": CI_AMS_NET_ID,
            "ams_port": CI_ADS_PORT,
            "name": "CI-SAME-COMPUTER",
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
        trap.assert_unused()
        payload = assert_native_same_computer_result(result)
    finally:
        trap.close()
    return {
        "command": result.evidence(),
        "error": payload["error"],
        "raw_tcp_48898_connection": False,
        "expected_backend": "TcAdsDll",
    }


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


def _set_exclusive_address_use(sock: socket.socket) -> None:
    option = getattr(socket, "SO_EXCLUSIVEADDRUSE", None)
    if option is not None:
        sock.setsockopt(socket.SOL_SOCKET, option, 1)


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
