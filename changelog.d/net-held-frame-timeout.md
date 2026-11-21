Fixed

- **A frame parked for the peer's send budget no longer hides a stalled
  writer from the ping timeout.** A decoded non-block frame is put back at
  the front of the decode queue while the writer is over budget. The
  heartbeat treated every non-empty queue as a frame still waiting on a
  decode permit, so the ping and block-download timeouts stayed suspended
  and the peer kept its slot until something else dropped it. A held frame
  now leaves those timeouts armed. A frame still waiting to decode at the
  front still suspends them, and request expiry stays suspended while any
  frame is queued, including one behind a held front.
