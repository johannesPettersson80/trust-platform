#!/usr/bin/env python3
"""Independent pyads smoke test for the truST ADS server.

This script is intentionally outside the Rust runtime. It proves that a
non-truST ADS client can use the server over AMS/TCP. It does not replace the
real TwinCAT engineering-station gate.
"""

from __future__ import annotations

import argparse
import ctypes
import json
import subprocess
import sys
import time
from dataclasses import dataclass
from typing import Any


ADSIGRP_SYM_VERSION = 0xF008


@dataclass(frozen=True)
class SymbolProbe:
    name: str
    type_name: str


@dataclass(frozen=True)
class WriteProbe:
    name: str
    type_name: str
    value: str


def parse_symbol_probe(raw: str) -> SymbolProbe:
    parts = raw.split(":")
    if len(parts) != 2 or not parts[0] or not parts[1]:
        raise argparse.ArgumentTypeError("expected SYMBOL:TYPE")
    return SymbolProbe(parts[0], parts[1].upper())


def parse_write_probe(raw: str) -> WriteProbe:
    parts = raw.split(":", 2)
    if len(parts) != 3 or not parts[0] or not parts[1]:
        raise argparse.ArgumentTypeError("expected SYMBOL:TYPE:VALUE")
    return WriteProbe(parts[0], parts[1].upper(), parts[2])


def load_pyads() -> Any:
    try:
        import pyads  # type: ignore
    except ImportError as exc:
        raise SystemExit(
            "pyads is not installed. Install it in a venv, for example: "
            "python3 -m venv .venv-pyads && .venv-pyads/bin/python -m pip install pyads"
        ) from exc
    return pyads


def plc_type(pyads: Any, type_name: str) -> Any:
    mapping = {
        "BOOL": pyads.PLCTYPE_BOOL,
        "SINT": pyads.PLCTYPE_SINT,
        "INT": pyads.PLCTYPE_INT,
        "DINT": pyads.PLCTYPE_DINT,
        "LINT": pyads.PLCTYPE_LINT,
        "USINT": pyads.PLCTYPE_USINT,
        "UINT": pyads.PLCTYPE_UINT,
        "UDINT": pyads.PLCTYPE_UDINT,
        "ULINT": pyads.PLCTYPE_ULINT,
        "REAL": pyads.PLCTYPE_REAL,
        "LREAL": pyads.PLCTYPE_LREAL,
        "BYTE": pyads.PLCTYPE_BYTE,
        "WORD": pyads.PLCTYPE_WORD,
        "DWORD": pyads.PLCTYPE_DWORD,
    }
    try:
        return mapping[type_name.upper()]
    except KeyError as exc:
        raise SystemExit(f"unsupported smoke type '{type_name}'") from exc


def parse_value(type_name: str, raw: str) -> Any:
    type_name = type_name.upper()
    if type_name == "BOOL":
        if raw.lower() in {"1", "true", "yes", "on"}:
            return True
        if raw.lower() in {"0", "false", "no", "off"}:
            return False
        raise SystemExit(f"cannot parse BOOL value '{raw}'")
    if type_name in {"REAL", "LREAL"}:
        return float(raw)
    if type_name in {
        "SINT",
        "INT",
        "DINT",
        "LINT",
        "USINT",
        "UINT",
        "UDINT",
        "ULINT",
        "BYTE",
        "WORD",
        "DWORD",
    }:
        return int(raw, 0)
    raise SystemExit(f"unsupported write type '{type_name}'")


def json_value(value: Any) -> Any:
    if isinstance(value, (bool, int, float, str)) or value is None:
        return value
    return str(value)


def near_equal(left: Any, right: Any) -> bool:
    if isinstance(left, float) or isinstance(right, float):
        return abs(float(left) - float(right)) < 0.0001
    return left == right


