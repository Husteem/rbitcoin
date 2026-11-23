Security

- **RPC and REST replies have a write deadline.** A client that asked for
  a large reply (for example `/rest/block/<hash>.bin` with `--rest`) and
  then stopped reading kept the reply in memory and its connection slot
  for as long as it liked. As on Esplora, a reply write that makes no
  progress for 30 seconds, or drains slower than 16 KiB/s on average past
  that grace, now closes the connection.
