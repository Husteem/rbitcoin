Changed

- **`tx.head` insert stores one slot.** A batch hops the live table and
  writes the empty slot. It does not copy the probe page.
