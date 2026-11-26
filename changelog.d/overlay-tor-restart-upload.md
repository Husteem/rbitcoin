Fixed

- **The tor restart journey no longer caches a dead onion descriptor.**
  After the client tor comes back, `tor_onion_comes_back_after_tor_restarts`
  waits for `HS_DESC UPLOADED` before it SOCKS-connects. The node's control
  reconnect waits until that subscription is on the new process. Tor does
  not replay the event, and the local descriptor cache is filled before a
  directory stores the upload. A connect in the gap stores the previous
  descriptor, whose introduction points are gone, and the connect keeps
  failing for the rest of the refetch wait.
