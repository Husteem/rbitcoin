Security

- **A headers message no longer copies the peer's pending headers.** Each
  non-empty `headers` message copied the sender's parked headers (up to
  8,000) twice, and each compact block once more. A peer that kept that
  map full and then sent one-header messages made every message cost two
  copies of it. The map now moves into the header job and back, and the
  batch is staged in place and undone. A message that evicts nothing costs
  its own headers. One that must make room in a full map still scans the
  parked headers once, as before.
