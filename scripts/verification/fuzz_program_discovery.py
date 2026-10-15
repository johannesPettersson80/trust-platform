"""Source-derived discovery for the Phase 9 fuzz-program inventory."""

from __future__ import annotations

import re
import subprocess
import tomllib
from collections.abc import Sequence
from dataclasses import asdict, dataclass, field
from pathlib import Path, PurePosixPath

from .test_catalog_models import InferredTestFact, ScanDiagnostic
from .test_catalog_rust import sanitize_rust, scan_rust_tests


FUZZ_LIKE_NAME_RE = re.compile(r"(?:^|_)fuzz(?:_|$)|(?:^|_)property_smoke(?:_|$)")
PROPERTY_FRAMEWORK_NAME_RE = re.compile(r"(?:^|_)(?:proptest|quickcheck)(?:_|$)")
UNMODELED_PROPERTY_FRAMEWORK_RE = re.compile(
    r"\b(?:proptest!\s*\{|quickcheck!\s*\{|quickcheck::|quickcheck_macros::quickcheck|bolero::)"
    r"|#\s*\[\s*(?:proptest|quickcheck)\b"
)
PROPTEST_BLOCK_RE = re.compile(r"\bproptest!\s*\{")
PROPTEST_TEST_FN_RE = re.compile(r"#\[test\]\s*(?:#\[[^\]]*\]\s*)*fn\s+([A-Za-z_][A-Za-z0-9_]*)")
PROPERTY_DRIFT_KIND = "property_framework_drift"


@dataclass(frozen=True)
class CargoFuzzFact:
    manifest_path: str
    package: str
    name: str
    path: str
    command: str
    corpus_path: str
    artifact_path: str

    @property
    def native_id(self) -> str:
        return f"{self.manifest_path}#{self.name}"

    def to_dict(self) -> dict[str, str]:
        return asdict(self)


@dataclass
class CargoFuzzScan:
    facts: list[CargoFuzzFact] = field(default_factory=list)
    diagnostics: list[ScanDiagnostic] = field(default_factory=list)
    input_paths: set[str] = field(default_factory=set)


@dataclass
class FuzzLikeScan:
    facts: list[InferredTestFact] = field(default_factory=list)
    diagnostics: list[ScanDiagnostic] = field(default_factory=list)
    input_paths: set[str] = field(default_factory=set)


def is_fuzz_like_test_name(name: str) -> bool:
    """Return whether a scanner test identity enters the reviewed candidate census."""

    lowered = name.lower()
    tokens = set(lowered.split("_"))
    generated_smoke = bool(
        tokens & {"random", "randomized", "arbitrary"}
        and tokens & {"smoke", "budget", "property"}
    )
    return bool(
        FUZZ_LIKE_NAME_RE.search(lowered)
        or generated_smoke
        or PROPERTY_FRAMEWORK_NAME_RE.search(lowered)
    )


def has_unmodeled_property_framework(text: str) -> bool:
    return bool(UNMODELED_PROPERTY_FRAMEWORK_RE.search(text))


@dataclass(frozen=True)
class ReviewedPropertyTestFile:
    """Pinned classification of a file whose tests use a property-test framework.

    The framework tests are property tests over valid generated inputs, checked against an
    oracle. They are not malformed-input fuzz targets, so they are not registered as smokes and
    the inventory counts do not include them. Every field is a source fact the classification
    depends on; a file whose source differs from these facts is refused again.
    """

    classification: str
    cfg_gate: str | None
    seed_literal: str
    cases_literal: str
    proptest_blocks: int
    test_names: tuple[str, ...]


