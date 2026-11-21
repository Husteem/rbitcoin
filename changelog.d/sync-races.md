Fixed

- **Three races in shared counters and flags:**
  - Two peers' heartbeats could both time out a noban peer's headers sync
    and both release the one sync slot. The count wrapped, so no headers
    sync could start again until restart while the header tip was more
    than a day old. A late heartbeat could also end the sync the peer had
    just restarted, so a second getheaders went out.
  - The IBD confirm queues counted a batch after handing it over, so the
    next stage could count it out first and leave a batch that was never
    there. Lookup's room for new work shrank by one each time.
  - A mempool change during a fee or transaction snapshot refresh could be
    lost, and that snapshot stayed stale until it aged out.
