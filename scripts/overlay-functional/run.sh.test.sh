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

# The angel applies the config and calls Security_setupComplete. It can die
# alone and leave the core (and the TUN) behind; that core then exits 232 at
# the 20s security deadline. The old supervisor called the survivor healthy,
# exited, and left the TUN dead with no one to restart it. The supervisor
# must own the pair: take the orphan core down and start again.
tmp2="$(mktemp -d /tmp/rbtc-cjdns-orphan.XXXXXX)"
trap 'rm -rf "$tmp" "$tmp2"' EXIT
# The name is real so the old supervisor sees "the TUN is up"; the fake
# sysfs tree and ip make sure neither this test nor a broken supervisor can
# touch the host's loopback.
dev="lo"
mkdir -p "$tmp2/sysfs" "$tmp2/bin"
: >"$tmp2/sysfs/$dev"
printf '#!/usr/bin/env bash\necho "unexpected ip $*" >&2\nexit 1\n' >"$tmp2/bin/ip"
chmod +x "$tmp2/bin/ip"
cat >"$tmp2/cjdroute" <<'FAKE'
#!/usr/bin/env bash
echo run >>"$CJDNS_TEST_COUNT"
# Stand in for the core the angel leaves behind: argv[0] is what the
# supervisor's pgrep looks for, and TERM simulates the TUN going away.
# cjdroute is called with only --nobg, so the socket and device come in
# through the test's environment, not argv. The short sleep widens the
# real fork-to-exec window so a supervisor that checks once before the
# core execs misses it deterministically.
(
  sleep 0.2
  exec -a "cjdroute core $CJDNS_TEST_SOCK" bash -c '
    dev="$1"
    sysfs="$2"
    trap "rm -f \"$sysfs/$dev\"; exit 0" TERM
    while :; do sleep 0.2; done
  ' _ "$CJDNS_TEST_DEV" "$CJDNS_TEST_SYSFS"
) &
exit 0
FAKE
chmod +x "$tmp2/cjdroute"
: >"$tmp2/conf"
CJDNS_TEST_COUNT="$tmp2/count" CJDNS_TEST_SYSFS="$tmp2/sysfs" \
CJDNS_TEST_SOCK="$tmp2/pipe.sock" CJDNS_TEST_DEV="$dev" \
CJDNS_SYSFS="$tmp2/sysfs" PATH="$tmp2/bin:$PATH" \
  "$ROOT/scripts/overlay-functional/cjdns-run.sh" \
  "$tmp2/cjdroute" "$tmp2/conf" "$tmp2/log" "$tmp2/supervisor.pid" \
  "$tmp2/pipe.sock" "$dev" &
sup2=$!
# Wait for the second pair: a supervisor that kept the survivor after the
# angel-only death exits or stalls here instead.
for _ in $(seq 1 50); do
  runs=0
  if [[ -f "$tmp2/count" ]]; then runs="$(wc -l <"$tmp2/count")"; fi
  if [[ "$runs" -ge 2 ]]; then break; fi
  if ! kill -0 "$sup2" 2>/dev/null; then break; fi
  sleep 0.1
done
runs=0
if [[ -f "$tmp2/count" ]]; then runs="$(wc -l <"$tmp2/count")"; fi
alive=no
kill -0 "$sup2" 2>/dev/null && alive=yes
kill "$sup2" 2>/dev/null || true
wait "$sup2" 2>/dev/null || true
# A core forked just before the angel died execs after the supervisor's
# first pass. Give it that window before asserting the supervisor took it
# down; a one-shot check leaves it running.
sleep 0.5
orphan=no
if pgrep -f "^cjdroute core $tmp2/pipe.sock" >/dev/null; then orphan=yes; fi
assert_ok cjdns_supervisor_restarts_after_orphan_core test "$runs" -ge 2
assert_ok cjdns_supervisor_does_not_exit_on_orphan_core test "$alive" = yes
assert_ok cjdns_supervisor_logs_orphan_kill grep -q 'killing the core the angel left behind' "$tmp2/log"
assert_ok cjdns_supervisor_stops_orphan_core test "$orphan" = no
pkill -f "^cjdroute core $tmp2/pipe.sock" 2>/dev/null || true

# The supervisor's TUN gate is a script the runner calls before every
# journey, so a dead TUN fails the harness with the daemon logs instead of
# panicking inside a journey that means to bind the address.
assert_ok tun_up_accepts_live_address \
  "$ROOT/scripts/overlay-functional/cjdns-tun-up.sh" --timeout 2 ::1
assert_fail_quiet tun_up_reports_dead_address \
  "$ROOT/scripts/overlay-functional/cjdns-tun-up.sh" --timeout 1 fd00:dead:beef::1

echo "$PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]]
