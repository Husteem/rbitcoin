Fixed

- **Esplora answers a failed txid lookup as a store error, not as an
  unknown or unspent tx.** Bulk `/txs/outspends` and the internal
  outspends-by-txid route wrote `[]` for it, `/tx/:txid/merkle-proof` and
  `/merkleblock-proof` answered 404, an `after_txid` cursor got 422 `after_txid
  not found`, and a confirmed cursor past the mempool page restarted the
  page. They now answer the store error.
