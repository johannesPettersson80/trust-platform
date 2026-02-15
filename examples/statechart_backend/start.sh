#!/bin/bash
# Start statechart backend runtime
#
# This script must be run with sudo for hardware access (EtherCAT/GPIO)
# Usage: sudo ./start.sh

set -e

if [ "$EUID" -ne 0 ]; then
  echo "❌ This script must be run with sudo"
  echo "   Usage: sudo ./start.sh"
  exit 1
fi

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUNTIME="../../target/release/trust-runtime"
SOCKET="/tmp/trust-debug.sock"

cd "$PROJECT_DIR"

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  StateChart Backend - trust-runtime"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Check if compiled runtime exists, otherwise use system
if [ ! -f "$RUNTIME" ]; then
  echo "ℹ️  Using system trust-runtime (not $RUNTIME)"
  RUNTIME="trust-runtime"
fi

# Build project
echo "🔨 Building project..."
$RUNTIME build --project .

echo ""
echo "✅ Build complete"
echo ""

# Clean old socket
rm -f "$SOCKET"

echo "🚀 Starting runtime..."
echo "   Control endpoint: $SOCKET"
echo "   Hardware driver: See io.toml"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Start runtime in background
$RUNTIME run --project . &
RUNTIME_PID=$!

# Wait for socket creation
echo "⏳ Waiting for control endpoint..."
for i in {1..50}; do
  if [ -S "$SOCKET" ]; then
    # Set group-based permissions instead of world-writable socket.
    if [ -n "${SUDO_GID:-}" ]; then
      chgrp "$SUDO_GID" "$SOCKET" 2>/dev/null || true
    fi
    chmod 660 "$SOCKET"
    echo "✅ Control endpoint ready: $SOCKET (rw-rw----)"
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
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  ✅ Backend is running!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Now you can:"
echo "  1. Press F5 in VS Code (Extension Development Host)"
echo "  2. Open any .statechart.json file"
echo "  3. Select '🔌 Hardware' mode"
echo "  4. Click '▶️ Start Hardware'"
echo ""
echo "Press Ctrl+C to stop the backend"
echo ""

# Wait for runtime process
wait $RUNTIME_PID
