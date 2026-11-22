Security

- **Transaction announcements share one global rate limit.** A burst of
  accepted transactions no longer queues one INV per peer. A count bucket
  (14 tx/s, 420 capacity) and a size bucket (20 kB/s, 50 MB capacity) release
  wtxids once for every peer; outbound peers refill both buckets at 2.5x.
  Excess transactions wait in one backlog instead of a per-peer queue, so a
  flood cannot multiply INV work by the peer count. A full bucket may
  overdraw to Core's -70 count / -50 kB size floor. Incoming INVs still
  handle each txid and wtxid once per message.
