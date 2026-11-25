Changed

- **Mempool admission reuses the confirmed parent lookup.** The spentness
  check uses the same archive row as the output read in both preparation
  and the recheck after script verification.
