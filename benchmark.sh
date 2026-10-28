#!/usr/bin/env bash
set -e

REPORT_FILE="animadb_performance_report.md"
SERVER_PID=""

echo "==========================================================="
echo "   ANIMADB AUTOMATED PRODUCTION BENCHMARK SUITE            "
echo "==========================================================="

cleanup() {
    if [ -n "$SERVER_PID" ]; then
        echo "[*] Cleaning up server process ($SERVER_PID)..."
        kill -9 "$SERVER_PID" 2>/dev/null || true
    fi
    rm -f animadb_store.wal animadb_test.wal
}
trap cleanup EXIT

echo "[1/5] Running Rust Cargo Unit Tests..."
cargo test --release

echo "[2/5] Compiling AnimaDB Release Target..."
cargo build --release

echo "[3/5] Starting Engine Server..."
./target/release/animadb > server_output.log 2>&1 &
SERVER_PID=$!
sleep 2

if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "[-] Error: Database server failed to start."
    cat server_output.log
    exit 1
fi

INITIAL_RSS=$(ps -p "$SERVER_PID" -o rss= 2>/dev/null | tr -d ' ' || echo "0")
INITIAL_RSS=${INITIAL_RSS:-0}
INITIAL_MEM_MB=$(echo "scale=2; $INITIAL_RSS / 1024" | bc)

echo "[4/5] Executing Multiprocessing Scale Traffic Test..."
python3 scale_test.py > client_output.log 2>&1

sleep 2
PEAK_RSS=$(ps -p "$SERVER_PID" -o rss= 2>/dev/null | tr -d ' ' || echo "0")
PEAK_RSS=${PEAK_RSS:-0}
PEAK_MEM_MB=$(echo "scale=2; $PEAK_RSS / 1024" | bc)
MEM_DELTA_MB=$(echo "scale=2; $PEAK_MEM_MB - $INITIAL_MEM_MB" | bc)

echo "[5/5] Extracting Server Telemetry & Compiling Report..."

INGESTED=$(grep "TELEMETRY" server_output.log 2>/dev/null | tail -n 1 | awk -F'Ingested: ' '{print $2}' | awk '{print $1}')
ACCEPTED=$(grep "TELEMETRY" server_output.log 2>/dev/null | tail -n 1 | awk -F'Accepted: ' '{print $2}' | awk '{print $1}')
DROPPED=$(grep "TELEMETRY" server_output.log 2>/dev/null | tail -n 1 | awk -F'Dropped: ' '{print $2}' | awk '{print $1}')

cat << REPORT > "$REPORT_FILE"
# AnimaDB Performance & Security Report

## Server Memory Profile

| Parameter | Measured Value |
| :--- | :--- |
| **Initial Memory Baseline** | ${INITIAL_MEM_MB} MB |
| **Peak Load Memory Allocation** | ${PEAK_MEM_MB} MB |
| **Measured Memory Delta** | ${MEM_DELTA_MB} MB |

## Server-Side Telemetry & Execution Counts

| Metric | Server Verified Count |
| :--- | :--- |
| **Total Frames Ingested** | ${INGESTED:-0} |
| **Valid Transactions Executed & Logged to WAL** | ${ACCEPTED:-0} |
| **Anomalous Vectors Intercepted & Quarantined** | ${DROPPED:-0} |
REPORT

echo "[+] Report generated successfully: $REPORT_FILE"
