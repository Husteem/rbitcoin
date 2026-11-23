Security

- **An Electrum idle deadline survives a passive push.** A client that
  subscribed to one scripthash and then sent nothing was never closed: the
  50 ms scripthash tick and every tip or mempool push ended a select pass, and
  the next pass rebuilt the read timeout. One deadline now covers the
  connection; only a complete request line or a silent-payments history step
  clears it, so a slow scan is not cut mid-history.
- **An Esplora response has a minimum drain rate.** The no-progress deadline
  reset on any accepted write, so a reader that took a few bytes before each
  deadline held its socket and accept slot for as long as it chose. A response
  write may now spend at most the request-timeout grace plus
  `bytes-written / --esplora-min-drain-rate` (default 16 KiB/s; NixOS
  `services.rbitcoin.esplora.minDrainRate`) in stalled time. Server compute
  between writes is not counted.
