Fixed

- **Electrum `mempool.get_fee_histogram` answers in sat/vB with each bin's
  own size.** It sent the fee snapshot's sat/kvB rate and a running total of
  vsize. The protocol's rate is sat/vbyte, and each pair's vsize covers only
  its own fee interval. The rate is now the snapshot rate divided by 1,000,
  as Esplora's `GET /mempool` already sends it.
