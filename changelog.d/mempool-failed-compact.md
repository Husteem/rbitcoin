Fixed

- **A failed mempool compaction no longer revives a removed transaction.**
  Compact swapped in the packed body and slot table before the temporary-file
  writes could fail, so the graph kept the old slot numbers while the store
  used the packed ones. A later removal then marked the wrong slot, and the
  next successful compact reloaded the removed transaction from its still-LIVE
  slot. The images are now installed before the store adopts them, and the
  graph is rebuilt from the slots the store holds even when the install
  returns an error.
