Security

- **Block size and weight on an unstamped range are rebuilt once.**
  Esplora's block summaries fell back to rebuilding the whole block when a
  block's `txstat` rows were not stamped (ranges left by an older store),
  and nothing stamped them, so every request rebuilt it again. The rebuild
  now stamps the rows of an active-chain block, as `getblockstats` does.
