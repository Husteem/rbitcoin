Security

- **A flooded or txid-only peer is now asked for a transaction later.** The
  node asked a peer for a new transaction at once, even when that peer
  already had 100 requests waiting for an answer, or announced it by txid
  while peers that relay by wtxid were connected. A peer that never
  answered could still hold a transaction back for up to a minute. As in
  Core, each of these adds 2 seconds to the wait before the node asks: a
  peer with 100 or more requests outstanding and no relay permission, and a
  txid announcement while a wtxid-relay peer is connected. They add to each
  other and to the 2 seconds a non-preferred peer already waits.
