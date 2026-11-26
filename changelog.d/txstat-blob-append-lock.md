Fixed

- **Concurrent block reads no longer overwrite each other's fee stamps.**
  Esplora block routes, `getblock` and `getblockheader` stamp an unstamped
  block's fee and size overflow on first read. Two such reads at once
  could write their overflow at the same offset of `txstat.ovf`, so one
  block decoded as corrupt and was stamped again, or with equal-length
  overflows served another block's fee or size. Appends are now
  serialised.
