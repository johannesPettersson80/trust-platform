#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
ASSET_DIR="$ROOT_DIR/editors/vscode/assets"
EXTENSION_DEV_PATH="$ROOT_DIR/editors/vscode"
USER_DATA_DIR="/tmp/trust-readme-vscode-user-data"
EXTENSIONS_DIR="$USER_DATA_DIR/extensions"
DISPLAY_OUTPUT="${TRUST_SCREEN_OUTPUT:-}"
WINDOW_SETTLE_SECS="${TRUST_WINDOW_SETTLE_SECS:-7}"

usage() {
  cat <<'EOF'
Capture README screenshots fully automatically.

Usage:
  scripts/capture-readme-screenshots-auto.sh [--assets-dir <path>] [--extension-dev-path <path>] [--output <display-name>] [--no-optimize] [--no-build-extension]

Options:
      --assets-dir    Where screenshots are written (default: editors/vscode/assets)
      --extension-dev-path  VS Code extension development path (default: editors/vscode)
      --output        Wayland output name for grim (default: first output from wlr-randr)
      --no-optimize   Skip media normalization/compression step
      --no-build-extension  Skip npm compile step for extension dev host
  -h, --help          Show this help
EOF
}

DO_OPTIMIZE=1
DO_BUILD_EXTENSION=1

