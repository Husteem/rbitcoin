Security

- **Compact filter requests are served off the network threads.**
  `getcfilters`, `getcfheaders` and `getcfcheckpt` read up to 1,000
  filters (or 2,000 filter headers) from disk on the async worker that runs
  every peer session, so a few peers asking for large ranges could stall
  all P2P traffic. With `--block-filter-index`, those reads now run on the
  blocking pool under the header-work permit, and a peer past its send
  budget is not served.
