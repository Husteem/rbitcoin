Fixed

- **A locator with the zero hash, or with no known hash, no longer serves
  genesis.** `headers_after_locator` treated the zero hash as "start at
  genesis" and broke out of the locator walk, so a valid hash later in the
  same locator was ignored. A locator that matched nothing also started at
  height 0. Both now follow Core's `FindForkInGlobalIndex`: unknown hashes
  are skipped, genesis is the fallback fork point, and the reply starts at
  the block after it. The header-cache fallback uses the same rule.
