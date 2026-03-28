#!/bin/bash
# Memory profiling script for ropy
# Usage:
#   1. Start ropy normally (e.g., open the app)
#   2. Run: bash scripts/memory_profile.sh
#
# This script uses macOS built-in tools (vmmap, heap) to analyze memory usage.

set -e

PID=$(pgrep -x ropy 2>/dev/null || true)

if [ -z "$PID" ]; then
    echo "ropy is not running. Please start ropy first, then run this script."
    echo ""
    echo "Tip: You can start ropy with:"
    echo "  open target/release/ropy   # or double-click the binary"
    exit 1
fi

echo "=== Ropy Memory Profile (PID: $PID) ==="
echo ""

OUTPUT_DIR="target/memory_profile_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$OUTPUT_DIR"

# 1. Basic process info
echo "--- Basic Process Info ---"
ps -o pid,rss,vsz,pcpu,pmem,command -p "$PID"
echo ""

# 2. vmmap summary - shows virtual memory regions
echo "--- vmmap Summary ---"
echo "(Full output saved to $OUTPUT_DIR/vmmap.txt)"
vmmap --summary "$PID" 2>/dev/null | tee "$OUTPUT_DIR/vmmap_summary.txt"
vmmap "$PID" > "$OUTPUT_DIR/vmmap.txt" 2>/dev/null
echo ""

# 3. heap analysis - shows heap allocations by type
echo "--- Heap Summary ---"
echo "(Full output saved to $OUTPUT_DIR/heap.txt)"
heap "$PID" 2>/dev/null | head -50 | tee "$OUTPUT_DIR/heap_summary.txt"
heap "$PID" > "$OUTPUT_DIR/heap.txt" 2>/dev/null
echo ""

# 4. malloc history (top allocations)
echo "--- Malloc Zones ---"
heap --addresses=all "$PID" 2>/dev/null | tail -20 | tee "$OUTPUT_DIR/heap_addresses.txt"
echo ""

echo "=== Full reports saved to $OUTPUT_DIR/ ==="
echo ""
echo "Key files:"
echo "  $OUTPUT_DIR/vmmap_summary.txt  - Virtual memory region summary"
echo "  $OUTPUT_DIR/vmmap.txt          - Detailed virtual memory map"
echo "  $OUTPUT_DIR/heap_summary.txt   - Heap allocation summary"
echo "  $OUTPUT_DIR/heap.txt           - Full heap analysis"
