Fixed

- **Refused listener sockets log at most once a second.** The health
  listener wrote one warning per refused connection past its cap, and every
  listener (P2P, Electrum, Esplora, health) wrote one per socket it refused
  to keep descriptors for the store, so a client that reconnected in a loop
  could fill the log. Those lines now share the per-thread limit the RPC
  max-connections line already has.
