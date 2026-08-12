"""Produce source-bound Phase 18 assertion-reachability evidence."""

from __future__ import annotations

import argparse
import contextlib
import hashlib
import json
import os
import shlex
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
from collections.abc import Iterator, Mapping, Sequence
from pathlib import Path
from typing import Any

from ..code_spec_provenance import input_closure_digest
from ..metadata_validator.mutation_contracts import MutationContractError
from ..mutation_execution import (
    apply_generated_mutant,
    classify_focused_mutant,
    clean_mutation_package,
    discover_mutants,
    run_command,
    select_generated_mutant,
    selected_test_failed,
)
from ..test_catalog_json_schema import validate_json_schema_instance
from .assertion_reachability import SCHEMA_PATH, validate_artifact


PLAN_PATH = Path("verification/p18-assertion-reachability-plan.toml")
PLAN_SCHEMA_PATH = Path("verification/schemas/p18-assertion-reachability-plan.schema.json")
ARTIFACT_PATH = Path("verification/p18/assertion-reachability.json")
PRODUCER = "p18-assertion-reachability.py v1"
TOOL_VERSION = "cargo-mutants 27.0.0"
BASE_INPUT_PATHS = {
    "Cargo.toml",
    "Cargo.lock",
    "crates/trust-runtime-core/Cargo.toml",
    "scripts/p18_assertion_reachability.py",
    "scripts/verification/mutation_execution.py",
    "scripts/verification/p18_zero_debt/assertion_reachability.py",
    "scripts/verification/p18_zero_debt/assertion_reachability_producer.py",
    PLAN_PATH.as_posix(),
    PLAN_SCHEMA_PATH.as_posix(),
    SCHEMA_PATH.as_posix(),
    "verification/code-spec-map.toml",
    "verification/test-catalog-denominator.toml",
    "verification/test-catalog.toml",
}


def canonical_json(payload: Mapping[str, Any]) -> str:
    return json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n"


def load_plan(root: Path) -> dict[str, Any]:
    plan = tomllib.loads((root / PLAN_PATH).read_text())
    schema = json.loads((root / PLAN_SCHEMA_PATH).read_text())
    failures = validate_json_schema_instance(plan, schema)
    failures.extend(validate_plan(root, plan))
    if failures:
        raise ValueError("; ".join(sorted(set(failures))))
    return plan


def validate_plan(root: Path, plan: Mapping[str, Any]) -> list[str]:
    failures: list[str] = []
    shards = _mapping_rows(plan.get("shards"), "plan shards", failures)
    shard_ids = [str(shard.get("id", "")) for shard in shards]
    if shard_ids != sorted(shard_ids) or len(set(shard_ids)) != len(shard_ids):
        failures.append("assertion reachability shards must be unique and ordered")
    catalog = _index(_load_toml(root / "verification/test-catalog.toml"), "tests", "id")
    mappings = _mapping_rows(
        _load_toml(root / "verification/code-spec-map.toml").get("mappings"),
        "code mappings",
        failures,
    )
    mapping_by_code = {str(row.get("code_id")): row for row in mappings}
    seen_tests: set[str] = set()
    seen_mutants: set[str] = set()
    for shard in shards:
        shard_id = shard.get("id")
        source_path = _safe_file(root, shard.get("source_file"), failures, shard_id)
        if source_path is not None:
            expected = _sha256(source_path.read_bytes())
            if shard.get("source_digest") != expected:
                failures.append(f"{shard_id} source_digest is stale")
        invariant_path = _safe_file(
            root, shard.get("invariant_path"), failures, shard_id
        )
        if invariant_path is not None:
            try:
                invariant = tomllib.loads(invariant_path.read_text())
                contract = {
                    key: invariant[key]
                    for key in ("claim", "spec", "oracle", "behavior")
                }
                digest = _sha256(canonical_json(contract).encode())
                if shard.get("invariant_contract_digest") != digest:
                    failures.append(f"{shard_id} invariant contract digest is stale")
                if invariant.get("id") != shard.get("invariant_id"):
                    failures.append(f"{shard_id} invariant identity is stale")
            except (KeyError, tomllib.TOMLDecodeError):
                failures.append(f"{shard_id} invariant contract is malformed")
        mapping = mapping_by_code.get(str(shard.get("code_id")))
        if mapping is None:
            failures.append(f"{shard_id} code_id is not live")
        elif shard.get("invariant_id") not in mapping.get("invariant_ids", []):
            failures.append(f"{shard_id} code mapping omits its invariant")
        tests = _mapping_rows(shard.get("tests"), f"{shard_id} tests", failures)
        for test in tests:
            test_id = str(test.get("test_id", ""))
            if test_id in seen_tests:
                failures.append(f"duplicate assertion reachability test {test_id}")
            seen_tests.add(test_id)
            catalog_row = catalog.get(test_id)
            if catalog_row is None:
                failures.append(f"{shard_id} test {test_id} is absent from the catalog")
                continue
            expected_fields = {
                "discovery_id": test.get("discovery_id"),
                "path": test.get("path"),
                "name": test.get("name"),
                "command": shlex.join(
                    str(value) for value in test.get("command", [])
                ),
            }
            for field, expected in expected_fields.items():
                if catalog_row.get(field) != expected:
                    failures.append(f"{shard_id} test {test_id} {field} is stale")
            if shard.get("invariant_id") not in catalog_row.get("invariants", []):
                failures.append(f"{shard_id} test {test_id} omits its invariant")
            if mapping is not None and test_id not in mapping.get("test_ids", []):
                failures.append(f"{shard_id} code mapping omits test {test_id}")
            _safe_file(root, test.get("path"), failures, test_id)
        mutations = _mapping_rows(
            shard.get("mutations"), f"{shard_id} mutations", failures
        )
        for mutation in mutations:
            mutation_id = str(mutation.get("id", ""))
            if mutation_id in seen_mutants:
                failures.append(f"duplicate assertion reachability mutant {mutation_id}")
            seen_mutants.add(mutation_id)
    return sorted(set(failures))


