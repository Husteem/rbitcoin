Fixed

- **A `getaddr` reply carries each row's learned time, not the reply time.**
  Every row used to be stamped with `now`, so a receiver stored a dead
  address as freshly seen and kept it in its book and on the wire (addr
  stamping). A row now keeps the `addrv2` time a peer gossiped, inside
  Core's window (after 1973, at most ten minutes ahead), discounted two
  hours as Core charges a source's claim; an untrusted time is five days
  back. The backdate is fixed on the row's first serve, so the row ages.
  The catch-up dial book keeps the same time. A full outbound handshake
  refreshes the time after twenty minutes (Core `Connected_`), and newer
  gossip refreshes it after Core's one-hour (online) or one-day (offline)
  interval. A stored time more than ten minutes ahead is backdated, never
  served as now. The relayed `addrv2` row carries the same sanitized time.
  The time survives a restart in `rbitcoin-peers-v3` (v1 and v2 still
  load), and `getnodeaddresses` reports it instead of `now`.
  `addpeeraddress` stamps the node clock, as Core does.
