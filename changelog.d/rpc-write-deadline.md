Security

- **RPC and REST replies have a write deadline.** A client that asked for
  a large reply (for example `/rest/block/<hash>.bin` with `--rest`) and
  then stopped reading kept the reply in memory and its connection slot
  for as long as it liked. As on Esplora, a reply write that makes no
  progress for 30 seconds now closes the connection, and so does a reply
  whose writes spend longer blocked, in total, than 30 seconds plus one
  second per 16 KiB written.
