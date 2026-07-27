"""Closed target profiles for frozen Rust nextest catalog batches."""

from __future__ import annotations

import hashlib
import json
import re
import shlex
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

from .coverage_trace_filter import exact_name_matcher


ADS_WIRE_DISABLED_CFG = '#[cfg(not(feature = "ads-wire"))]'
ADS_SERVER_CFG = '#[cfg(feature = "ads-server")]'
MESSAGE_FORMAT_VERSION = "0.1"
RUNTIME_CONTROL_PROFILE_KEY = "runtime-control-lib"
RUNTIME_REMAINING_PROFILE_KEY = "runtime-remaining"
HIR_SEMANTIC_CONTROL_FLOW_PROFILE_KEY = "hir-semantic-control-flow"
HIR_REMAINING_PROFILE_KEY = "hir-remaining"
SYNTAX_REMAINING_PROFILE_KEY = "syntax-remaining"
DEBUG_REMAINING_PROFILE_KEY = "debug-remaining"
RUNTIME_CORE_REMAINING_PROFILE_KEY = "runtime-core-remaining"
ADS_SERVER_REMAINING_PROFILE_KEY = "ads-server-remaining"
TERMINAL_EVENTS = frozenset({"ok", "failed", "ignored", "timeout"})


@dataclass(frozen=True)
class FrozenRustTargetProfile:
    key: str
    package: str
    target_kind: str
    target_name: str | None
    binary_id: str
    test_prefix: str
    encoded_test_name_prefix: str
    discovery_source_kind: str
    allowed_cfg_attributes: frozenset[str]
    no_default_features_cfg: str | None
    no_default_features_shard_name: str | None

    @property
    def cargo_target_args(self) -> tuple[str, ...]:
        if self.target_kind == "lib":
            return ("--lib",)
        if self.target_kind == "test" and self.target_name:
            return ("--test", self.target_name)
        raise ValueError(f"{self.key} has an invalid Cargo target")


@dataclass(frozen=True)
class TestShard:
    name: str
    full_names: tuple[str, ...]
    no_default_features: bool


@dataclass(frozen=True)
class FrozenSelection:
    path: str
    reviewed_nonmapping_count: int
    catalog_mapped_count: int
    names: tuple[str, ...]
    full_names: tuple[str, ...]


RUNTIME_CONTROL_PROFILE = FrozenRustTargetProfile(
    key=RUNTIME_CONTROL_PROFILE_KEY,
    package="trust-runtime",
    target_kind="lib",
    target_name=None,
    binary_id="trust-runtime",
    test_prefix="control::tests::",
    encoded_test_name_prefix="trust-runtime::trust_runtime$",
    discovery_source_kind="rust_unit_test",
    allowed_cfg_attributes=frozenset(
        {ADS_WIRE_DISABLED_CFG, ADS_SERVER_CFG}
    ),
    no_default_features_cfg=ADS_WIRE_DISABLED_CFG,
    no_default_features_shard_name="no-ads-wire",
)

HIR_SEMANTIC_CONTROL_FLOW_PROFILE = FrozenRustTargetProfile(
    key=HIR_SEMANTIC_CONTROL_FLOW_PROFILE_KEY,
    package="trust-hir",
    target_kind="test",
    target_name="semantic_type_checking",
    binary_id="trust-hir::semantic_type_checking",
    test_prefix="control_flow_and_calls::",
    encoded_test_name_prefix="trust-hir::semantic_type_checking$",
    discovery_source_kind="rust_integration_test",
    allowed_cfg_attributes=frozenset(),
    no_default_features_cfg=None,
    no_default_features_shard_name=None,
)

HIR_REMAINING_PROFILE = FrozenRustTargetProfile(
    key=HIR_REMAINING_PROFILE_KEY,
    package="trust-hir",
    target_kind="lib",
    target_name=None,
    binary_id="trust-hir",
    test_prefix="",
    encoded_test_name_prefix="trust-hir::trust_hir$",
    discovery_source_kind="rust_unit_test",
    allowed_cfg_attributes=frozenset(),
    no_default_features_cfg=None,
    no_default_features_shard_name=None,
)

