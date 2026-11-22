Fixed

- **The 4 MiB per-peer send budget counts every queued message.** Block
  announces (`headers`, `inv`, `cmpctblock`), `getheaders`, pings,
  `getaddr`, `feefilter`, `sendcmpct`, and our own `addr` announce were
  queued without a charge, yet the writer still subtracted their size, so
  a session read new requests while replies past the budget were still
  queued. Every message is now charged when it is queued and released
  once it is written, as Core's `nSendSize` does. A peer that stops
  reading now pauses at the budget, counting those messages too.
