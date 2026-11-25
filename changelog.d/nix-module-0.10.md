Changed

- **The NixOS module eval checks testnet5 ports.** `network = "testnet5"`
  must default the P2P port to 18353 and the RPC port to 18352 and pass
  `--network testnet5`.
- **`p2p.openFirewall` no longer opens a `v1` `netPermissionBind` port.**
  That listener is plaintext P2P for a local wallet indexer. To reach it
  from another LAN host, open the port on that interface with
  `networking.firewall.interfaces.<name>.allowedTCPPorts`.
