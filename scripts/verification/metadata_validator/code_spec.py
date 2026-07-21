"""Phase 17 static metadata/schema validation for the primary metadata gate."""

from __future__ import annotations

import json
import tomllib
from pathlib import Path

from ..test_catalog_json_schema import validate_json_schema_instance
from ..code_spec_provenance import (
    CodeSpecProvenanceError,
    load_private_input_manifest,
    validate_private_input_bindings,
)


CODE_SPEC_SCHEMA_FILES = {
    "code-spec-map.schema.json",
    "code-spec-debt-baseline.schema.json",
    "code-spec-coverage-report.schema.json",
    "code-spec-private-inputs.schema.json",
    "code-spec-risk-acknowledgements.schema.json",
}

CODE_SPEC_STATIC_INPUTS = (
    (
        Path("verification/code-spec-map.toml"),
        Path("verification/schemas/code-spec-map.schema.json"),
    ),
    (
        Path("verification/code-spec-debt-baseline.toml"),
        Path("verification/schemas/code-spec-debt-baseline.schema.json"),
    ),
    (
        Path("verification/code-spec-private-inputs.toml"),
        Path("verification/schemas/code-spec-private-inputs.schema.json"),
    ),
    (
        Path("verification/code-spec-risk-acknowledgements.toml"),
        Path("verification/schemas/code-spec-risk-acknowledgements.schema.json"),
    ),
)


def validate_code_spec_metadata_instances(root: Path) -> list[tuple[Path, str]]:
    """Return static schema failures for committed Phase 17 metadata."""

    failures: list[tuple[Path, str]] = []
    schema_root = root / "verification/schemas"
    for name in sorted(CODE_SPEC_SCHEMA_FILES):
        path = Path("verification/schemas") / name
        try:
            schema = json.loads((schema_root / name).read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            failures.append((path, f"cannot load Phase 17 schema: {error}"))
            continue
        for field in ("$schema", "$id", "title", "type", "required", "properties"):
            if field not in schema:
                failures.append((path, f"Phase 17 schema missing {field}"))
        if schema.get("type") != "object":
            failures.append((path, "Phase 17 schema root type must be object"))
    for document_path, schema_path in CODE_SPEC_STATIC_INPUTS:
        try:
            document = tomllib.loads((root / document_path).read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError) as error:
            failures.append((document_path, f"cannot load Phase 17 metadata: {error}"))
            continue
        try:
            schema = json.loads((root / schema_path).read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            failures.append((schema_path, f"cannot load Phase 17 schema: {error}"))
            continue
        failures.extend(
            (document_path, failure)
            for failure in validate_json_schema_instance(document, schema)
        )
    try:
        private_inputs = load_private_input_manifest(root)
    except CodeSpecProvenanceError as error:
        failures.append(
            (Path("verification/code-spec-private-inputs.toml"), str(error))
        )
    else:
        failures.extend(
            (Path("verification/code-spec-private-inputs.toml"), failure)
            for failure in validate_private_input_bindings(root, private_inputs)
        )
    return failures
