Changed

- **Each `header.head` overflow generation is twice the one before it.** A
  header lookup probes every generation, and equal-size generations made it
  linear in the header count past the first: 10,000 headers on the 64-slot
  test scale were 179 files per lookup. They are now 8. Older stores with
  equal-size generations open as before; no schema bump.
