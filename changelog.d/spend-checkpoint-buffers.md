Changed

- **A spend delta checkpoint merges into two buffers, not a map per
  page.** The merge read each touched page's committed record into its own
  copy, built a B-tree of its slots, laid the sealed notes over it, and
  encoded a new record per page: about seven allocations a page, and a
  mainnet checkpoint touches about 180,000 pages every 20 seconds in IBD.
  It now reads the committed records into one buffer sized from the
  directory and writes the merged records straight into one more, which
  the segment append writes from. Records that reclaim carries go into a
  buffer of their own, so they never regrow the merge's. Records are byte
  for byte what they
  were, and a committed record whose slots are out of order still refuses
  the merge. On a mainnet IBD, anonymous RSS stayed about 220 MiB higher
  for several seconds after each checkpoint while mimalloc held the freed
  memory; how much of that this removes needs the next IBD to tell.
- **Spend delta fold rounds reuse their buffers.** A checkpoint composes
  every folded page into one page buffer and every round into one `fold`
  buffer, emptied but kept after each round's copy. It used to allocate a
  4 KiB image per folded page and regrow the round's `fold` buffer (up to
  16 MiB) from empty every 4,096 pages: about 12.5 KiB asked of the
  allocator per folded page, now about 0.7 KiB. Peak RSS is unchanged:
  mimalloc already reused those blocks on the same thread.
