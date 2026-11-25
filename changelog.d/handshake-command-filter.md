Security

- **Non-handshake P2P frames are ignored before `verack` without
  decoding.** Before `version`, only `version` and `verack` are decoded.
  Before `verack`, only `version`, `verack`, `wtxidrelay`, `sendaddrv2`,
  `sendtxrcncl`, and `ping` are decoded (and feeler connections decode only
  `version`). Any other frame is dropped by its header command and logged
  at debug level without parsing or allocating its payload (#1988).
