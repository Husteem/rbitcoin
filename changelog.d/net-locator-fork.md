Fixed

- **A locator with the zero hash, or with no known hash, no longer serves
  genesis.** `headers_after_locator` treated the zero hash as "start at
  genesis" and broke out of the locator walk, so a valid hash later in the
  same locator was ignored. A locator that matched nothing also started at
  height 0. Both now skip unknown hashes, use genesis as the fallback fork
  point, and reply from the block after it. A known hash on a stale branch
  that contains our tip is still not mapped to that tip; the walk skips it
  and keeps looking for an active-chain entry, falling back to genesis only
  when none matches. The header-cache fallback starts after genesis the
  same way, at `chain[0]` when its window no longer includes genesis.