SYNTAX_REMAINING_PROFILE = FrozenRustTargetProfile(
    key=SYNTAX_REMAINING_PROFILE_KEY,
    package="trust-syntax",
    target_kind="test",
    target_name=None,
    binary_id="trust-syntax",
    test_prefix="",
    encoded_test_name_prefix="trust-syntax::",
    discovery_source_kind="rust_integration_test",
    allowed_cfg_attributes=frozenset(),
    no_default_features_cfg=None,
    no_default_features_shard_name=None,
)

DEBUG_REMAINING_PROFILE = FrozenRustTargetProfile(
    key=DEBUG_REMAINING_PROFILE_KEY,
    package="trust-debug",
    target_kind="lib",
    target_name=None,
    binary_id="trust-debug",
    test_prefix="",
    encoded_test_name_prefix="trust-debug::trust_debug$",
    discovery_source_kind="rust_unit_test",
    allowed_cfg_attributes=frozenset(),
    no_default_features_cfg=None,
    no_default_features_shard_name=None,
)

RUNTIME_CORE_REMAINING_PROFILE = FrozenRustTargetProfile(
    key=RUNTIME_CORE_REMAINING_PROFILE_KEY,
    package="trust-runtime-core",
    target_kind="lib",
    target_name=None,
    binary_id="trust-runtime-core",
    test_prefix="",
    encoded_test_name_prefix="trust-runtime-core::trust_runtime_core$",
    discovery_source_kind="rust_unit_test",
    allowed_cfg_attributes=frozenset(),
    no_default_features_cfg=None,
    no_default_features_shard_name=None,
)

ADS_SERVER_REMAINING_PROFILE = FrozenRustTargetProfile(
    key=ADS_SERVER_REMAINING_PROFILE_KEY,
    package="trust-ads-server",
    target_kind="lib",
    target_name=None,
    binary_id="trust-ads-server",
    test_prefix="",
    encoded_test_name_prefix="trust-ads-server::trust_ads_server$",
    discovery_source_kind="rust_unit_test",
    allowed_cfg_attributes=frozenset(),
    no_default_features_cfg=None,
    no_default_features_shard_name=None,
)

ADS_SERVER_REMAINING_UNIT_PREFIXES: Mapping[str, str] = {
    "crates/trust-ads-server/src/ams.rs": "ams::tests::",
}

RUNTIME_REMAINING_LIB_PROFILE = FrozenRustTargetProfile(
    key=RUNTIME_REMAINING_PROFILE_KEY,
    package="trust-runtime",
    target_kind="lib",
    target_name=None,
    binary_id="trust-runtime",
    test_prefix="",
    encoded_test_name_prefix="trust-runtime::trust_runtime$",
    discovery_source_kind="rust_unit_test",
    allowed_cfg_attributes=frozenset(),
    no_default_features_cfg=None,
    no_default_features_shard_name=None,
)

RUNTIME_REMAINING_INTEGRATION_PATHS = frozenset(
    {
        "crates/trust-runtime/tests/bounded_value_semantics.rs",
        "crates/trust-runtime/tests/bytecode_container.rs",
        "crates/trust-runtime/tests/bytecode_decode_resource_bounds.rs",
        "crates/trust-runtime/tests/bytecode_metadata.rs",
        "crates/trust-runtime/tests/bytecode_roundtrip.rs",
        "crates/trust-runtime/tests/bytecode_sections.rs",
        "crates/trust-runtime/tests/bytecode_validation.rs",
        "crates/trust-runtime/tests/openot_telemetry.rs",
        "crates/trust-runtime/tests/retain_integrity.rs",
        "crates/trust-runtime/tests/retain_store.rs",
        "crates/trust-runtime/tests/runtime_safety_fail_closed.rs",
        "crates/trust-runtime/tests/stdlib_select.rs",
        "crates/trust-runtime/tests/vars_retain.rs",
    }
)

RUNTIME_REMAINING_UNIT_PREFIXES: Mapping[str, str] = {
    "crates/trust-runtime/src/bytecode/validate/resource_limits.rs": (
        "bytecode::validate::resource_limits::tests::"
    ),
    "crates/trust-runtime/src/memory/tests.rs": "memory::tests::",
    "crates/trust-runtime/src/host/mesh/tests.rs": "mesh::tests::",
    "crates/trust-runtime/src/openot_authoring/tests.rs": (
        "openot_authoring::tests::"
    ),
    "crates/trust-runtime/src/retain/store_tests.rs": (
        "retain::"
    ),
    "crates/trust-runtime/src/scheduler/runner_loop.rs": (
        "scheduler::runner_loop_poison_tests::"
    ),
}

