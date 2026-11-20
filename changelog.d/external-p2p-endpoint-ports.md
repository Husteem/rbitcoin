Changed

- `--external-ip` and `external_ip=` accept explicit public P2P ports for IPv4, bracketed IPv6, and Tor v3 onion addresses.
- Append `=onion` to a P2P listen address to dedicate it to Tor forwarding and keep source-IP permission rules on ordinary listeners.
- With an external onion configured and no dedicated Tor listener, inbound TCP peers skip source-IP permission grants. Explicit bind grants still apply.
