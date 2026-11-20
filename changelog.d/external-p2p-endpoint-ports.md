Changed

- `--external-ip` and `external_ip=` accept explicit public P2P ports for IPv4, bracketed IPv6, and Tor v3 onion addresses.
- With an external onion configured, inbound TCP peers skip source-IP permission grants. Explicit bind grants still apply.
