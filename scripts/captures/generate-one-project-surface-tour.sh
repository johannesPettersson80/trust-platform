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

FONT_REGULAR="${TRUST_CAPTURE_FONT_REGULAR:-DejaVu-Sans}"
FONT_BOLD="${TRUST_CAPTURE_FONT_BOLD:-DejaVu-Sans-Bold}"

captioned_scene() {
  local src="$1"
  local title="$2"
  local subtitle="$3"
  local out="$4"

  magick "${src}" \
    -resize "960x540^" -gravity center -extent 960x540 \
    \( -size 960x540 xc:none \
      -fill "rgba(2,6,23,0.97)" -draw "rectangle 0,0 960,140" \
      -fill "#f8fafc" -font "${FONT_BOLD}" -pointsize 28 -gravity northwest -annotate +48+38 "${title}" \
      -fill "#cbd5e1" -font "${FONT_REGULAR}" -pointsize 18 -gravity northwest -annotate +48+78 "${subtitle}" \
    \) -composite "${out}"
}

title_scene() {
  local out="$1"
  magick -size 960x540 "gradient:#0f172a-#0f766e" -dither FloydSteinberg \
    \( -size 960x540 xc:none \
      -fill "rgba(2,6,23,0.42)" -draw "rectangle 0,0 960,540" \
      -fill "#f8fafc" -font "${FONT_BOLD}" -pointsize 44 -gravity center -annotate +0-64 "One Project, Every Surface" \
      -fill "#d1fae5" -font "${FONT_REGULAR}" -pointsize 25 -gravity center -annotate +0-10 "VS Code, Diagnostics, Debug, Browser IDE, Browser HMI" \
      -fill "#f8fafc" -font "${FONT_BOLD}" -pointsize 24 -gravity center -annotate +0+52 "All live from the same truST project" \
    \) -composite "${out}"
}

title_scene "${WORK}/scenes/00-title.png"
captioned_scene \
  "${ROOT_DIR}/docs/public/assets/images/hero-runtime.png" \
  "VS Code engineering surface" \
  "Edit ST, inspect live I/O and memory, and debug beside the runtime panel." \
  "${WORK}/scenes/01-vscode.png"
captioned_scene \
  "${ROOT_DIR}/docs/public/assets/images/vscode/iec-diagnostics.png" \
  "Diagnostics as structured context" \
  "The editor and AI tools can start from real IEC-aware diagnostics." \
  "${WORK}/scenes/02-diagnostics.png"
captioned_scene \
  "${ROOT_DIR}/docs/public/assets/images/vscode/debugger-stopped-at-breakpoint.png" \
  "Live debug state" \
  "Breakpoints, locals, call stack, inline values, and runtime state stay together." \
  "${WORK}/scenes/03-debug.png"
captioned_scene \
  "${ROOT_DIR}/docs/public/assets/images/browser/ide-tutorial-loaded.png" \
  "Browser IDE" \
  "The same project can be opened through the runtime-hosted browser surface." \
  "${WORK}/scenes/04-browser-ide.png"
captioned_scene \
  "${ROOT_DIR}/docs/public/assets/images/browser/hmi-home.png" \
  "Browser HMI" \
  "Operators and technicians see the same running project from the HMI surface." \
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
  for _ in $(seq 1 22); do
    add_frame "${SCENES[$i]}"
  done
  if (( i + 1 < ${#SCENES[@]} )); then
    for pct in 15 30 45 60 75 90; do
      blend_frame "${SCENES[$i]}" "${SCENES[$((i + 1))]}" "${pct}"
    done
  fi
done

mkdir -p "$(dirname "${OUT}")"
magick -delay 8 "${WORK}/frames/"*.png -loop 0 -layers Optimize "${OUT}"

if [[ -n "${GIFSICLE_BIN}" ]]; then
  "${GIFSICLE_BIN}" --batch --optimize=3 --lossy=30 --colors 160 --dither "${OUT}"
else
  echo "gifsicle not found; kept ImageMagick-optimized GIF." >&2
fi

echo "wrote ${OUT}"
