#!/usr/bin/env python3
"""Structural contract for the shipped OpenOT database examples and docs."""

from __future__ import annotations

import pathlib
import sys
import tomllib


ROOT = pathlib.Path(__file__).resolve().parents[1]
EXAMPLES = ROOT / "examples" / "openot_database"
MULTI_PROGRAM_EXAMPLES = ROOT / "examples" / "openot_multi_program"
PRODUCTS = {
    "sqlite": "sqlite",
    "postgresql": "postgresql",
    "timescaledb": "timescaledb",
    "mysql": "mysql",
    "mariadb": "mysql",
    "sqlserver": "sqlserver",
    "influxdb3": "influxdb3",
}
EXPECTED_STORAGE = {
    "sqlite": ("sqlite", "path", "history/trust-logging.sqlite3"),
    "postgresql": ("postgresql", "schema", "trust_logging"),
    "timescaledb": ("timescaledb", "schema", "trust_logging"),
    "mysql": ("mysql", "database", "trust_logging"),
    "mariadb": ("mysql", "database", "trust_logging"),
    "sqlserver": ("sqlserver", "schema", "trust_logging"),
    "influxdb3": ("influxdb3", "database", "trust_logging"),
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
    persistence_spec_normalized = " ".join(persistence_spec.split())
    for contract in (
        "exactly one product schema generation: `1`",
        "MUST NOT contain or advertise legacy schema migrations",
        "fail closed before consuming documents or changing stored state",
    ):
        if contract not in persistence_spec_normalized:
            fail(f"database persistence specification omits initial-schema contract {contract!r}")

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
        section, key, expected_value = EXPECTED_STORAGE[product]
        configured_value = persistence[section][key]
        if configured_value != expected_value:
            fail(
                f"{product}: public {key} must be descriptive {expected_value!r}, "
                f"got {configured_value!r}"
            )
        if product == "influxdb3":
            expected_spool = "history/trust-logging-influx-spool.sqlite3"
            if persistence[section]["spool_path"] != expected_spool:
                fail(f"influxdb3: spool path must be {expected_spool!r}")
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

    multi_program_configs = {
        "sqlite": MULTI_PROGRAM_EXAMPLES / "runtime.toml",
        **{
            product: MULTI_PROGRAM_EXAMPLES / f"runtime.{product}.toml"
            for product in PRODUCTS
            if product != "sqlite"
        },
    }
    for product, config_path in multi_program_configs.items():
        persistence = tomllib.loads(config_path.read_text())["runtime"]["openot"]["persistence"]
        section, key, expected_value = EXPECTED_STORAGE[product]
        configured_value = persistence[section][key]
        if configured_value != expected_value:
            fail(
                f"openot_multi_program/{config_path.name}: public {key} must be "
                f"descriptive {expected_value!r}, got {configured_value!r}"
            )
        if product == "influxdb3":
            expected_spool = "history/trust-logging-influx-spool.sqlite3"
            if persistence[section]["spool_path"] != expected_spool:
                fail(
                    f"openot_multi_program/{config_path.name}: spool path must be "
                    f"{expected_spool!r}"
                )

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
        "Every backend and the InfluxDB durable spool use the same initial schema generation, 1",
        "does not migrate pre-release development databases",
        "documented public objects or a documented export boundary",
        "Canonical JSON remains an internal recovery authority",
    ):
        if contract not in operator_guide_normalized:
            fail(f"operator guide omits public schema/query contract {contract!r}")

    gate = (ROOT / "scripts/openot_real_database_gate.sh").read_text()
    if "evidence directory must be empty" not in gate.lower():
        fail("real-database gate does not reject a stale evidence directory")
    if '("schema_generation", "PRAGMA user_version")' not in gate:
        fail("real-database evidence must describe the shared value as schema_generation")
    if '("schema_version", "PRAGMA user_version")' in gate:
        fail("real-database evidence must not expose the shared value as schema_version")
    for artifact in (
        "sqlite-artifact",
        "runtime-configs",
        "trust-logging.sqlite3",
        "openot-definition.json",
        "openot-coverage-manifest.json",
        "Main.st",
        "evidence-sha256.txt",
    ):
        if artifact not in gate:
            fail(f"real-database gate omits required retained artifact {artifact}")

    runner = (ROOT / "scripts/openot_database_runner_prepare.sh").read_text()
    for legacy_storage_name in (
        "POSTGRES_DB=openot",
        "MYSQL_DATABASE=openot",
        "MARIADB_DATABASE=openot",
        "db=openot",
        "/openot?sslmode",
        "/openot\\n",
    ):
        if legacy_storage_name in runner:
            fail(f"real-database runner still provisions ambiguous storage name {legacy_storage_name!r}")
    print("OpenOT database example and documentation contract passed")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AssertionError, KeyError, OSError, tomllib.TOMLDecodeError) as error:
        print(f"OpenOT database example check failed: {error}", file=sys.stderr)
        raise SystemExit(1)
