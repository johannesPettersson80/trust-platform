#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="${ROOT_DIR}/docs/public/assets/images/one-project-surface-tour.gif"
WORK="${TMPDIR:-/tmp}/trust-one-project-surface-tour"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

require_cmd magick
GIFSICLE_BIN="${GIFSICLE:-}"
if [[ -z "${GIFSICLE_BIN}" ]] && command -v gifsicle >/dev/null 2>&1; then
  GIFSICLE_BIN="$(command -v gifsicle)"
fi

rm -rf "${WORK}"
mkdir -p "${WORK}/scenes" "${WORK}/frames"

surface_scene() {
  local src="$1"
  local out="$2"

  magick "${src}" \
    -resize "1280x720^" -gravity center -extent 1280x720 \
    -strip "${out}"
}

surface_scene \
  "${ROOT_DIR}/docs/public/assets/images/hero-runtime.png" \
  "${WORK}/scenes/01-vscode.png"
surface_scene \
  "${ROOT_DIR}/docs/public/assets/images/vscode/iec-diagnostics.png" \
  "${WORK}/scenes/02-diagnostics.png"
surface_scene \
  "${ROOT_DIR}/docs/public/assets/images/vscode/debugger-stopped-at-breakpoint.png" \
  "${WORK}/scenes/03-debug.png"
surface_scene \
  "${ROOT_DIR}/docs/public/assets/images/browser/ide-tutorial-loaded.png" \
  "${WORK}/scenes/04-browser-ide.png"
surface_scene \
  "${ROOT_DIR}/docs/public/assets/images/browser/hmi-home.png" \
  "${WORK}/scenes/05-hmi.png"

mapfile -t SCENES < <(find "${WORK}/scenes" -maxdepth 1 -type f -name "*.png" | sort)

frame_index=0
add_frame() {
  local src="$1"
  printf -v name "%s/frames/%04d.png" "${WORK}" "${frame_index}"
  cp "${src}" "${name}"
  frame_index=$((frame_index + 1))
}

blend_frame() {
  local left="$1"
  local right="$2"
  local pct="$3"
  printf -v name "%s/frames/%04d.png" "${WORK}" "${frame_index}"
  magick "${left}" "${right}" -define "compose:args=${pct}" -compose blend -composite "${name}"
  frame_index=$((frame_index + 1))
}

for ((i = 0; i < ${#SCENES[@]}; i++)); do
  for _ in $(seq 1 12); do
    add_frame "${SCENES[$i]}"
  done
  if (( i + 1 < ${#SCENES[@]} )); then
    for pct in 25 50 75; do
      blend_frame "${SCENES[$i]}" "${SCENES[$((i + 1))]}" "${pct}"
    done
  fi
done

mkdir -p "$(dirname "${OUT}")"
magick -delay 14 "${WORK}/frames/"*.png -loop 0 -layers OptimizeTransparency "${OUT}"

if [[ -n "${GIFSICLE_BIN}" ]]; then
  "${GIFSICLE_BIN}" --batch --optimize=3 --colors 256 "${OUT}"
else
  echo "gifsicle not found; kept ImageMagick-optimized GIF." >&2
fi

echo "wrote ${OUT}"
