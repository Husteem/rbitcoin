Fixed

- **A header at or below its parent's median time past is a permanent
  reject.** `time-too-old` was classed with `time-too-new` as a temporary
  time failure, so tip follow did not punish the peer that sent it and a
  failed accept did not mark the block invalid. Only `time-too-new` (more
  than two hours past the node's clock) stays temporary, as in Core: the
  median depends only on the block's ancestors, so a too-old header never
  becomes valid. IBD's confirm path already treated it as invalid. An
  incomplete median window is now a local fault (no score, no failed mark),
  and a too-old `block` message is marked failed as well as scored.
