Security

- **More P2P item counts are checked against the frame before decode.**
  An `addr`, `addrv2`, `inv`, `getdata`, `notfound`, `getheaders`,
  `getblocks`, `cfheaders`, `cfcheckpt` or `getblocktxn` whose
  count needs more bytes than the frame has left is now refused before the
  decoder reserves memory for that count, as `tx`, `block`,
  `cmpctblock` and `blocktxn` already were. The peer is disconnected
  like for any oversize message. Core decodes the frame, fails at the end
  of the data, and keeps the peer (#1988).
