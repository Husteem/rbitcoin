Fixed

- **A scripthash history read keeps creates committed between its two lists.**
  The durable posting list and the pending RAM list come from one even
  publish sequence. A commit that appends those creates and drops the RAM
  copy in the middle stays in the result, once. A later read of the same
  script does not repeat them.
