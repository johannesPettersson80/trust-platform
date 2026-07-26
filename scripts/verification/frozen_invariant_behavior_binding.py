"""Bind frozen catalog plans to exact reviewed invariant behavior rows."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

from .frozen_catalog_identity import digest_document


def validate_invariant_behavior_digests(
    *,
    plan: Mapping[str, Any],
    invariant_ids: Sequence[str],
    invariants: Mapping[str, Mapping[str, Any]],
) -> None:
    digests = plan.get("invariant_behavior_digests")
    if not isinstance(digests, Mapping) or set(digests) != set(invariant_ids):
        raise ValueError(
            "plan invariant behavior digests do not equal selected invariants"
        )
    for invariant_id in invariant_ids:
        invariant = invariants.get(invariant_id)
        if invariant is None:
            raise ValueError(f"selected invariant {invariant_id} is missing")
        expected = digest_document({"behavior": invariant.get("behavior")})
        if digests.get(invariant_id) != expected:
            raise ValueError(
                f"selected invariant {invariant_id} behavior rows drifted"
            )
