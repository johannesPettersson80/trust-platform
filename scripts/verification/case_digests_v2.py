"""Digest closure for the non-bytecode decision-table generator."""

from __future__ import annotations

import hashlib

from .metadata_validator.constants import ROOT


GENERATOR_DIGEST_PATHS = (
    ROOT / "scripts/gen_cases_v2.py",
    ROOT / "scripts/verification/case_generator_v2.py",
    ROOT / "scripts/verification/case_digests_v2.py",
    ROOT / "scripts/verification/case_generator.py",
    ROOT / "scripts/verification/case_digests.py",
    ROOT / "scripts/verification/execution_contract.py",
    ROOT / "scripts/verification/metadata_validator/constants.py",
)


def current_generator_digest() -> str:
    digest = hashlib.sha256()
    for path in GENERATOR_DIGEST_PATHS:
        digest.update(str(path.relative_to(ROOT)).encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return "sha256:" + digest.hexdigest()
