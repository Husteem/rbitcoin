Fixed

- **The tor restart journey no longer caches a dead onion descriptor.**
  After the client tor comes back, `tor_onion_comes_back_after_tor_restarts`
  waits for `HS_DESC UPLOADED` before it SOCKS-connects. A connect in that
  gap stores the previous descriptor, whose introduction points are gone,
  and the connect keeps failing for the rest of the refetch wait.
