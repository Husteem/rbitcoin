Fixed

- **A `getaddr` reply carries each row's learned time, not the reply time.**
  Every row used to be stamped with `now`, so a receiver stored a dead
  address as freshly seen and kept it in its book and on the wire (addr
  stamping). A row now keeps the `addrv2` time a peer gossiped, inside
  Core's window (after 1973, at most ten minutes ahead); a row with no time
  is five days old; repeating gossip cannot refresh a held row. The time
  survives a restart in `rbitcoin-peers-v3` (v1 and v2 still load), and
  `getnodeaddresses` reports it instead of `now`. `addpeeraddress` stamps
  the call time, as Core does.
