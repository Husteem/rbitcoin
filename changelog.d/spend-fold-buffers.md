Changed

- **Spend delta fold rounds reuse their buffers.** A checkpoint composes
  every folded page into one page buffer and every round into one `fold`
  buffer, emptied but kept after each round's copy. It used to allocate a
  4 KiB image per folded page and regrow the round's `fold` buffer (up to
  16 MiB) from empty every 4,096 pages: about 12.5 KiB asked of the
  allocator per folded page, now about 0.7 KiB. Peak RSS is unchanged:
  mimalloc already reused those blocks on the same thread. The cut is
  allocator calls and the copies a doubling buffer makes.
