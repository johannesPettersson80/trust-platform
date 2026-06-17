#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
OUT_DIR="$ROOT_DIR/docs/diagrams/generated"

mkdir -p "$OUT_DIR"

# Render all PlantUML diagrams to SVG. Prefer the official container when it is
# available, but allow builder environments to use the packaged PlantUML binary.
mapfile -t DIAGRAMS < <(find "$ROOT_DIR/docs/diagrams" -name "*.puml" -print)
if [[ "${#DIAGRAMS[@]}" -eq 0 ]]; then
  echo "No diagram found"
  exit 1
fi

REL_DIAGRAMS=()
for path in "${DIAGRAMS[@]}"; do
  REL_DIAGRAMS+=("${path#$ROOT_DIR/}")
done

if command -v docker >/dev/null 2>&1; then
  docker run --rm \
    -v "$ROOT_DIR":/workspace \
    -w /workspace \
    plantuml/plantuml:latest \
    -tsvg -o ../../diagrams/generated "${REL_DIAGRAMS[@]}"
elif command -v java >/dev/null 2>&1; then
  PLANTUML_JAR_PATH="${PLANTUML_JAR:-${XDG_CACHE_HOME:-$HOME/.cache}/plantuml/plantuml.jar}"
  if [[ ! -f "$PLANTUML_JAR_PATH" ]]; then
    if [[ "${PLANTUML_NO_DOWNLOAD:-0}" == "1" ]]; then
      echo "PlantUML jar not found at $PLANTUML_JAR_PATH and PLANTUML_NO_DOWNLOAD=1" >&2
      exit 1
    fi
    mkdir -p "$(dirname "$PLANTUML_JAR_PATH")"
    TMP_JAR="${PLANTUML_JAR_PATH}.tmp"
    if command -v curl >/dev/null 2>&1; then
      curl -fsSL \
        https://github.com/plantuml/plantuml/releases/latest/download/plantuml.jar \
        -o "$TMP_JAR"
    elif command -v wget >/dev/null 2>&1; then
      wget -q \
        https://github.com/plantuml/plantuml/releases/latest/download/plantuml.jar \
        -O "$TMP_JAR"
    else
      echo "PlantUML jar download requires curl or wget" >&2
      exit 1
    fi
    mv "$TMP_JAR" "$PLANTUML_JAR_PATH"
  fi
  (
    cd "$ROOT_DIR"
    java -jar "$PLANTUML_JAR_PATH" -tsvg -o ../../diagrams/generated "${REL_DIAGRAMS[@]}"
  )
elif command -v plantuml >/dev/null 2>&1; then
  (
    cd "$ROOT_DIR"
    plantuml -tsvg -o ../../diagrams/generated "${REL_DIAGRAMS[@]}"
  )
else
  echo "PlantUML rendering requires docker or plantuml on PATH" >&2
  exit 1
fi

if [[ -n "${PYTHON:-}" ]]; then
  PYTHON_BIN="$PYTHON"
elif command -v python >/dev/null 2>&1; then
  PYTHON_BIN=python
elif command -v python3 >/dev/null 2>&1; then
  PYTHON_BIN=python3
else
  echo "python or python3 is required to update the diagram manifest" >&2
  exit 1
fi

"$PYTHON_BIN" scripts/check_diagram_drift.py --update
