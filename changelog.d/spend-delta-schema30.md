Changed

- **Schema 30 records every spend in a spend delta, and `spent.body`
  becomes a sparse file that only folds write.** Class A no longer writes
  `spent.body` bytes: an append moves its published end, and every spend,
  same-block and same-batch ones included, is a delta note. A page no fold
  wrote is never read and reads zero, so a torn `spent.body` block can only
  lose a page a fold wrote, which the fold's own copy covers. A checkpoint
  appends one record per touched page to a 16 MiB segment in
  `store/spent.delta/`, appends that generation's directory entries as one
  checksummed batch to `log.<id>`, and renames `commit`. A touched page
  whose record already spends half its unset slots folds into
  `spent.body` instead of being rewritten. Reclaim takes the sparsest
  segments under twice the delta's share of `spent.body` live (at least
  5%), reading only their live records: a record whose spends cover half
  its page's unset slots folds into `spent.body`, others are copied
  forward. A first fold of a page writes it once; a later one copies it
  through `spent.delta/fold` first. Folds run in rounds of 4,096 pages.
  Each page's filled-slot count sits in the directory and only a fold
  changes it, so deciding reads no page. The directory costs 12 bytes per
  page on disk and 8 in RAM. No write touches a 4 KiB block holding bytes
  the commit names, so a block a crash tears to zeroes loses no committed
  spend. Open reads the commit before anything else, trusts it alone and
  ignores whatever a crash left past it. Before replay, even when no block
  is left, open drops delta spends sealed above a recovered tip whose
  spender is not on the chain. A checkpoint runs every 10 minutes or at
  2,000,000 open slot notes; after 60 failed checkpoints in a row, confirm
  waiting at that cap stops with a store error instead of stalling. Each
  checkpoint logs the delta size and
  lifetime writes against `spent.body` (`store: spend delta …`) and its
  per-step reads, writes and times (`store: spend delta … timing`);
  `ibd: perf` gains `spend_room_wait_ms`. Opening a schema-29 datadir reads
  its `spent.body` once, publishes it and its `spend.pages` log as the
  first delta generation and removes the 29 files; a held log is no longer
  refused. `spent.body` and `create.loc` offsets are unchanged, so there is
  no Class A reindex. A 29 binary refuses 30 `meta`. A datadir from an
  earlier build of schema 30 refuses open with a wipe line.
