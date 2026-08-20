"""Shared release-evidence validation primitives.

This module is intentionally side-effect free. CLI entrypoints own filesystem
and GitHub access; tests and release workflows share these closed checks.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from datetime import date
from typing import Any, Iterable, Mapping


SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
MAX_EXCEPTION_DAYS = 90


class ReleaseEvidenceError(ValueError):
    """Release evidence violates the normative contract."""


def decode_github_api_object(
    body: bytes, *, status: int, endpoint: str
) -> dict[str, Any]:
    """Decode one successful GitHub response without leaking response content."""

    if not body:
        return {}
    try:
        text = body.decode("utf-8")
        payload = json.loads(text)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ReleaseEvidenceError(
            f"GitHub API endpoint {endpoint} returned HTTP {status} with malformed JSON"
        ) from exc
    if not isinstance(payload, dict):
        raise ReleaseEvidenceError(
            f"GitHub API endpoint {endpoint} returned HTTP {status} with a non-object JSON body"
        )
    return payload


@dataclass(frozen=True)
class DependencyException:
    advisory_id: str
    owner: str
    rationale: str
    removal: str
    reviewed: date
    expires: date | None


@dataclass(frozen=True)
class ReleaseArtifact:
    path: str
    kind: str
    platform: str
    sha256: str


def validate_dependency_exception(exception: DependencyException) -> None:
    for label, value in (
        ("advisory id", exception.advisory_id),
        ("owner", exception.owner),
        ("rationale", exception.rationale),
        ("removal", exception.removal),
    ):
        if not value.strip():
            raise ReleaseEvidenceError(f"dependency exception {label} is required")
    if exception.expires is None:
        raise ReleaseEvidenceError(
            f"dependency exception {exception.advisory_id} requires an expiry date"
        )
    duration = (exception.expires - exception.reviewed).days
    if duration < 0:
        raise ReleaseEvidenceError(
            f"dependency exception {exception.advisory_id} expires before review"
        )
    if duration > MAX_EXCEPTION_DAYS:
        raise ReleaseEvidenceError(
            f"dependency exception {exception.advisory_id} exceeds 90 days"
        )


def validate_release_artifacts(
    artifacts: Iterable[ReleaseArtifact], *, required_paths: set[str]
) -> None:
    rows = list(artifacts)
    paths = [row.path for row in rows]
    duplicates = sorted({path for path in paths if paths.count(path) > 1})
    if duplicates:
        raise ReleaseEvidenceError(
            "release artifact paths are duplicated: " + ", ".join(duplicates)
        )
    missing = sorted(required_paths - set(paths))
    extra = sorted(set(paths) - required_paths)
    if missing or extra:
        raise ReleaseEvidenceError(
            f"release artifact inventory mismatch; missing={missing}, extra={extra}"
        )
    for row in rows:
        if not row.path or not row.kind or not row.platform:
            raise ReleaseEvidenceError(
                f"release artifact {row.path!r} has incomplete identity"
            )
        if not SHA256_RE.fullmatch(row.sha256):
            raise ReleaseEvidenceError(
                f"release artifact {row.path!r} has invalid SHA-256"
            )


def validate_release_publication(
    *,
    expected_tag: str,
    release: Mapping[str, Any],
    latest_release: Mapping[str, Any],
    required_assets: set[str],
) -> None:
    if release.get("tag_name") != expected_tag:
        raise ReleaseEvidenceError(
            f"release tag {release.get('tag_name')!r} does not match {expected_tag}"
        )
    if release.get("draft") or release.get("prerelease"):
        raise ReleaseEvidenceError(f"release {expected_tag} is not a final publication")
    if latest_release.get("tag_name") != expected_tag:
        raise ReleaseEvidenceError(
            f"GitHub Latest points to {latest_release.get('tag_name')!r}, not {expected_tag}"
        )
    assets = release.get("assets", [])
    if not isinstance(assets, list):
        raise ReleaseEvidenceError(
            f"release {expected_tag} has a malformed asset collection"
        )
    asset_names: list[str] = []
    for asset in assets:
        if not isinstance(asset, Mapping):
            raise ReleaseEvidenceError(
                f"release {expected_tag} has an asset with incomplete identity"
            )
        name = asset.get("name")
        if not isinstance(name, str) or not name.strip():
            raise ReleaseEvidenceError(
                f"release {expected_tag} has an asset with incomplete identity"
            )
        asset_names.append(name)
    duplicates = sorted({name for name in asset_names if asset_names.count(name) > 1})
    if duplicates:
        raise ReleaseEvidenceError(
            "release asset names are duplicated: " + ", ".join(duplicates)
        )
    missing = sorted(required_assets - set(asset_names))
    if missing:
        raise ReleaseEvidenceError(
            f"release {expected_tag} is missing required assets: {', '.join(missing)}"
        )
