Changed

- **Sealed `tx.head` probes keep their MPHF page buffers.** Each probe of
  a sealed segment's FdOnly `g` allocated and zeroed a pool of up to 128
  page buffers (512 KiB), then zeroed each buffer again before its read.
  The resolve thread now keeps the pool between waves, and a page buffer is
  not zeroed before the read that fills it (a short read already re-reads
  the whole page).
