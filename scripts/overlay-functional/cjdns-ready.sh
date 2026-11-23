#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="${1:?dir}"
IPV6_A="$(cat "$ROOT/a.ipv6")"
IPV6_B="$(cat "$ROOT/b.ipv6")"

# Kernel TCP over the TUNs (the product path). ICMP through cjdns is not
# reliable on GitHub-hosted runners even when the ifaces are up.
if ! python3 - "$IPV6_A" "$IPV6_B" <<'PY'
import socket, sys, time
a, b = sys.argv[1], sys.argv[2]
end = time.time() + 60
last = ""
while time.time() < end:
    srv = socket.socket(socket.AF_INET6, socket.SOCK_STREAM)
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        srv.bind((b, 0))
        srv.listen(1)
        srv.settimeout(1)
        port = srv.getsockname()[1]
        cli = socket.socket(socket.AF_INET6, socket.SOCK_STREAM)
        try:
            cli.bind((a, 0))
            cli.settimeout(1)
            cli.connect((b, port))
            conn, _ = srv.accept()
            conn.close()
            sys.exit(0)
        except OSError as e:
            last = str(e)
        finally:
            cli.close()
    except OSError as e:
        last = str(e)
    finally:
        srv.close()
    time.sleep(0.4)
print(f"cjdns TCP timeout ({a} -> {b}): {last}", file=sys.stderr)
sys.exit(1)
PY
then
  echo "cjdns TCP timeout ($IPV6_A <-> $IPV6_B)" >&2
  for name in a b; do
    echo "--- $name pid ---" >&2
    if [[ -f "$ROOT/$name.pid" ]] && sudo kill -0 "$(cat "$ROOT/$name.pid")" 2>/dev/null; then
      echo "alive $(cat "$ROOT/$name.pid")" >&2
    else
      echo "dead" >&2
    fi
    echo "--- $name.log ---" >&2
    tail -n 80 "$ROOT/$name.log" >&2 || true
  done
  sudo ip -6 addr show rbtc0 >&2 || true
  sudo ip -6 addr show rbtc1 >&2 || true
  sudo ip -6 route get "$IPV6_B" >&2 || true
  exit 1
fi

for pair in a:rbtc0 b:rbtc1; do
  name="${pair%%:*}"
  dev="${pair#*:}"
  # A supervisor that kept the survivor after an angel crash is fine: the
  # device and its address are what the journeys bind, so require those.
  if [[ ! -e "/sys/class/net/$dev" ]]; then
    echo "cjdns: $dev not up after TCP wait" >&2
    tail -n 40 "$ROOT/$name.log" >&2 || true
    exit 1
  fi
done
# GHA: the core can crash ~1s after the first TCP. The supervisor puts a
# fresh one back, TUN and address included; wait for both addresses to bind
# again so the journeys never get a dead TUN. A crash loop never settles
# and fails here, with the log.
sleep 2
if ! bash "$HERE/cjdns-tun-up.sh" --timeout 15 "$IPV6_A" "$IPV6_B"; then
  echo "cjdns: TUN still dead after supervisor restart" >&2
  exit 1
fi
