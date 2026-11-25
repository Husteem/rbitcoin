Security

- **A new block restatuses only the Electrum outpoint subscriptions it
  touches.** Every block looked up and pushed every outpoint subscription
  of every connection, up to 10,000 each, so a few clients could turn each
  block into millions of store reads. Each block's created transactions
  and spent outpoints are now read from the store and cached for every
  connection, and only the subscriptions they touch are looked up and
  notified, with those whose mempool spender changed since the last
  notification (a pool lookup, no store read). A reorg, a skipped or
  missed tip, or a block that cannot be read still restatuses every
  subscription, and so does the tip after a status that could not be read.

- **An Electrum outpoint's confirmed-spent status reports the spending
  block's height.** It reported the current tip, so the status changed on
  every block without anything happening to the outpoint.
