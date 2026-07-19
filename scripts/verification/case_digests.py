"""Digest helpers for generated verification case files."""

from __future__ import annotations

import hashlib
from pathlib import Path

from .metadata_validator.constants import ROOT


GENERATOR_DIGEST_PATHS = [
    ROOT / "scripts/gen_cases.py",
    ROOT / "scripts/verification/bytecode_transforms.py",
    ROOT / "scripts/verification/case_generator.py",
    ROOT / "scripts/verification/case_digests.py",
    ROOT / "scripts/verification/metadata_validator/constants.py",
]


def file_digest(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def current_generator_digest() -> str:
    digest = hashlib.sha256()
    for path in GENERATOR_DIGEST_PATHS:
        digest.update(str(path.relative_to(ROOT)).encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return "sha256:" + digest.hexdigest()
