Added

- **`--sv2-tp-netgroup-sessions N` sets the SV2 netgroup share.** It is
  how many of the 8 template-provider sessions one IPv4 `/16` or IPv6
  `/32` may hold, 1 to 8 (default 4). Miners behind one NAT, one LAN or
  one loopback tunnel can set 8 to turn the share off. The NixOS module
  option is `services.rbitcoin.sv2.tp.netgroupSessions`.
