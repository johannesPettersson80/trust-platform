#!/bin/bash
# Test hardware runtime with updated version
#
# Usage: sudo ./test-hardware-now.sh

if [ "$EUID" -ne 0 ]; then
  echo "Please run with sudo:"
  echo "  sudo $0"
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../statechart_backend" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
RUNTIME="$REPO_ROOT/target/release/trust-runtime"
SOCKET="/tmp/trust-debug.sock"

cd "$PROJECT_DIR"

if [ ! -f "$RUNTIME" ]; then
  RUNTIME="trust-runtime"
fi

echo "🔨 Building..."
$RUNTIME build --project .

echo ""
echo "✅ Build complete"
echo ""
echo "🚀 Starting runtime (Ctrl+C to stop)..."
echo "   EtherCAT: enp111s0"
echo "   Control endpoint: $SOCKET"
echo ""

# Remove old socket if exists
rm -f "$SOCKET"

# Start runtime in background
$RUNTIME run --project . &
RUNTIME_PID=$!

# Wait for socket to be created (max 5 seconds)
echo "⏳ Waiting for control endpoint..."
for i in {1..50}; do
  if [ -S "$SOCKET" ]; then
    # Keep socket writable by root + invoking user's group only.
    if [ -n "${SUDO_GID:-}" ]; then
      chgrp "$SUDO_GID" "$SOCKET" 2>/dev/null || true
    fi
    chmod 660 "$SOCKET"
    echo "✅ Control endpoint ready (rw-rw----)"
    break
  fi
  sleep 0.1
done

if [ ! -S "$SOCKET" ]; then
  echo "❌ Failed to create control endpoint"
  kill $RUNTIME_PID 2>/dev/null
  exit 1
fi

echo ""
echo "🎯 Runtime is running. You can now:"
echo "   1. Open VS Code Extension Development Host (F5)"
echo "   2. Open a .statechart.json file"
echo "   3. Select 'Hardware' mode and click 'Start Hardware'"
echo ""
echo "Press Ctrl+C to stop the runtime"

# Wait for the runtime process
wait $RUNTIME_PID
