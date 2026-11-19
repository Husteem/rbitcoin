Changed

- **IBD keeps less in RAM.** At a ~962k-block tip:
  - `has_block` reads the confirmed hash → height index, which also keys
    by the block hash itself instead of SipHash. The second set of every
    confirmed hash (~66 MiB) and its background fill at open are gone, so
    a restart answers every height at once.
  - Peer readers no longer share an unpruned header hash → fk map
    (41–82 MiB). The coordinator already held those fks.
  - The resume header index, built on each IBD start, is flat arrays
    (~45 MiB) instead of a map of per-parent lists (~210 MiB).
  - Opening `strong_tx` reads in 16 MiB chunks instead of through a second
    ~177 MiB copy, and a spend-delta directory rewrite no longer copies
    its entries again before writing.
  - Load batches no longer build and clone a per-parent vout list nothing
    read, and each queued block's spend keys drop the unused vout.
