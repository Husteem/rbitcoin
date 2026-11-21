Changed

- **IBD logs less and says more.** A spend checkpoint now writes one compact
  INFO summary; its detailed timing is DEBUG (category `store`) and is promoted
  to INFO only when the checkpoint is slow. Repeated checkpoint failures warn
  once a minute, with one recovery line.
- **`ibd: perf` is split by cadence.** The 5s line (category `ibd`) is a
  compact headline: pace, queues, stage totals, the spend window, and RSS. The
  full `ibd: perf_full` object is written every 30s or on a slow phase.
  `ibd: perf` gains a nested `spend` object with the checkpoint count, worst
  wall time, and `notes_ram`.
- **The 5s RSS path no longer reads `smaps_rollup`.** `rss_kb` and the
  `RssAnon` / `RssFile` split stay fresh from `/proc/status`; the
  `smaps_rollup` fallback (and `Locked`) is on the 30s deep read.
- **A slow confirm phase warns on entry, once a minute, and on recovery**
  instead of every 5s.
- **Dropped duplicate `ibd: perf` keys:** the six `stamp_batch_*` child
  timers (use `arch_prep_*`), `plan_blks` / `plan_ms` / `plan_collect_ms` /
  `plan_head_ms` / `plan_cold_io_ms` (use the `lookup_*` names, including the
  new `lookup_blocks` / `lookup_ms` / `lookup_collect_ms` / `lookup_head_ms` /
  `lookup_cold_io_ms`), the duplicate `arch_prep_probe_ms` /
  `arch_prep_idx_ms` / `arch_prep_body_txid_ms`, the `_ns` twins of the
  `_ms` stage timers, `wf_body_store` / `wf_store_body_ms`,
  `rss_locked_kb`, the cumulative `uring_recover_n` /
  `uring_slow_drain`, and the duplicate age histogram
  (`arch_prep_age_hit_compact` plus `cdf7/15/31`; keep `cdf0`, `cdf3`, and
  `age_hit_n`). The full key list is `SAMPLE_FIELDS`.
