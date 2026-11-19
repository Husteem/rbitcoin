Fixed

- **A zero scripthash occupancy sidecar does not wipe a live head.**
  A kill between the slot-table write and the sidecar rename used to
  leave `.occ` at 0. The next insert treated that head as empty and
  bulk-filled over the live slots. A large head scans before that fill
  and keeps the slots it finds.
