Changed

- **Startup warns when Tor or I2P forwarding drops a loopback
  `--net-permission` grant.** A rule such as `noban@127.0.0.1` does not
  apply on a P2P listener that a forwarder delivers into (see the 0.10.0
  security note). The warning names a `--net-permission-bind` port the
  forwarder does not use, and, without I2P incoming, a marked `=onion`
  bind.
