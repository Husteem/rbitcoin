Fixed

- **Esplora outspends no longer call a coin unspent when a block spends
  it mid-request.** The outspends routes read the pool's settle generation
  after pinning the chain view. When a block that spent the coin connected
  and the pool settled in between, the spender was above the view and gone
  from the mempool, so `/tx/:txid/outspend/:vout`, `/tx/:txid/outspends`,
  `/txs/outspends` and the internal outspends routes answered
  `{"spent":false}`. The generation is now read before the pin, and that
  case answers 503 `chain view moved` (#2082).