def build_open_records(root: Path) -> list[dict[str, Any]]:
    catalog = _index(_load_toml(root / "verification/test-catalog.toml"), "tests", "id")
    denominator = _load_toml(root / "verification/test-catalog-denominator.toml")
    mappings = _load_toml(root / "verification/code-spec-map.toml").get("mappings", [])
    reverse_codes: dict[str, list[str]] = {}
    for mapping in mappings if isinstance(mappings, list) else []:
        if not isinstance(mapping, Mapping):
            continue
        for test_id in mapping.get("test_ids", []):
            if isinstance(test_id, str):
                reverse_codes.setdefault(test_id, []).append(str(mapping.get("code_id")))
    records: list[dict[str, Any]] = []
    for review in denominator.get("reviews", []):
        if not isinstance(review, Mapping) or review.get("disposition") != "catalog_mapped":
            continue
        test_id = str(review.get("catalog_test_id", ""))
        catalog_row = catalog.get(test_id)
        if catalog_row is None:
            raise ValueError(f"catalog-mapped denominator row references unknown {test_id}")
        code_ids = sorted(set(reverse_codes.get(test_id, [])))
        invariant_ids = sorted(
            value
            for value in catalog_row.get("invariants", [])
            if isinstance(value, str)
        )
        blockers = ["caught_adversarial_mutation_required", "source_bound_execution_required"]
        if not code_ids:
            blockers.append("exact_code_binding_required")
        if not invariant_ids:
            blockers.append("invariant_binding_required")
        records.append(
            {
                "test_id": test_id,
                "discovery_id": str(review.get("discovery_id")),
                "status": "open",
                "code_ids": code_ids,
                "invariant_ids": invariant_ids,
                "blockers": sorted(blockers),
                "baseline": {
                    "command": [],
                    "exit_status": None,
                    "timed_out": False,
                    "stdout": "",
                    "stderr": "",
                    "result": "not_run",
                },
                "mutations": [],
            }
        )
    records.sort(key=lambda record: record["discovery_id"])
    return records


