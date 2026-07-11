#!/usr/bin/env python3
"""Deterministic ADS UI fixture that delegates every unrelated CLI command.

This is visual acceptance infrastructure, not TwinCAT hardware proof.  The real
truST runtime still supplies schema/topology/config behavior.  Only ADS
``comm discover`` and ``comm browse-symbols`` are intercepted so the same real
VS Code webview can be captured in repeatable UI states.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
STATE_FILE = Path(os.environ.get("TRUST_ADS_UI_FIXTURE_STATE", ROOT / "fixture-state.json"))
TRANSCRIPT = Path(
    os.environ.get(
        "TRUST_ADS_UI_FIXTURE_TRANSCRIPT",
        ROOT / "logs" / "fixture-transcript.jsonl",
    )
)
REAL_RUNTIME = os.environ.get(
    "TRUST_REAL_RUNTIME",
    "/home/johannes/projects/trust-platform/target/debug/trust-runtime",
)


def option(args: list[str], flag: str) -> str | None:
    try:
        return args[args.index(flag) + 1]
    except (ValueError, IndexError):
        return None


def fixture_state() -> str:
    try:
        payload = json.loads(STATE_FILE.read_text(encoding="utf-8"))
        value = str(payload.get("state", "sole_runtime"))
    except (OSError, json.JSONDecodeError, TypeError):
        value = "sole_runtime"
    return value


def log_invocation(args: list[str], state: str, handled: bool) -> None:
    TRANSCRIPT.parent.mkdir(parents=True, exist_ok=True)
    row = {
        "at": datetime.now(timezone.utc).isoformat(),
        "state": state,
        "handled_by_fixture": handled,
        "args": args,
    }
    with TRANSCRIPT.open("a", encoding="utf-8") as stream:
        stream.write(json.dumps(row, sort_keys=True) + "\n")


def print_json(payload: dict[str, Any]) -> int:
    sys.stdout.write(json.dumps(payload, separators=(",", ":")) + "\n")
    return 0


def candidate_for(state: str, args: list[str]) -> dict[str, Any]:
    supplied_host = option(args, "--host")
    supplied_net_id = option(args, "--target-net-id")
    defaults = {
        "sole_runtime": ("LOCAL-TWINCAT", "127.0.0.1", "127.0.0.1.1.1"),
        "manual_declared": ("MANUALLY-ENTERED-TWINCAT", "192.168.77.11", "100.67.6.217.1.1"),
        "route_required": ("ROUTE-REQUIRED-TWINCAT", "192.168.77.21", "5.23.91.12.1.1"),
        "multiple_ports": ("MULTI-RUNTIME-TWINCAT", "192.168.77.31", "5.44.33.22.1.1"),
    }
    name, host, net_id = defaults.get(state, defaults["sole_runtime"])
    host = supplied_host or host
    net_id = supplied_net_id or net_id
    manual = state == "manual_declared"
    local_router = state == "sole_runtime"
    return {
        "id": f"fixture-ads-{state}",
        "label": f"{name} · {net_id}",
        "source": (
            "manual"
            if manual
            else "ads_local_router"
            if local_router
            else "directed_identify"
        ),
        "confidence": "declared" if manual else "confirmed",
        "protocol": "ads",
        "params": {
            "name": name,
            "host": host,
            "ip": host,
            "ams_net_id": net_id,
            "target_net_id": net_id,
            "ams_port": 851,
            "tc_version": "3.1.4026-fixture",
        },
        "warnings": [
            "Deterministic visual fixture; this candidate is not hardware evidence."
        ],
    }


def symbol_tree(count: int, prefix: str) -> list[dict[str, Any]]:
    children = []
    for index in range(1, count + 1):
        name = f"{prefix}_{index:02d}"
        children.append(
            {
                "id": f"ads:symbol:MAIN.{name}",
                "name": name,
                "path": f"MAIN.{name}",
                "data_type": "BOOL" if index % 2 else "REAL",
                "writable": index % 3 == 0,
            }
        )
    return [
        {
            "id": f"ads:group:{prefix}",
            "name": "MAIN",
            "path": "MAIN",
            "children": children,
        }
    ]


def unavailable(port: int) -> int:
    sys.stderr.write(
        f"ADS target port {port} is not registered (deterministic visual fixture)\n"
    )
    return 1


def handle_browse(state: str, args: list[str]) -> int:
    raw_target = option(args, "--target") or "{}"
    try:
        target = json.loads(raw_target)
    except json.JSONDecodeError:
        target = {}
    port = int(target.get("ams_port", 851))

    base: dict[str, Any] = {"schema_version": 1, "protocol": "ads"}
    if state == "route_required":
        if port != 851:
            return unavailable(port)
        return print_json(
            {
                **base,
                "tree": [],
                "route": {
                    "status": "missing",
                    "route_plan": {
                        "route_name": "truST-visual-fixture",
                        "artifacts": [
                            {
                                "kind": "powershell",
                                "label": "TwinCAT route PowerShell",
                                "filename": "setup-trust-route.ps1",
                                "content_type": "text/x-powershell",
                                "content": (
                                    "# Deterministic UI fixture only\n"
                                    "Write-Host 'Add a TwinCAT route back to the discovery computer'"
                                ),
                            }
                        ],
                    },
                },
            }
        )

    if state == "multiple_ports":
        if port == 851:
            return print_json({**base, "tree": symbol_tree(6, "RUNTIME1")})
        if port == 853:
            return print_json({**base, "tree": symbol_tree(4, "RUNTIME3")})
        if port == 854:
            return print_json({**base, "tree": []})
        if port == 301:
            return print_json(
                {
                    **base,
                    "tree": [],
                    "error": {
                        "code": "symbol_upload_unsupported",
                        "message": (
                            "The ADS I/O service is reachable, but Symbol Upload "
                            "is not supported by this namespace."
                        ),
                    },
                }
            )
        if port == 501:
            return unavailable(port)
        if port == 9000:
            return print_json({**base, "tree": []})
        return unavailable(port)

    if port == 851:
        count = 3 if state == "manual_declared" else 12
        prefix = "MANUAL" if state == "manual_declared" else "LOCAL"
        return print_json({**base, "tree": symbol_tree(count, prefix)})
    if port == 301:
        return print_json(
            {
                **base,
                "tree": [],
                "error": {
                    "code": "symbol_upload_unsupported",
                    "message": (
                        "Additional task 1 is reachable, but Symbol Upload is not "
                        "supported by this namespace."
                    ),
                },
            }
        )
    return unavailable(port)


def main() -> int:
    args = sys.argv[1:]
    state = fixture_state()
    is_discover = len(args) >= 2 and args[:2] == ["comm", "discover"]
    is_browse = len(args) >= 2 and args[:2] == ["comm", "browse-symbols"]
    handled = (is_discover and option(args, "--protocol") == "ads") or (
        is_browse and option(args, "--protocol") == "ads"
    )
    log_invocation(args, state, handled)

    if is_discover and option(args, "--protocol") == "ads":
        if state == "identity_not_found":
            target = option(args, "--host") or "255.255.255.255"
            sys.stderr.write(
                "ADS discovery failed: UdpIdentifyBlocked: "
                f"ADS UDP identify failed for {target}: no target answered\n"
            )
            return 1
        return print_json(
            {
                "schema_version": 1,
                "protocol": "ads",
                "candidates": [candidate_for(state, args)],
            }
        )
    if is_browse and option(args, "--protocol") == "ads":
        return handle_browse(state, args)

    if not Path(REAL_RUNTIME).is_file():
        sys.stderr.write(f"real trust-runtime not found: {REAL_RUNTIME}\n")
        return 127
    completed = subprocess.run([REAL_RUNTIME, *args], check=False)
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
