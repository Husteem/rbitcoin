#!/usr/bin/env bash
# One cjdroute, restarted when it dies. The GitHub runner's cjdns 22.3
# segfaults around the first TCP through the TUN and takes the interface
# with it; a journey that binds then sees a dead address. cjdns.sh starts
# this under setsid, records this PID as the session leader, and cleanup
# kills the group, so the loop never outlives the run.
set -uo pipefail

BIN="${1:?cjdroute}"
CONF="${2:?conf}"
LOG="${3:?log}"
PIDFILE="${4:?pidfile}"
SOCK="${5:?pipe socket}"
DEV="${6:?tun dev}"

echo $$ >"$PIDFILE"

# CJDNS_SYSFS is a test seam so the supervisor contract can run without a
# real TUN; production reads the kernel's net class.
dev_up() { [[ -e "${CJDNS_SYSFS:-/sys/class/net}/$DEV" ]]; }
# cjdroute is an angel over a core. The angel applies the config and calls
# Security_setupComplete; `cjdroute is already running` means a core still
# holds the pipe, so a fresh angel must wait for it.
core_gone() { ! pgrep -f "cjdroute core $SOCK" >/dev/null 2>&1; }

kill_core() {
  local signalled=0 deadline=$((SECONDS + 10))
  while (( SECONDS < deadline )); do
    if ! core_gone; then
      if [[ "$signalled" -eq 0 ]]; then
        echo "cjdns: killing the core the angel left behind" >>"$LOG"
        signalled=1
      fi
      pkill -f "cjdroute core $SOCK" 2>/dev/null || true
    fi
    sleep 0.1
    core_gone || continue
    # The angel can fork its core just before it exits. That process does
    # not match the pgrep until it has exec'd, so require a longer clean
    # window before calling the pair gone.
    sleep 0.4
    core_gone && return 0
  done
  pkill -KILL -f "cjdroute core $SOCK" 2>/dev/null || true
  sleep 0.2
  core_gone
}

drop_dev() {
  local i
  for i in $(seq 1 50); do
    dev_up || return 0
    sleep 0.1
  done
  # A killed core can leave the device behind; a fresh core would then fail
  # to create it and hit the 20s security deadline.
  ip link del "$DEV" 2>/dev/null || true
  for i in $(seq 1 50); do
    dev_up || return 0
    sleep 0.1
  done
  return 1
}

child=0
stop() {
  if [[ "$child" -ne 0 ]]; then
    kill -TERM "$child" 2>/dev/null || true
    local i
    for i in $(seq 1 50); do
      kill -0 "$child" 2>/dev/null || break
      sleep 0.1
    done
    kill -KILL "$child" 2>/dev/null || true
    wait "$child" 2>/dev/null || true
  fi
  # The angel must be reaped first: a live one can fork after the clean pass.
  kill_core || true
  exit 0
}
trap stop TERM INT

while :; do
  "$BIN" --nobg <"$CONF" >>"$LOG" 2>&1 &
  child=$!
  wait "$child"
  status=$?
  echo "cjdns: cjdroute exited $status — restarting" >>"$LOG"
  # Never exit while the daemon is down: this loop is the only thing that
  # brings the TUN back. The angel can die alone and leave its core (and the
  # device) behind. That core is unmanaged: it exits 232 at the 20s security
  # deadline, and the old "keeping it" exit here left a dead TUN behind.
  # Always run the kill pass: a core forked just before the angel died may
  # not have exec'd yet, so a single core_gone check can miss it. Take the
  # orphan down with the angel, clear the device, start a fresh pair on the
  # same address.
  if ! kill_core; then
    echo "cjdns: could not stop the core the angel left behind" >>"$LOG"
  fi
  if ! drop_dev; then
    echo "cjdns: $DEV still present after the core exited" >>"$LOG"
  fi
  rm -f "$SOCK"
  sleep 0.2
done
