Security

- **A new block restatuses only the Electrum outpoint subscriptions it
  touches.** Every block looked up and pushed every outpoint subscription
  of every connection, up to 10,000 each, so a few clients could turn each
  block into millions of store reads. Each block's created transactions
  and spent outpoints are now read once and shared, and only the
  subscriptions they touch are looked up and notified. A reorg, a skipped
  height, or a block that cannot be read still restatuses every
  subscription.
