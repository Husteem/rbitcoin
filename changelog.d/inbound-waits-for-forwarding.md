Fixed

- **Tor and I2P peers that connect during startup are no longer capped as
  one loopback source.** The P2P listeners accepted before the node had
  installed its permission table, its `=onion` bind and its forwarding
  setting, so forwarded peers arriving in that window (which includes the
  mempool load) counted against the 8 unfinished handshakes allowed per
  address. Listeners now bind at start but accept once that state is in
  place; connections that arrive earlier wait in the kernel's backlog.
