Fixed

- **The overlay cjdns supervisor no longer exits with the TUN down.** The
  angel can die alone and leave its core behind. The supervisor used to call
  that survivor healthy and exit; the core then exited 232 at the 20-second
  `Security_setupComplete` deadline, the TUN went with it, and nothing
  restarted the pair. It now kills the orphan core, clears the device and
  starts a fresh pair on the same address. `run.sh` also waits for both
  cjdns addresses to bind before each journey, so a dead TUN fails the
  harness with the daemon logs instead of panicking in a journey that means
  to bind it.
