Security

- **TCP RPC clients can no longer lock out the local RPC socket.** The TCP
  listener and the unix socket shared one 256-connection cap, taken when a
  connection was accepted and before authentication, and a `401` kept its
  connection open. Anyone who could reach `--rpc-listen` could hold every
  slot without a password, so `rbitcoin-cli` (including `stop`) on the unix
  socket was refused. Each listener now has its own cap, and a `401` closes
  its connection.
