Security

- **A new block checks Electrum scripthash subscriptions with a set lookup.**
  Each block loaded the full history list of every subscribed script that
  had any history, to see whether the block touched it. One connection
  could hold 10,000 busy public scripts, and many connections could do the
  same, so every block cost millions of list entries. Each block's script
  hashes are now read once, shared by every connection, and a subscription
  is checked against that set without loading its history.
