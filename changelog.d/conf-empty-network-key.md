Fixed

- **`testnet4=` with an empty value selects the network again.** The
  network keys (`regtest`, `signet`, `testnet`, `testnet4`, `testnet5`) in
  `rbitcoin.conf` and as `--key=` flags refused an empty value and stopped
  startup. Core reads it as true, and so did 0.9.x for the four keys it had.
  Other values still have to be `0`, `1`, `true`, `false`, `yes`, `no`, `on`
  or `off`.
