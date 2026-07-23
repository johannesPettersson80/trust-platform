"""Istanbul source-range attribution for focused TypeScript test traces."""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path, PurePosixPath
from typing import Any
from urllib.parse import unquote, urlparse

from .coverage_trace_attribution import CoveredRegion


def parse_istanbul_coverage(
    document: Mapping[str, Any],
    *,
    root: Path,
    sources: Mapping[str, bytes],
) -> tuple[CoveredRegion, ...]:
    """Return executed original-source ranges from a coverage-final document."""

    normalized_sources = {
        _normalize_repo_path(path): content for path, content in sources.items()
    }
    regions: list[CoveredRegion] = []
    for raw_key, raw_record in sorted(document.items(), key=lambda item: str(item[0])):
        if not isinstance(raw_key, str) or not raw_key:
            raise ValueError("Istanbul coverage keys must be non-empty paths")
        if not isinstance(raw_record, Mapping):
            raise ValueError(f"Istanbul coverage record {raw_key!r} must be an object")
        raw_path = raw_record.get("path", raw_key)
        if not isinstance(raw_path, str) or not raw_path:
            raise ValueError(f"Istanbul coverage record {raw_key!r} requires path")
        path = _repo_path(root, raw_path)
        if path is None or path not in normalized_sources:
            continue
        source = normalized_sources[path]
        regions.extend(
            _counted_regions(
                path=path,
                source=source,
                locations=raw_record.get("statementMap"),
                counts=raw_record.get("s"),
                label=f"{path} statements",
            )
        )
        regions.extend(
            _function_regions(
                path=path,
                source=source,
                functions=raw_record.get("fnMap"),
                counts=raw_record.get("f"),
                label=f"{path} functions",
            )
        )
    return tuple(
        sorted(
            set(regions),
            key=lambda region: (
                region.path,
                region.start,
                region.end,
                region.count,
            ),
        )
    )


def merge_istanbul_coverage(
    documents: list[Mapping[str, Any]],
) -> dict[str, Any]:
    """Merge identical Istanbul maps by summing counters deterministically."""

    merged: dict[str, Any] = {}
    for document in documents:
        for path, raw_record in sorted(document.items()):
            if not isinstance(path, str) or not isinstance(raw_record, Mapping):
                raise ValueError("Istanbul merge requires path/object records")
            record = dict(raw_record)
            if path not in merged:
                merged[path] = record
                continue
            current = merged[path]
            for map_field, count_field in (
                ("statementMap", "s"),
                ("fnMap", "f"),
                ("branchMap", "b"),
            ):
                if current.get(map_field) != record.get(map_field):
                    raise ValueError(
                        f"Istanbul {path} {map_field} changed between profiles"
                    )
                current[count_field] = _sum_counters(
                    current.get(count_field),
                    record.get(count_field),
                    f"{path}.{count_field}",
                )
    return dict(sorted(merged.items()))


def _sum_counters(left: Any, right: Any, label: str) -> dict[str, Any]:
    if not isinstance(left, Mapping) or not isinstance(right, Mapping):
        raise ValueError(f"Istanbul {label} counters must be objects")
    if set(left) != set(right):
        raise ValueError(f"Istanbul {label} counter identities changed")
    summed: dict[str, Any] = {}
    for key in sorted(left):
        left_value = left[key]
        right_value = right[key]
        if isinstance(left_value, list) and isinstance(right_value, list):
            if (
                len(left_value) != len(right_value)
                or any(
                    not isinstance(item, int) or isinstance(item, bool)
                    for item in [*left_value, *right_value]
                )
            ):
                raise ValueError(f"Istanbul {label}[{key}] branch counts are invalid")
            summed[key] = [
                left_item + right_item
                for left_item, right_item in zip(
                    left_value, right_value, strict=True
                )
            ]
        elif (
            isinstance(left_value, int)
            and not isinstance(left_value, bool)
            and isinstance(right_value, int)
            and not isinstance(right_value, bool)
        ):
            summed[key] = left_value + right_value
        else:
            raise ValueError(f"Istanbul {label}[{key}] count is invalid")
    return summed


def _counted_regions(
    *,
    path: str,
    source: bytes,
    locations: Any,
    counts: Any,
    label: str,
) -> list[CoveredRegion]:
    location_map = _object_map(locations, f"{label}.map")
    count_map = _count_map(counts, f"{label}.counts")
    if set(location_map) != set(count_map):
        raise ValueError(
            f"{label} map/count identities differ: "
            f"map_only={sorted(set(location_map) - set(count_map))}, "
            f"counts_only={sorted(set(count_map) - set(location_map))}"
        )
    return [
        _covered_region(path, source, location_map[item_id], count, label)
        for item_id, count in sorted(count_map.items())
        if count > 0
    ]


