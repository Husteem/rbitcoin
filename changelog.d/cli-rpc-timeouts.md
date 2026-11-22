Added

- **`rbitcoin-cli --rpcclienttimeout` and `--rpcwaittimeout`.** Both take
  seconds, and `0` means no limit, as in Core. `--rpcclienttimeout`
  applies to TCP and the unix socket. It defaults to Core's 900 seconds
  on TCP, up from the fixed 30 seconds, and to no limit on the unix
  socket as before. It also bounds the connect and the host name
  lookup, as does what is left of `--rpcwait`. A timeout names the
  option that ran out. A negative value is refused; Core reads it as no
  limit.
  `--rpcwaittimeout` caps how long `--rpcwait` retries and still defaults
  to 30 seconds (Core waits forever) (#1705).
- **`rbitcoin-cli` takes Core's single dash for every option.**
  `-rpcwait`, `-named`, `-stdin`, `-getinfo`, `-datadir=PATH`,
  `-rpcclienttimeout=0` and the rest parse as their `--` form, so
  `rbitcoin-cli -rpcclienttimeout=0 importmempool PATH` runs as written.
  `-h`, `-V` and a negative number parameter are unchanged, and a Core
  option the client lacks is still an unknown argument. A switch given a
  value (`--rpcwait=1`) is refused.
