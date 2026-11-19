Fixed

- **NixOS module fixes.** `sv2.tp.authoritySecretFile` is loaded as a
  systemd credential, so a root-only path such as `/run/keys/...` works
  (the unit used to restart forever). `silentPaymentIndex` with
  `pruneSeqSigWit` is allowed, as the node has allowed it since 0.9.1. A
  `$` in `alertNotify`, `startupNotify`, or `extraArgs` reaches the node
  instead of being expanded by systemd. `rpc.rest` works with only the
  unix socket. A `v1` `netPermissionBind` on the P2P, Electrum, or Esplora
  port, or with `p2p.maxInbound = 0`, fails evaluation instead of startup.
