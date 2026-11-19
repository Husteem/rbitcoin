Changed

- **The spend delta's in-RAM notes take about 100 MiB at the cap in IBD,
  peaking near 170 MiB while a checkpoint sorts, instead of several
  hundred, and a few hundred KiB at the tip.** Open
  notes were a SipHash map of pages, each holding a B-tree of its slots:
  about 34 bytes a note plus about 100 bytes a touched page. Open notes are
  now one hash map keyed by slot index, and a checkpoint sorts the notes it
  seals into a slot-ordered array of 16 bytes a note with a small
  key-range directory, read and merged in page order. Each checkpoint
  allocates the next window's map before the freeze, sized from the window
  it closes: after a window at the 2,000,000-note cap, a map of about
  68 MiB that holds the cap and a full write phase past it without
  regrowing. The freeze under the confirm exclusion swaps maps, and after
  a disconnect also copies the notes to judge them; the sort runs after
  it. The `store: spend delta` open and checkpoint lines gain
  `notes_ram=`, the RAM the notes allocate after the commit.
