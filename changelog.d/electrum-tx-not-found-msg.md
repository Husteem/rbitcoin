Fixed

- Electrum `blockchain.transaction.get` now returns Bitcoin Core's standard missing-transaction error text (`RAW_TX_NOT_FOUND`) instead of `tx not found` when a txid is absent from both chain and mempool.
