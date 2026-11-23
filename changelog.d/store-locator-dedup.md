Changed

- **Store locator, spill, and durable-sidecar code has one implementation
  each.** `create.loc` and `seqsigwit.loc` share their append bookkeeping;
  the keys and postings spill directories share one protocol; and the RPC
  request envelope is parsed and answered once for both entry points.
- **Durable store sidecars fsync the parent directory.** Cold-progress,
  inclusion-HWM, phase-mark, and scripthash-occupancy writes now use the
  store's shared temp+rename helper instead of a private copy, so a renamed
  sidecar survives a power loss like the rest of the store.
