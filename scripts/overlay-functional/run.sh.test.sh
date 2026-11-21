#!/usr/bin/env bash
# Contract pin for overlay-functional/run.sh (no daemons, no cargo).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
RUN="$ROOT/scripts/overlay-functional/run.sh"
PASS=0
FAIL=0
source "$ROOT/scripts/contract-assert.sh"
assert_ok() { assert_ok_quiet "$@"; }


assert_stdout() {
  local name="$1"
  local needle="$2"
  shift 2
  local out
  if ! out="$("$@" 2>/dev/null)"; then
    echo "not ok - $name (unexpected failure)"
    FAIL=$((FAIL + 1))
    return
  fi
  if printf '%s' "$out" | grep -q -- "$needle"; then
    echo "ok - $name"
    PASS=$((PASS + 1))
  else
    echo "not ok - $name (missing '$needle' in stdout: $out)"
    FAIL=$((FAIL + 1))
  fi
}

assert_stdout tor_list tor_onion_two_node_v2 "$RUN" --list
assert_stdout wallet_list tor_wallet_hs_electrum_esplora "$RUN" --list
assert_stdout i2p_list i2p_sam_two_node "$RUN" --list
assert_stdout cjdns_list cjdns_tun_two_node "$RUN" --list
assert_stdout eph_list ephemeral_socks_isolated_broadcast "$RUN" --list
assert_ok help "$RUN" --help

# The build runs before the meshes: a broken build fails in seconds instead
# of after three minutes of daemons, and the TUN the journeys bind is fresh.
build_line="$(grep -n 'cargo build -p rbitcoin-node' "$RUN" | cut -d: -f1 | head -n1 || true)"
mesh_line="$(grep -n 'tornet.py' "$RUN" | cut -d: -f1 | head -n1 || true)"
assert_ok build_before_meshes test "${build_line:-0}" -lt "${mesh_line:-1}"

# The runner's GHA cjdroute core segfaults around the first TCP and takes
# the TUN with it. A crashed daemon must come back, not hand a dead TUN to
# the journeys. The fake below exits at once; the supervisor keeps starting it.
tmp="$(mktemp -d /tmp/rbtc-cjdns-supervisor.XXXXXX)"
trap 'rm -rf "$tmp"' EXIT
printf '#!/usr/bin/env bash\necho run >>"$CJDNS_TEST_COUNT"\nexit 7\n' >"$tmp/cjdroute"
chmod +x "$tmp/cjdroute"
: >"$tmp/conf"
CJDNS_TEST_COUNT="$tmp/count" \
  "$ROOT/scripts/overlay-functional/cjdns-run.sh" \
  "$tmp/cjdroute" "$tmp/conf" "$tmp/log" "$tmp/supervisor.pid" \
  "$tmp/pipe.sock" rbtc-none &
sup=$!
for _ in $(seq 1 50); do
  runs=0
  if [[ -f "$tmp/count" ]]; then runs="$(wc -l <"$tmp/count")"; fi
  if [[ "$runs" -ge 2 ]]; then break; fi
  sleep 0.1
done
runs=0
if [[ -f "$tmp/count" ]]; then runs="$(wc -l <"$tmp/count")"; fi
kill "$sup" 2>/dev/null || true
wait "$sup" 2>/dev/null || true
assert_ok cjdns_supervisor_restarts_crashed_core test "$runs" -ge 2
assert_ok cjdns_supervisor_logs_crash grep -q 'cjdroute exited 7' "$tmp/log"
assert_ok cjdns_supervisor_owns_pidfile test -s "$tmp/supervisor.pid"

echo "$PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]]
