Fixed

- **A failed mempool compaction no longer revives a removed transaction.**
  Compact swapped in the packed body and slot table before the temporary-file
  writes could fail, so the graph kept the old slot numbers while the store
  used the packed ones. A later removal then marked the wrong slot, and the
  next successful compact reloaded the removed transaction from its still-LIVE
  slot. The images are now installed before the store adopts them, so a failed
  write leaves body, slots, and the live counts as they were. If the body
  rename lands and the slots rename fails, the pool follows the packed body
  and keeps writing the live table to `slots.tmp` until a later persist or the
  next open links it, so removals made in that window survive the restart.
