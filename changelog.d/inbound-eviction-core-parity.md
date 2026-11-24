Security

- **Inbound eviction follows Core's `SelectNodeToEvict`.** When inbound
  slots were full, the peer to drop was predictable. Among equal netgroups
  the one with the lowest network prefix lost, and the netgroup protection
  favoured the lowest prefixes. A peer connecting from chosen networks
  could push out honest peers one by one. Netgroups are now ordered by a
  secret per-process key. A tie goes to the group with the newest member.
  Half of the remaining peers are kept by uptime, with a share reserved
  for onion, local, I2P and CJDNS peers, and up to 8 block-relay-only peers
  that send blocks are kept. The 10-minute refill wait for an evicted
  netgroup is unchanged.
