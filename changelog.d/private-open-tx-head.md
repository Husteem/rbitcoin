Changed

- **Open `tx.head` stays in memory.** The unsealed segment is a private
  128 MiB table. Insert no longer rewrites its slot file. A clean shutdown
  writes that table once. A crash rebuilds the open tail from `txid.body`.
