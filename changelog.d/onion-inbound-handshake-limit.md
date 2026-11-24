Fixed

- **Forwarded Tor peers can use the shared inbound limit.** A P2P listener marked `=onion` uses the node's inbound connection limit and handshake timeout for all forwarded peers. Ordinary TCP listeners still cap unfinished handshakes per source IP.
