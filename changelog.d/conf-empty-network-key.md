Fixed

- **`testnet4=` with an empty value selects the network again.** The
  network keys in `rbitcoin.conf` (`regtest`, `signet`, `testnet`,
  `testnet4`, `testnet5`) refused an empty value and stopped startup. 0.9.x
  and Core read it as true. Other values still have to be `0`, `1`,
  `true`, `false`, `yes`, `no`, `on` or `off`.