RUNTIME_REMAINING_NESTED_INTEGRATION_PREFIXES: Mapping[
    str, tuple[str, str]
] = {
    (
        "crates/trust-runtime/tests/bytecode_encoder/"
        "bytecode_encoder_part_01.rs"
    ): ("bytecode_encoder", "bytecode_encoder_part_01::"),
    (
        "crates/trust-runtime/tests/bytecode_encoder/"
        "bytecode_encoder_part_02.rs"
    ): ("bytecode_encoder", "bytecode_encoder_part_02::"),
    (
        "crates/trust-runtime/tests/bytecode_encoder/"
        "bytecode_encoder_part_03.rs"
    ): ("bytecode_encoder", "bytecode_encoder_part_03::"),
    (
        "crates/trust-runtime/tests/bytecode_encoder/"
        "bytecode_encoder_part_04.rs"
    ): ("bytecode_encoder", "bytecode_encoder_part_04::"),
    (
        "crates/trust-runtime/tests/bytecode_encoder/"
        "bytecode_encoder_part_05.rs"
    ): ("bytecode_encoder", "bytecode_encoder_part_05::"),
}

HIR_REMAINING_INTEGRATION_PATHS = frozenset(
    {
        "crates/trust-hir/tests/declaration_catalog.rs",
        "crates/trust-hir/tests/generic_types.rs",
        "crates/trust-hir/tests/iec_table_coverage.rs",
        "crates/trust-hir/tests/namespaces.rs",
        "crates/trust-hir/tests/semantic_interfaces.rs",
        "crates/trust-hir/tests/semantic_name_resolution.rs",
        "crates/trust-hir/tests/semantic_parameters.rs",
        "crates/trust-hir/tests/semantic_scope.rs",
        "crates/trust-hir/tests/semantic_standard_functions.rs",
        "crates/trust-hir/tests/semantic_type_checking/array_wildcard_compatibility.rs",
        "crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs",
        "crates/trust-hir/tests/semantic_type_checking/bounded_value_semantics.rs",
        "crates/trust-hir/tests/semantic_type_checking/enum_unqualified_in_expressions.rs",
        "crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs",
        "crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs",
        "crates/trust-hir/tests/semantic_type_checking/pointer_param_write_through.rs",
        "crates/trust-hir/tests/semantic_type_checking/sizeof_semantics.rs",
        "crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs",
        "crates/trust-hir/tests/semantic_type_checking/types_and_references.rs",
        "crates/trust-hir/tests/semantic_type_checking/wrong_kind_resolution.rs",
        "crates/trust-hir/tests/semantic_type_definitions.rs",
        "crates/trust-hir/tests/var_sections.rs",
    }
)

HIR_REMAINING_UNIT_PREFIXES: Mapping[str, str] = {
    "crates/trust-hir/src/db/diagnostics/context.rs": (
        "db::diagnostics::context::tests::"
    ),
    "crates/trust-hir/src/db/diagnostics/oop/mod.rs": (
        "db::diagnostics::oop::tests::"
    ),
    "crates/trust-hir/src/db/diagnostics/tests.rs": "db::diagnostics::tests::",
    "crates/trust-hir/src/db/diagnostics/type_check.rs": (
        "db::diagnostics::type_check::tests::"
    ),
    "crates/trust-hir/src/db/queries/database/database_tests_part_01.rs": (
        "db::queries::database::tests::"
    ),
    "crates/trust-hir/src/db/queries/database/database_tests_part_02.rs": (
        "db::queries::database::tests::"
    ),
    "crates/trust-hir/src/db/queries/database/database_tests_part_03.rs": (
        "db::queries::database::tests::"
    ),
    "crates/trust-hir/src/diagnostics.rs": "diagnostics::tests::",
    "crates/trust-hir/src/ident.rs": "ident::tests::",
    "crates/trust-hir/src/openot_authoring.rs": "openot_authoring::tests::",
    "crates/trust-hir/src/project.rs": "project::tests::",
    "crates/trust-hir/src/semantic.rs": "semantic::tests::",
    "crates/trust-hir/src/symbols/table.rs": "symbols::table::tests::",
    "crates/trust-hir/src/type_check/compatibility.rs": (
        "type_check::compatibility::tests::"
    ),
    "crates/trust-hir/src/type_check/mod.rs": "type_check::tests::",
    "crates/trust-hir/src/types/defs.rs": "types::defs::tests::",
    "crates/trust-hir/src/types/registry.rs": "types::registry::tests::",
}

