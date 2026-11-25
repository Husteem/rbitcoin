Security

- **Non-handshake P2P frames are ignored before `verack` without
  decoding.** Before `version`, only `version` and `verack` are decoded.
  Before `verack`, only `version`, `verack`, `wtxidrelay`, `sendaddrv2`,
  `sendtxrcncl`, and `ping` are decoded (and feeler connections decode only
  `version`). Other frames are dropped by command without decoding their
  payloads; the version and verack waits log them at debug level, and the
  feeler does not (#1988).
