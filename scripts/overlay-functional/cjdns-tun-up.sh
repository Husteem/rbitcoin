#!/usr/bin/env bash
# Wait until every address is assigned and bindable. Used after a cjdns
# restart and by run.sh before each journey: a journey must never bind an
# address whose TUN has gone away.
set -euo pipefail

TIMEOUT=60
if [[ "${1:-}" == "--timeout" ]]; then
  TIMEOUT="${2:?seconds}"
  shift 2
fi
if [[ "$#" -eq 0 ]]; then
  echo "usage: $0 [--timeout SECONDS] ADDR..." >&2
  exit 2
fi

python3 - "$TIMEOUT" "$@" <<'PY'
import socket, sys, time

timeout = float(sys.argv[1])
addrs = sys.argv[2:]
end = time.time() + timeout
last = "no attempt"
while time.time() < end:
    live = 0
    for ip in addrs:
        s = socket.socket(socket.AF_INET6, socket.SOCK_STREAM)
        try:
            s.bind((ip, 0))
            live += 1
        except OSError as e:
            last = f"{ip}: {e}"
        finally:
            s.close()
    if live == len(addrs):
        sys.exit(0)
    time.sleep(0.25)
print(f"cjdns: TUN address not bindable after {timeout:g}s: {last}", file=sys.stderr)
sys.exit(1)
PY