SYNTAX_REMAINING_INTEGRATION_PATHS = frozenset(
    {
        "crates/trust-syntax/tests/parser_complex.rs",
        "crates/trust-syntax/tests/parser_expressions.rs",
        "crates/trust-syntax/tests/parser_literals.rs",
        "crates/trust-syntax/tests/parser_pous.rs",
        "crates/trust-syntax/tests/parser_statements.rs",
        "crates/trust-syntax/tests/parser_types.rs",
        "crates/trust-syntax/tests/parser_variables.rs",
    }
)

DEBUG_REMAINING_UNIT_PREFIXES: Mapping[str, str] = {
    "crates/trust-debug/src/adapter/stop.rs": "adapter::stop::tests::",
    "crates/trust-debug/src/adapter/tests_part_01.rs": (
        "adapter::tests::tests_part_01::"
    ),
    "crates/trust-debug/src/adapter/tests_part_02.rs": (
        "adapter::tests::tests_part_02::"
    ),
    "crates/trust-debug/src/adapter/tests_part_03.rs": (
        "adapter::tests::tests_part_03::"
    ),
    "crates/trust-debug/src/adapter/tests_part_04.rs": (
        "adapter::tests::tests_part_04::"
    ),
    "crates/trust-debug/src/adapter/tests_part_05.rs": (
        "adapter::tests::tests_part_05::"
    ),
    "crates/trust-debug/src/adapter/tests_part_06.rs": (
        "adapter::tests::tests_part_06::"
    ),
    "crates/trust-debug/src/adapter/variables/format.rs": (
        "adapter::variables::format::tests::"
    ),
    "crates/trust-debug/src/protocol.rs": "protocol::tests::",
    "crates/trust-debug/src/session/tests.rs": "session::tests::",
}

RUNTIME_CORE_REMAINING_UNIT_PREFIXES: Mapping[str, str] = {
    "crates/trust-runtime-core/src/bytecode/mod.rs": "bytecode::tests::",
    "crates/trust-runtime-core/src/cycle.rs": "cycle::tests::",
    "crates/trust-runtime-core/src/datetime.rs": "datetime::tests::",
    "crates/trust-runtime-core/src/error_code.rs": "error_code::tests::",
    "crates/trust-runtime-core/src/memory.rs": "memory::tests::",
    "crates/trust-runtime-core/src/numeric.rs": "numeric::tests::",
    "crates/trust-runtime-core/src/program_model/expr.rs": (
        "program_model::expr::tests::"
    ),
    "crates/trust-runtime-core/src/program_model/initializers.rs": (
        "program_model::initializers::tests::"
    ),
    "crates/trust-runtime-core/src/program_model/ops.rs": (
        "program_model::ops::tests::"
    ),
    "crates/trust-runtime-core/src/program_model/util.rs": (
        "program_model::util::tests::"
    ),
    "crates/trust-runtime-core/src/retain.rs": "retain::tests::",
    "crates/trust-runtime-core/src/scheduler.rs": "scheduler::tests::",
    "crates/trust-runtime-core/src/task.rs": "task::tests::",
    "crates/trust-runtime-core/src/value/datetime.rs": (
        "value::datetime::tests::"
    ),
    "crates/trust-runtime-core/src/value/defaults.rs": (
        "value::defaults::tests::"
    ),
    "crates/trust-runtime-core/src/value/partial_access.rs": (
        "value::partial_access::tests::"
    ),
    "crates/trust-runtime-core/src/value/reference.rs": (
        "value::reference::tests::"
    ),
    "crates/trust-runtime-core/src/value/size.rs": "value::size::tests::",
    "crates/trust-runtime-core/src/value/types/tests.rs": (
        "value::types::tests::"
    ),
    "crates/trust-runtime-core/src/vm/mod.rs": "vm::tests::",
    "crates/trust-runtime-core/src/watchdog.rs": "watchdog::tests::",
}

