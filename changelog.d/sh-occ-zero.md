Fixed

- **A zero scripthash occupancy sidecar does not wipe a live head.**
  A kill between the slot-table write and the sidecar rename used to
  leave `.occ` at 0. Open treated that large head as empty, so the next
  insert bulk-filled over the live slots and a packed shard skipped the
  ingest soft-clear. Open scans that head and keeps the count it finds.
