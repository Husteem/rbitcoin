Security

- **Repeated handshake messages no longer flood the log.** A redundant
  `version` or `verack`, and every received `addrv2`, each wrote an INFO
  line, so peers could make the node log at their message rate. As in
  Core, these are now `p2p` debug lines, and an oversized `addrv2` is
  refused before it is serialized to count its bytes.