TARGET_PROFILES: Mapping[str, FrozenRustTargetProfile] = {
    profile.key: profile
    for profile in (
        RUNTIME_CONTROL_PROFILE,
        HIR_SEMANTIC_CONTROL_FLOW_PROFILE,
        HIR_REMAINING_PROFILE,
        SYNTAX_REMAINING_PROFILE,
        DEBUG_REMAINING_PROFILE,
        RUNTIME_CORE_REMAINING_PROFILE,
        ADS_SERVER_REMAINING_PROFILE,
        RUNTIME_REMAINING_LIB_PROFILE,
    )
}


def get_target_profile(
    key: str,
    *,
    path: str | None = None,
) -> FrozenRustTargetProfile:
    """Return one explicitly supported frozen target profile."""

    if key == HIR_REMAINING_PROFILE_KEY:
        return _hir_remaining_profile(path)
    if key == SYNTAX_REMAINING_PROFILE_KEY:
        return _syntax_remaining_profile(path)
    if key == DEBUG_REMAINING_PROFILE_KEY:
        return _unit_path_profile(
            DEBUG_REMAINING_PROFILE,
            DEBUG_REMAINING_UNIT_PREFIXES,
            path,
            "debug",
        )
    if key == RUNTIME_CORE_REMAINING_PROFILE_KEY:
        return _unit_path_profile(
            RUNTIME_CORE_REMAINING_PROFILE,
            RUNTIME_CORE_REMAINING_UNIT_PREFIXES,
            path,
            "runtime-core",
        )
    if key == ADS_SERVER_REMAINING_PROFILE_KEY:
        return _unit_path_profile(
            ADS_SERVER_REMAINING_PROFILE,
            ADS_SERVER_REMAINING_UNIT_PREFIXES,
            path,
            "ads-server",
        )
    if key == RUNTIME_REMAINING_PROFILE_KEY:
        return _runtime_remaining_profile(path)
    try:
        return TARGET_PROFILES[key]
    except KeyError as error:
        supported = ", ".join(sorted(TARGET_PROFILES))
        raise ValueError(
            f"unsupported frozen Rust target profile {key!r}; "
            f"expected one of: {supported}"
        ) from error


def resolve_plan_target_profile(
    plan: Mapping[str, object],
) -> FrozenRustTargetProfile:
    """Resolve a plan only when its target fields equal one closed profile."""

    key = plan.get("target_profile")
    candidates = (
        (
            get_target_profile(
                key,
                path=plan.get("path") if isinstance(plan.get("path"), str) else None,
            ),
        )
        if isinstance(key, str) and key
        else tuple(
            profile
            for profile in TARGET_PROFILES.values()
            if profile.key != HIR_REMAINING_PROFILE_KEY
        )
    )
    fields = ("package", "target_kind", "binary_id", "test_prefix")
    matches = [
        profile
        for profile in candidates
        if all(plan.get(field) == getattr(profile, field) for field in fields)
    ]
    if len(matches) != 1:
        raise ValueError(
            "Rust plan target fields do not equal one closed target profile"
        )
    return matches[0]


def _hir_remaining_profile(path: str | None) -> FrozenRustTargetProfile:
    if path in HIR_REMAINING_UNIT_PREFIXES:
        return FrozenRustTargetProfile(
            **{
                **HIR_REMAINING_PROFILE.__dict__,
                "test_prefix": HIR_REMAINING_UNIT_PREFIXES[path],
            }
        )
    if path in HIR_REMAINING_INTEGRATION_PATHS:
        relative = path.removeprefix("crates/trust-hir/tests/")
        if relative.startswith("semantic_type_checking/"):
            target_name = "semantic_type_checking"
            test_prefix = relative.rsplit("/", 1)[-1].removesuffix(".rs") + "::"
        else:
            target_name = relative.removesuffix(".rs")
            test_prefix = ""
        return FrozenRustTargetProfile(
            key=HIR_REMAINING_PROFILE_KEY,
            package="trust-hir",
            target_kind="test",
            target_name=target_name,
            binary_id=f"trust-hir::{target_name}",
            test_prefix=test_prefix,
            encoded_test_name_prefix=f"trust-hir::{target_name}$",
            discovery_source_kind="rust_integration_test",
            allowed_cfg_attributes=frozenset(),
            no_default_features_cfg=None,
            no_default_features_shard_name=None,
        )
    raise ValueError(f"{path!r} is outside the closed HIR remaining path set")


