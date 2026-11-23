Fixed

- **Every outbound message kind takes its own size on the send budget.**
  `getheaders`, `getblocktxn`, and `version` were charged a flat 64 bytes
  whatever their length. The size table now lists every message kind, so
  a kind it does not list fails the build.
