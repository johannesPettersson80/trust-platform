"""Closed quarantine contract for generated test-catalog mapping proposals."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .code_spec_common import sha256_digest


CANDIDATE_TIERS = {"C1", "C2", "C3"}
ELIGIBLE_INVARIANT_STATUSES = {
    "planned",
    "mapped",
    "specified",
    "implemented",
    "validated",
}
PROPOSAL_ID_RE = re.compile(r"^MAP_CAND_[A-F0-9_]+$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
BEHAVIOR_REF_RE = re.compile(r"^([A-Z][A-Z0-9_]+)#behavior:([0-9]+)$")
TRACE_COMMAND_AUTHORITIES = {"exact", "verified_synthesized"}
DURATION_CLASSES = {"very_fast", "fast", "medium", "slow", "long"}


def build_mapping_proposals(
    *,
    root: Path,
    candidates: Sequence[Mapping[str, Any]],
    trace_records: Mapping[str, Mapping[str, Any]],
    facts: Mapping[str, Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
    assertion_inventories: Mapping[str, Mapping[str, Any]],
) -> list[dict[str, Any]]:
    """Generate deterministic quarantined proposals without hand-authored metadata."""

    proposals: list[dict[str, Any]] = []
    seen: set[str] = set()
    for candidate in sorted(
        candidates, key=lambda row: str(row.get("discovery_id", ""))
    ):
        discovery_id = _required_string(candidate, "discovery_id", "candidate")
        if discovery_id in seen:
            raise ValueError(f"duplicate candidate for {discovery_id}")
        seen.add(discovery_id)
        tier = _required_string(candidate, "candidate_tier", discovery_id)
        if tier not in CANDIDATE_TIERS:
            raise ValueError(f"{discovery_id} cannot become a proposal at tier {tier}")
        fact = facts.get(discovery_id)
        trace = trace_records.get(discovery_id)
        if fact is None or trace is None:
            raise ValueError(f"{discovery_id} lacks exact live fact or trace record")
        assertion_inventory = assertion_inventories.get(discovery_id)
        if assertion_inventory is None:
            raise ValueError(f"{discovery_id} lacks generated assertion inventory")
        if trace.get("trace_status") != "observed":
            raise ValueError(f"{discovery_id} lacks an observed trace")
        invariant_ids = _required_string_array(
            candidate.get("candidate_invariant_ids"),
            f"{discovery_id} candidate invariants",
        )
        behavior_refs: list[str] = []
        oracle_refs: set[str] = set()
        rationales: list[str] = []
        failure_rationales: list[str] = []
        for invariant_id in invariant_ids:
            invariant = invariants.get(invariant_id)
            if invariant is None:
                raise ValueError(f"{discovery_id} names unknown invariant {invariant_id}")
            if not _semantically_eligible_invariant(invariant):
                raise ValueError(
                    f"{discovery_id} invariant {invariant_id} is not semantically eligible"
                )
            oracle = invariant.get("oracle")
            oracle_ref = oracle.get("ref") if isinstance(oracle, Mapping) else None
            if not isinstance(oracle_ref, str) or not oracle_ref:
                raise ValueError(f"{invariant_id} lacks an oracle reference")
            oracle_refs.add(oracle_ref)
            behaviors = invariant.get("behavior")
            if not isinstance(behaviors, Sequence) or isinstance(
                behaviors, (str, bytes)
            ) or not behaviors:
                raise ValueError(f"{invariant_id} lacks oracle-backed behavior rows")
            for index, behavior in enumerate(behaviors):
                if not isinstance(behavior, Mapping):
                    raise ValueError(f"{invariant_id} behavior {index} is not a table")
                rationale = _required_string(
                    behavior, "rationale", f"{invariant_id} behavior {index}"
                )
                behavior_refs.append(f"{invariant_id}#behavior:{index}")
                rationales.append(rationale)
                if behavior.get("outcome") == "reject":
                    failure_rationales.append(rationale)

        path = _required_string(fact, "path", discovery_id)
        source_path = (root.resolve() / path).resolve()
        try:
            source_path.relative_to(root.resolve())
        except ValueError as error:
            raise ValueError(f"{discovery_id} source path escapes repository") from error
        if not source_path.is_file() or source_path.is_symlink():
            raise ValueError(f"{discovery_id} source must be a regular non-symlink file")
        test_source_digest = sha256_digest(source_path.read_bytes())
        assertion_source_digests = validate_source_digest_closure(
            root=root,
            value=assertion_inventory.get("source_digests"),
            label=f"{discovery_id} assertion source closure",
            required_path=path,
            required_digest=test_source_digest,
        )
        semantic_payload = {
            "schema_version": 2,
            "discovery_id": discovery_id,
            "discovery_source_kind": _required_string(
                fact, "discovery_source_kind", discovery_id
            ),
            "path": path,
            "name": _required_string(fact, "name", discovery_id),
            "ignore_state": _required_string(fact, "ignore_state", discovery_id),
            "command": _required_string(trace, "command", discovery_id),
            "command_authority": _trace_command_authority(trace, discovery_id),
            "duration_ms": _duration_ms(trace, discovery_id),
            "duration_class": duration_class(_duration_ms(trace, discovery_id)),
            "status": "pending",
            "candidate_tier": tier,
            "candidate_invariant_ids": invariant_ids,
            "covered_code_ids": _required_string_array(
                candidate.get("covered_code_ids"), f"{discovery_id} covered code"
            ),
            "specified_code_ids": _required_string_array(
                candidate.get("specified_code_ids"), f"{discovery_id} specified code"
            ),
            "spec_gap_code_ids": _required_string_array(
                candidate.get("spec_gap_code_ids"), f"{discovery_id} spec-gap code"
            ),
            "spec_gap_refs": _required_string_array(
                candidate.get("spec_gap_refs"), f"{discovery_id} spec-gap refs"
            ),
            "mutation_kill_ids": _required_string_array(
                candidate.get("mutation_kill_ids"), f"{discovery_id} mutation kills"
            ),
            "test_source_digest": test_source_digest,
            "assertion_inventory_digest": _required_string(
                assertion_inventory,
                "inventory_digest",
                f"{discovery_id} assertion inventory",
            ),
            "assertion_source_digests": assertion_source_digests,
            "assertions": _required_object_array(
                assertion_inventory.get("assertions"),
                f"{discovery_id} assertions",
            ),
            "unresolved_assertion_helpers": _required_string_array(
                assertion_inventory.get("unresolved_assertion_helpers"),
                f"{discovery_id} unresolved assertion helpers",
            ),
            "trace_record_digest": _canonical_digest(trace),
            "candidate_signal_digest": _canonical_digest(candidate),
            "oracle_refs": sorted(oracle_refs),
            "behavior_refs": sorted(behavior_refs),
            "suggestions": {
                "authoritative": False,
                "test_class": "behavior_lock",
                "expected_result": " ".join(rationales),
                "expected_failure_mode": " ".join(failure_rationales or rationales),
                "source_behavior_refs": sorted(behavior_refs),
            },
        }
        proposal_digest = proposal_payload_digest(semantic_payload)
        proposal_id = (
            f"MAP_CAND_{discovery_id.removeprefix('DISC_')}_"
            f"{proposal_digest.removeprefix('sha256:')[:12].upper()}"
        )
        proposals.append(
            {
                **semantic_payload,
                "id": proposal_id,
                "proposal_digest": proposal_digest,
            }
        )
    return proposals


def validate_mapping_quarantine(
    *,
    root: Path | None = None,
    proposals: Sequence[Mapping[str, Any]],
    facts: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    denominator_reviews: Mapping[str, Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
) -> list[str]:
    """Validate that pending candidates cannot become catalog or proof state."""

    failures: list[str] = []
    catalog_by_discovery = {
        str(record.get("discovery_id")): test_id
        for test_id, record in tests.items()
        if record.get("subject_kind") == "generated_test"
        and isinstance(record.get("discovery_id"), str)
    }
    seen_ids: set[str] = set()
    seen_discovery_ids: set[str] = set()
    for index, proposal in enumerate(proposals):
        label = f"proposal[{index}]"
        proposal_id = proposal.get("id")
        if not isinstance(proposal_id, str) or not PROPOSAL_ID_RE.fullmatch(proposal_id):
            failures.append(f"{label} has invalid proposal ID")
        elif proposal_id in seen_ids:
            failures.append(f"{label} duplicates proposal ID {proposal_id}")
        else:
            seen_ids.add(proposal_id)
            label = proposal_id
        if proposal.get("schema_version") != 2:
            failures.append(f"{label} must use schema_version 2")
        if proposal.get("status") != "pending":
            failures.append(f"{label} quarantine status must equal pending")
        if proposal.get("candidate_tier") not in CANDIDATE_TIERS:
            failures.append(f"{label} has invalid candidate tier")
        if not isinstance(proposal.get("command"), str) or not proposal.get("command"):
            failures.append(f"{label} requires an exact executable command")
        if proposal.get("command_authority") not in TRACE_COMMAND_AUTHORITIES:
            failures.append(f"{label} has invalid trace command authority")
        duration_ms = proposal.get("duration_ms")
        if (
            not isinstance(duration_ms, int)
            or isinstance(duration_ms, bool)
            or duration_ms < 0
        ):
            failures.append(f"{label} requires nonnegative measured duration_ms")
        elif proposal.get("duration_class") != duration_class(duration_ms):
            failures.append(f"{label} duration class does not match measured duration")
        for field in (
            "proposal_digest",
            "test_source_digest",
            "assertion_inventory_digest",
            "trace_record_digest",
            "candidate_signal_digest",
        ):
            if not _is_digest(proposal.get(field)):
                failures.append(f"{label} requires a tagged SHA-256 {field}")
        if (
            _is_digest(proposal.get("proposal_digest"))
            and proposal.get("proposal_digest") != proposal_payload_digest(proposal)
        ):
            failures.append(f"{label} proposal digest does not bind its semantic payload")

        discovery_id = proposal.get("discovery_id")
        if not isinstance(discovery_id, str) or not discovery_id:
            failures.append(f"{label} requires discovery_id")
            continue
        if discovery_id in seen_discovery_ids:
            failures.append(f"{label} duplicates discovery candidate {discovery_id}")
        seen_discovery_ids.add(discovery_id)
        fact = facts.get(discovery_id)
        if fact is None:
            failures.append(f"{label} is stale: discovery ID is absent from live facts")
        else:
            for field in (
                "discovery_source_kind",
                "path",
                "name",
                "ignore_state",
            ):
                if proposal.get(field) != fact.get(field):
                    failures.append(f"{label} {field} does not match live fact")
        if root is not None and isinstance(proposal.get("path"), str):
            resolved_root = root.resolve()
            source_path = resolved_root / str(proposal["path"])
            try:
                resolved_source = source_path.resolve()
                resolved_source.relative_to(resolved_root)
            except (OSError, ValueError):
                failures.append(f"{label} source path escapes repository")
            else:
                if source_path.is_symlink() or not resolved_source.is_file():
                    failures.append(f"{label} source path is not a regular file")
                elif proposal.get("test_source_digest") != sha256_digest(
                    resolved_source.read_bytes()
                ):
                    failures.append(f"{label} source digest is stale")
        try:
            validate_source_digest_closure(
                root=root,
                value=proposal.get("assertion_source_digests"),
                label=f"{label} assertion source closure",
                required_path=proposal.get("path")
                if isinstance(proposal.get("path"), str)
                else None,
                required_digest=proposal.get("test_source_digest")
                if _is_digest(proposal.get("test_source_digest"))
                else None,
            )
        except ValueError as error:
            failures.append(str(error))
        assertions = proposal.get("assertions")
        if not isinstance(assertions, Sequence) or isinstance(
            assertions, (str, bytes)
        ) or any(not isinstance(item, Mapping) for item in assertions):
            failures.append(f"{label} assertions must be an object array")
        helpers = _string_list(
            proposal.get("unresolved_assertion_helpers"),
            f"{label} unresolved_assertion_helpers",
            failures,
        )
        if helpers != sorted(set(helpers)):
            failures.append(
                f"{label} unresolved assertion helpers must be sorted and distinct"
            )

        if discovery_id in catalog_by_discovery:
            failures.append(
                f"{label} quarantine leakage: discovery ID already appears in catalog "
                f"{catalog_by_discovery[discovery_id]}"
            )
        review = denominator_reviews.get(discovery_id)
        if review is None:
            failures.append(f"{label} lacks denominator review")
        elif review.get("disposition") != "reviewed_nonmapping":
            failures.append(
                f"{label} pending proposal requires reviewed_nonmapping denominator"
            )

        candidate_invariant_ids = _string_list(
            proposal.get("candidate_invariant_ids"),
            f"{label} candidate_invariant_ids",
            failures,
            nonempty=True,
        )
        expected_oracle_refs: set[str] = set()
        for invariant_id in candidate_invariant_ids:
            invariant = invariants.get(invariant_id)
            if invariant is None:
                failures.append(f"{label} references unknown invariant {invariant_id}")
                continue
            if not _semantically_eligible_invariant(invariant):
                failures.append(
                    f"{label} candidate invariant {invariant_id} must be semantically eligible"
                )
            oracle = invariant.get("oracle")
            oracle_ref = oracle.get("ref") if isinstance(oracle, Mapping) else None
            if not isinstance(oracle_ref, str) or not oracle_ref:
                failures.append(f"{label} candidate invariant {invariant_id} lacks oracle ref")
            else:
                expected_oracle_refs.add(oracle_ref)

        oracle_refs = set(
            _string_list(
                proposal.get("oracle_refs"),
                f"{label} oracle_refs",
                failures,
                nonempty=True,
            )
        )
        if oracle_refs != expected_oracle_refs:
            failures.append(f"{label} oracle reference set does not match candidate invariants")

        behavior_refs = _string_list(
            proposal.get("behavior_refs"),
            f"{label} behavior_refs",
            failures,
            nonempty=True,
        )
        for behavior_ref in behavior_refs:
            match = BEHAVIOR_REF_RE.fullmatch(behavior_ref)
            if match is None:
                failures.append(f"{label} behavior reference is invalid: {behavior_ref}")
                continue
            invariant_id, raw_index = match.groups()
            invariant = invariants.get(invariant_id)
            behaviors = invariant.get("behavior") if isinstance(invariant, Mapping) else None
            if invariant_id not in candidate_invariant_ids:
                failures.append(
                    f"{label} behavior reference does not name a candidate invariant: "
                    f"{behavior_ref}"
                )
            elif not isinstance(behaviors, Sequence) or int(raw_index) >= len(behaviors):
                failures.append(f"{label} behavior reference does not resolve: {behavior_ref}")

        suggestions = proposal.get("suggestions")
        if not isinstance(suggestions, Mapping):
            failures.append(f"{label} requires suggestions object")
        else:
            if suggestions.get("authoritative") is not False:
                failures.append(f"{label} suggestions must remain non-authoritative")
            source_refs = _string_list(
                suggestions.get("source_behavior_refs"),
                f"{label} suggestion source_behavior_refs",
                failures,
                nonempty=True,
            )
            if not set(source_refs).issubset(set(behavior_refs)):
                failures.append(
                    f"{label} suggestion sources must be exact proposal behavior references"
                )
            for field in ("test_class", "expected_result", "expected_failure_mode"):
                value = suggestions.get(field)
                if not isinstance(value, str) or not value:
                    failures.append(f"{label} suggestions require non-empty {field}")
    return sorted(set(failures))


def _semantically_eligible_invariant(
    invariant: Mapping[str, Any],
) -> bool:
    status = invariant.get("status")
    if status not in ELIGIBLE_INVARIANT_STATUSES:
        return False
    if status != "planned":
        return True
    spec = invariant.get("spec")
    return isinstance(spec, Mapping) and spec.get("status") == "specified"


def _is_digest(value: Any) -> bool:
    return isinstance(value, str) and DIGEST_RE.fullmatch(value) is not None


def validate_source_digest_closure(
    *,
    root: Path | None,
    value: Any,
    label: str,
    required_path: str | None = None,
    required_digest: str | None = None,
) -> dict[str, str]:
    """Return one canonical, optionally live-validated path-to-digest closure."""

    if not isinstance(value, Mapping) or not value:
        raise ValueError(f"{label} must be a non-empty path-to-digest table")
    closure: dict[str, str] = {}
    resolved_root = root.resolve() if root is not None else None
    for raw_path, digest in sorted(value.items()):
        if not isinstance(raw_path, str) or not raw_path:
            raise ValueError(f"{label} contains an empty or non-string path")
        relative = Path(raw_path)
        if (
            relative.is_absolute()
            or ".." in relative.parts
            or "\\" in raw_path
            or relative.as_posix() != raw_path
            or raw_path in {".", ""}
        ):
            raise ValueError(f"{label} path is not canonical: {raw_path}")
        if not _is_digest(digest):
            raise ValueError(f"{label} requires a tagged SHA-256 for {raw_path}")
        if resolved_root is not None:
            source = resolved_root
            for part in relative.parts:
                source /= part
                if source.is_symlink():
                    raise ValueError(
                        f"{label} path must be a regular non-symlink file: {raw_path}"
                    )
            try:
                resolved_source = source.resolve()
                resolved_source.relative_to(resolved_root)
            except (OSError, ValueError) as error:
                raise ValueError(
                    f"{label} path escapes repository: {raw_path}"
                ) from error
            if not resolved_source.is_file():
                raise ValueError(
                    f"{label} path must be a regular non-symlink file: {raw_path}"
                )
            if digest != sha256_digest(resolved_source.read_bytes()):
                raise ValueError(f"{label} digest is stale: {raw_path}")
        closure[raw_path] = digest
    if required_path is not None:
        if closure.get(required_path) != required_digest:
            raise ValueError(
                f"{label} must bind selected test {required_path} to its source digest"
            )
    return closure


def proposal_payload_digest(proposal: Mapping[str, Any]) -> str:
    """Digest every semantic proposal field, excluding only ID and its digest."""

    payload = {
        key: value
        for key, value in proposal.items()
        if key not in {"id", "proposal_digest"}
    }
    return _canonical_digest(payload)


def duration_class(duration_ms: int) -> str:
    """Classify one measured focused execution with fixed millisecond bands."""

    if not isinstance(duration_ms, int) or isinstance(duration_ms, bool) or duration_ms < 0:
        raise ValueError("mapping proposal duration must be a nonnegative integer")
    if duration_ms < 100:
        return "very_fast"
    if duration_ms < 1_000:
        return "fast"
    if duration_ms < 10_000:
        return "medium"
    if duration_ms < 60_000:
        return "slow"
    return "long"


def _trace_command_authority(
    trace: Mapping[str, Any],
    label: str,
) -> str:
    value = _required_string(trace, "command_authority", label)
    if value not in TRACE_COMMAND_AUTHORITIES:
        raise ValueError(f"{label} lacks exact trace command authority")
    return value


def _duration_ms(trace: Mapping[str, Any], label: str) -> int:
    value = trace.get("duration_ms")
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ValueError(f"{label} lacks measured trace duration")
    return value


def _canonical_digest(value: Mapping[str, Any]) -> str:
    encoded = json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def _required_string_array(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be an array")
    if any(not isinstance(item, str) or not item for item in value):
        raise ValueError(f"{label} must contain only non-empty strings")
    if len(value) != len(set(value)):
        raise ValueError(f"{label} must not contain duplicates")
    return sorted(value)


def _required_object_array(value: Any, label: str) -> list[dict[str, Any]]:
    if (
        not isinstance(value, Sequence)
        or isinstance(value, (str, bytes))
        or any(not isinstance(item, Mapping) for item in value)
    ):
        raise ValueError(f"{label} must be an object array")
    return [dict(item) for item in value]


def _string_list(
    value: Any,
    label: str,
    failures: list[str],
    *,
    nonempty: bool = False,
) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        failures.append(f"{label} must be an array of strings")
        return []
    result = [item for item in value if isinstance(item, str) and item]
    if len(result) != len(value):
        failures.append(f"{label} must contain only non-empty strings")
    if len(result) != len(set(result)):
        failures.append(f"{label} must not contain duplicates")
    if nonempty and not result:
        failures.append(f"{label} must not be empty")
    return result