def _syntax_remaining_profile(path: str | None) -> FrozenRustTargetProfile:
    if path in SYNTAX_REMAINING_INTEGRATION_PATHS:
        target_name = path.removeprefix(
            "crates/trust-syntax/tests/"
        ).removesuffix(".rs")
        return FrozenRustTargetProfile(
            key=SYNTAX_REMAINING_PROFILE_KEY,
            package="trust-syntax",
            target_kind="test",
            target_name=target_name,
            binary_id=f"trust-syntax::{target_name}",
            test_prefix="",
            encoded_test_name_prefix=f"trust-syntax::{target_name}$",
            discovery_source_kind="rust_integration_test",
            allowed_cfg_attributes=frozenset(),
            no_default_features_cfg=None,
            no_default_features_shard_name=None,
        )
    raise ValueError(
        f"{path!r} is outside the closed syntax remaining path set"
    )


def _runtime_remaining_profile(path: str | None) -> FrozenRustTargetProfile:
    if path in RUNTIME_REMAINING_UNIT_PREFIXES:
        return _unit_path_profile(
            RUNTIME_REMAINING_LIB_PROFILE,
            RUNTIME_REMAINING_UNIT_PREFIXES,
            path,
            "runtime",
        )
    if path in RUNTIME_REMAINING_NESTED_INTEGRATION_PREFIXES:
        target_name, prefix = RUNTIME_REMAINING_NESTED_INTEGRATION_PREFIXES[
            path
        ]
        return FrozenRustTargetProfile(
            key=RUNTIME_REMAINING_PROFILE_KEY,
            package="trust-runtime",
            target_kind="test",
            target_name=target_name,
            binary_id=f"trust-runtime::{target_name}",
            test_prefix=prefix,
            encoded_test_name_prefix=f"trust-runtime::{target_name}$",
            discovery_source_kind="rust_integration_test",
            allowed_cfg_attributes=frozenset(),
            no_default_features_cfg=None,
            no_default_features_shard_name=None,
        )
    if path in RUNTIME_REMAINING_INTEGRATION_PATHS:
        target_name = str(path).removeprefix(
            "crates/trust-runtime/tests/"
        ).removesuffix(".rs")
        return FrozenRustTargetProfile(
            key=RUNTIME_REMAINING_PROFILE_KEY,
            package="trust-runtime",
            target_kind="test",
            target_name=target_name,
            binary_id=f"trust-runtime::{target_name}",
            test_prefix="",
            encoded_test_name_prefix=f"trust-runtime::{target_name}$",
            discovery_source_kind="rust_integration_test",
            allowed_cfg_attributes=frozenset(),
            no_default_features_cfg=None,
            no_default_features_shard_name=None,
        )
    raise ValueError(
        f"{path!r} is outside the closed runtime remaining path set"
    )


def _unit_path_profile(
    base: FrozenRustTargetProfile,
    prefixes: Mapping[str, str],
    path: str | None,
    label: str,
) -> FrozenRustTargetProfile:
    try:
        prefix = prefixes[path]  # type: ignore[index]
    except KeyError as error:
        raise ValueError(
            f"{path!r} is outside the closed {label} remaining path set"
        ) from error
    return FrozenRustTargetProfile(
        **{
            **base.__dict__,
            "test_prefix": prefix,
        }
    )


def qualify_test_names(
    profile: FrozenRustTargetProfile,
    names: Sequence[str],
) -> tuple[str, ...]:
    """Qualify distinct bare names under the profile's fixed module prefix."""

    if any(
        not isinstance(name, str)
        or not name
        or (
            bool(profile.test_prefix)
            and name.startswith(profile.test_prefix)
        )
        for name in names
    ):
        raise ValueError("test names must be nonempty and unqualified")
    full_names = tuple(profile.test_prefix + name for name in names)
    if len(full_names) != len(set(full_names)):
        raise ValueError("duplicate selected test name")
    return full_names


