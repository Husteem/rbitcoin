Fixed

- **Electrum `blockchain.transaction.broadcast`, Esplora `POST /tx` and
  BlindBit `forward-tx` answer the txid for a transaction already in the
  mempool,** as `sendrawtransaction` does and as electrs and Esplora in
  front of Core do. They returned `duplicate <txid>`, so a protocol where
  both parties broadcast the same transaction (Bisq MuSig) aborted (#1701).
