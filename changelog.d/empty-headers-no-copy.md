Security

- **An empty `headers` message no longer runs a header job.** Each one
  cloned the peer's map of up to 8,000 parked headers twice, cloned its
  requested-block set, and waited for a header-pool permit that honest
  header sync shares. A peer could send empty `headers` in a loop to burn
  about a core. An empty reply now just answers the getheaders.
