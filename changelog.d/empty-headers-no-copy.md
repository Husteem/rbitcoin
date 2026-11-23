Security

- **An empty `headers` message no longer copies the peer's pending
  headers.** Each one cloned the peer's map of up to 8,000 parked headers
  and its requested-block set, and waited for a header-pool permit that
  honest header sync shares. A peer could send empty `headers` in a loop to
  burn about a core. It now answers the getheaders without either.