def execute(
    root: Path,
    plan: Mapping[str, Any],
    *,
    shard_ids: set[str],
    target_dir: Path,
    scratch_root: Path,
    build_timeout: float,
    test_timeout: float,
) -> dict[str, Any]:
    source_sha = _clean_head(root)
    installed = _command_output(root, ["cargo", "mutants", "--version"])
    if installed != TOOL_VERSION:
        raise ValueError(f"installed cargo-mutants is {installed!r}, expected {TOOL_VERSION!r}")
    input_paths = collect_input_paths(plan)
    records = build_open_records(root)
    by_test = {record["test_id"]: record for record in records}
    target_dir.mkdir(parents=True, exist_ok=True)
    scratch_root.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(target_dir.resolve())
    with isolated_head(root, scratch_root) as scratch:
        for shard in plan.get("shards", []):
            if not isinstance(shard, Mapping) or shard.get("id") not in shard_ids:
                continue
            package = str(shard["package"])
            tests = list(shard["tests"])
            clean_mutation_package(package, scratch, environment, build_timeout)
            baseline_by_test = {}
            for test in tests:
                result = run_command(
                    test["command"], cwd=scratch, env=environment, timeout=test_timeout
                )
                baseline_by_test[test["test_id"]] = _baseline_record(result)
                if result.timed_out or result.returncode != 0:
                    raise MutationContractError(
                        f"baseline failed for {test['test_id']}: {list(test['command'])}"
                    )
            mutation_results: dict[str, list[dict[str, Any]]] = {
                str(test["test_id"]): [] for test in tests
            }
            source_path = scratch / str(shard["source_file"])
            candidates = discover_mutants(source_path, scratch, environment)
            for mutation in shard["mutations"]:
                clean_mutation_package(package, scratch, environment, build_timeout)
                original = source_path.read_text()
                config = {
                    "function": shard["function"],
                    "genre": mutation["genre"],
                    "replacement": mutation["replacement"],
                    "selector_name": mutation["selector_name"],
                }
                candidate = select_generated_mutant(candidates, config)
                source_path.write_text(apply_generated_mutant(original, candidate))
                try:
                    build = run_command(
                        shard["build_command"],
                        cwd=scratch,
                        env=environment,
                        timeout=build_timeout,
                    )
                    for test in tests:
                        test_result = None
                        if not build.timed_out and build.returncode == 0:
                            test_result = run_command(
                                test["command"],
                                cwd=scratch,
                                env=environment,
                                timeout=test_timeout,
                            )
                        classified = classify_focused_mutant(
                            source_file=str(shard["source_file"]),
                            selected_test_name=str(test["name"]),
                            build=build,
                            test=test_result,
                        )
                        mutation_results[str(test["test_id"])].append(
                            _mutation_record(
                                mutation,
                                shard,
                                candidate,
                                build,
                                test_result,
                                classified,
                                str(test["name"]),
                            )
                        )
                finally:
                    source_path.write_text(original)
                    clean_mutation_package(package, scratch, environment, build_timeout)
            for test in tests:
                record = by_test.get(str(test["test_id"]))
                if record is None:
                    raise ValueError(f"planned test is absent from denominator: {test['test_id']}")
                mutations = mutation_results[str(test["test_id"])]
                record["baseline"] = baseline_by_test[str(test["test_id"])]
                record["mutations"] = mutations
                if (
                    record["code_ids"]
                    and record["invariant_ids"]
                    and record["baseline"]["result"] == "passed"
                    and mutations
                    and all(item["result"] == "caught" for item in mutations)
                ):
                    record["status"] = "proven"
                    record["blockers"] = []
    payload = {
        "schema_version": 1,
        "proof_class": "assertion_reachability",
        "producer": PRODUCER,
        "source_sha": source_sha,
        "input_digest": input_closure_digest(root, input_paths),
        "input_paths": input_paths,
        "status": "open",
        "summary": {},
        "records": records,
    }
    payload["summary"] = reconstruct_summary(records)
    payload["status"] = "closed" if payload["summary"]["debt"] == 0 else "open"
    failures = validate_artifact(payload)
    if failures:
        raise ValueError("; ".join(failures))
    return payload


def reconstruct_summary(records: Sequence[Mapping[str, Any]]) -> dict[str, int]:
    results = [
        mutation.get("result")
        for record in records
        for mutation in record.get("mutations", [])
        if isinstance(mutation, Mapping)
    ]
    proven = sum(record.get("status") == "proven" for record in records)
    return {
        "denominator": len(records),
        "proven": proven,
        "debt": len(records) - proven,
        "executed_tests": sum(
            isinstance(record.get("baseline"), Mapping)
            and record["baseline"].get("result") == "passed"
            for record in records
        ),
        "caught_mutants": results.count("caught"),
        "survived_mutants": results.count("survived"),
        "unviable_mutants": results.count("unviable"),
        "timeouts": results.count("timeout"),
        "harness_failures": results.count("harness_failure"),
    }


