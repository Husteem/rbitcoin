Fixed

- **The overlay cjdns mesh survives a `cjdroute` crash.** `cjdns-run.sh`
  restarts the angel and its core as a pair after the runner's cjdns 22.3
  segfaults around the first TCP, so a journey is never handed a dead TUN.
  The restarted pair reclaims the same address, and the runner fails with
  the cjdns log when the device cannot come back.

Changed

- **The overlay runner builds the node before it starts the meshes.** A
  broken build fails in seconds instead of after three minutes of daemons,
  and the TUN a journey binds no longer sits idle through the build.
