#!/usr/bin/env python3
"""Structural contract for the shipped OpenOT database examples and docs."""

from __future__ import annotations

import pathlib
import sys
import tomllib


ROOT = pathlib.Path(__file__).resolve().parents[1]
EXAMPLES = ROOT / "examples" / "openot_database"
PRODUCTS = {
    "sqlite": "sqlite",
    "postgresql": "postgresql",
    "timescaledb": "timescaledb",
    "mysql": "mysql",
    "mariadb": "mysql",
    "sqlserver": "sqlserver",
    "influxdb3": "influxdb3",
}
REQUIRED_GUIDANCE = (
    "Prerequisites",
    "Prepare and run",
    "Verify",
    "Outage and restart",
    "Backup and restore",
    "Clean up",
)


def fail(message: str) -> None:
    raise AssertionError(message)


def main() -> int:
    workload = (EXAMPLES / "workload" / "Main.st").read_text()
    if "SQL" in workload.upper():
        fail("canonical ST workload must not contain database calls")
    if not (EXAMPLES / "workload" / "openot-coverage-manifest.json").is_file():
        fail("canonical OpenOT coverage manifest is missing")

    for product, expected_backend in PRODUCTS.items():
        directory = EXAMPLES / product
        config_path = directory / "runtime.toml"
        readme_path = directory / "README.md"
        config_text = config_path.read_text()
        config = tomllib.loads(config_text)
        persistence = config["runtime"]["openot"]["persistence"]
        if persistence["backend"] != expected_backend:
            fail(f"{product}: unexpected TOML backend {persistence['backend']!r}")
        forbidden = ("password=", "token =", "secret =", "TrustServerCertificate=true")
        for needle in forbidden:
            if needle.lower() in config_text.lower():
                fail(f"{product}: tracked TOML contains unsafe credential/TLS text {needle!r}")

        readme = readme_path.read_text()
        for heading in REQUIRED_GUIDANCE:
            if f"## {heading}" not in readme:
                fail(f"{product}: README is missing '{heading}' guidance")
        if "../workload/Main.st" not in readme:
            fail(f"{product}: README does not bind the shared canonical workload")
        if "runtime.toml" not in readme or "```bash" not in readme:
            fail(f"{product}: README lacks executable configuration/run commands")

    verification = (ROOT / "docs/public/operate/openot-database-verification.md").read_text()
    for identity in (
        "openot_real_database_gate.sh",
        "openot_database_example_persists_same_real_st_workload_to_every_network_backend",
        "runtime_system_loss_and_placeholder_documents_round_trip_through_every_real_product",
    ):
        if identity not in verification:
            fail(f"verification page omits executable identity {identity}")
    print("OpenOT database example and documentation contract passed")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AssertionError, KeyError, OSError, tomllib.TOMLDecodeError) as error:
        print(f"OpenOT database example check failed: {error}", file=sys.stderr)
        raise SystemExit(1)
