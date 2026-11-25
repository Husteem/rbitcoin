Fixed

- **A coin is not called unspent while its spend re-enters the mempool.**
  After a block is disconnected, its transactions are briefly neither
  confirmed nor back in the mempool. In that window Esplora
  `/tx/:txid/outspend/:vout` and `/outspends` answered `{"spent":false}`,
  and `/address/:a/utxo`, `/scripthash/:hash/utxo` and Electrum
  `blockchain.scripthash.listunspent` listed the coin again, so a client
  could sign a conflicting spend. Those answers now wait for the pool to
  settle, as the tx routes already did: Esplora answers 503 `chain view
  moved` and Electrum an error. An empty UTXO list and a spent answer are
  unchanged.
