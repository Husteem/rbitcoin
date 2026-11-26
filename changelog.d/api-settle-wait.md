Fixed

- **Balance, UTXO and outspend reads no longer fail at every block.** Each
  block connect holds the mempool for a moment, and an Esplora or Electrum
  read that arrived then answered "chain view moved" (Esplora 503) at once.
  That hit Electrum `get_balance` and `listunspent`, Esplora `/utxo`,
  address stats, `/outspend(s)` and the tx routes about once per block. The
  read now waits for the hold to end, up to 5 s, and then answers. A longer
  hold, or a tip move during the read, still answers "chain view moved".
  `/metrics` counts the waits (`rbitcoin_api_settle_waits_total`,
  `_wait_seconds_total`, `_gave_up_total`, `_refused_total`), and a 5 s window
  with a wait logs one INFO `api: settle waits=` line.
