Fixed

- **A locator with the zero hash, or with no known hash, no longer serves
  genesis.** `headers_after_locator` treated the zero hash as "start at
  genesis" and broke out of the locator walk, so a valid hash later in the
  same locator was ignored. A locator that matched nothing also started at
  height 0. Both now skip unknown hashes, use genesis as the fallback fork
  point, and reply from the block after it. The header-cache fallback uses
  the same rule. A known hash on a stale branch that contains our tip is
  still not mapped to that tip; it falls through to the genesis fallback.