REVIEWED_PROPERTY_TEST_FILES: dict[str, ReviewedPropertyTestFile] = {
    "crates/trust-runtime/tests/property_executor_differential.rs": ReviewedPropertyTestFile(
        classification=(
            "Property test over valid generated programs; the oracle is agreement between the "
            "stack, register and Tier-1 executors, not malformed-input fuzzing."
        ),
        cfg_gate=None,
        seed_literal="0x7472_7573_7445_5841",
        cases_literal="48",
        proptest_blocks=1,
        test_names=("executors_agree_on_generated_programs", "generated_programs_compile"),
    ),
    "crates/trust-runtime/tests/property_metamorphic.rs": ReviewedPropertyTestFile(
        classification=(
            "Property test over valid generated programs; the oracle is equal behaviour under "
            "semantics-preserving transformations, not malformed-input fuzzing."
        ),
        cfg_gate=None,
        seed_literal="0x7472_7573_744D_4554",
        cases_literal="48",
        proptest_blocks=1,
        test_names=(
            "artifact_round_trip_preserves_behaviour",
            "pou_order_preserves_behaviour",
            "positional_arguments_preserve_behaviour",
            "repeated_preparation_preserves_behaviour",
        ),
    ),
    "crates/trust-runtime/tests/stateful_lifecycle_sequences.rs": ReviewedPropertyTestFile(
        classification=(
            "Property test over generated operation sequences; the oracle is agreement with a "
            "spec state model, not malformed-input fuzzing."
        ),
        cfg_gate='feature = "full"',
        seed_literal="0x7472_7573_7453_4551",
        cases_literal="64",
        proptest_blocks=1,
        test_names=("sequences_follow_the_lifecycle_model",),
    ),
}


def reviewed_property_file_drift(
    text: str,
    reviewed: ReviewedPropertyTestFile,
    live_names: Sequence[str],
) -> list[str]:
    """Return every way a reviewed property-test file differs from its pinned classification.

    Comments and literals are blanked before counting, so only code can match. `live_names` are
    the scanner's test names for this file; each pinned test must be exactly one of them.
    """

    raw_lines = text.split("\n")
    code_lines = sanitize_rust(text).split("\n")
    code = "\n".join(code_lines)
    reasons: list[str] = []
    expected_gate = [] if reviewed.cfg_gate is None else [f"#![cfg({reviewed.cfg_gate})]"]
    gates = [
        raw.strip()
        for raw, code_line in zip(raw_lines, code_lines, strict=False)
        if code_line.strip().startswith("#![cfg(")
    ]
    if gates != expected_gate:
        reasons.append(f"cfg gate is {gates!r}, reviewed {expected_gate!r}")
    seeds = re.findall(r"^\s*const\s+SEED\s*:\s*u64\s*=\s*([^;]+?)\s*;", code, re.MULTILINE)
    if seeds != [reviewed.seed_literal]:
        reasons.append(f"SEED literal is {seeds!r}, reviewed {reviewed.seed_literal!r}")
    cases = re.findall(
        r"^\s*const\s+DEFAULT_CASES\s*:\s*u32\s*=\s*([^;]+?)\s*;", code, re.MULTILINE
    )
    if cases != [reviewed.cases_literal]:
        reasons.append(f"DEFAULT_CASES literal is {cases!r}, reviewed {reviewed.cases_literal!r}")
    if code.count("RngSeed::Fixed(SEED)") != 1:
        reasons.append("proptest RNG is not pinned to SEED exactly once")
    if code.count("unwrap_or(DEFAULT_CASES)") != 1:
        reasons.append("PROPTEST_CASES fallback is not DEFAULT_CASES exactly once")
    blocks, unclosed = _proptest_blocks(code)
    markers = UNMODELED_PROPERTY_FRAMEWORK_RE.findall(code)
    if unclosed:
        reasons.append("a proptest! block is not closed")
    if len(blocks) != reviewed.proptest_blocks or len(markers) != reviewed.proptest_blocks:
        reasons.append(
            f"found {len(blocks)} proptest! block(s) and {len(markers)} framework marker(s), "
            f"reviewed {reviewed.proptest_blocks}"
        )
    names = sorted(name for block in blocks for name in PROPTEST_TEST_FN_RE.findall(block))
    if names != sorted(reviewed.test_names):
        reasons.append(f"proptest tests are {names!r}, reviewed {sorted(reviewed.test_names)!r}")
    for name in sorted(set(names) | set(reviewed.test_names)):
        live_count = sum(1 for live in live_names if live == name)
        if live_count != 1:
            reasons.append(f"test {name} has {live_count} live scanner facts, expected 1")
    return reasons


def _proptest_blocks(code: str) -> tuple[list[str], bool]:
    """Return the text of each proptest! block in sanitized code, and whether any is unclosed."""

    blocks: list[str] = []
    unclosed = False
    for match in PROPTEST_BLOCK_RE.finditer(code):
        opening = match.end() - 1
        depth = 0
        closing = len(code)
        for index in range(opening, len(code)):
            if code[index] == "{":
                depth += 1
            elif code[index] == "}":
                depth -= 1
                if depth == 0:
                    closing = index + 1
                    break
        else:
            unclosed = True
        blocks.append(code[opening:closing])
    return blocks, unclosed


