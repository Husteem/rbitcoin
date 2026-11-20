Changed

- **Two equal-work tip siblings are scripted together.** A body already held
  for the same parent now runs its script wave beside the preferred block's,
  with each wave capped at half the worker pool while the other still claims.
  The first candidate whose strict scripts and write succeed is the tip; the
  other is parked. An invalid first block no longer delays a valid sibling for
  a full script wave.
- **A fully held, strictly heavier branch is connected before an expensive
  tip child** instead of after it.

Fixed

- **A replay of a known-invalid body costs no lane slot.** `on_block` drops it
  after the header checks, before relay and enqueue. A bad-proof-of-work
  header still scores the peer.
- **A rejected tip block no longer runs index work.** The tip path publishes
  the index wave only after the script wave passes.
- **`getblockcount` does not stall on a tip-pair script race.**
