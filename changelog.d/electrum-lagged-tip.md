Fixed

- **A lagged Electrum session no longer loses a reorg for silent-payment
  and outpoint subscribers.** When a slow connection dropped tip events,
  only scripthash statuses were sent again: a lost reorg left live
  silent-payment subscriptions scanned above the fork, running scans were
  not restarted, and outpoint subscribers got no push. A lag now sends the
  outpoint, silent-payment and scripthash notes for the store's tip; the
  queued events still push their headers. When the last tip the session
  handled is no longer on the chain, the lag counts as a reorg six blocks
  below the lowest silent-payment height, so those blocks are scanned
  again.
