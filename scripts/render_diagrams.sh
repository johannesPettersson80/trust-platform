#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
OUT_DIR="$ROOT_DIR/docs/diagrams/generated"

mkdir -p "$OUT_DIR"

# Render all PlantUML diagrams to SVG. Use the pinned official container so
# local and GitHub renders use the same fonts, Java, and Graphviz environment.
# Update the digest and committed SVGs together in a reviewed pull request.
DEFAULT_PLANTUML_IMAGE="plantuml/plantuml@sha256:47870c1f76cfb3747bc7090bfe83013a4e3105b5a0bb1515e2baf5d3e2b3ee9d"
PLANTUML_IMAGE="${PLANTUML_IMAGE:-$DEFAULT_PLANTUML_IMAGE}"
PLANTUML_JAR_SHA256="89948f14c93756c7a3fb7b69078ff37e8489fd79dd430c582b931e2f65358690"
PLANTUML_JAR_URL="https://github.com/plantuml/plantuml/releases/download/v1.2026.6/plantuml.jar"

mapfile -t DIAGRAMS < <(find "$ROOT_DIR/docs/diagrams" -name "*.puml" -print)
if [[ "${#DIAGRAMS[@]}" -eq 0 ]]; then
  echo "No diagram found"
  exit 1
fi

REL_DIAGRAMS=()
for path in "${DIAGRAMS[@]}"; do
  REL_DIAGRAMS+=("${path#$ROOT_DIR/}")
done

CONTAINER_RUNTIME="${PLANTUML_CONTAINER_RUNTIME:-}"
if [[ -z "$CONTAINER_RUNTIME" ]] && command -v docker >/dev/null 2>&1; then
  CONTAINER_RUNTIME=docker
elif [[ -z "$CONTAINER_RUNTIME" ]] && command -v podman >/dev/null 2>&1; then
  CONTAINER_RUNTIME=podman
fi

if [[ -n "$CONTAINER_RUNTIME" ]]; then
  "$CONTAINER_RUNTIME" run --rm \
    -v "$ROOT_DIR":/workspace \
    -w /workspace \
    "$PLANTUML_IMAGE" \
    -tsvg -o ../../diagrams/generated "${REL_DIAGRAMS[@]}"
elif [[ "${PLANTUML_ALLOW_HOST_RENDERER:-0}" == "1" ]]; then
  echo "WARNING: using the noncanonical host renderer; do not commit its SVG output." >&2
  if ! command -v java >/dev/null 2>&1; then
    echo "PLANTUML_ALLOW_HOST_RENDERER=1 requires java on PATH" >&2
    exit 1
  fi
  PLANTUML_JAR_PATH="${PLANTUML_JAR:-${XDG_CACHE_HOME:-$HOME/.cache}/plantuml/plantuml-1.2026.6.jar}"
  if [[ ! -f "$PLANTUML_JAR_PATH" ]]; then
    if [[ "${PLANTUML_NO_DOWNLOAD:-0}" == "1" ]]; then
      echo "PlantUML jar not found at $PLANTUML_JAR_PATH and PLANTUML_NO_DOWNLOAD=1" >&2
      exit 1
    fi
    mkdir -p "$(dirname "$PLANTUML_JAR_PATH")"
    TMP_JAR="${PLANTUML_JAR_PATH}.tmp"
    if command -v curl >/dev/null 2>&1; then
      curl -fsSL "$PLANTUML_JAR_URL" -o "$TMP_JAR"
    elif command -v wget >/dev/null 2>&1; then
      wget -q "$PLANTUML_JAR_URL" -O "$TMP_JAR"
    else
      echo "PlantUML jar download requires curl or wget" >&2
      exit 1
    fi
    mv "$TMP_JAR" "$PLANTUML_JAR_PATH"
  fi
  printf '%s  %s\n' "$PLANTUML_JAR_SHA256" "$PLANTUML_JAR_PATH" | sha256sum -c -
  (
    cd "$ROOT_DIR"
    java -jar "$PLANTUML_JAR_PATH" -tsvg -o ../../diagrams/generated "${REL_DIAGRAMS[@]}"
  )
else
  echo "Canonical PlantUML rendering requires Docker or Podman for $PLANTUML_IMAGE." >&2
  echo "Host Java/Graphviz changes SVG bytes; use PLANTUML_ALLOW_HOST_RENDERER=1 only for a noncanonical preview." >&2
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
