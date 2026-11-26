Fixed

- **Electrum `blockchain.transaction.broadcast` and Esplora `POST /tx` refuse a
  transaction with the reason `sendrawtransaction` gives, as electrs reports
  it.** They answered internal text (`broadcast reject: policy: min
  relay fee`, `orphaned <txid>`, `missing prevout <txid>:0`), so a client that
  matches Core's reason, such as rgb-lib's `min relay fee not met`, missed it.
  Electrum's message is the bare reason at electrs's daemon-error code 2
  (a malformed request is still 1). Esplora answers 400
  `sendrawtransaction RPC error: {"code":-26,"message":"min relay fee not met"}`;
  BlindBit `forward-tx` carries the same reason inside its `{"error"}` body.
  A transaction whose outputs are still coins is
  `-27 Transaction outputs already in utxo set` on all three, as on RPC with
  the tx index on (#1904). Electrum (single tx and package), Esplora
  `POST /tx`, and BlindBit `forward-tx` apply the same nonstandard-version
  and default max-fee rejects as `sendrawtransaction` (`"version"`; `-25`
  `Fee exceeds maximum configured by user (e.g. -maxtxfee, maxfeerate)` at
  0.10 BTC/kvB) (#2020). A one-parent one-child submit whose parent is only
  in the extra pool is held to that same cap (the caller's `maxfeerate` on
  RPC): the parent rolls back and the answer is the same `-25`. A child
  whose parent is already in the mempool at admit time is held to that cap
  before it commits. An Electrum package over 25 transactions or 404,000
  weight is refused with the package-too-large reason before the fee walk.
