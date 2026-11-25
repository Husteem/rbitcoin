Changed

- **The NixOS module eval checks testnet5 ports.** `network = "testnet5"`
  must default the P2P port to 18353 and the RPC port to 18352 and pass
  `--network testnet5`.
- **`p2p.openFirewall` no longer opens a `v1` `netPermissionBind` port.**
  That listener is plaintext P2P for a local wallet indexer. To reach it
  from another LAN host, open the port on that interface with
  `networking.firewall.interfaces.<name>.allowedTCPPorts`.
- **NixOS `p2p.onionListen` marks the Tor forwarding listener.**
  `p2p.onionListen = "127.0.0.1:8334"` passes `--listen 127.0.0.1:8334=onion`
  after the P2P listen flags, so `listenOnion` publishes that listener and
  source-IP grants stay on the P2P listener. Evaluation refuses a wildcard
  address, a shared P2P, Electrum, Esplora, or `v1` port, `maxInbound = 0`,
  `trustPeer`, a missing `listenOnion` or onion `externalIp`, and I2P
  incoming without `p2p.listen`. The firewall never opens it. It replaces
  the `extraArgs` recipe in `docs/operator/operations.md`.
