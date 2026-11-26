Fixed

- **A peer that builds on a side block this node holds is no longer
  disconnected.** A `submitblock` side block that lost a race was kept as
  a body with no header row, while the node still counted it as a known
  header. When the network built on it, the child's header was refused
  as "header parent unknown", or the child was judged hopeless at height
  1, and its sender was discouraged and disconnected, so a solo or SV2
  miner could lose every peer announcing the chain it should reorg to.
  A held body whose parent has a header row now gets its own row, as Core
  stores a side block's header. A held orphan has none and no longer
  counts as a known header, so a header built on it is answered with
  `getheaders` instead of a penalty.
