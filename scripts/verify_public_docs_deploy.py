#!/usr/bin/env python3

from __future__ import annotations

import argparse
import sys
import time
import urllib.error
import urllib.request


CHECKS: list[tuple[str, str]] = [
    ("", "truST"),
    ("start/agent-quickstart/", "Agent Quickstart"),
    ("connect/protocol-matrix/", "Protocol Matrix"),
    ("operate/compile-validate-reload/", "Compile, Validate, Reload"),
    ("reference/agent-api/v1/", "Agent API v1"),
]


def fetch(url: str) -> str:
    with urllib.request.urlopen(url, timeout=30) as response:
        return response.read().decode("utf-8", errors="ignore")


def main() -> int:
    parser = argparse.ArgumentParser(description="Verify the deployed public docs site.")
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--retries", type=int, default=12)
    parser.add_argument("--sleep-seconds", type=int, default=10)
    args = parser.parse_args()

    base_url = args.base_url.rstrip("/") + "/"
    last_error = None
    for attempt in range(1, args.retries + 1):
        failures: list[str] = []
        for path, needle in CHECKS:
            url = base_url + path
            try:
                body = fetch(url)
            except urllib.error.URLError as exc:
                failures.append(f"{url}: {exc}")
                continue
            if needle not in body:
                failures.append(f"{url}: missing expected text '{needle}'")
        if not failures:
            print("public docs deploy verification passed")
            return 0
        last_error = failures
        if attempt < args.retries:
            time.sleep(args.sleep_seconds)

    print("public docs deploy verification failed:", file=sys.stderr)
    for failure in last_error or []:
        print(f"  - {failure}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
