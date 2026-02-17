#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PORT="${PORT:-4175}"
HOST="${HOST:-127.0.0.1}"
REPO_NAME="${REPO_NAME:-$(basename "${ROOT_DIR}")}"
BUILD=true
BUILD_ONLY=false

usage() {
  cat <<EOF
Usage: $(basename "$0") [--port <port>] [--host <host>] [--repo <name>] [--no-build] [--build-only]

Builds docs/demo assets and serves a local GitHub Pages replica at /<repo>/.

Options:
  --port <port>   HTTP port (default: ${PORT})
  --host <host>   Bind host (default: ${HOST})
  --repo <name>   URL path prefix, e.g. trust-platform (default: ${REPO_NAME})
  --no-build      Skip scripts/build_demo.sh
  --build-only    Build demo assets and exit
  -h, --help      Show this help message

Example:
  scripts/run_demo_local_replica.sh --port 4175 --repo trust-platform
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port)
      PORT="${2:-}"
      shift 2
      ;;
    --host)
      HOST="${2:-}"
      shift 2
      ;;
    --repo)
      REPO_NAME="${2:-}"
      shift 2
      ;;
    --no-build)
      BUILD=false
      shift
      ;;
    --build-only)
      BUILD_ONLY=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument '$1'"
      usage
      exit 1
      ;;
  esac
done

if [[ "${BUILD}" == true ]]; then
  "${ROOT_DIR}/scripts/build_demo.sh"
fi

if [[ "${BUILD_ONLY}" == true ]]; then
  exit 0
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "error: python3 is required."
  exit 1
fi

exec python3 "${ROOT_DIR}/scripts/serve_demo_local_replica.py" \
  --host "${HOST}" \
  --port "${PORT}" \
  --repo "${REPO_NAME}"