def validate_live_artifact(root: Path, artifact: Mapping[str, Any]) -> list[str]:
    failures = validate_artifact(artifact)
    try:
        plan = load_plan(root)
        expected_paths = collect_input_paths(plan)
        expected_records = build_open_records(root)
    except (OSError, ValueError, tomllib.TOMLDecodeError) as error:
        return [*failures, f"assertion reachability live inputs failed: {error}"]
    if artifact.get("input_paths") != expected_paths:
        failures.append("assertion_reachability input path closure is stale")
    elif artifact.get("input_digest") != input_closure_digest(root, expected_paths):
        failures.append("assertion_reachability input digest is stale")
    expected_bindings = {
        record["discovery_id"]: (
            record["test_id"],
            record["code_ids"],
            record["invariant_ids"],
        )
        for record in expected_records
    }
    actual_bindings = {
        str(record.get("discovery_id")): (
            record.get("test_id"),
            record.get("code_ids"),
            record.get("invariant_ids"),
        )
        for record in artifact.get("records", [])
        if isinstance(record, Mapping)
    }
    if actual_bindings != expected_bindings:
        failures.append("assertion_reachability live denominator or bindings are stale")
    source_sha = str(artifact.get("source_sha", ""))
    if not _is_ancestor(root, source_sha):
        failures.append("assertion_reachability source_sha is not an ancestor of HEAD")
    else:
        for path in expected_paths:
            historical = subprocess.run(
                ["git", "-C", str(root), "show", f"{source_sha}:{path}"],
                check=False,
                capture_output=True,
            )
            if historical.returncode != 0 or historical.stdout != (root / path).read_bytes():
                failures.append(
                    f"assertion_reachability input differs from source_sha: {path}"
                )
    return sorted(set(failures))


def collect_input_paths(plan: Mapping[str, Any]) -> list[str]:
    paths = set(BASE_INPUT_PATHS)
    for shard in plan.get("shards", []):
        if not isinstance(shard, Mapping):
            continue
        paths.add(str(shard["source_file"]))
        # The reviewed plan pins the invariant's claim/spec/oracle/behavior
        # digest. Excluding the whole TOML here lets evidence_refs and missing
        # bookkeeping change without invalidating the behavior contract.
        for test in shard.get("tests", []):
            if isinstance(test, Mapping):
                paths.add(str(test["path"]))
    return sorted(paths)


@contextlib.contextmanager
def isolated_head(root: Path, scratch_root: Path) -> Iterator[Path]:
    with tempfile.TemporaryDirectory(
        prefix="trust-p18-assertion-", dir=scratch_root
    ) as temporary:
        scratch = Path(temporary)
        archive_path = scratch / "head.tar"
        with archive_path.open("wb") as archive_file:
            result = subprocess.run(
                ["git", "-C", str(root), "archive", "--format=tar", "HEAD"],
                stdout=archive_file,
                check=False,
            )
        if result.returncode != 0:
            raise MutationContractError("failed to archive clean assertion source")
        with tarfile.open(archive_path) as archive:
            archive.extractall(scratch, filter="data")
        archive_path.unlink()
        shutil.copy2(root / "AGENTS.md", scratch / "AGENTS.md")
        archived_skills = scratch / ".codex/skills"
        if archived_skills.exists():
            shutil.rmtree(archived_skills)
        shutil.copytree(root / ".codex/skills", scratch / ".codex/skills")
        if _tree_digest(root / ".codex/skills") != _tree_digest(scratch / ".codex/skills"):
            raise MutationContractError("isolated assertion workspace skills bootstrap drifted")
        if (root / "AGENTS.md").read_bytes() != (scratch / "AGENTS.md").read_bytes():
            raise MutationContractError("isolated assertion workspace AGENTS bootstrap drifted")
        yield scratch


def _baseline_record(result: Any) -> dict[str, Any]:
    if result.timed_out:
        state = "timeout"
    elif result.returncode == 0:
        state = "passed"
    else:
        state = "harness_failure"
    return {
        "command": list(result.command),
        "exit_status": result.returncode,
        "timed_out": result.timed_out,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "result": state,
    }


def _mutation_record(
    mutation: Mapping[str, Any],
    shard: Mapping[str, Any],
    candidate: Mapping[str, Any],
    build: Any,
    test: Any,
    classified: str,
    selected_name: str,
) -> dict[str, Any]:
    result = "harness_failure" if classified == "error" else classified
    return {
        "id": mutation["id"],
        "source_file": shard["source_file"],
        "function": shard["function"],
        "genre": candidate.get("genre"),
        "replacement": candidate.get("replacement"),
        "generated_mutant_name": candidate.get("name"),
        "build_command": list(build.command),
        "build_exit_status": build.returncode,
        "build_timed_out": build.timed_out,
        "build_stdout": build.stdout,
        "build_stderr": build.stderr,
        "test_command": list(test.command) if test is not None else [],
        "test_exit_status": test.returncode if test is not None else None,
        "test_timed_out": test.timed_out if test is not None else False,
        "test_stdout": test.stdout if test is not None else "",
        "test_stderr": test.stderr if test is not None else "",
        "selected_test_failed": (
            selected_test_failed(test, selected_name) if test is not None else False
        ),
        "result": result,
    }


