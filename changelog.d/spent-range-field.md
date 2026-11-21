Changed

- **The confirm write thread reads a parent's spent range without a lock.**
  The spent-slot position of every spend was read through the pin's layout
  snapshot: a mutex and an `Arc` clone per spend, twice per spend per batch.
  Setting it cloned the whole layout, spender rels included. A create's
  spent range is fixed at its append, so the pin now holds it in a
  write-once cell beside the layout.
