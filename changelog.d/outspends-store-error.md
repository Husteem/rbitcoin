Fixed

- **Bulk outspends no longer answers "no spenders" when a txid lookup
  fails.** `GET /txs/outspends` and the internal outspends-by-txid route
  treated a store error from the txid lookup like an unknown txid and
  wrote `[]` for it. They now answer the store error.