def selection_from_combined_evidence(
    *,
    profile: FrozenRustTargetProfile,
    path: str,
    raw: str,
    raw_results_path: str,
    raw_results_sha256: str,
    source_sha256: str,
    metadata: Mapping[str, Any],
) -> FrozenSelection:
    """Recover a frozen name set from immutable combined nextest evidence."""

    if metadata.get("path") != path:
        raise ValueError("execution metadata path does not match requested path")
    if metadata.get("raw_results_path") != raw_results_path:
        raise ValueError("execution metadata raw path does not match requested raw")
    if metadata.get("raw_results_sha256") != raw_results_sha256:
        raise ValueError("execution metadata raw digest does not match evidence")
    if metadata.get("source_sha256") != source_sha256:
        raise ValueError("execution metadata source digest does not match source")

    terminal: dict[str, str] = {}
    for line_number, line in enumerate(raw.splitlines(), start=1):
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise ValueError(
                f"raw nextest line {line_number} is not JSON: {error.msg}"
            ) from error
        if not isinstance(event, Mapping) or event.get("type") != "test":
            continue
        event_name = event.get("event")
        if event_name not in TERMINAL_EVENTS:
            continue
        raw_name = event.get("name")
        if not isinstance(raw_name, str) or not raw_name:
            raise ValueError(
                f"terminal test event on line {line_number} lacks a name"
            )
        name = normalize_event_test_name(profile, raw_name)
        if name in terminal:
            raise ValueError(f"duplicate terminal result for {name}")
        terminal[name] = str(event_name)

    full_names = tuple(sorted(terminal))
    if not full_names:
        raise ValueError("combined raw evidence contains no terminal tests")
    if metadata.get("selected_test_count") != len(full_names):
        raise ValueError(
            "execution metadata selected count does not match combined raw"
        )
    if metadata.get("selected_names_sha256") != selected_names_digest(
        full_names
    ):
        raise ValueError(
            "execution metadata selected names do not match combined raw"
        )
    reviewed_count = metadata.get("reviewed_nonmapping_count")
    mapped_count = metadata.get("catalog_mapped_count")
    if (
        not isinstance(reviewed_count, int)
        or isinstance(reviewed_count, bool)
        or reviewed_count < 0
        or not isinstance(mapped_count, int)
        or isinstance(mapped_count, bool)
        or mapped_count < 0
        or reviewed_count + mapped_count != len(full_names)
    ):
        raise ValueError("execution metadata frozen counts are invalid")
    return FrozenSelection(
        path=path,
        reviewed_nonmapping_count=reviewed_count,
        catalog_mapped_count=mapped_count,
        names=tuple(
            name.removeprefix(profile.test_prefix) for name in full_names
        ),
        full_names=full_names,
    )


