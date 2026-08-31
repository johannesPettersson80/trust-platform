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
    persistence_spec = (ROOT / "docs" / "specs" / "33-openot-database-persistence.md").read_text()
    for contract in (
        "SQLite schema version 4",
        "explicit common provenance columns",
        "preserves `logging_records` and `logging_checkpoint`",
    ):
        if contract not in persistence_spec:
            fail(f"database persistence specification omits migration contract {contract!r}")

    persistence_sources = ROOT / "crates" / "trust-runtime" / "src" / "host" / "openot_persistence"
    for source_path in persistence_sources.rglob("*.rs"):
        source_text = source_path.read_text()
        for unix_only in ('"/tmp/', '"unix://'):
            if unix_only in source_text:
                fail(
                    f"{source_path.relative_to(ROOT)}: production persistence code contains "
                    f"Unix-only path text {unix_only!r}"
                )

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
        control_endpoint = config["runtime"]["control"]["endpoint"]
        if not control_endpoint.startswith("tcp://"):
            fail(
                f"{product}: control endpoint must be portable TCP, got {control_endpoint!r}"
            )
        if config["runtime"]["control"].get("auth_token") != "openot-example-local-token":
            fail(f"{product}: portable TCP control endpoint requires the documented local-only token")
        persistence = config["runtime"]["openot"]["persistence"]
        if persistence["backend"] != expected_backend:
            fail(f"{product}: unexpected TOML backend {persistence['backend']!r}")
        forbidden = ("password=", "secret =", "TrustServerCertificate=true")
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

    operator_guide = (ROOT / "docs/public/operate/openot-database-persistence.md").read_text()
    operator_guide_normalized = " ".join(operator_guide.split())
    for contract in (
        "SQLite uses schema version 4",
        "documented public objects or a documented export boundary",
        "Canonical JSON remains an internal recovery authority",
    ):
        if contract not in operator_guide_normalized:
            fail(f"operator guide omits public migration/query contract {contract!r}")

    gate = (ROOT / "scripts/openot_real_database_gate.sh").read_text()
    if "evidence directory must be empty" not in gate.lower():
        fail("real-database gate does not reject a stale evidence directory")
    for artifact in (
        "sqlite-artifact",
        "runtime-configs",
        "openot.sqlite3",
        "openot-definition.json",
        "openot-coverage-manifest.json",
        "Main.st",
        "evidence-sha256.txt",
    ):
        if artifact not in gate:
            fail(f"real-database gate omits required retained artifact {artifact}")
    print("OpenOT database example and documentation contract passed")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AssertionError, KeyError, OSError, tomllib.TOMLDecodeError) as error:
        print(f"OpenOT database example check failed: {error}", file=sys.stderr)
        raise SystemExit(1)
