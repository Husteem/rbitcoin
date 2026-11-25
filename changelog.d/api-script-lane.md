Security

- **API transaction tests and broadcasts can no longer fill the script
  pool.** Esplora, Electrum, BlindBit and RPC transaction tests,
  broadcasts and packages ran their script checks on the pool block
  validation uses, with no limit beyond the listeners' request caps. They
  now share one lane of half the script workers, at least one, so on a
  pool of two or more workers block validation keeps the rest.
  Peer-relayed transactions are not affected.