def selected_names_digest(full_names: Sequence[str]) -> str:
    encoded = json.dumps(
        sorted(full_names),
        ensure_ascii=False,
        separators=(",", ":"),
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def split_feature_shards(
    *,
    profile: FrozenRustTargetProfile,
    source: str,
    expected_full_names: Sequence[str],
) -> tuple[TestShard, ...]:
    """Derive exact feature shards using only the profile's closed cfg policy."""

    expected = set(expected_full_names)
    if not expected or len(expected) != len(expected_full_names):
        raise ValueError("feature shard selection must be nonempty and unique")
    _validate_full_names(profile, expected)

    attributes: list[str] = []
    classified: dict[str, bool] = {}
    function_pattern = re.compile(
        r"^(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+"
        r"([A-Za-z_][A-Za-z0-9_]*)"
    )
    for raw_line in source.splitlines():
        line = raw_line.strip()
        if line.startswith("#[") and line.endswith("]"):
            attributes.append(line)
            continue
        match = function_pattern.match(line)
        if match:
            full_name = profile.test_prefix + match.group(1)
            if full_name in expected:
                cfg_attributes = [
                    attribute
                    for attribute in attributes
                    if attribute.startswith("#[cfg")
                ]
                unsupported = [
                    attribute
                    for attribute in cfg_attributes
                    if attribute not in profile.allowed_cfg_attributes
                ]
                if unsupported:
                    raise ValueError(
                        f"{full_name} has unsupported cfg attributes: "
                        + ", ".join(unsupported)
                    )
                if full_name in classified:
                    raise ValueError(f"duplicate selected source test {full_name}")
                disabled = (
                    profile.no_default_features_cfg is not None
                    and profile.no_default_features_cfg in cfg_attributes
                )
                if disabled and len(cfg_attributes) != 1:
                    raise ValueError(
                        f"{full_name} combines incompatible supported "
                        "cfg attributes"
                    )
                classified[full_name] = disabled
            attributes.clear()
            continue
        if line and not line.startswith("//"):
            attributes.clear()

    missing = sorted(expected - set(classified))
    if missing:
        raise ValueError(
            "selected tests missing from source classification: "
            + ", ".join(missing)
        )
    default_names = tuple(
        sorted(name for name, disabled in classified.items() if not disabled)
    )
    disabled_names = tuple(
        sorted(name for name, disabled in classified.items() if disabled)
    )
    shards = []
    if default_names:
        shards.append(TestShard("default", default_names, False))
    if disabled_names:
        if profile.no_default_features_shard_name is None:
            raise ValueError(
                f"{profile.key} does not define a no-default-features shard"
            )
        shards.append(
            TestShard(
                profile.no_default_features_shard_name,
                disabled_names,
                True,
            )
        )
    return tuple(shards)


def no_default_feature_test_names(
    *,
    profile: FrozenRustTargetProfile,
    source: str,
    test_names: Sequence[str],
) -> set[str]:
    """Return bare names whose exact source cfg selects the disabled shard."""

    shards = split_feature_shards(
        profile=profile,
        source=source,
        expected_full_names=qualify_test_names(profile, test_names),
    )
    return {
        name.removeprefix(profile.test_prefix)
        for shard in shards
        if shard.no_default_features
        for name in shard.full_names
    }


def build_nextest_command(
    profile: FrozenRustTargetProfile,
    full_names: Sequence[str],
    *,
    no_default_features: bool = False,
) -> list[str]:
    """Build one exact-name nextest command for a closed target profile."""

    ordered = tuple(sorted(full_names))
    if not ordered:
        raise ValueError("nextest batch requires at least one test")
    if len(ordered) != len(set(ordered)):
        raise ValueError("nextest batch contains duplicate full test names")
    _validate_full_names(profile, ordered)
    clauses = [
        f"(binary_id({exact_name_matcher(profile.binary_id)}) & "
        f"test({exact_name_matcher(name)}))"
        for name in ordered
    ]
    command = [
        "cargo",
        "nextest",
        "run",
        "-p",
        profile.package,
        *profile.cargo_target_args,
    ]
    if no_default_features:
        if profile.no_default_features_cfg is None:
            raise ValueError(
                f"{profile.key} has no no-default-features shard"
            )
        command.append("--no-default-features")
    command.extend(
        [
            "--no-fail-fast",
            "--message-format",
            "libtest-json-plus",
            "--message-format-version",
            MESSAGE_FORMAT_VERSION,
            "--user-config-file",
            "none",
            "-E",
            " | ".join(clauses),
        ]
    )
    return command


def build_catalog_command(
    profile: FrozenRustTargetProfile,
    test_name: str,
    *,
    no_default_features: bool = False,
) -> str:
    """Return the canonical exact command for one profile-bound catalog test."""

    full_name = qualify_test_names(profile, (test_name,))[0]
    expression = (
        f"binary_id({exact_name_matcher(profile.binary_id)}) & "
        f"test({exact_name_matcher(full_name)})"
    )
    command = [
        "cargo",
        "nextest",
        "run",
        "-p",
        profile.package,
        *profile.cargo_target_args,
    ]
    if no_default_features:
        if profile.no_default_features_cfg is None:
            raise ValueError(
                f"{profile.key} has no no-default-features shard"
            )
        command.append("--no-default-features")
    command.extend(
        [
            "--no-fail-fast",
            "--user-config-file",
            "none",
            "-E",
            expression,
        ]
    )
    return shlex.join(command)


def normalize_event_test_name(
    profile: FrozenRustTargetProfile,
    raw_name: str,
) -> str:
    """Remove only the profile's exact nextest encoding and validate the name."""

    name = raw_name
    if name.startswith(profile.encoded_test_name_prefix):
        name = name.removeprefix(profile.encoded_test_name_prefix)
    if not name.startswith(profile.test_prefix):
        raise ValueError(
            f"test result {raw_name!r} is outside required prefix "
            f"{profile.test_prefix}"
        )
    return name


def _validate_full_names(
    profile: FrozenRustTargetProfile,
    names: Sequence[str] | set[str],
) -> None:
    if any(
        not isinstance(name, str) or not name.startswith(profile.test_prefix)
        for name in names
    ):
        raise ValueError(
            f"nextest test name is outside required prefix "
            f"{profile.test_prefix}"
        )
