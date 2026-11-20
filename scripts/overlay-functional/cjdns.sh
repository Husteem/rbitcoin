#!/usr/bin/env bash
# Two-node cjdns TUN that UDP-peers on 127.0.0.1. Fail if TUN cannot be created.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="${1:?dir}"
mkdir -p "$ROOT"

if [[ ! -e /dev/net/tun ]]; then
  sudo modprobe tun || true
fi
if [[ ! -e /dev/net/tun ]]; then
  echo "cjdns: /dev/net/tun missing — no silent 127.0.0.1 fallback" >&2
  exit 1
fi
if ! sudo -n true 2>/dev/null; then
  echo "cjdns: passwordless sudo required for TUN" >&2
  exit 1
fi

free_port() {
  python3 -c 'import socket; s=socket.socket(socket.AF_INET, socket.SOCK_DGRAM); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()'
}

UDP_A="$(free_port)"
UDP_B="$(free_port)"
ADMIN_A="$(free_port)"
ADMIN_B="$(free_port)"

cjdroute --genconf | cjdroute --cleanconf >"$ROOT/a.raw.json"
cjdroute --genconf | cjdroute --cleanconf >"$ROOT/b.raw.json"

python3 - "$ROOT" "$UDP_A" "$UDP_B" "$ADMIN_A" "$ADMIN_B" <<'PY'
import json, sys
root, udp_a, udp_b, admin_a, admin_b = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5])

def load(name):
    with open(f"{root}/{name}.raw.json") as f:
        return json.load(f)

a, b = load("a"), load("b")
pw = a["authorizedPasswords"][0]["password"]
a_pub = a["publicKey"]
b_pub = b["publicKey"]

def udp(cfg):
    return cfg["interfaces"]["UDPInterface"][0]

udp(a)["bind"] = f"0.0.0.0:{udp_a}"
udp(b)["bind"] = f"0.0.0.0:{udp_b}"
udp(a)["connectTo"] = {
    f"127.0.0.1:{udp_b}": {
        "password": b["authorizedPasswords"][0]["password"],
        "publicKey": b_pub,
        "peerName": "b",
    }
}
udp(b)["connectTo"] = {
    f"127.0.0.1:{udp_a}": {"password": pw, "publicKey": a_pub, "peerName": "a"}
}
# One UDP bind (IPv4 loopback). Drop the IPv6 UDPInterface row.
a["interfaces"]["UDPInterface"] = [udp(a)]
b["interfaces"]["UDPInterface"] = [udp(b)]
a["router"]["dnsSeeds"] = []
b["router"]["dnsSeeds"] = []
a["security"] = [{"keepNetAdmin": 1}, {"noforks": 1}, {"setupComplete": 1}]
b["security"] = [{"keepNetAdmin": 1}, {"noforks": 1}, {"setupComplete": 1}]
a["logging"] = {"logTo": "stdout"}
b["logging"] = {"logTo": "stdout"}
a["admin"]["bind"] = f"127.0.0.1:{admin_a}"
b["admin"]["bind"] = f"127.0.0.1:{admin_b}"
a["pipe"] = f"{root}/a.cjdns.sock"
b["pipe"] = f"{root}/b.cjdns.sock"

def tun(cfg, dev):
    iface = cfg.setdefault("router", {}).setdefault("interface", {})
    iface["type"] = "TUNInterface"
    iface["tunDevice"] = dev
    cfg["noBackground"] = 1

tun(a, "rbtc0")
tun(b, "rbtc1")

for name, cfg in (("a", a), ("b", b)):
    with open(f"{root}/{name}.conf", "w") as f:
        json.dump(cfg, f, indent=2)
        f.write("\n")
    with open(f"{root}/{name}.ipv6", "w") as f:
        f.write(cfg["ipv6"] + "\n")
PY

CJDNS_BIN="$(command -v cjdroute)"

# cjdroute reads stdin until EOF as JSON conf, then `--nobg` keeps the
# client in the event loop so the core is not reaped (parent would
# otherwise `return 0`). cjdns-run.sh restarts the angel/core pair after a
# crash: cjdns 22.3 segfaults on the runner around the first TCP and would
# otherwise take the TUN (and the address a journey binds) with it. setsid:
# cargo/nix must not SIGHUP the pair, and the supervisor is the session
# leader so cleanup can kill the whole group.
start_cjdroute() {
  local name="$1"
  local dev="$2"
  local i
  rm -f "$ROOT/$name.pid"
  sudo bash -c 'exec setsid bash "$1" "$2" "$3" "$4" "$5" "$6" "$7"' \
    _ "$HERE/cjdns-run.sh" "$CJDNS_BIN" "$ROOT/$name.conf" "$ROOT/$name.log" \
    "$ROOT/$name.pid" "$ROOT/$name.cjdns.sock" "$dev" &
  for i in $(seq 1 50); do
    if [[ -s "$ROOT/$name.pid" ]]; then
      return 0
    fi
    sleep 0.1
  done
  echo "cjdns: $name supervisor never wrote $name.pid" >&2
  return 1
}

wait_tun() {
  local dev="$1"
  local i
  # A crashed pair comes back on the same device; allow the supervisor's
  # 5s stuck-core gate plus a fresh start before calling it dead.
  for i in $(seq 1 125); do
    if ip link show "$dev" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.2
  done
  echo "cjdns: $dev never appeared" >&2
  return 1
}

start_cjdroute a rbtc0
wait_tun rbtc0
start_cjdroute b rbtc1
wait_tun rbtc1

IPV6_A="$(cat "$ROOT/a.ipv6")"
IPV6_B="$(cat "$ROOT/b.ipv6")"
bash "$HERE/cjdns-ready.sh" "$ROOT"

cat >"$ROOT/restart-b.sh" <<RESTART
#!/usr/bin/env bash
set -euo pipefail
pid="\$(cat "$ROOT/b.pid")"
# b runs under cjdns-run.sh, which owns the session: the angel's core is a
# grandchild, so only a group kill takes the TUN down with the pair.
sudo kill -TERM -- "-\$pid" 2>/dev/null || true
for _ in \$(seq 1 100); do ip link show rbtc1 >/dev/null 2>&1 || break; sleep 0.2; done
if ip link show rbtc1 >/dev/null 2>&1; then
  echo "cjdns: rbtc1 still up after stop" >&2
  exit 1
fi
rm -f "$ROOT/b.pid"
sudo bash -c 'exec setsid bash "\$@"' _ "$HERE/cjdns-run.sh" "$CJDNS_BIN" \
  "$ROOT/b.conf" "$ROOT/b.log" "$ROOT/b.pid" "$ROOT/b.cjdns.sock" rbtc1 &
for _ in \$(seq 1 50); do
  [[ -s "$ROOT/b.pid" ]] && break
  sleep 0.1
done
bash "$HERE/cjdns-ready.sh" "$ROOT"
RESTART
chmod +x "$ROOT/restart-b.sh"

cat >"$ROOT/env" <<EOF
OVERLAY_CJDNS_A=${IPV6_A}
OVERLAY_CJDNS_B=${IPV6_B}
OVERLAY_CJDNS_RESTART_B=$ROOT/restart-b.sh
EOF
cat "$ROOT/env"
