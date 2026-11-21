Fixed

- **A compact block whose reconstruct fails its witness commitment is asked
  for again as a full block from the same peer.** Only other high-bandwidth
  peers were asked, so a single `--connect` or noban peer left the node one
  block behind until another header or a child arrived. Core's `FillBlock`
  asks the same peer and does not score it, because a short-id collision is
  not the sender's fault. The peer is no longer scored (#1791).
