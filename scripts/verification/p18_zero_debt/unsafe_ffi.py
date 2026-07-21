"""Unsafe/FFI validator boundary."""
from typing import Any, Mapping

from .proof_common import validate_proof_row


def validate(row: Mapping[str, Any], budget: Mapping[str, Any]) -> list[str]:
    return validate_proof_row(row, budget, "unsafe_ffi")
