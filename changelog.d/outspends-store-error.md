Fixed

- **Esplora answers a failed txid lookup as a store error, not as an
  unknown or unspent tx.** Bulk `/txs/outspends` and the internal
  outspends-by-txid route wrote `[]` for it, `/tx/:txid/merkle-proof` and
  `/merkleblock-proof` answered 404, an `after_txid` cursor got 422 `after_txid
  not found`, a confirmed cursor past the mempool page restarted the
  page, a script's mempool rows dropped or misdated a tx whose lookup
  failed, and `POST /internal/txs` left out a confirmed row it could not
  build. They now answer the store error. A confirmed tx whose wire body
  is pruned is still included, as the pruned partial `GET /tx/:txid`
  returns. A txid that is neither confirmed nor in the mempool is omitted.