def run_doctor_with_external_evidence(args: argparse.Namespace, evidence: dict[str, Any]) -> Any:
    if not args.doctor_endpoint:
        return None
    command = [
        args.trust_runtime,
        "ads",
        "server",
        "doctor",
        "--endpoint",
        args.doctor_endpoint,
        "--external-kind",
        evidence["kind"],
        "--external-name",
        evidence["name"],
        "--json",
    ]
    if args.doctor_token:
        command.extend(["--token", args.doctor_token])
    result = subprocess.run(
        command,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        raise SystemExit(
            "ADS server doctor refused external evidence after pyads smoke passed:\n"
            f"{result.stderr.strip() or result.stdout.strip()}"
        )
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise SystemExit(f"ADS server doctor returned non-JSON output: {result.stdout}") from exc


def run_smoke(args: argparse.Namespace) -> dict[str, Any]:
    pyads = load_pyads()
    operations: list[dict[str, Any]] = []
    notification_values: list[Any] = []

    pyads.open_port()
    pyads.set_local_address(args.local_net_id)
    pyads.add_route(args.target_net_id, args.target_ip)

    plc = pyads.Connection(args.target_net_id, args.ads_port, args.target_ip)
    plc.set_timeout(args.timeout_ms)
    try:
        plc.open()

        device_info = plc.read_device_info()
        operations.append({"step": "device_info", "status": "pass", "value": str(device_info)})

        state = plc.read_state()
        operations.append({"step": "read_state", "status": "pass", "value": str(state)})

        symbols = plc.get_all_symbols()
        symbol_names = sorted(getattr(symbol, "name", str(symbol)) for symbol in symbols)
        missing_symbols = sorted({probe.name for probe in args.read} - set(symbol_names))
        if missing_symbols:
            raise SystemExit(f"symbol browse missed expected symbols: {missing_symbols}")
        operations.append(
            {
                "step": "browse",
                "status": "pass",
                "symbol_count": len(symbol_names),
                "checked": [probe.name for probe in args.read],
            }
        )

        for probe in args.read:
            typ = plc_type(pyads, probe.type_name)
            handle = plc.get_handle(probe.name)
            if handle is None:
                raise SystemExit(f"failed to resolve handle for {probe.name}")
            value_by_handle = plc.read_by_name(probe.name, typ, handle=handle)
            plc.release_handle(handle)
            value_by_name = plc.read_by_name(probe.name, typ)
            operations.append(
                {
                    "step": "read",
                    "status": "pass",
                    "symbol": probe.name,
                    "type": probe.type_name,
                    "value": json_value(value_by_name),
                    "value_by_handle": json_value(value_by_handle),
                }
            )

        if args.read:
            sumup_values = plc.read_list_by_name([probe.name for probe in args.read])
            operations.append(
                {
                    "step": "sumup_read",
                    "status": "pass",
                    "values": {key: json_value(value) for key, value in sumup_values.items()},
                }
            )

        symbol_version = plc.read(ADSIGRP_SYM_VERSION, 0, pyads.PLCTYPE_DWORD)
        operations.append(
            {
                "step": "symbol_version",
                "status": "pass",
                "value": json_value(symbol_version),
            }
        )

        if args.notification:
            probe = args.notification
            typ = plc_type(pyads, probe.type_name)

            @plc.notification(typ, timestamp_as_filetime=True)
            def callback(handle: int, name: str, timestamp: int, value: Any) -> None:
                notification_values.append(
                    {
                        "handle": handle,
                        "name": name,
                        "timestamp_filetime": timestamp,
                        "value": json_value(value),
                    }
                )

            attr = pyads.NotificationAttrib(
                ctypes.sizeof(typ),
                pyads.ADSTRANS_SERVERCYCLE,
                max_delay=args.notification_cycle_s,
                cycle_time=args.notification_cycle_s,
            )
            handles = plc.add_device_notification(probe.name, attr, callback)
            if not handles:
                raise SystemExit(f"failed to subscribe notification for {probe.name}")
            deadline = time.monotonic() + args.notification_timeout_s
            while not notification_values and time.monotonic() < deadline:
                time.sleep(0.05)
            plc.del_device_notification(handles[0], handles[1])
            if not notification_values:
                raise SystemExit(f"no notification received for {probe.name}")
            operations.append(
                {
                    "step": "notification",
                    "status": "pass",
                    "symbol": probe.name,
                    "sample": notification_values[0],
                }
            )

        if args.write:
            probe = args.write
            typ = plc_type(pyads, probe.type_name)
            requested = parse_value(probe.type_name, probe.value)
            original = plc.read_by_name(probe.name, typ)
            plc.write_by_name(probe.name, requested, typ)
            time.sleep(args.write_settle_s)
            observed = plc.read_by_name(probe.name, typ)
            plc.write_by_name(probe.name, original, typ)
            time.sleep(args.write_settle_s)
            restored = plc.read_by_name(probe.name, typ)
            if not near_equal(observed, requested):
                raise SystemExit(
                    f"write read-back mismatch for {probe.name}: expected {requested}, got {observed}"
                )
            if not near_equal(restored, original):
                raise SystemExit(
                    f"restore read-back mismatch for {probe.name}: expected {original}, got {restored}"
                )
            operations.append(
                {
                    "step": "guarded_write",
                    "status": "pass",
                    "symbol": probe.name,
                    "type": probe.type_name,
                    "requested": json_value(requested),
                    "observed": json_value(observed),
                    "restored": json_value(restored),
                }
            )
    finally:
        try:
            plc.close()
        finally:
            pyads.close_port()

    evidence = {
        "kind": "pyads",
        "name": args.evidence_name,
        "timestamp_ms": int(time.time() * 1000),
    }
    report: dict[str, Any] = {
        "status": "pass",
        "target": {
            "ip": args.target_ip,
            "ams_net_id": args.target_net_id,
            "ads_port": args.ads_port,
        },
        "local": {"ams_net_id": args.local_net_id},
        "operations": operations,
        "external_client_evidence": evidence,
        "twinCAT_merge_gate_satisfied": False,
    }
    doctor = run_doctor_with_external_evidence(args, evidence)
    if doctor is not None:
        report["doctor"] = doctor
    return report


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Run an independent pyads smoke test against a truST ADS server.",
    )
    parser.add_argument("--target-ip", required=True, help="truST runtime host IP")
    parser.add_argument("--target-net-id", required=True, help="truST ADS server AMS Net ID")
    parser.add_argument("--ads-port", type=int, default=851, help="logical ADS port")
    parser.add_argument(
        "--local-net-id",
        default="127.0.0.1.1.100",
        help="pyads client AMS Net ID allowed by runtime.ads_server.clients",
    )
    parser.add_argument(
        "--read",
        action="append",
        type=parse_symbol_probe,
        default=[],
        metavar="SYMBOL:TYPE",
        help="symbol to resolve/read; repeatable",
    )
    parser.add_argument(
        "--notification",
        type=parse_symbol_probe,
        metavar="SYMBOL:TYPE",
        help="symbol to subscribe with ADSTRANS_SERVERCYCLE",
    )
    parser.add_argument(
        "--write",
        type=parse_write_probe,
        metavar="SYMBOL:TYPE:VALUE",
        help="optional guarded write; original value is restored",
    )
    parser.add_argument("--timeout-ms", type=int, default=3000)
    parser.add_argument("--notification-timeout-s", type=float, default=3.0)
    parser.add_argument("--notification-cycle-s", type=float, default=0.05)
    parser.add_argument("--write-settle-s", type=float, default=0.2)
    parser.add_argument("--evidence-name", default="pyads-smoke")
    parser.add_argument(
        "--doctor-endpoint",
        help="optional runtime control endpoint; when set, attach pyads evidence to ads.server.doctor",
    )
    parser.add_argument("--doctor-token", help="runtime control token for --doctor-endpoint")
    parser.add_argument("--trust-runtime", default="trust-runtime")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    report = run_smoke(args)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
