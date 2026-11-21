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
child=0
stop() {
  if [[ "$child" -ne 0 ]]; then
    kill -TERM "$child" 2>/dev/null || true
  fi
  exit 0
}
trap stop TERM INT

dev_up() { [[ -e "/sys/class/net/$DEV" ]]; }
# cjdroute is an angel over a core. `cjdroute is already running` means a
# core still holds the pipe, so a fresh angel must wait for it.
core_gone() { ! pgrep -f "cjdroute core $SOCK" >/dev/null 2>&1; }

while :; do
  "$BIN" --nobg <"$CONF" >>"$LOG" 2>&1 &
  child=$!
  wait "$child"
  status=$?
  echo "cjdns: cjdroute exited $status — restarting" >>"$LOG"
  # The angel can die alone; its core keeps the device and the pipe. Keep
  # that survivor: a fresh angel would abort as "already running". A core
  # left stuck in startup (no device) is killed so the pair comes back.
  for _ in $(seq 1 50); do
    core_gone && break
    dev_up || break
    sleep 0.1
  done
  if ! core_gone && dev_up; then
    echo "cjdns: core still serves $DEV; keeping it" >>"$LOG"
    exit 0
  fi
  if ! core_gone; then
    pkill -f "cjdroute core $SOCK" 2>/dev/null || true
    for _ in $(seq 1 100); do
      core_gone && break
      sleep 0.1
    done
  fi
  for _ in $(seq 1 100); do
    dev_up || break
    sleep 0.1
  done
  if dev_up; then
    echo "cjdns: $DEV held with no core; keeping it" >>"$LOG"
    exit 0
  fi
  rm -f "$SOCK"
  sleep 0.2
done
