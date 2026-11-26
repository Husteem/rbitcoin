Fixed

- **Peers from an operator's own Tor hidden service that connect during
  startup are no longer capped as one loopback source.** The P2P
  listeners accepted before the node had installed its permission table,
  its `=onion` bind and its forwarding setting, so peers forwarded from
  loopback in that window (which includes the mempool load) counted
  against the 8 unfinished handshakes allowed per address. Listeners now
  bind at start but accept once that state, the asmap and the prune flag
  are in place. Connections that arrive earlier wait in the listen
  backlog (128); past that, new connection attempts are dropped and
  retried by the peer.
