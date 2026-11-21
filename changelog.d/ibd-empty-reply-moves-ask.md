Fixed

- **An empty headers reply below the work floor no longer re-asks the peer
  that just answered empty.** The reservation now moves to another taller
  peer that has not answered empty at the walk tip. The all-peers-empty
  rewind therefore hears from every taller peer instead of waiting for the
  reserved peer to time out.
