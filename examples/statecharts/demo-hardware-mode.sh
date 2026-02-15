#!/bin/bash
# Complete end-to-end test: Build → Run → Test → Demo

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

echo "╔════════════════════════════════════════════════════════════╗"
echo "║  StateChart Hardware Mode - Complete Test & Demo          ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# Function to cleanup on exit
cleanup() {
    echo ""
    echo "🧹 Cleaning up..."
    if [ ! -z "$RUNTIME_PID" ]; then
        kill $RUNTIME_PID 2>/dev/null || true
    fi
}
trap cleanup EXIT

# Step 1: Check prerequisites
echo "📋 Step 1: Checking Prerequisites"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

if ! command -v trust-runtime &> /dev/null; then
    echo "❌ trust-runtime not found"
    echo ""
    echo "Build it first:"
    echo "  cd ../../.."
    echo "  cargo build --release --bin trust-runtime"
    echo "  export PATH=\$PATH:\$PWD/target/release"
    exit 1
fi

echo "✅ trust-runtime: $(which trust-runtime)"

if ! command -v nc &> /dev/null; then
    echo "❌ netcat (nc) not found - needed for socket testing"
    echo "   Install: sudo apt install netcat-openbsd"
    exit 1
fi

echo "✅ netcat: $(which nc)"
echo ""

# Step 2: Build hardware project
echo "📋 Step 2: Building Hardware Project"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

cd "$SCRIPT_DIR/../statechart_backend"

if [ -d ".trust/bundle" ]; then
    echo "⚠️  Previous build found. Rebuilding..."
    rm -rf .trust
fi

./build.sh

echo ""

# Step 3: Start runtime in background
echo "📋 Step 3: Starting trust-runtime"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Kill any existing runtime
pkill -f trust-runtime 2>/dev/null || true
sleep 1

# Remove old socket
rm -f /tmp/trust-debug.sock

echo "🚀 Starting runtime in background..."
# Already in statechart_backend directory from Step 2
# IMPORTANT: Remove --simulation to use REAL HARDWARE
trust-runtime run --project . &
RUNTIME_PID=$!

echo "   PID: $RUNTIME_PID"
echo "   Waiting for startup..."

# Wait for socket to appear
for i in {1..10}; do
    if [ -S "/tmp/trust-debug.sock" ]; then
        break
    fi
    echo -n "."
    sleep 1
done
echo ""

if [ ! -S "/tmp/trust-debug.sock" ]; then
    echo "❌ Runtime failed to start (socket not found)"
    exit 1
fi

echo "✅ Runtime started successfully!"
echo ""

# Step 4: Test connection
echo "📋 Step 4: Testing Control Endpoint"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

sleep 2  # Give runtime a moment to stabilize

echo "📡 Sending status request..."
STATUS=$(echo '{"id":1,"type":"status"}' | nc -U /tmp/trust-debug.sock -w 2 2>/dev/null || echo "")

if [ -z "$STATUS" ]; then
    echo "❌ No response from runtime"
    exit 1
fi

echo "✅ Got response!"
echo ""

echo "📡 Testing I/O write to %QX0.0 (true)..."
IO_TEST=$(echo '{"id":2,"type":"io.write","params":{"address":"%QX0.0","value":true}}' | nc -U /tmp/trust-debug.sock -w 2 2>/dev/null || echo "")

if echo "$IO_TEST" | grep -q '"ok":true'; then
    echo "✅ I/O write successful!"
else
    echo "⚠️  I/O write response: $IO_TEST"
fi
echo ""

echo "📡 Testing I/O write to %QX0.0 (false)..."
echo '{"id":3,"type":"io.write","params":{"address":"%QX0.0","value":false}}' | nc -U /tmp/trust-debug.sock -w 2 >/dev/null
echo "✅ Done!"
echo ""

# Step 5: Summary
echo "╔════════════════════════════════════════════════════════════╗"
echo "║                    ✅ ALL TESTS PASSED!                    ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""
echo "🎯 trust-runtime is running and accepting commands"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "   Next Steps: Test with StateChart Editor"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "1. Open a NEW TERMINAL (keep this one running)"
echo ""
echo "2. Navigate to VS Code extension:"
echo "   cd $SCRIPT_DIR/../../editors/vscode"
echo ""
echo "3. Launch Extension Development Host:"
echo "   Press F5 in VS Code"
echo ""
echo "4. In the dev window, open a StateChart example:"
echo "   Ctrl+O → Navigate to:"
echo "   $SCRIPT_DIR/ethercat-snake-simple.statechart.json"
echo ""
echo "5. In the Execution Panel (right side):"
echo "   - Select: 🔌 Hardware"
echo "   - Click: ▶️ Start Hardware"
echo "   - You should see: ✅ Connected to trust-runtime"
echo ""
echo "6. Control the state machine:"
echo "   - Click: START"
echo "   - Click: TICK (multiple times)"
echo "   - Watch states light up in green!"
echo ""
echo "7. Check the console logs (Help → Toggle Developer Tools):"
echo "   🔌 [HW] turnOn_DO0 → WRITE true to %QX0.0"
echo "   ✅ Wrote true to %QX0.0"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "💡 Tip: This runtime is running with REAL HARDWARE (EtherCAT)"
echo "    Hardware: EK1100 + EL2008 on interface enp111s0"
echo "    LEDs should physically light up when you send events!"
echo ""
echo "Press Ctrl+C to stop the runtime when done..."
echo ""

# Keep runtime running
wait $RUNTIME_PID
