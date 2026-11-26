Changed

- **Electrum closes a quiet connection after 600 seconds, not 120, and
  `--electrum-idle-timeout SECS` sets it.** Since one idle window began
  counting from the last request rather than restarting on every push, a
  wallet that subscribed and waited, or pinged less often than every two
  minutes (Electrum's header-only interfaces, BDK's rust-electrum-client),
  was closed and missed the pushes sent before it reconnected. The default
  is now ElectrumX's 600 seconds. `--electrum-idle-timeout` (conf
  `electrum_idle_timeout=`, NixOS `services.rbitcoin.electrum.idleTimeout`)
  takes 1 to 86400 seconds. A write to a client that stops reading still
  times out at 120 seconds, and Esplora's idle timeout is unchanged (#2080).
