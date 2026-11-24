Fixed

- **Esplora `/tx/:txid/status` answers `200 {"confirmed":false}` for a txid
  that is neither stored nor in the mempool, as electrs, blockstream.info and
  mempool.space do.** It
  answered 404, which `esplora-client`'s `get_tx_status` returns as an error,
  so a client asking before it broadcasts (rgb-lib's refresh after an RGB ACK)
  failed. `/tx/:txid`, `/hex`, `/raw` and the outspend routes stay 404 for that
  txid, and so does `/status?asof=`. A stored transaction that was reorged out
  and is not in the mempool keeps its 404 (#1916).
