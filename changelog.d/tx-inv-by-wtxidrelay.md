Fixed

- **A peer that did not send `wtxidrelay` hears of a transaction as
  `MSG_TX` by txid.** Every peer was sent `MSG_WTX` by wtxid, which BIP339
  allows only after `wtxidrelay`. A pre-70016 client such as bitcoinj could
  not parse the inventory type and dropped the connection on every relayed
  transaction. The per-peer announced set now holds the hash the peer was
  sent, as Core's known-inventory filter does (#1700).
