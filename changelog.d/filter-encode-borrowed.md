Changed

- **Block filters are built without copying every script.** The BIP158
  encoder took rust-bitcoin's `GcsFilterWriter`, which copies each output
  and spent script into an ordered set. It now hashes borrowed scripts,
  sorts them by hash, and drops exact duplicates; the filter bytes are the
  same. The live filter job also stopped cloning every transaction's
  cached prevouts before encoding, and reads the block's txids from the
  batch instead of hashing each transaction again for in-block spends.