while [[ $# -gt 0 ]]; do
  case "$1" in
    --assets-dir)
      ASSET_DIR="${2:-}"
      shift 2
      ;;
    --extension-dev-path)
      EXTENSION_DEV_PATH="${2:-}"
      shift 2
      ;;
    --output)
      DISPLAY_OUTPUT="${2:-}"
      shift 2
      ;;
    --no-optimize)
      DO_OPTIMIZE=0
      shift
      ;;
    --no-build-extension)
      DO_BUILD_EXTENSION=0
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ "$ASSET_DIR" != /* ]]; then
  ASSET_DIR="$ROOT_DIR/$ASSET_DIR"
fi

if [[ "$EXTENSION_DEV_PATH" != /* ]]; then
  EXTENSION_DEV_PATH="$ROOT_DIR/$EXTENSION_DEV_PATH"
fi

mkdir -p "$ASSET_DIR"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require_cmd code
require_cmd ydotool
require_cmd wtype
require_cmd grim
require_cmd ffmpeg
require_cmd wlr-randr
require_cmd wlrctl
require_cmd npm

if [[ -z "$DISPLAY_OUTPUT" ]]; then
  DISPLAY_OUTPUT=$(wlr-randr | awk 'NR==1 {print $1; exit}')
fi

if [[ -z "$DISPLAY_OUTPUT" ]]; then
  echo "Could not detect a Wayland output for grim capture." >&2
  exit 1
fi

key() {
  ydotool key "$@"
}

type_text() {
  local text="$1"
  # wtype injects exact text and avoids keyboard-layout keycode mismatches.
  wtype "$text"
  sleep 0.12
}

prepare_profile() {
  rm -rf "$USER_DATA_DIR"
  mkdir -p "$USER_DATA_DIR/User"
  mkdir -p "$EXTENSIONS_DIR"
  cat >"$USER_DATA_DIR/User/settings.json" <<'JSON'
{
  "security.workspace.trust.enabled": false,
  "workbench.startupEditor": "none",
  "workbench.welcome.enabled": false,
  "workbench.tips.enabled": false,
  "window.commandCenter": false,
  "chat.commandCenter.enabled": false,
  "workbench.editor.enablePreview": false
}
JSON
}

build_extension() {
  if [[ ! -d "$EXTENSION_DEV_PATH" ]]; then
    echo "Extension development path does not exist: $EXTENSION_DEV_PATH" >&2
    exit 1
  fi
  if (( DO_BUILD_EXTENSION == 1 )); then
    echo "Building VS Code extension from $EXTENSION_DEV_PATH..."
    npm --prefix "$EXTENSION_DEV_PATH" run compile >/tmp/trust-readme-shots-build.log 2>&1
  fi
}

wait_for_code_window() {
  for _ in $(seq 1 40); do
    if wlrctl toplevel find app_id:code >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  echo "Timed out waiting for VS Code window." >&2
  exit 1
}

focus_and_fullscreen_code() {
  wlrctl toplevel focus app_id:code || true
  sleep 0.3
  wlrctl toplevel fullscreen app_id:code || true
  sleep 0.8
}

launch_code() {
  local file_path="$1"
  wlrctl toplevel close app_id:code >/dev/null 2>&1 || true
  sleep 0.8
  code --new-window \
    --user-data-dir "$USER_DATA_DIR" \
    --extensions-dir "$EXTENSIONS_DIR" \
    --extensionDevelopmentPath "$EXTENSION_DEV_PATH" \
    "$ROOT_DIR" \
    -g "$file_path" >/tmp/trust-readme-shots-code.log 2>&1 &
  wait_for_code_window
  sleep "$WINDOW_SETTLE_SECS"
  focus_and_fullscreen_code
}

close_window() {
  wlrctl toplevel close app_id:code >/dev/null 2>&1 || true
  sleep 1
}

capture() {
  local path="$1"
  grim -o "$DISPLAY_OUTPUT" "$path"
  echo "Captured $path"
}

shot_diagnostics() {
  local out="$ASSET_DIR/screenshot-diagnostics.png"
  launch_code "$ROOT_DIR/manual-tests/root-a/src/MissingEndIf.st:1"
  # Ctrl+Shift+M (Problems panel)
  key 29:1 42:1 50:1 50:0 42:0 29:0
  sleep 2
  capture "$out"
  close_window
}

shot_refactor() {
  local out="$ASSET_DIR/screenshot-refactor.png"
  launch_code "$ROOT_DIR/manual-tests/root-a/src/NamespaceA.st:1"
  # F1 command palette, then type a layout-safe query (no punctuation).
  key 59:1 59:0
  sleep 0.8
  type_text "move namespace"
  sleep 2
  capture "$out"
  close_window
}

shot_debug() {
  local out="$ASSET_DIR/screenshot-debug.png"
  launch_code "$ROOT_DIR/manual-tests/root-a/src/DebugMain.st:1"
  # Ctrl+Shift+D (Run and Debug view), then command palette query without punctuation.
  key 29:1 42:1 32:1 32:0 42:0 29:0
  sleep 1
  key 59:1 59:0
  sleep 0.8
  type_text "start debugging"
  sleep 2
  capture "$out"
  close_window
}

shot_ladder_editor() {
  local out="$ASSET_DIR/screenshot-ladder-editor.png"
  launch_code "$ROOT_DIR/examples/ladder/simple-start-stop.ladder.json:1"
  sleep 4
  capture "$out"
  close_window
}

shot_statechart_editor() {
  local out="$ASSET_DIR/screenshot-statechart-editor.png"
  launch_code "$ROOT_DIR/examples/statecharts/traffic-light.statechart.json:1"
  sleep 4
  capture "$out"
  close_window
}

shot_blockly_editor() {
  local out="$ASSET_DIR/screenshot-blockly-editor.png"
  launch_code "$ROOT_DIR/examples/blockly/snake-simple-v2.blockly.json:1"
  sleep 4
  capture "$out"
  close_window
}

shot_sfc_editor() {
  local out="$ASSET_DIR/screenshot-sfc-editor.png"
  launch_code "$ROOT_DIR/examples/sfc/ethercat-snake-simple.sfc.json:1"
  sleep 4
  capture "$out"
  close_window
}

echo "Using display output: $DISPLAY_OUTPUT"
prepare_profile
build_extension
shot_diagnostics
shot_refactor
shot_debug
shot_ladder_editor
shot_statechart_editor
shot_blockly_editor
shot_sfc_editor

if (( DO_OPTIMIZE == 1 )); then
  "$ROOT_DIR/scripts/prepare-readme-media.sh" --dir "$ASSET_DIR"
fi

echo "Done. Screenshots written to $ASSET_DIR"
