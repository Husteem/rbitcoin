Fixed

- **Nightly fuzz builds again.** `drain_pending_now` takes a `PeerOutTx`,
  so the block and compact drain harnesses build their writer channel with
  `peer_out_channel()`.
