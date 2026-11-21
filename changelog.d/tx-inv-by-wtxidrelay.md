Fixed

- **A peer that did not send `wtxidrelay` hears of a transaction as
  `MSG_TX` by txid.** Every peer was sent `MSG_WTX` by wtxid, which BIP339
  allows only after `wtxidrelay`. A pre-70016 client such as bitcoinj could
  not parse the inventory type and dropped the connection on every relayed
  transaction. The per-peer announced set now holds the hash the peer was
  sent, as Core's known-inventory filter does (#1700).
- **`getdata` `MSG_TX` is answered without the witness,** as Core does.
  Only `MSG_WITNESS_TX` and `MSG_WTX` carry it, so a client that asks by
  `MSG_TX` gets a transaction it can parse (#1700).
- **A peer that sends `wtxidrelay` after `verack` is disconnected,** as
  Core does. The relay mode is fixed when the handshake ends, so a tx INV is
  never matched against the other hash (#1700).
- **A `wtxidrelay` from a peer below protocol 70016 is ignored,** as Core
  does. We send our own only from that version up, so such a peer still
  hears of a transaction by txid (#1700).