def _function_regions(
    *,
    path: str,
    source: bytes,
    functions: Any,
    counts: Any,
    label: str,
) -> list[CoveredRegion]:
    function_map = _object_map(functions, f"{label}.map")
    count_map = _count_map(counts, f"{label}.counts")
    if set(function_map) != set(count_map):
        raise ValueError(
            f"{label} map/count identities differ: "
            f"map_only={sorted(set(function_map) - set(count_map))}, "
            f"counts_only={sorted(set(count_map) - set(function_map))}"
        )
    regions: list[CoveredRegion] = []
    for item_id, count in sorted(count_map.items()):
        if count <= 0:
            continue
        record = function_map[item_id]
        location = record.get("loc")
        if not isinstance(location, Mapping):
            raise ValueError(f"{label}[{item_id}] requires loc")
        regions.append(_covered_region(path, source, location, count, label))
    return regions


def _covered_region(
    path: str,
    source: bytes,
    location: Mapping[str, Any],
    count: int,
    label: str,
) -> CoveredRegion:
    start = _position(location.get("start"), source, f"{label}.start")
    end = _position(location.get("end"), source, f"{label}.end")
    if end <= start:
        raise ValueError(f"{label} has an empty or reversed source range")
    return CoveredRegion(path=path, start=start, end=end, count=count)


def _position(value: Any, source: bytes, label: str) -> int:
    if not isinstance(value, Mapping):
        raise ValueError(f"{label} must be an object")
    line = value.get("line")
    column = value.get("column")
    if (
        not isinstance(line, int)
        or isinstance(line, bool)
        or line < 1
        or not isinstance(column, int)
        or isinstance(column, bool)
        or column < 0
    ):
        raise ValueError(f"{label} requires positive line and nonnegative column")
    line_starts = [0]
    line_starts.extend(
        index + 1 for index, byte in enumerate(source) if byte == 0x0A
    )
    if line > len(line_starts):
        raise ValueError(f"{label} line is outside source")
    line_start = line_starts[line - 1]
    line_end = line_starts[line] if line < len(line_starts) else len(source)
    try:
        text = source[line_start:line_end].decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValueError(f"{label} source is not UTF-8") from error
    if column > len(text):
        raise ValueError(f"{label} column is outside source line")
    return line_start + len(text[:column].encode("utf-8"))


def _object_map(value: Any, label: str) -> dict[str, Mapping[str, Any]]:
    if not isinstance(value, Mapping):
        raise ValueError(f"{label} must be an object")
    result: dict[str, Mapping[str, Any]] = {}
    for item_id, record in value.items():
        if not isinstance(item_id, str) or not item_id:
            raise ValueError(f"{label} keys must be non-empty strings")
        if not isinstance(record, Mapping):
            raise ValueError(f"{label}[{item_id}] must be an object")
        result[item_id] = record
    return result


def _count_map(value: Any, label: str) -> dict[str, int]:
    if not isinstance(value, Mapping):
        raise ValueError(f"{label} must be an object")
    result: dict[str, int] = {}
    for item_id, count in value.items():
        if (
            not isinstance(item_id, str)
            or not item_id
            or not isinstance(count, int)
            or isinstance(count, bool)
            or count < 0
        ):
            raise ValueError(f"{label} requires string keys and nonnegative counts")
        result[item_id] = count
    return result


def _repo_path(root: Path, raw_path: str) -> str | None:
    if raw_path.startswith("file://"):
        parsed = urlparse(raw_path)
        if parsed.scheme != "file":
            return None
        candidate = Path(unquote(parsed.path))
    else:
        candidate = Path(raw_path)
    if candidate.is_absolute():
        try:
            return _normalize_repo_path(
                candidate.resolve().relative_to(root.resolve()).as_posix()
            )
        except ValueError:
            return None
    return _normalize_repo_path(raw_path)


def _normalize_repo_path(path: str) -> str:
    normalized = path.replace("\\", "/")
    parsed = PurePosixPath(normalized)
    if parsed.is_absolute() or not normalized or any(
        part == ".." for part in parsed.parts
    ):
        raise ValueError(f"coverage source path must stay repository-relative: {path!r}")
    parts = tuple(part for part in parsed.parts if part not in {"", "."})
    if not parts:
        raise ValueError(f"coverage source path must name a file: {path!r}")
    return PurePosixPath(*parts).as_posix()