def _mapping_rows(value: Any, label: str, failures: list[str]) -> list[Mapping[str, Any]]:
    if not isinstance(value, list) or not all(isinstance(item, Mapping) for item in value):
        failures.append(f"{label} must be an object array")
        return []
    return list(value)


def _index(document: Mapping[str, Any], collection: str, identity: str) -> dict[str, Mapping[str, Any]]:
    rows = document.get(collection, [])
    if not isinstance(rows, list):
        raise ValueError(f"{collection} must be an array")
    result: dict[str, Mapping[str, Any]] = {}
    for row in rows:
        if not isinstance(row, Mapping) or not isinstance(row.get(identity), str):
            raise ValueError(f"{collection} contains an invalid identity")
        key = str(row[identity])
        if key in result:
            raise ValueError(f"{collection} duplicates {key}")
        result[key] = row
    return result


def _load_toml(path: Path) -> dict[str, Any]:
    return tomllib.loads(path.read_text())


def _safe_file(
    root: Path, value: Any, failures: list[str], owner: Any
) -> Path | None:
    if not isinstance(value, str) or not value:
        failures.append(f"{owner} path is invalid")
        return None
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts or "\\" in value:
        failures.append(f"{owner} path is unsafe")
        return None
    path = root / relative
    if not path.is_file() or path.is_symlink():
        failures.append(f"{owner} path is not a regular file: {value}")
        return None
    return path


def _sha256(payload: bytes) -> str:
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def _tree_digest(root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(item for item in root.rglob("*") if item.is_file()):
        digest.update(path.relative_to(root).as_posix().encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def _clean_head(root: Path) -> str:
    status = _command_output(
        root, ["git", "status", "--porcelain", "--untracked-files=all"]
    )
    if status:
        raise ValueError("assertion reachability execution requires a clean worktree")
    return _command_output(root, ["git", "rev-parse", "HEAD"])


def _is_ancestor(root: Path, commit: str) -> bool:
    if len(commit) != 40:
        return False
    result = subprocess.run(
        ["git", "-C", str(root), "merge-base", "--is-ancestor", commit, "HEAD"],
        check=False,
        capture_output=True,
    )
    return result.returncode == 0


def _command_output(root: Path, command: list[str]) -> str:
    result = subprocess.run(command, cwd=root, check=False, capture_output=True, text=True)
    if result.returncode != 0:
        raise ValueError(f"command failed: {command}: {result.stderr.strip()}")
    return result.stdout.strip()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--artifact", type=Path, default=ARTIFACT_PATH)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--shard-id", action="append", default=[])
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--scratch-root", type=Path)
    parser.add_argument("--build-timeout", type=float, default=1800.0)
    parser.add_argument("--test-timeout", type=float, default=900.0)
    args = parser.parse_args(argv)
    root = args.root.resolve()
    artifact_path = args.artifact if args.artifact.is_absolute() else root / args.artifact
    try:
        plan = load_plan(root)
        known_shards = {
            str(shard["id"])
            for shard in plan["shards"]
            if isinstance(shard, Mapping)
        }
        selected = set(args.shard_id) if args.shard_id else known_shards
        unknown = selected - known_shards
        if unknown:
            raise ValueError(f"unknown assertion reachability shards: {sorted(unknown)}")
        if args.write:
            if args.target_dir is None or args.scratch_root is None:
                raise ValueError("--write requires --target-dir and --scratch-root")
            payload = execute(
                root,
                plan,
                shard_ids=selected,
                target_dir=args.target_dir,
                scratch_root=args.scratch_root,
                build_timeout=args.build_timeout,
                test_timeout=args.test_timeout,
            )
            artifact_path.parent.mkdir(parents=True, exist_ok=True)
            artifact_path.write_text(canonical_json(payload))
        else:
            payload = json.loads(artifact_path.read_text())
            failures = validate_live_artifact(root, payload)
            if artifact_path.read_text() != canonical_json(payload):
                failures.append("assertion_reachability artifact is not canonical JSON")
            if failures:
                raise ValueError("; ".join(failures))
        print(
            "P18 assertion reachability validated: "
            f"{payload['summary']['proven']}/{payload['summary']['denominator']} proven; "
            f"{payload['summary']['caught_mutants']} caught mutants"
        )
        return 0
    except (
        OSError,
        ValueError,
        KeyError,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
        MutationContractError,
    ) as error:
        print(f"P18 assertion reachability failed: {error}", file=os.sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
