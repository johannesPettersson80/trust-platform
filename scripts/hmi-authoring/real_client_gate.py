#!/usr/bin/env python3
"""Controlled real-client CI adapter; the protected runner owns the logged-in IDE profile.

This validates driver artifacts, not product behavior by metadata. The driver must operate the
actual named client, preserve its own raw private logs, and export only sanitized evidence.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--client", choices=["codex-ide", "vscode-agent"], required=True)
    parser.add_argument("--head", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    runner = Path(os.environ.get("HMI_REAL_AGENT_DRIVER", ""))
    if not runner.is_absolute() or not runner.is_file() or not os.access(runner, os.X_OK):
        (args.output / "status.json").write_text(json.dumps({"status": "unavailable", "reason": "protected runner has no configured real-client driver", "client": args.client, "head": args.head}))
        return 77
    # The executable and account policy come from protected environment configuration, never PR input.
    try:
        result = subprocess.run([str(runner), "--client", args.client, "--head", args.head,
                                 "--projects", "tank-pid,multi-unit,branching-process,withheld-project",
                                 "--max-tool-calls", "80", "--output", str(args.output.resolve())],
                                timeout=1200, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except subprocess.TimeoutExpired:
        (args.output / "status.json").write_text(json.dumps({"status": "timeout", "client": args.client, "head": args.head}))
        return 1
    if result.returncode:
        return result.returncode
    report = json.loads((args.output / "acceptance.json").read_text())
    if report.get("head") != args.head or report.get("client") != args.client or not report.get("clientVersion"):
        raise ValueError("real-client report identity does not match this candidate")
    cases = report.get("projects", [])
    expected = {"tank-pid", "multi-unit", "branching-process", "withheld-project"}
    if {case.get("name") for case in cases} != expected or len(cases) != 4:
        raise ValueError("four distinct actual-client projects are required")
    for case in cases:
        if case.get("status") != "passed" or not case.get("actualClientSession"):
            raise ValueError("an actual client journey is missing or failed")
        for artifact in case.get("artifacts", []):
            file = (args.output / artifact["file"]).resolve()
            if not file.is_relative_to(args.output.resolve()) or not file.is_file():
                raise ValueError("artifact path is missing or escapes the report")
            if hashlib.sha256(file.read_bytes()).hexdigest() != artifact["sha256"]:
                raise ValueError("artifact content does not match its recorded hash")
        if not any(item["file"].endswith(".png") for item in case.get("artifacts", [])):
            raise ValueError("actual rendered images are required for every project")
    print(f"Verified artifact identity for {args.client}; qualitative acceptance remains with the recorded reviewer.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, KeyError, OSError, TypeError) as error:
        print(f"Real-client artifact gate failed: {error}", file=sys.stderr)
        sys.exit(1)
