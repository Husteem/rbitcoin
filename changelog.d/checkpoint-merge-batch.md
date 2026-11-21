Changed

- **A spend delta checkpoint reads merge records in one completion-session batch.**
  The merge used to `pread` each span on its own. A mainnet checkpoint
  does that for every sealed page that already has a record, on the order
  of a few hundred thousand reads, and confirm waits out the whole
  checkpoint. Those spans now go out together, at ring depth 128, into
  the one buffer the merge already sizes. Records that touch in a segment
  still share one span, so the gap between them is not read. `merge_reads`
  still counts spans. `RBITCOIN_IO=pread`, and a ring that will not open,
  keep the libc pread bulk fallback.
