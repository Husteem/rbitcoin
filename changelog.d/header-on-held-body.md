Fixed

- **A peer that builds on a block this node holds without its header is
  no longer disconnected.** A `submitblock` side block that lost a race
  is kept as a body with no header row. When the network then built on
  it, the child's header failed "header parent unknown" and its sender
  was discouraged and disconnected, so a solo or SV2 miner could lose
  every peer announcing the chain it should reorg to. The held block's
  header is now written before its child's. A header whose parent's
  height this node cannot work out is a local fault and no longer scores
  the sender.
