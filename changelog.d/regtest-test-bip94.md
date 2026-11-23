Added

- **`--test bip94` enforces BIP94 on regtest.** It matches Core's
  `-test=bip94` (conf `test=bip94`): a difficulty period's first block
  more than 600 seconds before its parent is `time-timewarp-attack`, and
  a retarget starts from the period's first `nBits`. Another network or
  another option fails start. The Core-functional `bitcoind` shim
  forwards `-test=bip94` (#1779).
