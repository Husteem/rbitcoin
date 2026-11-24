Security

- **One replacement may conflict with at most 100 clusters, as in Core.**
  There was no limit, so one transaction could evict any number of cheap
  transactions it conflicted with, along with all their descendants, under
  the mempool write lock. A replacement whose conflicts sit in more than
  100 distinct clusters is now refused with `too many potential
  replacements` (Core `MAX_REPLACEMENT_CANDIDATES`).
