Changed

- **`ibd: perf_full` shows what page-grouped body reads copy and drop.**
  `load_cold_range_read_kb` is what the confirm load's `txout.body` preads
  asked for (a merged group reads its whole page span, holes included);
  `load_cold_range_kept_kb` is what reached the parent jobs. The gap is the
  hole the grouping copied, the number a change to the merge limits needs.
- **Merged spend-slot reads reuse the thread's kept buffer.** The write
  thread's `spent.body` peeks allocated and zeroed a fresh buffer for every
  merged group. They now read into the same per-thread bounce pool as the
  body peeks.
