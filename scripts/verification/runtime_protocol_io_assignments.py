"""Generate reviewed runtime protocol/I/O assertion assignments from semantic rules."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any


BehaviorRef = tuple[str, int]
UNRESOLVED_REASON = "assertion_oracle_unresolved"
NONPRODUCT_REASON = "not_catalogable_product_oracle"
NONPRODUCT_TESTS = {
    "generator_rejects_reserved_words_as_generated_identifiers",
    "ads_setup_page_assets_are_served_without_runtime_chooser",
    "opcua_client_subscription_api_surface_is_available_for_phase3_worker",
    "broker_lifetime_outlasts_test_phases",
}


def generate_assignments(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> tuple[dict[str, Any], dict[str, Any]]:
    inventories = inventory_document.get("inventories")
    if not isinstance(inventories, Mapping):
        raise ValueError("assertion inventory document lacks inventories")
    invariants = _load_invariants(root)
    assignment_rows = []
    classifications = []
    seen_names: set[str] = set()
    for discovery_id, raw in sorted(inventories.items()):
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        path = _string(raw, "path", str(discovery_id))
        name = _string(raw, "test_name", str(discovery_id))
        if name in seen_names:
            raise ValueError(f"duplicate selected test name {name}")
        seen_names.add(name)
        unresolved = raw.get("unresolved_assertion_helpers")
        if not isinstance(unresolved, Sequence) or isinstance(
            unresolved, (str, bytes)
        ):
            raise ValueError(f"{discovery_id} unresolved helper list is invalid")
        if unresolved or name in NONPRODUCT_TESTS:
            classifications.append(
                {
                    "path": path,
                    "name": name,
                    "reason_code": (
                        UNRESOLVED_REASON if unresolved else NONPRODUCT_REASON
                    ),
                }
            )
            continue
        assertions = raw.get("assertions")
        if (
            not isinstance(assertions, Sequence)
            or isinstance(assertions, (str, bytes))
            or not assertions
        ):
            raise ValueError(f"{discovery_id} mappable test lacks assertions")
        default = _default_behavior(path, name)
        assertion_rows = []
        setup_ids = []
        non_oracle_ids = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{discovery_id} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            expression = _string(assertion, "expression", assertion_id)
            if _is_setup(expression):
                setup_ids.append(assertion_id)
                behavior = default
            elif _is_non_oracle(expression):
                non_oracle_ids.append(assertion_id)
                behavior = default
            else:
                behavior = _behavior_override(
                    path=path,
                    name=name,
                    expression=expression,
                    default=default,
                )
            _validate_behavior(invariants, behavior, name)
            assertion_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": behavior[0],
                    "behavior_index": behavior[1],
                }
            )
        if len(setup_ids) + len(non_oracle_ids) == len(assertion_rows):
            raise ValueError(f"{name} has no product-oracle assertion")
        assignment_rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assertion_rows,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": non_oracle_ids,
            }
        )
    if len(assignment_rows) + len(classifications) != len(inventories):
        raise ValueError("runtime protocol/I/O partition is not exhaustive")
    return (
        {
            "schema_version": 3,
            "basis": "reviewed_semantic_test_and_assertion_rules",
            "assignments": assignment_rows,
        },
        {
            "schema_version": 1,
            "basis": "reviewed_assertion_oracle_limitations",
            "classifications": classifications,
        },
    )


def _default_behavior(path: str, name: str) -> BehaviorRef:
    if path.endswith("/apply/validation.rs"):
        return (
            ("RT_COMM_APPLY_MUTATION_001", 0)
            if name.startswith("ethercat_")
            else ("RT_COMM_APPLY_MUTATION_001", 2)
        )
    if path.endswith("/browse_symbols.rs"):
        if "timeout" in name:
            return ("RT_ADS_ERROR_RECOVERY_001", 1)
        return ("RT_COMM_DISCOVERY_BROWSE_001", 4)
    if path.endswith("/comm_handlers/contract.rs"):
        return ("RT_COMM_SCHEMA_CAPABILITY_001", 0)
    if path.endswith("/discover_tests.rs"):
        rules = (
            ("requires_runtime_origin", ("RT_COMM_DISCOVERY_BROWSE_001", 0)),
            ("known_unimplemented", ("RT_COMM_DISCOVERY_BROWSE_001", 1)),
            ("is_server_only", ("RT_COMM_DISCOVERY_BROWSE_001", 1)),
            ("large_cidr", ("RT_COMM_DISCOVERY_BROWSE_001", 2)),
            ("unknown_protocol", ("RT_COMM_DISCOVERY_BROWSE_001", 3)),
            ("modbus_discovery_reports_tcp", ("PROTO_DISCOVERY_TRUTH_001", 2)),
            ("targeted_mqtt", ("PROTO_DISCOVERY_TRUTH_001", 6)),
            ("runtime_ethercat", ("PROTO_ETHERCAT_001", 1)),
        )
        return _match_name(name, rules)
    if path.endswith("/discovery_probe.rs"):
        if "safe_read" in name:
            return ("PROTO_DISCOVERY_TRUTH_001", 1)
        rules = (
            ("device_id", ("PROTO_DISCOVERY_TRUTH_001", 0)),
            ("auth_rejected", ("PROTO_DISCOVERY_TRUTH_001", 4)),
            ("clean_session", ("PROTO_DISCOVERY_TRUTH_001", 3)),
        )
        return _match_name(name, rules)
    if path.endswith("/comm_handlers/probe.rs"):
        return ("RT_COMM_DISCOVERY_BROWSE_001", 5)
    if path.endswith("/comm_handlers/schema.rs"):
        return (
            ("RT_COMM_SCHEMA_CAPABILITY_001", 3)
            if "schema_v4" in name
            else ("RT_COMM_SCHEMA_CAPABILITY_001", 0)
        )
    if path.endswith("/ads_product_surfaces.rs"):
        return ("RT_ADS_GENERATED_PRODUCT_SURFACE_001", 0)
    if path.endswith("/onboarding/errors.rs"):
        return ("RT_ADS_ERROR_RECOVERY_001", 0)
    if path.endswith("/ads/server/tests.rs"):
        rules = (
            ("descriptor_for_array", ("RT_ADS_SERVER_PUBLICATION_LIFECYCLE_001", 7)),
            ("marks_old_snapshot", ("PROTO_STATUS_TRUTH_001", 2)),
            ("reads_snapshot", ("RT_ADS_SERVER_PUBLICATION_LIFECYCLE_001", 1)),
            ("refresh_bumps", ("RT_ADS_SERVER_PUBLICATION_LIFECYCLE_001", 6)),
        )
        return _match_name(name, rules)
    if path.endswith("/host/ads/tests.rs"):
        if "offline_validation_rejects_stale" in name:
            return ("RT_ADS_CLI_AUTHORING_001", 1)
        if "offline_validation_accepts" in name:
            return ("RT_ADS_CLI_AUTHORING_001", 0)
        rules = (
            ("quality_name_collision", ("RT_ADS_SYMBOL_IMPORT_APPLY_001", 7)),
            ("byte_size_mismatch", ("RT_ADS_SYMBOL_IMPORT_APPLY_001", 8)),
            ("string_and_array", ("RT_ADS_SYMBOL_IMPORT_APPLY_001", 4)),
            ("mock_transport", ("RT_ADS_CLIENT_WORKER_CACHE_INTEGRITY_001", 0)),
            ("generated_ads_interface", ("RT_ADS_SYMBOL_IMPORT_APPLY_001", 0)),
            ("generates_deterministic", ("RT_ADS_SYMBOL_IMPORT_APPLY_001", 0)),
        )
        return _match_name(name, rules)
    if path.endswith("/host/discovery.rs"):
        return (
            ("RT_COMM_DISCOVERY_BROWSE_001", 7)
            if "service_removed" in name
            else ("RT_COMM_DISCOVERY_BROWSE_001", 6)
        )
    if path.endswith("/host/opcua/tests.rs"):
        rules = (
            ("maps_enum", ("RT_OPCUA_SERVER_SURFACE_001", 0)),
            ("maps_scalar", ("RT_OPCUA_SERVER_SURFACE_001", 0)),
            ("rejects_invalid_security", ("RT_OPCUA_SERVER_SURFACE_001", 3)),
            ("rejects_non_scalar", ("RT_OPCUA_SERVER_SURFACE_001", 1)),
            ("secure_profile_defaults", ("RT_OPCUA_SERVER_SURFACE_001", 2)),
        )
        return _match_name(name, rules)
    if path.endswith("/io/ethercat/tests.rs"):
        rules = (
            ("config_", ("RT_ETHERCAT_DRIVER_001", 0)),
            ("mock_reads", ("PROTO_ETHERCAT_001", 1)),
            ("fault_policy", ("RT_ETHERCAT_DRIVER_001", 2)),
            ("warn_policy", ("RT_ETHERCAT_DRIVER_001", 3)),
            ("hardware_open", ("PROTO_ETHERCAT_001", 0)),
        )
        return _match_name(name, rules)
    if path.endswith("/io/gpio.rs"):
        if "failure" in name:
            return ("RT_GPIO_DRIVER_001", 2)
        if "rejects_" in name:
            return ("RT_GPIO_DRIVER_001", 1)
        return ("RT_GPIO_DRIVER_001", 0)
    if path.endswith("/io/interface.rs"):
        rules = (
            ("over_reads", ("RT_PROCESS_IMAGE_CONTRACT_001", 2)),
            ("above_", ("RT_PROCESS_IMAGE_CONTRACT_001", 1)),
            ("snapshot_", ("RT_PROCESS_IMAGE_CONTRACT_001", 6)),
            ("string_", ("RT_PROCESS_IMAGE_CONTRACT_001", 3)),
        )
        return _match_name(name, rules)
    if "/io/mqtt/tests" in path:
        rules = (
            ("safe_state", ("RT_MQTT_DRIVER_001", 11)),
            ("fail_closed", ("RT_MQTT_DRIVER_001", 6)),
            ("drop_is_bounded", ("RT_MQTT_DRIVER_001", 10)),
            ("reconnect_backoff", ("RT_MQTT_DRIVER_001", 9)),
            ("retries_after", ("RT_MQTT_DRIVER_001", 8)),
            ("on_error_warn", ("RT_MQTT_DRIVER_001", 7)),
            ("allows_", ("RT_MQTT_DRIVER_001", 2)),
            ("mqtts_scheme", ("RT_MQTT_DRIVER_001", 2)),
            ("builds_rumqttc", ("RT_MQTT_DRIVER_001", 2)),
            ("security_test_rejects", ("RT_MQTT_DRIVER_001", 3)),
            ("sparkplug_payload", ("RT_MQTT_DRIVER_001", 4)),
            ("sparkplug_profile_configures", ("RT_MQTT_DRIVER_001", 4)),
            ("publishes_birth", ("RT_MQTT_DRIVER_001", 4)),
            ("rejects_unsupported", ("RT_MQTT_DRIVER_001", 5)),
            ("typed_point_map_rejects", ("RT_MQTT_DRIVER_001", 1)),
            ("typed_point_map_writes", ("RT_MQTT_DRIVER_001", 0)),
        )
        return _match_name(name, rules)
    if path.endswith("/io/registry.rs"):
        return (
            ("RT_IO_CYCLE_BINDING_001", 6)
            if "alias_" in name
            else ("RT_IO_CYCLE_BINDING_001", 5)
        )
    if path.endswith("/runtime/ads_subsystem.rs"):
        return (
            ("RT_ADS_DIAGNOSTIC_REPORT_TRUTH_001", 2)
            if "config_hash" in name
            else ("PROTO_STATUS_TRUTH_001", 0)
        )
    if path.endswith("/web/ads_routes.rs"):
        return (
            ("RT_ADS_ONBOARDING_ROUTE_SECURITY_001", 0)
            if "route_add_channel" in name
            else ("RT_ADS_ONBOARDING_ROUTE_SECURITY_001", 1)
        )
    if path.endswith("/tests/ads_web_api.rs"):
        if "internal_token" in name:
            return ("RT_PROTOCOL_WEB_CONTROL_BRIDGE_001", 0)
        if "does_not_forward" in name:
            return ("RT_PROTOCOL_WEB_CONTROL_BRIDGE_001", 1)
        if "route_add" in name:
            return ("RT_ADS_ONBOARDING_ROUTE_SECURITY_001", 0)
        if "server_web_status" in name:
            return ("RT_ADS_SERVER_PUBLICATION_LIFECYCLE_001", 4)
        if "status_and_import" in name:
            return ("RT_CONTROL_REQUEST_JOB_001", 2)
        return ("RT_CONTROL_REQUEST_JOB_001", 5)
    if path.endswith("/tests/connectors_status.rs"):
        rules = (
            ("serializes_stable_schema", ("RT_CONNECTOR_REPORT_PROJECTION_001", 0)),
            ("discovery_confidence", ("RT_CONNECTOR_REPORT_PROJECTION_001", 7)),
            ("io_driver_health", ("RT_CONNECTOR_REPORT_PROJECTION_001", 2)),
            ("process_image_protocol", ("RT_CONNECTOR_REPORT_PROJECTION_001", 6)),
            ("opcua_mapping", ("PROTO_STATUS_TRUTH_001", 1)),
            ("ads_state_mapping", ("PROTO_STATUS_TRUTH_001", 0)),
            ("ads_status_report_projects_reconnect", ("PROTO_STATUS_TRUTH_001", 0)),
            ("ads_point_statuses", ("RT_CONNECTOR_REPORT_PROJECTION_001", 5)),
            ("role_endpoint", ("RT_CONNECTOR_REPORT_PROJECTION_001", 0)),
            ("active_ads", ("RT_CONNECTOR_REPORT_PROJECTION_001", 5)),
            ("opcua_client_status", ("RT_CONNECTOR_REPORT_PROJECTION_001", 5)),
            ("stale_connector", ("RT_CONNECTOR_REPORT_PROJECTION_001", 5)),
        )
        return _match_name(name, rules)
    if path.endswith("/tests/ethercat_driver.rs"):
        if "image_size_mismatch" in name:
            return ("RT_ETHERCAT_DRIVER_001", 4)
        rules = (
            ("cycle_warn_threshold", ("RT_ETHERCAT_DRIVER_001", 1)),
            ("ignore_policy", ("RT_ETHERCAT_DRIVER_001", 3)),
            ("warn_policy", ("RT_ETHERCAT_DRIVER_001", 3)),
            ("missing_adapter", ("PROTO_ETHERCAT_001", 0)),
            ("mock_profile", ("PROTO_ETHERCAT_001", 1)),
        )
        return _match_name(name, rules)
    if path.endswith("/tests/io_address.rs"):
        return ("RT_IO_TYPED_ROUNDTRIP_001", 0)
    if path.endswith("/tests/io_cycle.rs"):
        return ("RT_IO_CYCLE_BINDING_001", 0)
    if path.endswith("/tests/io_driver.rs"):
        return (
            ("RT_IO_CYCLE_BINDING_001", 1)
            if "composed_" in name
            else ("RT_IO_CYCLE_BINDING_001", 0)
        )
    if path.endswith("/tests/io_fb_vars.rs"):
        return (
            ("RT_IO_CYCLE_BINDING_001", 3)
            if "wildcard_" in name
            else ("RT_IO_CYCLE_BINDING_001", 2)
        )
    if path.endswith("/tests/io_multidriver_live.rs"):
        return ("RT_IO_CYCLE_BINDING_001", 1)
    if path.endswith("/tests/io_struct_array.rs"):
        return ("RT_IO_CYCLE_BINDING_001", 2)
    if path.endswith("/tests/io_wildcard.rs"):
        return (
            ("RT_IO_CYCLE_BINDING_001", 3)
            if "requires_var_config" in name
            else ("RT_IO_CYCLE_BINDING_001", 4)
        )
    if path.endswith("/tests/modbus_driver.rs"):
        rules = (
            ("drop_is_bounded", ("RT_MODBUS_DRIVER_001", 9)),
            ("ignore_policy", ("RT_MODBUS_DRIVER_001", 5)),
            ("warn_policy", ("RT_MODBUS_DRIVER_001", 5)),
            ("write_failure", ("RT_MODBUS_DRIVER_001", 4)),
            ("exception_", ("RT_MODBUS_DRIVER_001", 6)),
            ("explicit_", ("RT_MODBUS_DRIVER_001", 0)),
            ("point_map_reads", ("RT_MODBUS_DRIVER_001", 1)),
            ("point_map_writes", ("RT_MODBUS_DRIVER_001", 1)),
            ("point_map_rejects", ("RT_MODBUS_DRIVER_001", 3)),
            ("reconnect_backoff", ("RT_MODBUS_DRIVER_001", 8)),
            ("rejects_unknown", ("RT_MODBUS_DRIVER_001", 2)),
        )
        return _match_name(name, rules)
    if path.endswith("/tests/process_image.rs"):
        rules = (
            ("invalid_metadata", ("RT_PROCESS_IMAGE_CONTRACT_001", 4)),
            ("above_process_image_cap", ("RT_PROCESS_IMAGE_CONTRACT_001", 1)),
            ("accepts_single_legacy", ("RT_PROCESS_IMAGE_CONTRACT_001", 5)),
            ("rejects_unknown", ("RT_PROCESS_IMAGE_CONTRACT_001", 4)),
            ("sized_from_metadata", ("RT_PROCESS_IMAGE_CONTRACT_001", 0)),
        )
        return _match_name(name, rules)
    raise ValueError(f"no reviewed runtime protocol/I/O rule for {path}:{name}")


def _behavior_override(
    *,
    path: str,
    name: str,
    expression: str,
    default: BehaviorRef,
) -> BehaviorRef:
    if name == "io_driver_health_mapping_honors_error_policy":
        if "warn_projection" in expression:
            return ("RT_CONNECTOR_REPORT_PROJECTION_001", 4)
        if "faulted" in expression:
            return ("RT_CONNECTOR_REPORT_PROJECTION_001", 3)
        if "degraded" in expression:
            return ("RT_CONNECTOR_REPORT_PROJECTION_001", 2)
        return ("RT_CONNECTOR_REPORT_PROJECTION_001", 1)
    if name == "bit_and_word_access":
        if 'IoAddress :: parse ("%IX1.2")' in expression or 'IoAddress :: parse ("%QW0")' in expression:
            return ("RT_DIRECT_ADDRESS_PARSER_001", 0)
        if "word" in expression.lower() or "0x1234" in expression:
            return ("RT_IO_TYPED_ROUNDTRIP_001", 1)
    if name == "mock_transport_resolves_reads_writes_and_tracks_symbol_version":
        if "symbol_version" in expression or "reconnect" in expression:
            return ("RT_ADS_CLIENT_WORKER_CACHE_INTEGRITY_001", 2)
    if name == "ads_status_report_projects_role_endpoint_and_point_counts":
        if any(token in expression for token in (". state", ". health")):
            return ("PROTO_STATUS_TRUTH_001", 0)
        if "point_counts" in expression:
            return ("RT_CONNECTOR_REPORT_PROJECTION_001", 5)
    if name == "active_ads_device_snapshot_projects_point_rows_and_counts":
        if any(token in expression for token in ('["state"]', '["health"]')):
            return ("PROTO_STATUS_TRUTH_001", 0)
        if "stale" in expression:
            return ("PROTO_STATUS_TRUTH_001", 2)
    if name == "opcua_client_status_projects_point_quality_and_metadata":
        if any(token in expression for token in ('["state"]', '["health"]')):
            return ("PROTO_STATUS_TRUTH_001", 1)
        if "stale" in expression:
            return ("PROTO_STATUS_TRUTH_001", 2)
    if name == "ads_web_doctor_job_start_and_status_route_to_control_job_store":
        if "wire support" in expression:
            return ("RT_CONTROL_REQUEST_JOB_001", 4)
    return default


def _is_setup(expression: str) -> bool:
    setup_tokens = (
        "TcpListener :: bind",
        "listener . local_addr",
        "listener . accept",
        "state . lock () . expect",
        'toml :: from_str (& format !',
        ':: from_params (& params) . expect ("driver")',
    )
    return any(token in expression for token in setup_tokens)


def _is_non_oracle(expression: str) -> bool:
    return "unreachable !" in expression


def _match_name(
    name: str,
    rules: Sequence[tuple[str, BehaviorRef]],
) -> BehaviorRef:
    matches = [behavior for token, behavior in rules if token in name]
    if len(matches) != 1:
        raise ValueError(f"reviewed name rule matched {len(matches)} times: {name}")
    return matches[0]


def _load_invariants(root: Path) -> dict[str, Mapping[str, Any]]:
    result = {}
    for path in (root / "verification/invariants").rglob("*.toml"):
        record = tomllib.loads(path.read_text())
        invariant_id = record.get("id")
        if isinstance(invariant_id, str):
            result[invariant_id] = record
    return result


def _validate_behavior(
    invariants: Mapping[str, Mapping[str, Any]],
    behavior: BehaviorRef,
    name: str,
) -> None:
    invariant_id, index = behavior
    invariant = invariants.get(invariant_id)
    rows = invariant.get("behavior") if isinstance(invariant, Mapping) else None
    if (
        not isinstance(rows, Sequence)
        or isinstance(rows, (str, bytes))
        or index < 0
        or index >= len(rows)
    ):
        raise ValueError(f"{name} selects absent behavior {invariant_id}#{index}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--inventory", type=Path, required=True)
    parser.add_argument("--assignments-out", type=Path, required=True)
    parser.add_argument("--nonmappings-out", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        inventory_path = args.inventory if args.inventory.is_absolute() else root / args.inventory
        document = json.loads(inventory_path.read_text())
        if not isinstance(document, Mapping):
            raise ValueError("inventory must be a JSON object")
        assignments, nonmappings = generate_assignments(
            root=root,
            inventory_document=document,
        )
        for destination, payload in (
            (args.assignments_out, assignments),
            (args.nonmappings_out, nonmappings),
        ):
            path = destination if destination.is_absolute() else root / destination
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
        print(
            "runtime protocol/I/O assignments generated: "
            f"{len(assignments['assignments'])} mapped, "
            f"{len(nonmappings['classifications'])} explicit nonmapping"
        )
        return 0
    except (OSError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        print(f"runtime protocol/I/O assignment generation failed: {error}")
        return 2


def _string(row: Mapping[str, Any], field: str, label: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


if __name__ == "__main__":
    raise SystemExit(main())
