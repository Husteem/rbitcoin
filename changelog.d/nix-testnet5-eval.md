Changed

- **The NixOS module eval checks testnet5 ports.** `network = "testnet5"`
  must default the P2P port to 18353 and the RPC port to 18352 and pass
  `--network testnet5`.
