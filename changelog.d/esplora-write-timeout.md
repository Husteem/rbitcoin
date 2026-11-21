Security

- **An Esplora response write has a timeout.** A client that stops reading
  can no longer hold the connection and its accept slot. A write that makes
  no progress is cancelled at the request timeout, the same bound the
  Electrum writer and the internal history body use.
