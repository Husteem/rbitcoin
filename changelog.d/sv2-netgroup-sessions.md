Added

- **`--sv2-tp-netgroup-sessions N` sets the SV2 netgroup share.** It is
  how many of the 8 template-provider sessions one IPv4 `/16` or IPv6
  `/32` may hold, 1 to 8 (default 4). Miners behind one NAT, one LAN or
  one loopback tunnel can set 8 to turn the share off.
- **SV2 sessions use TCP keepalive.** A client that drops without closing
  its connection used to keep its slot and netgroup share until a template
  write failed in the kernel, about 15 minutes; four such drops locked its
  netgroup out for that long. Its session now closes about a minute after
  its last packet.
