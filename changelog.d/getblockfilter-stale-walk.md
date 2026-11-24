Fixed

- **`getblockfilter` walks at most 10,000 stale blocks.** A stale block
  is rebuilt from its best-chain fork point. That walk had no depth bound,
  so a request for the tip of a long stored side branch rebuilt every block
  on it. Past 10,000 stale blocks, the same bound as Esplora's stored-header
  walk, the request now fails with "Filter not found. Stale branch is
  deeper than 10000 blocks." before any body is rebuilt. Core keeps every
  connected block's filter and has no such limit (#1684).
