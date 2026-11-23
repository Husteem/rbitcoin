Fixed

- **A block that connects during a getheaders reply is still announced.**
  An empty headers reply marked our current tip as sent to the peer. A
  block that connected while the reply was built became that tip, so its
  announcement was skipped and the peer waited up to two minutes for the
  headers poll. The reply now marks the block where the peer's locator met
  our chain.