def _property_drift(relative: str, reasons: list[str]) -> ScanDiagnostic:
    return ScanDiagnostic(
        severity="error",
        kind=PROPERTY_DRIFT_KIND,
        path=relative,
        line=1,
        message="reviewed property test classification drifted: " + "; ".join(reasons),
    )


def scan_fuzz_like_tests(root: Path) -> FuzzLikeScan:
    """Reuse the production Rust scanner, then select the closed candidate vocabulary.

    A file with a property-framework marker is refused unless it is listed in
    REVIEWED_PROPERTY_TEST_FILES and its source matches that classification exactly.
    """

    result = scan_rust_tests(root.resolve())
    diagnostics = list(result.diagnostics)
    for relative in sorted(set(result.input_paths) | set(REVIEWED_PROPERTY_TEST_FILES)):
        reviewed = REVIEWED_PROPERTY_TEST_FILES.get(relative)
        try:
            text = (root / relative).read_text()
        except (OSError, UnicodeError) as exc:
            if reviewed is not None:
                diagnostics.append(_property_drift(relative, [f"file cannot be read: {exc}"]))
            continue
        if reviewed is not None:
            live_names = [fact.name for fact in result.facts if fact.path == relative]
            reasons = reviewed_property_file_drift(text, reviewed, live_names)
            if reasons:
                diagnostics.append(_property_drift(relative, reasons))
            continue
        match = UNMODELED_PROPERTY_FRAMEWORK_RE.search(text)
        if match:
            diagnostics.append(
                ScanDiagnostic(
                    severity="error",
                    kind="unsupported_fuzz_like_framework",
                    path=relative,
                    line=text.count("\n", 0, match.start()) + 1,
                    message="property/fuzz framework marker requires an explicit Phase 9 discovery contract",
                )
            )
    return FuzzLikeScan(
        facts=sorted(
            (fact for fact in result.facts if is_fuzz_like_test_name(fact.name)),
            key=lambda fact: (fact.path, fact.name, fact.stable_id),
        ),
        diagnostics=diagnostics,
        input_paths=set(result.input_paths),
    )


def scan_cargo_fuzz_targets(
    root: Path,
    *,
    tracked_paths: set[str] | None = None,
) -> CargoFuzzScan:
    """Discover every tracked root or crate-local cargo-fuzz manifest target."""

    root = root.resolve()
    tracked = tracked_paths if tracked_paths is not None else _git_tracked_paths(root)
    result = CargoFuzzScan()
    manifests = sorted(
        path
        for path in tracked
        if PurePosixPath(path).parts[-2:] == ("fuzz", "Cargo.toml")
    )
    for manifest_path in manifests:
        _scan_manifest(root, manifest_path, tracked, result)
    result.facts.sort(key=lambda fact: (fact.manifest_path, fact.name, fact.path))
    result.diagnostics.sort(
        key=lambda item: (item.path, item.line, item.severity, item.kind, item.message)
    )
    return result


