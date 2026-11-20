Fixed

- **An IBD peer's ping that lands while its pong is written is answered.**
  The IBD pong slot used release/acquire ordering, so the reader and the
  writer could each miss the other's store and the ping was never answered.
  The tip-follow session had the same race fixed earlier. Both now share one
  pong type.
