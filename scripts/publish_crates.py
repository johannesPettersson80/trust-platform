#!/usr/bin/env python3
"""Publishes the SDK's crates to crates.io at the release's version (specification 48 RU-12; specification 24,
"Publishing the SDK's crates").

The crates are `trust-plc-abi`, `trust-plc-lib`, `trust-plc-decl`, `trust-plc-macros` and `trust-plc`, and the SDK of
services, `trust-runtime-client` and `trust-service` (specification 48 RU-62), each after the crates it depends on, published from the released commit and each verified by cargo's build of its package.
A crate whose version crates.io already holds is skipped, so a re-run continues where an earlier run stopped.
Without the token in `CARGO_REGISTRY_TOKEN` it publishes nothing and says so in a notice: crates.io keeps a
published version for good, and only the owner of the crates' account gives the token.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from typing import Callable, Mapping, Sequence

# The SDK's crates in the order they are published: each after the crates it depends on.
CRATES: tuple[str, ...] = (
    "trust-plc-abi",
    "trust-plc-lib",
    "trust-plc-decl",
    "trust-plc-macros",
    "trust-plc",
    "trust-runtime-client",
    "trust-service",
)
API = "https://crates.io/api/v1/crates/{name}/{version}"
# crates.io asks an automated client to name itself.
USER_AGENT = "trust-platform-release-workflow (https://github.com/{repository})"
# A transient answer of crates.io is asked again after these pauses, in seconds.
RETRY_PAUSES = (10, 30)

Run = Callable[..., subprocess.CompletedProcess]
Fetch = Callable[[str, str], int]


def http_status(url: str, user_agent: str) -> int:
    """The HTTP status crates.io answers `url` with."""
    request = urllib.request.Request(url, headers={"User-Agent": user_agent})
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return response.status
    except urllib.error.HTTPError as error:
        return error.code


def workspace_version(run: Run) -> str:
    """The version every crate of `CRATES` has in the workspace, which is the release's."""
    output = run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    versions = {
        package["name"]: package["version"]
        for package in json.loads(output)["packages"]
        if package["name"] in CRATES
    }
    missing = [name for name in CRATES if name not in versions]
    if missing:
        raise SystemExit(f"the workspace has no crate {', '.join(missing)}")
    if len(set(versions.values())) != 1:
        raise SystemExit(f"the SDK's crates have different versions: {versions}")
    return versions[CRATES[0]]


def is_published(
    name: str, version: str, fetch: Fetch, user_agent: str, pause: Callable[[float], None]
) -> bool:
    """Whether crates.io holds `name` at `version`; a transient answer is asked again."""
    url = API.format(name=name, version=version)
    status = 0
    for attempt, wait in enumerate((0, *RETRY_PAUSES)):
        if wait:
            pause(wait)
        try:
            status = fetch(url, user_agent)
        except OSError as error:
            print(f"crates.io did not answer for {name} {version} (attempt {attempt + 1}): {error}")
            continue
        if status == 200:
            return True
        if status == 404:
            return False
        print(f"crates.io answered {status} for {name} {version} (attempt {attempt + 1})")
    raise SystemExit(f"crates.io gave no answer for {name} {version}: the last status was {status}")


def main(
    argv: Sequence[str] | None = None,
    env: Mapping[str, str] = os.environ,
    run: Run = subprocess.run,
    fetch: Fetch = http_status,
    pause: Callable[[float], None] = time.sleep,
) -> int:
    """Publishes every crate of `CRATES` that crates.io does not hold at the release's version."""
    del argv
    if not env.get("CARGO_REGISTRY_TOKEN"):
        print(
            "::notice::crates.io publication skipped: the repository holds no CARGO_REGISTRY_TOKEN "
            "(specification 24, \"Publishing the SDK's crates\")"
        )
        return 0
    version = workspace_version(run)
    user_agent = USER_AGENT.format(repository=env.get("GITHUB_REPOSITORY", "johannesPettersson80/trust-platform"))
    for name in CRATES:
        if is_published(name, version, fetch, user_agent, pause):
            print(f"{name} {version} is on crates.io: skipped")
            continue
        print(f"publishing {name} {version}")
        # Cargo verifies the package by building it, and waits until crates.io serves the version before it
        # returns, so the next crate's verification finds it.
        run(["cargo", "publish", "--package", name], check=True)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
