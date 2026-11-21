Changed

- **Confirm reads spend-delta records in one completion-session batch.**
  A lookup that missed the open notes used to `pread` each page's record
  on its own. Those records now go out together, at ring depth 128, on
  the same session as the `spent.body` peeks. Records that touch in a
  segment share one span. `RBITCOIN_IO=pread`, and a ring that will not
  open, keep the libc pread bulk fallback. A folded page with no record
  is still not read from the delta, and a RAM note is still not read.
