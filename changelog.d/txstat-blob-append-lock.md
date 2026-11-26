Fixed

- **Concurrent block reads no longer overwrite each other's fee stamps.**
  Esplora `/block/:hash` routes (including `/txs/summary`), `getblock`
  at verbosity 1 and `getblockstats` stamp an unstamped block's fee and
  size overflow on first read, as the archive writer does during sync.
  Two stamps at once could write their overflow at the same offset of
  `txstat.ovf`, so one block decoded as corrupt and was stamped again,
  or, with equal-length overflows, served another block's fee or size.
  Appends are now serialised (#2093).
