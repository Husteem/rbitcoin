Security

- **A wide Esplora history gap stays inside `--max-sh-creates`.** A newest-first
  page that reaches back more than 8064 blocks joins spend slots through the
  same cap, and the same heavy-join turn, as an unpaged scripthash read. A
  page inside that window still scans those blocks. Electrum
  `blockchain.scripthash.subscribe` records a hash only after its status is
  computed, so a refused status does not stay watched. A subscribed hash with
  no postings is detected from the create count.
