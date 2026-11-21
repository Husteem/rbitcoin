Fixed

- **Four races in shared counters and flags:**
  - Two peers' heartbeats could both time out a noban peer's headers sync
    and both release the one sync slot. The count wrapped, so no headers
    sync could start again until restart while the header tip was more
    than a day old.
  - The IBD confirm queues counted a batch after handing it over, so the
    next stage could count it out first and leave a batch that was never
    there. Lookup's room for new work shrank by one each time.
  - A mempool change during a fee or transaction snapshot refresh could be
    lost, and that snapshot stayed stale until it aged out.
  - A store table's record count and body end could be read from two
    different writes on weakly ordered CPUs (aarch64).
