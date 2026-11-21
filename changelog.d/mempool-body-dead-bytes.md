Fixed

- **A full mempool no longer grows `mempool/tx.body` on every admission.**
  Each admit reused the slot its eviction freed but appended the new payload,
  leaving the dead payload in the body. Compaction only ran on block connect
  and only when dead slots were a large share of the table, so a pool that
  stayed near full kept every admitted payload and open read the whole body.
  An admit now compacts first when dead payloads reach the live payload bytes
  (#1674).
