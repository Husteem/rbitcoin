#!/usr/bin/env bash
# Overlay functional gate: private Tor / i2pd / cjdns meshes, then cargo test.
# Default cargo test never calls this.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
cd "$ROOT"

LIST_NAMES=(
  tor_onion_two_node_v2
  tor_wallet_hs_electrum_esplora
  i2p_sam_two_node
  cjdns_tun_two_node
  ephemeral_socks_isolated_broadcast
  tor_onion_comes_back_after_tor_restarts
  i2p_destination_comes_back_after_i2pd_restarts
  cjdns_listener_works_after_cjdroute_restarts
)

if [[ "${1:-}" == "--list" ]]; then
  printf '%s\n' "${LIST_NAMES[@]}"
  exit 0
fi

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  echo "usage: $0 [--list] [cargo-test-filter...]"
  echo "starts private overlay daemons, then:"
  echo "  cargo test -p rbitcoin-test --features overlay --test overlay -- --quiet FILTER"
  exit 0
fi

if [[ -z "${OVERLAY_IN_NIX:-}" ]]; then
  if ! command -v nix >/dev/null 2>&1; then
    echo "overlay-functional: nix is required (flake overlayFunctional shell)" >&2
    exit 1
  fi
  exec nix develop "$ROOT#overlayFunctional" --command \
    env OVERLAY_IN_NIX=1 \
    CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target/dev}" \
    "$0" "$@"
fi

need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "overlay-functional: missing $1 (use flake overlayFunctional)" >&2
    exit 1
  fi
}
need tor
need tor-gencert
need i2pd
need cjdroute
need python3
need cargo

WORKDIR="$(mktemp -d /tmp/rbitcoin-overlay.XXXXXX)"
cleanup() {
  local st=$?
  set +e
  if [[ "$st" -ne 0 && -f "$WORKDIR/tor/client/notice.log" ]]; then
    echo "overlay-functional: tor client log" >&2
    tail -n 80 "$WORKDIR/tor/client/notice.log" >&2 || true
  fi
  if [[ "$st" -ne 0 && -d "$WORKDIR/i2p" ]]; then
    echo "overlay-functional: i2pd logs" >&2
    local log
    for log in "$WORKDIR"/i2p/n*/i2pd.log; do
      [[ -f "$log" ]] || continue
      echo "----- $log -----" >&2
      tail -n 40 "$log" >&2
    done
  fi
  if [[ "$st" -ne 0 && -d "$WORKDIR/cjdns" ]]; then
    echo "overlay-functional: cjdns logs" >&2
    local log
    for log in "$WORKDIR"/cjdns/a.log "$WORKDIR"/cjdns/b.log; do
      [[ -f "$log" ]] || continue
      echo "----- $log -----" >&2
      tail -n 60 "$log" >&2
    done
  fi
  if [[ -d "$WORKDIR/tor" ]]; then
    find "$WORKDIR/tor" -name tor.pid -print0 2>/dev/null \
      | xargs -0 -r -I{} sh -c 'kill "$(cat "$1")" 2>/dev/null' _ {}
  fi
  if [[ -d "$WORKDIR/i2p" ]]; then
    for p in "$WORKDIR/i2p"/n*/i2pd.pid; do
      [[ -f "$p" ]] && kill "$(cat "$p")" 2>/dev/null
    done
    if [[ -f "$WORKDIR/i2p/loopbacks" ]]; then
      while read -r ip; do
        sudo ip addr del "$ip/32" dev lo 2>/dev/null || sudo ip addr del "$ip/8" dev lo 2>/dev/null || true
      done <"$WORKDIR/i2p/loopbacks"
    fi
  fi
  if [[ -d "$WORKDIR/cjdns" ]]; then
    for p in "$WORKDIR/cjdns/a.pid" "$WORKDIR/cjdns/b.pid"; do
      [[ -f "$p" ]] || continue
      pid="$(cat "$p")"
      # Only the session leader we recorded gets a group kill: a reused PID
      # must not take out an unrelated group on the runner.
      if tr '\0' ' ' <"/proc/$pid/cmdline" 2>/dev/null | grep -q 'cjdns-run\.sh'; then
        sudo kill -TERM -- "-$pid" 2>/dev/null
      fi
    done
    sudo pkill -f "cjdroute core $WORKDIR/cjdns" 2>/dev/null || true
    sudo ip link del rbtc0 2>/dev/null
    sudo ip link del rbtc1 2>/dev/null
  fi
  rm -rf "$WORKDIR"
  exit "$st"
}
trap cleanup EXIT INT TERM

echo "overlay-functional: workdir $WORKDIR" >&2

# Build before the meshes. A broken build fails here in seconds instead of
# after three minutes of daemons, and the TUN a journey binds is fresh: a
# cjdroute crash during a long build would otherwise leave a dead address.
cargo build -p rbitcoin-node

python3 "$HERE/tornet.py" "$WORKDIR/tor"
# shellcheck disable=SC1091
source "$WORKDIR/tor/env"
chmod +x "$HERE/i2pnet.sh" "$HERE/cjdns.sh" "$HERE/cjdns-run.sh"
"$HERE/i2pnet.sh" "$WORKDIR/i2p"
# shellcheck disable=SC1091
source "$WORKDIR/i2p/env"
"$HERE/cjdns.sh" "$WORKDIR/cjdns"
# shellcheck disable=SC1091
source "$WORKDIR/cjdns/env"

export OVERLAY_TOR_SOCKS OVERLAY_TOR_CONTROL OVERLAY_TOR_COOKIE OVERLAY_TOR_RESTART
export OVERLAY_I2P_SAM OVERLAY_I2P_SAM_B OVERLAY_I2P_RESTART_B
export OVERLAY_CJDNS_A OVERLAY_CJDNS_B OVERLAY_CJDNS_RESTART_B

echo "overlay-functional: TOR_SOCKS=$OVERLAY_TOR_SOCKS TOR_CONTROL=$OVERLAY_TOR_CONTROL" >&2
echo "overlay-functional: I2P_SAM=$OVERLAY_I2P_SAM I2P_SAM_B=$OVERLAY_I2P_SAM_B" >&2
echo "overlay-functional: CJDNS_A=$OVERLAY_CJDNS_A CJDNS_B=$OVERLAY_CJDNS_B" >&2

# A daemon can die between journeys (the runner's cjdns does). Do not hand a
# dead TUN to a test that means to bind it: wait for the supervisor to bring
# the addresses back, then fail the harness here with the daemon logs.
cjdns_tun_up() {
  if ! "$HERE/cjdns-tun-up.sh" --timeout 30 "$OVERLAY_CJDNS_A" "$OVERLAY_CJDNS_B"; then
    echo "overlay-functional: cjdns TUN not ready before a journey" >&2
    exit 1
  fi
}

if [[ "$#" -gt 0 ]]; then
  cjdns_tun_up
  cargo test -p rbitcoin-test --features overlay --test overlay -- --quiet --test-threads=1 "$@"
else
  skips=()
  for name in "${LIST_NAMES[@]}"; do
    cjdns_tun_up
    cargo test -p rbitcoin-test --features overlay --test overlay -- --quiet --exact "$name"
    skips+=(--skip "$name")
  done
  cargo test -p rbitcoin-test --features overlay --test overlay -- --quiet --exact "${skips[@]}"
fi
echo "overlay-functional: journeys done"
