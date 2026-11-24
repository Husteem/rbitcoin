Security

- **The recent-rejects filter no longer fills up.** It stopped recording
  after 4,096 rejected transactions until the next block, so a peer could
  fill it with cheap junk and make the node download and check every later
  rejected transaction again. It now holds up to 120,000 entries, as Core
  does, and drops the oldest when full.
