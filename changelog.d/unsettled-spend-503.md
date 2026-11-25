Fixed

- **A coin is not called unspent while its spend re-enters the mempool.**
  After a block is disconnected, its transactions are briefly neither
  confirmed nor back in the mempool. In that window Esplora
  `/tx/:txid/outspend/:vout`, `/outspends`, `/txs/outspends` and the
  internal outspends routes answered `{"spent":false}`; `/utxo`, address
  and scripthash stats, and Electrum `blockchain.scripthash.listunspent`
  and `get_balance` counted the coin again; and Electrum
  `blockchain.outpoint.get_status` and `subscribe` called it unspent. A
  client could sign a conflicting spend. Those answers now wait for the
  pool to settle, as the tx routes already did: Esplora answers 503
  `chain view moved` and Electrum that error. An empty UTXO list, a zero
  balance, and a spent answer are unchanged.
