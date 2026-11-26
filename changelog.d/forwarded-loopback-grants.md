Changed

- **A `--net-permission` rule for loopback no longer applies on a P2P
  listener that Tor or I2P forwards into.** With `--i2p-accept-incoming`,
  with `--listen-onion` and no `=onion` listener, or with an onion
  `--external-ip` and no `=onion` listener, every overlay peer arrives from
  `127.0.0.1`, so a rule such as `noban,relay,download@127.0.0.1` would
  grant it to every Tor or I2P peer. 0.9.x applied it. A local wallet or
  indexer on that listener now loses those grants, and startup warns. Give
  it its own `--net-permission-bind=<perms>@127.0.0.1:<port>`, or mark a
  separate Tor bind with `--listen <addr>:<port>=onion`.
