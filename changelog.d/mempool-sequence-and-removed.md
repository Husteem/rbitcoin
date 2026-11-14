Added

- **Esplora `GET /mempool/removed` says why a transaction left the mempool.**
  A dialect route on both Esplora listeners lists the recent committed
  removals, oldest first, each with its sequence, txid, and Core's reason
  (`expiry`, `sizelimit`, `reorg`, `block`, `conflict`, or `replaced` with
  the replacing txid). To make the rows usable, `getrawmempool false true`
  now returns Core's `mempool_sequence`: it advances on every add and every
  removal, a mined block included, and is read with the txid list. A client
  that lists the pool at S1 and at S2 finds the reason for each txid that
  left in the row numbered `S1 <= seq < S2`.