def _scan_manifest(
    root: Path,
    manifest_path: str,
    tracked: set[str],
    result: CargoFuzzScan,
) -> None:
    manifest = root / manifest_path
    result.input_paths.add(manifest_path)
    if not _regular_contained_file(root, manifest):
        result.diagnostics.append(
            _diagnostic("fuzz_manifest_path", manifest_path, "manifest is missing, escaping, or symlinked")
        )
        return
    try:
        text = manifest.read_text()
        data = tomllib.loads(text)
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as exc:
        result.diagnostics.append(_diagnostic("fuzz_manifest_parse", manifest_path, str(exc)))
        return
    package = data.get("package")
    if not isinstance(package, dict):
        result.diagnostics.append(_diagnostic("fuzz_manifest_package", manifest_path, "package table is missing"))
        return
    package_name = package.get("name")
    metadata = package.get("metadata")
    cargo_fuzz = metadata.get("cargo-fuzz") if isinstance(metadata, dict) else None
    if not isinstance(package_name, str) or not package_name or cargo_fuzz is not True:
        result.diagnostics.append(
            _diagnostic(
                "fuzz_manifest_contract",
                manifest_path,
                "package name and package.metadata.cargo-fuzz = true are required",
            )
        )
        return
    manifest_parent = PurePosixPath(manifest_path).parent
    declared_paths: set[str] = set()
    seen_names: set[str] = set()
    bins = data.get("bin")
    if not isinstance(bins, list) or not bins:
        result.diagnostics.append(_diagnostic("fuzz_target_missing", manifest_path, "manifest has no [[bin]] targets"))
        return
    for item in bins:
        if not isinstance(item, dict):
            result.diagnostics.append(_diagnostic("fuzz_target_invalid", manifest_path, "bin entry must be a table"))
            continue
        name = item.get("name")
        value = item.get("path")
        if not isinstance(name, str) or not name or not isinstance(value, str) or not value:
            result.diagnostics.append(_diagnostic("fuzz_target_invalid", manifest_path, "bin needs nonempty name and path"))
            continue
        if name in seen_names:
            result.diagnostics.append(_diagnostic("fuzz_target_duplicate", manifest_path, f"duplicate target name {name}"))
            continue
        seen_names.add(name)
        if any(item.get(field, False) is not False for field in ("test", "doc", "bench")):
            result.diagnostics.append(
                _diagnostic(
                    "fuzz_target_contract",
                    manifest_path,
                    f"target {name} must set test/doc/bench = false",
                )
            )
            continue
        relative = _target_path(manifest_parent, value)
        if relative is None:
            result.diagnostics.append(
                _diagnostic("fuzz_target_path", manifest_path, f"target escapes fuzz_targets: {value}")
            )
            continue
        target_path = relative.as_posix()
        declared_paths.add(target_path)
        target = root / target_path
        if target_path not in tracked or not _regular_contained_file(root, target):
            result.diagnostics.append(
                _diagnostic("fuzz_target_missing", target_path, "tracked regular target file is missing")
            )
            continue
        result.input_paths.add(target_path)
        workspace = manifest_parent.as_posix()
        prefix = f"cd {workspace} && " if workspace != "." else ""
        result.facts.append(
            CargoFuzzFact(
                manifest_path=manifest_path,
                package=package_name,
                name=name,
                path=target_path,
                command=f"{prefix}cargo fuzz run {name}",
                corpus_path=(manifest_parent / "corpus" / name).as_posix(),
                artifact_path=(manifest_parent / "artifacts" / name).as_posix(),
            )
        )
    target_root = (manifest_parent / "fuzz_targets").as_posix().rstrip("/") + "/"
    for path in sorted(
        value for value in tracked if value.startswith(target_root) and value.endswith(".rs")
    ):
        if path not in declared_paths:
            result.input_paths.add(path)
            result.diagnostics.append(
                _diagnostic("fuzz_target_unregistered", path, "tracked fuzz target is absent from manifest [[bin]] entries")
            )


def _target_path(manifest_parent: PurePosixPath, value: str) -> PurePosixPath | None:
    if not value or "\\" in value:
        return None
    relative = PurePosixPath(value)
    if relative.is_absolute() or ".." in relative.parts or "." in relative.parts:
        return None
    if len(relative.parts) != 2 or relative.parts[0] != "fuzz_targets" or relative.suffix != ".rs":
        return None
    return manifest_parent / relative


def _git_tracked_paths(root: Path) -> set[str]:
    try:
        process = subprocess.run(
            ["git", "-C", str(root), "ls-files", "-z"],
            check=False,
            capture_output=True,
        )
    except OSError as exc:
        raise ValueError(f"could not enumerate tracked fuzz sources: {exc}") from exc
    if process.returncode != 0:
        raise ValueError("could not enumerate tracked fuzz sources")
    return {item.decode() for item in process.stdout.split(b"\0") if item}


def _regular_contained_file(root: Path, path: Path) -> bool:
    try:
        path.resolve(strict=True).relative_to(root)
    except (OSError, ValueError):
        return False
    current = root
    try:
        relative = path.relative_to(root)
    except ValueError:
        return False
    for part in relative.parts:
        current /= part
        if current.is_symlink():
            return False
    return path.is_file()


def _diagnostic(kind: str, path: str, message: str) -> ScanDiagnostic:
    return ScanDiagnostic(severity="error", kind=kind, path=path, line=1, message=message)
