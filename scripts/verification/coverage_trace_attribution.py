"""LLVM source-region attribution for focused per-test traces."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

from .code_spec_common import CodeSourceSpan

ROOT_FIELDS = frozenset({"type", "version", "data"})
DATA_FIELDS = frozenset({"files", "functions", "totals"})
FILE_FIELDS = frozenset(
    {
        "filename",
        "regions",
        "segments",
        "summary",
        "branches",
        "mcdc_records",
        "expansions",
    }
)
FUNCTION_FIELDS = frozenset(
    {"name", "count", "filenames", "regions", "branches", "mcdc_records"}
)


CodeSpan = CodeSourceSpan


@dataclass(frozen=True)
class CoveredRegion:
    """One executed LLVM code region converted to repository byte offsets."""

    path: str
    start: int
    end: int
    count: int


def parse_llvm_export(
    document: Mapping[str, Any],
    *,
    root: Path,
    sources: Mapping[str, bytes],
) -> tuple[CoveredRegion, ...]:
    """Parse the closed subset of `llvm-cov export` needed for attribution."""

    _require_exact_fields(document, ROOT_FIELDS, "LLVM export root")
    if document.get("type") != "llvm.coverage.json.export":
        raise ValueError("LLVM export type must equal llvm.coverage.json.export")
    version = document.get("version")
    if not isinstance(version, str) or not version.startswith(("2.", "3.")):
        raise ValueError(f"unsupported LLVM coverage export version {version!r}")
    data = _object_list(document.get("data"), "LLVM export data")
    normalized_sources = {
        _normalize_repo_path(path): content for path, content in sources.items()
    }
    regions: list[CoveredRegion] = []
    for data_index, record in enumerate(data):
        label = f"LLVM export data[{data_index}]"
        _require_allowed_fields(record, DATA_FIELDS, label)
        files = _object_list(record.get("files"), f"{label}.files")
        for file_index, file_record in enumerate(files):
            file_label = f"{label}.files[{file_index}]"
            _require_allowed_fields(file_record, FILE_FIELDS, file_label)
            path = _repo_path(root, _string(file_record.get("filename"), file_label))
            if path is None:
                continue
            source = normalized_sources.get(path)
            if source is None:
                continue
            raw_regions = file_record.get("regions", [])
            if not isinstance(raw_regions, Sequence) or isinstance(
                raw_regions, (str, bytes)
            ):
                raise ValueError(f"{file_label}.regions must be an array")
            for region_index, raw_region in enumerate(raw_regions):
                region_label = f"{file_label}.regions[{region_index}]"
                values = _region_values(raw_region, region_label)
                (
                    line_start,
                    column_start,
                    line_end,
                    column_end,
                    count,
                    _file_id,
                    _expanded_file_id,
                    region_kind,
                ) = values
                if count <= 0 or region_kind != 0:
                    continue
                start = _source_offset(source, line_start, column_start, region_label)
                end = _source_offset(source, line_end, column_end, region_label)
                if end <= start:
                    raise ValueError(f"{region_label} has an empty or reversed range")
                regions.append(CoveredRegion(path, start, end, count))
            raw_segments = file_record.get("segments", [])
            if not isinstance(raw_segments, Sequence) or isinstance(
                raw_segments, (str, bytes)
            ):
                raise ValueError(f"{file_label}.segments must be an array")
            segments = [
                _segment_values(raw_segment, f"{file_label}.segments[{index}]")
                for index, raw_segment in enumerate(raw_segments)
            ]
            for segment_index, segment in enumerate(segments):
                line, column, count, has_count, _is_entry, is_gap = segment
                if count <= 0 or not has_count or is_gap:
                    continue
                segment_label = f"{file_label}.segments[{segment_index}]"
                start = _source_offset(source, line, column, segment_label)
                if segment_index + 1 < len(segments):
                    next_line, next_column, *_rest = segments[segment_index + 1]
                    end = _source_offset(
                        source,
                        next_line,
                        next_column,
                        f"{file_label}.segments[{segment_index + 1}]",
                    )
                else:
                    end = len(source)
                if end < start:
                    raise ValueError(f"{segment_label} is not source ordered")
                if end > start:
                    regions.append(CoveredRegion(path, start, end, count))
        functions = _object_list(record.get("functions", []), f"{label}.functions")
        for function_index, function in enumerate(functions):
            function_label = f"{label}.functions[{function_index}]"
            _require_allowed_fields(function, FUNCTION_FIELDS, function_label)
            filenames = function.get("filenames")
            if not isinstance(filenames, Sequence) or isinstance(
                filenames, (str, bytes)
            ) or any(not isinstance(item, str) or not item for item in filenames):
                raise ValueError(f"{function_label}.filenames must be a string array")
            raw_regions = function.get("regions")
            if not isinstance(raw_regions, Sequence) or isinstance(
                raw_regions, (str, bytes)
            ):
                raise ValueError(f"{function_label}.regions must be an array")
            for region_index, raw_region in enumerate(raw_regions):
                region_label = f"{function_label}.regions[{region_index}]"
                values = _region_values(raw_region, region_label)
                (
                    line_start,
                    column_start,
                    line_end,
                    column_end,
                    count,
                    file_id,
                    _expanded_file_id,
                    region_kind,
                ) = values
                if count <= 0 or region_kind != 0:
                    continue
                if file_id < 0 or file_id >= len(filenames):
                    raise ValueError(f"{region_label} has invalid filename index")
                path = _repo_path(root, filenames[file_id])
                if path is None:
                    continue
                source = normalized_sources.get(path)
                if source is None:
                    continue
                start = _source_offset(source, line_start, column_start, region_label)
                end = _source_offset(source, line_end, column_end, region_label)
                if end <= start:
                    raise ValueError(f"{region_label} has an empty or reversed range")
                regions.append(CoveredRegion(path, start, end, count))
    return tuple(sorted(set(regions), key=lambda item: (item.path, item.start, item.end, item.count)))


def attributed_code_ids(
    regions: Sequence[CoveredRegion],
    spans: Mapping[str, CodeSpan],
) -> tuple[str, ...]:
    """Return live code facts intersected by at least one executed code region."""

    covered: set[str] = set()
    for code_id, span in spans.items():
        if code_id != span.code_id:
            raise ValueError(f"code span key does not match embedded ID {code_id}")
        if span.body_start < 0 or span.body_end <= span.body_start:
            raise ValueError(f"{code_id} has invalid source span")
        if any(
            region.path == span.path
            and region.start < span.body_end
            and span.body_start < region.end
            for region in regions
        ):
            covered.add(code_id)
    return tuple(sorted(covered))


def _source_offset(source: bytes, line: int, column: int, label: str) -> int:
    if line < 1 or column < 1:
        raise ValueError(f"{label} line and column must be positive")
    line_starts = [0]
    line_starts.extend(index + 1 for index, value in enumerate(source) if value == 10)
    if line > len(line_starts):
        raise ValueError(f"{label} line {line} is outside source")
    start = line_starts[line - 1]
    next_start = line_starts[line] if line < len(line_starts) else len(source)
    offset = start + column - 1
    if offset > next_start:
        raise ValueError(f"{label} column {column} is outside source line")
    if offset < len(source) and 0x80 <= source[offset] <= 0xBF:
        raise ValueError(f"{label} column {column} is not a UTF-8 boundary")
    return offset


def _segment_values(
    value: Any,
    label: str,
) -> tuple[int, int, int, bool, bool, bool]:
    if (
        not isinstance(value, Sequence)
        or isinstance(value, (str, bytes))
        or len(value) != 6
    ):
        raise ValueError(f"{label} must contain three integers and three booleans")
    line, column, count, has_count, is_entry, is_gap = value
    if any(
        not isinstance(item, int) or isinstance(item, bool)
        for item in (line, column, count)
    ) or any(not isinstance(item, bool) for item in (has_count, is_entry, is_gap)):
        raise ValueError(f"{label} must contain three integers and three booleans")
    return line, column, count, has_count, is_entry, is_gap


def _repo_path(root: Path, filename: str) -> str | None:
    root = root.resolve()
    candidate = Path(filename)
    if candidate.is_absolute():
        resolved = candidate.resolve()
        try:
            relative = resolved.relative_to(root)
        except ValueError:
            return None
        return _normalize_repo_path(relative.as_posix())
    return _normalize_repo_path(filename)


def _normalize_repo_path(path: str) -> str:
    normalized = path.replace("\\", "/")
    parsed = PurePosixPath(normalized)
    if parsed.is_absolute() or not normalized or any(part == ".." for part in parsed.parts):
        raise ValueError(f"coverage source path must stay repository-relative: {path!r}")
    parts = tuple(part for part in parsed.parts if part not in {"", "."})
    if not parts:
        raise ValueError(f"coverage source path must name a file: {path!r}")
    return PurePosixPath(*parts).as_posix()


def _region_values(value: Any, label: str) -> tuple[int, ...]:
    if (
        not isinstance(value, Sequence)
        or isinstance(value, (str, bytes))
        or len(value) != 8
        or any(not isinstance(item, int) or isinstance(item, bool) for item in value)
    ):
        raise ValueError(f"{label} must contain exactly eight integers")
    return tuple(value)


def _object_list(value: Any, label: str) -> list[Mapping[str, Any]]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be an array")
    if any(not isinstance(item, Mapping) for item in value):
        raise ValueError(f"{label} must contain only objects")
    return list(value)


def _string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires a non-empty string")
    return value


def _require_exact_fields(
    record: Mapping[str, Any],
    expected: frozenset[str],
    label: str,
) -> None:
    actual = set(record)
    if actual != expected:
        raise ValueError(
            f"{label} has unexpected fields: missing={sorted(expected - actual)}, "
            f"extra={sorted(actual - expected)}"
        )


def _require_allowed_fields(
    record: Mapping[str, Any],
    allowed: frozenset[str],
    label: str,
) -> None:
    extra = sorted(set(record) - allowed)
    if extra:
        raise ValueError(f"{label} has unexpected fields: extra={extra}")
